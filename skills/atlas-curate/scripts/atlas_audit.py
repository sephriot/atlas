#!/usr/bin/env python3
"""Inventory an Atlas project so curation runs on evidence instead of taste.

Read-only. Resolves every atom's sources against the filesystem and every link
against the atoms that actually exist, then prints what a curation pass has to
decide about. It classifies nothing as useless — that judgement stays with the
reader.

It reads the other projects in the same org too. Atlas writes both halves of a
link now, so an edge only one atom holds predates that and is invisible from the
side that is missing it.
"""

import argparse
import json
import os
import re
import subprocess
import sys

FQ = re.compile(r"^[^/]+/[^/]+/(K-\d+)$")
BARE = re.compile(r"^K-\d+$")
CROSS_BRACKET = re.compile(r"^\[repo (?:[\w.-]+/)?([\w.-]+)\]\s*(\S+)")
CROSS_PREFIX = re.compile(r"^([\w.-]+):(\S+\.\w+)$")
SIBLING_ROOTS = {}


def atlas(*args):
    proc = subprocess.run(("atlas",) + args, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit("atlas %s failed: %s" % (" ".join(args), proc.stderr.strip()))
    return proc.stdout


def current_scope():
    out = atlas("context")
    org = project = None
    for line in out.splitlines():
        if line.startswith("org:"):
            org = line.split(":", 1)[1].strip()
        elif line.startswith("project:"):
            project = line.split(":", 1)[1].strip()
    if not org or not project:
        raise SystemExit("atlas context has no org/project; pass --scope org/project")
    return "%s/%s" % (org, project)


def scope_ids(scope):
    # Per project rather than per org, so no single listing can reach the
    # result limit and drop atoms without saying so.
    if "/" not in scope:
        raise SystemExit("scope must be org/project, got %r" % scope)
    return [line.strip() for line in atlas("atoms", "--scope", scope, "--ids").splitlines() if line.strip()]


def load_atoms(ids):
    """Return ({id: atom}, [unreadable ids]). One bad ID fails a whole batch."""
    if not ids:
        return {}, []
    proc = subprocess.run(("atlas", "get", "-", "--format", "json"),
                          input="\n".join(ids), capture_output=True, text=True)
    if proc.returncode == 0:
        batch = json.loads(proc.stdout)
        # A one-ID batch comes back as the atom itself, not a list of one.
        if isinstance(batch, dict):
            batch = [batch]
        return dict(zip(ids, batch)), []
    atoms, missing = {}, []
    for atom_id in ids:
        one = subprocess.run(("atlas", "get", atom_id, "--format", "json"),
                             capture_output=True, text=True)
        if one.returncode == 0:
            atoms[atom_id] = json.loads(one.stdout)
        else:
            missing.append(atom_id)
    return atoms, missing


def sibling_scopes(org, exclude):
    projects = json.loads(atlas("projects", "--format", "json"))
    return sorted("%s/%s" % (p["org"], p["project"]) for p in projects
                  if p["org"] == org and p["atom_count"] > 0
                  and "%s/%s" % (p["org"], p["project"]) != exclude)


def resolve(*candidates):
    """Return the first candidate present on disk, else the first, which is reported."""
    for candidate in candidates:
        if os.path.exists(candidate):
            return candidate
    return candidates[0]


def bare_path(source):
    """Drop a trailing prose qualifier such as a symbol name or a parenthesised note."""
    return source.split(" ")[0]


def cross_repo(source, org):
    """Split a citation naming another repository into (repo, path), or None."""
    bracket = CROSS_BRACKET.match(source)
    if bracket:
        return bracket.group(1), bracket.group(2)
    if source.startswith(org + "/") and source.count("/") >= 2:
        return tuple(source[len(org) + 1:].split("/", 1))
    prefixed = CROSS_PREFIX.match(source)
    if prefixed:
        return prefixed.group(1), prefixed.group(2)
    return None


def nested_checkout(parent, repo):
    """Find repo one directory below parent, accepting only a git checkout."""
    try:
        entries = sorted(os.listdir(parent))
    except OSError:
        return None
    for entry in entries:
        candidate = os.path.join(parent, entry, repo)
        if os.path.exists(os.path.join(candidate, ".git")):
            return candidate
    return None


def sibling_root(root, repo):
    """Find a sibling checkout by directory name, or None if it is not cloned here."""
    if repo not in SIBLING_ROOTS:
        found, current = None, root
        # Sibling checkouts sit near the audited repo at a depth the store cannot know,
        # and a grouping repository such as a superproject holds its own a level deeper.
        for _ in range(4):
            parent = os.path.dirname(current)
            if parent == current:
                break
            found = (os.path.join(parent, repo) if os.path.isdir(os.path.join(parent, repo))
                     else nested_checkout(parent, repo))
            if found:
                break
            current = parent
        SIBLING_ROOTS[repo] = found
    return SIBLING_ROOTS[repo]


def classify_source(source, root, org):
    """Return (kind, resolved_path). Only 'file' entries are checked on disk."""
    if source.startswith(("http://", "https://", "git@")):
        return "external", None
    elsewhere = cross_repo(source, org)
    if elsewhere:
        repo, relative = elsewhere
        base = sibling_root(root, repo)
        # Without that checkout the citation can be neither confirmed nor called dead.
        if base is None:
            return "unchecked", source
        return "file", resolve(os.path.join(base, relative),
                              os.path.join(base, bare_path(relative)))
    if source.startswith("~"):
        return "file", resolve(os.path.expanduser(source),
                               os.path.expanduser(bare_path(source)))
    if os.path.isabs(source):
        return "file", resolve(source, bare_path(source))
    if "/" in source or "." in source:
        return "file", resolve(os.path.join(root, source),
                               os.path.join(root, bare_path(source)))
    return "freeform", None


def qualify(link, scope):
    """Resolve a stored link against the project of the atom holding it."""
    if BARE.match(link):
        return "%s/%s" % (scope, link)
    if FQ.match(link):
        return link
    return "%s/%s" % (scope.split("/")[0], link)


def project_of(atom_id):
    return atom_id.rsplit("/", 1)[0]


def title_tokens(title):
    return {t for t in re.findall(r"[a-z0-9]+", title.lower()) if len(t) > 3}


def jaccard(a, b):
    if not a or not b:
        return 0.0
    return len(a & b) / len(a | b)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scope", help="org/project to audit (default: atlas context)")
    parser.add_argument("--root", default=".", help="repo root that sources resolve against")
    parser.add_argument("--skip-org-scan", action="store_true",
                        help="audit this project alone; edges into sibling projects go unchecked")
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args()

    scope = args.scope or current_scope()
    root = os.path.abspath(args.root)
    org = scope.split("/")[0]

    ids = scope_ids(scope)
    if not ids:
        raise SystemExit("no atoms in scope %s" % scope)
    atoms, index_orphans = load_atoms(ids)
    if not atoms:
        raise SystemExit("no readable atoms in scope %s" % scope)

    siblings, scanned = {}, []
    if not args.skip_org_scan:
        for other in sibling_scopes(org, scope):
            found, orphaned = load_atoms(scope_ids(other))
            siblings.update(found)
            index_orphans += orphaned
            scanned.append((other, len(found)))

    everything = dict(atoms)
    everything.update(siblings)
    known = set(everything)

    outbound = {atom_id: [qualify(link, project_of(atom_id)) for link in atom.get("links", [])]
                for atom_id, atom in everything.items()}
    inbound = {}
    for src, targets in outbound.items():
        for target in targets:
            inbound.setdefault(target, []).append(src)

    report = []
    for atom_id, atom in atoms.items():
        dead, external, freeform, unchecked = [], [], [], []
        for source in atom.get("sources", []):
            kind, path = classify_source(source, root, org)
            if kind == "external":
                external.append(source)
            elif kind == "freeform":
                freeform.append(source)
            elif kind == "unchecked":
                unchecked.append(source)
            elif not os.path.exists(path):
                dead.append(source)

        dangling = []
        for link in atom.get("links", []):
            target = qualify(link, scope)
            if target in known:
                continue
            # Outside the scanned org, only atlas can say whether it exists.
            probe = subprocess.run(("atlas", "get", target, "--format", "json"),
                                   capture_output=True, text=True)
            if probe.returncode != 0:
                dangling.append(target)

        edges_out = sorted(t for t in outbound[atom_id] if t not in dangling)
        edges_in = sorted(inbound.get(atom_id, []))
        asymmetric = []
        for peer in edges_out:
            if peer in known and atom_id not in outbound.get(peer, []):
                asymmetric.append({"peer": peer, "held_by": "this atom only",
                                   "cross_project": project_of(peer) != scope})
        for peer in edges_in:
            if peer not in edges_out:
                asymmetric.append({"peer": peer, "held_by": "the peer only",
                                   "cross_project": project_of(peer) != scope})

        total = len([s for s in atom.get("sources", []) if classify_source(s, root, org)[0] == "file"])
        report.append({
            "id": atom_id,
            "title": atom["title"],
            "type": atom["type"],
            "updated_at": atom.get("updated_at"),
            "sources_checked": total,
            "dead_sources": dead,
            "external_sources": external,
            "freeform_sources": freeform,
            "unchecked_sources": unchecked,
            "dangling_links": dangling,
            "inbound_links": edges_in,
            "outbound_links": edges_out,
            "asymmetric_links": asymmetric,
            "sourceless": total == 0,
            "isolated": not edges_in and not edges_out,
        })

    pairs, cross_pairs = [], []
    items = sorted(atoms.items())
    for i, (id_a, a) in enumerate(items):
        for id_b, b in items[i + 1:]:
            shared = set(a.get("tags", [])) & set(b.get("tags", []))
            overlap = jaccard(title_tokens(a["title"]), title_tokens(b["title"]))
            if len(shared) >= 3 and overlap >= 0.4:
                pairs.append({"a": id_a, "b": id_b, "shared_tags": sorted(shared),
                              "title_overlap": round(overlap, 2)})
        for id_b, b in sorted(siblings.items()):
            if id_b in outbound[id_a] or id_a in outbound.get(id_b, []):
                continue
            shared = set(a.get("tags", [])) & set(b.get("tags", []))
            overlap = jaccard(title_tokens(a["title"]), title_tokens(b["title"]))
            if len(shared) >= 3 or (len(shared) >= 2 and overlap >= 0.3):
                cross_pairs.append({"a": id_a, "b": id_b, "shared_tags": sorted(shared),
                                    "title_overlap": round(overlap, 2)})

    if args.format == "json":
        json.dump({"scope": scope, "root": root, "org_scanned": scanned,
                   "index_orphans": index_orphans, "atoms": report,
                   "duplicate_candidates": pairs,
                   "cross_project_link_candidates": cross_pairs},
                  sys.stdout, indent=2)
        sys.stdout.write("\n")
        return

    print("scope %s, %d atoms, sources resolved against %s" % (scope, len(report), root))
    if args.skip_org_scan:
        print("org scan skipped: edges into sibling projects unchecked\n")
    else:
        print("org %s: %d sibling atoms across %d projects\n" % (org, len(siblings), len(scanned)))

    if index_orphans:
        print("== listed by atlas atoms but unreadable: %d ==" % len(index_orphans))
        for atom_id in index_orphans:
            print(atom_id)
        print()

    flagged = [r for r in report if r["dead_sources"] or r["dangling_links"]]
    print("== atoms with mechanical evidence of drift: %d ==" % len(flagged))
    for r in sorted(flagged, key=lambda r: -len(r["dead_sources"])):
        print("%s  %s" % (r["id"], r["title"]))
        if r["dead_sources"]:
            print("    dead sources %d/%d: %s" % (len(r["dead_sources"]), r["sources_checked"],
                                                  ", ".join(r["dead_sources"])))
        if r["dangling_links"]:
            print("    dangling links: %s" % ", ".join(r["dangling_links"]))

    offsite = [r for r in report if r["unchecked_sources"]]
    print("\n== sources in a repository not cloned here, unjudged: %d ==" % len(offsite))
    for r in offsite:
        print("%s  %s" % (r["id"], r["title"]))
        print("    %s" % ", ".join(r["unchecked_sources"]))

    sourceless = [r for r in report if r["sourceless"]]
    print("\n== atoms with no checkable source: %d ==" % len(sourceless))
    for r in sourceless:
        print("%s  %s  (updated %s)" % (r["id"], r["title"], r["updated_at"]))

    lopsided = [r for r in report if r["asymmetric_links"]]
    total_edges = sum(len(r["asymmetric_links"]) for r in lopsided)
    print("\n== edges only one atom holds: %d across %d atoms ==" % (total_edges, len(lopsided)))
    for r in lopsided:
        print("%s  %s" % (r["id"], r["title"]))
        for edge in r["asymmetric_links"]:
            print("    %s  held by %s%s" % (edge["peer"], edge["held_by"],
                                            "  [cross-project]" if edge["cross_project"] else ""))

    isolated = [r for r in report if r["isolated"]]
    print("\n== atoms with no edges at all: %d ==" % len(isolated))
    for r in isolated:
        print("%s  %s" % (r["id"], r["title"]))

    print("\n== duplicate candidates: %d ==" % len(pairs))
    for p in pairs:
        print("%s <-> %s  tags %s  title overlap %s"
              % (p["a"], p["b"], ",".join(p["shared_tags"]), p["title_overlap"]))

    print("\n== unlinked atoms that share ground with a sibling project: %d ==" % len(cross_pairs))
    for p in cross_pairs:
        print("%s <-> %s  tags %s  title overlap %s"
              % (p["a"], p["b"], ",".join(p["shared_tags"]), p["title_overlap"]))

    print("\nfull edge lists per atom: rerun with --format json")


if __name__ == "__main__":
    main()
