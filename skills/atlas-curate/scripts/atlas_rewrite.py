#!/usr/bin/env python3
"""Edit one atom without losing the fields you did not mention.

`atlas update` is a full replace: every field left off the command is cleared,
tags, sources, links and pitfalls included. This reads the atom, applies the
requested change, repasses everything else, then re-reads and fails if any
field moved that was not asked to move.

Runs atlas through argv rather than a shell, so payloads containing quotes,
backticks or `$(...)` are passed through literally instead of being executed.
"""

import argparse
import json
import subprocess
import sys
import tempfile

ROUNDTRIP = ("title", "type", "confidence", "summary", "details", "tags", "sources", "links", "pitfalls")


def read_atom(atom_id):
    proc = subprocess.run(("atlas", "get", atom_id, "--format", "json"),
                          capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit("cannot read %s: %s" % (atom_id, proc.stderr.strip()))
    atom = json.loads(proc.stdout)
    # get omits empty fields entirely, so absent means empty, not unreadable.
    return {field: atom.get(field, [] if field in ("tags", "sources", "links", "pitfalls") else "")
            for field in ROUNDTRIP}


def apply_edits(atom, args):
    edited = dict(atom)
    for field in ("title", "type", "confidence", "summary"):
        value = getattr(args, field)
        if value is not None:
            edited[field] = value
    if args.details_file:
        with open(args.details_file) as handle:
            edited["details"] = handle.read()

    def listedit(field, add, remove):
        items = list(edited[field])
        for item in remove:
            if item not in items:
                raise SystemExit("%s has no %s %r" % (args.id, field[:-1], item))
            items.remove(item)
        for item in add:
            if item not in items:
                items.append(item)
        edited[field] = items

    sources = list(edited["sources"])
    for spec in args.replace_source:
        if "=" not in spec:
            raise SystemExit("--replace-source needs OLD=NEW, got %r" % spec)
        old, new = spec.split("=", 1)
        if not any(old in s for s in sources):
            raise SystemExit("%s has no source containing %r" % (args.id, old))
        sources = [s.replace(old, new) for s in sources]
    edited["sources"] = sources

    listedit("tags", args.add_tag, args.remove_tag)
    listedit("sources", args.add_source, args.remove_source)
    listedit("links", args.add_link, args.remove_link)
    return edited


def build_command(atom_id, atom, details_path):
    command = ["atlas", "update", atom_id,
               "--title", atom["title"],
               "--type", atom["type"],
               "--confidence", atom["confidence"],
               "--summary", atom["summary"],
               "--details", "-"]
    for tag in atom["tags"]:
        command += ["--tag", tag]
    for source in atom["sources"]:
        command += ["--source", source]
    for pitfall in atom["pitfalls"]:
        command += ["--pitfall", pitfall]
    return command


def rewrite_links(atom_id, before, after):
    for link in before:
        if link not in after:
            command = ["atlas", "unlink", atom_id, link]
            proc = subprocess.run(command, capture_output=True, text=True)
            if proc.returncode != 0:
                raise SystemExit("unlink failed: %s" % proc.stderr.strip())
    for link in after:
        if link not in before:
            command = ["atlas", "link", atom_id, link]
            proc = subprocess.run(command, capture_output=True, text=True)
            if proc.returncode != 0:
                raise SystemExit("link failed: %s" % proc.stderr.strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--id", required=True, help="atom ID, ideally org/project/K-NNNNNN")
    parser.add_argument("--title")
    parser.add_argument("--type", choices=("note", "gotcha", "recipe", "decision"))
    parser.add_argument("--confidence", choices=("high", "medium", "low"))
    parser.add_argument("--summary")
    parser.add_argument("--details-file", help="file whose contents become the new details")
    parser.add_argument("--replace-source", action="append", default=[],
                        metavar="OLD=NEW", help="substring rewrite across every source")
    parser.add_argument("--add-tag", action="append", default=[])
    parser.add_argument("--remove-tag", action="append", default=[])
    parser.add_argument("--add-source", action="append", default=[])
    parser.add_argument("--remove-source", action="append", default=[])
    parser.add_argument("--add-link", action="append", default=[])
    parser.add_argument("--remove-link", action="append", default=[])
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    before = read_atom(args.id)
    after = apply_edits(before, args)

    changed = [f for f in ROUNDTRIP if before[f] != after[f]]
    if not changed:
        print("%s: nothing to change" % args.id)
        return

    print("%s: changing %s" % (args.id, ", ".join(changed)))
    for field in changed:
        if field == "details":
            print("  details: %d -> %d chars" % (len(before[field]), len(after[field])))
        else:
            print("  %s: %r -> %r" % (field, before[field], after[field]))

    if args.dry_run:
        print("dry run, nothing written")
        return

    with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as handle:
        handle.write(after["details"])
        details_path = handle.name

    with open(details_path) as stdin:
        proc = subprocess.run(build_command(args.id, after, details_path),
                              stdin=stdin, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit("update failed: %s" % proc.stderr.strip())

    rewrite_links(args.id, before["links"], after["links"])

    verify = read_atom(args.id)
    drift = [f for f in ROUNDTRIP if verify[f] != after[f]]
    if drift:
        print("DRIFT after write: %s" % ", ".join(drift), file=sys.stderr)
        for field in drift:
            print("  wanted %r" % (after[field],), file=sys.stderr)
            print("  got    %r" % (verify[field],), file=sys.stderr)
        raise SystemExit(1)
    print("%s: written and verified" % args.id)


if __name__ == "__main__":
    main()
