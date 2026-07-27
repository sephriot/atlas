#!/usr/bin/env python3
"""Inventory an Atlas project so curation runs on evidence instead of taste.

Read-only. Resolves every atom's sources against the filesystem and every link
against the atoms that actually exist, then prints what a curation pass has to
decide about. It classifies nothing as useless — that judgement stays with the
reader.
"""

import argparse
import json
import os
import re
import subprocess
import sys

FQ = re.compile(r"^[^/]+/[^/]+/(K-\d+)$")
BARE = re.compile(r"^K-\d+$")


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


def classify_source(source, root):
    """Return (kind, resolved_path). Only 'file' entries are checked on disk."""
    if source.startswith(("http://", "https://", "git@")):
        return "external", None
    if os.path.isabs(source):
        return "file", source
    if "/" in source or "." in source:
        return "file", os.path.join(root, source)
    return "freeform", None


def qualify(link, scope):
    if BARE.match(link):
        return "%s/%s" % (scope, link)
    return link


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
    parser.add_argument("--format", choices=("text", "json"), default="text")
    args = parser.parse_args()

    scope = args.scope or current_scope()
    root = os.path.abspath(args.root)

    ids = [line.strip() for line in atlas("atoms", "--scope", scope, "--ids").splitlines() if line.strip()]
    if not ids:
        raise SystemExit("no atoms in scope %s" % scope)

    atoms = {}
    for atom_id in ids:
        atoms[atom_id] = json.loads(atlas("get", atom_id, "--format", "json"))

    known = set(ids)
    report = []
    for atom_id, atom in atoms.items():
        dead, external, freeform = [], [], []
        for source in atom.get("sources", []):
            kind, path = classify_source(source, root)
            if kind == "external":
                external.append(source)
            elif kind == "freeform":
                freeform.append(source)
            elif not os.path.exists(path):
                dead.append(source)

        dangling = []
        for link in atom.get("links", []):
            target = qualify(link, scope)
            if target in known:
                continue
            # Out-of-scope target: only atlas can say whether it exists.
            probe = subprocess.run(("atlas", "get", target, "--format", "json"),
                                   capture_output=True, text=True)
            if probe.returncode != 0:
                dangling.append(link)

        total = len([s for s in atom.get("sources", []) if classify_source(s, root)[0] == "file"])
        report.append({
            "id": atom_id,
            "title": atom["title"],
            "type": atom["type"],
            "updated_at": atom.get("updated_at"),
            "sources_checked": total,
            "dead_sources": dead,
            "external_sources": external,
            "freeform_sources": freeform,
            "dangling_links": dangling,
            "sourceless": total == 0,
        })

    pairs = []
    items = sorted(atoms.items())
    for i, (id_a, a) in enumerate(items):
        for id_b, b in items[i + 1:]:
            shared = set(a.get("tags", [])) & set(b.get("tags", []))
            overlap = jaccard(title_tokens(a["title"]), title_tokens(b["title"]))
            if len(shared) >= 3 and overlap >= 0.4:
                pairs.append({"a": id_a, "b": id_b, "shared_tags": sorted(shared),
                              "title_overlap": round(overlap, 2)})

    if args.format == "json":
        json.dump({"scope": scope, "root": root, "atoms": report, "duplicate_candidates": pairs},
                  sys.stdout, indent=2)
        sys.stdout.write("\n")
        return

    print("scope %s, %d atoms, sources resolved against %s\n" % (scope, len(report), root))

    flagged = [r for r in report if r["dead_sources"] or r["dangling_links"]]
    print("== atoms with mechanical evidence of drift: %d ==" % len(flagged))
    for r in sorted(flagged, key=lambda r: -len(r["dead_sources"])):
        print("%s  %s" % (r["id"], r["title"]))
        if r["dead_sources"]:
            print("    dead sources %d/%d: %s" % (len(r["dead_sources"]), r["sources_checked"],
                                                  ", ".join(r["dead_sources"])))
        if r["dangling_links"]:
            print("    dangling links: %s" % ", ".join(r["dangling_links"]))

    sourceless = [r for r in report if r["sourceless"]]
    print("\n== atoms with no checkable source: %d ==" % len(sourceless))
    for r in sourceless:
        print("%s  %s  (updated %s)" % (r["id"], r["title"], r["updated_at"]))

    print("\n== duplicate candidates: %d ==" % len(pairs))
    for p in pairs:
        print("%s <-> %s  tags %s  title overlap %s"
              % (p["a"], p["b"], ",".join(p["shared_tags"]), p["title_overlap"]))


if __name__ == "__main__":
    main()
