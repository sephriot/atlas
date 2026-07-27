---
name: atlas-curate
description: "Curate the Atlas atoms of one project: repair atoms the code has outgrown, delete atoms that stopped being knowledge, and add atoms for what the store is missing. Use when the user asks to curate, audit, clean up, prune, or review Atlas memory, asks whether the knowledge base is still accurate, or names a project whose atoms should be gone over."
disable-model-invocation: true
---

# Atlas Curate

Bring one project's atoms back in line with the code. This is a review pass with
writes in it, so it runs on evidence and stops for confirmation before removing
anything. Atlas updates are patches; the rewrite helper retains its round-trip
check as a verification layer, not as a workaround for destructive updates.

Two bundled scripts do the mechanical work: `scripts/atlas_audit.py` gathers the
evidence, `scripts/atlas_rewrite.py` performs and verifies a targeted edit. Use
both when multiple fields need coordinated changes.

## 0. Resolve the script directory

Those two paths are relative to this file, and the working directory during a
curation pass is the project being curated. Resolve them once, before anything else:

```bash
SKILL_DIR=<the directory this SKILL.md was loaded from>
ls "$SKILL_DIR/scripts"      # expect atlas_audit.py and atlas_rewrite.py
```

Hosts report that directory when they load a skill. If this one did not, the
control-plane repository holds the original at `~/.ai/skills/invoked/atlas-curate`,
and every host's copy is a symlink to it.

Every later command uses `"$SKILL_DIR/scripts/..."`. Both scripts take their scope
and their repository root as explicit arguments, so the shell's working directory
never affects the result.

## 1. Scope the pass

Run `atlas context`. If it reports the wrong project, or the user named a different
one, get the scope from `atlas projects` and pass `--scope <org>/<project>` to every
command below. Curating the wrong store is worse than not curating: it writes into a
project nobody was asking about.

Confirm the scope and the atom count with the user before touching anything.

## 2. Inventory

```bash
python3 "$SKILL_DIR/scripts/atlas_audit.py" --scope <org>/<project> --root <repo-root>
```

Read-only. It resolves every atom's sources against the filesystem and every link
against the atoms that exist, then reports: atoms with dead sources or dangling
links, atoms with no checkable source, edges only one atom holds, atoms with no
edges at all, duplicate candidates, and unlinked atoms that share ground with a
sibling project. IDs the index lists but cannot be read are reported first.

It reads every project in the org, since an edge is only half visible from one
side. `--skip-org-scan` drops that, and with it every cross-project finding.

A dead source path is mechanical evidence that the atom describes a world that no
longer exists. It is the one signal here that needs no interpretation — start there,
and treat the rest as reading lists rather than verdicts. `--format json` carries the
full inbound and outbound edge list per atom; the text report only summarises.

## 3. Read before judging

Read the full text of every flagged atom with `atlas get <id>`. Then read what the
atom points at as it exists now — the surviving sources, the code that replaced the
dead ones. An atom is outdated when the current code contradicts it, not when it
sounds old.

## 4. Assign a verdict

| Evidence | Verdict |
|---|---|
| Sources moved or were renamed, content still true | Repair the paths, leave the text |
| Code contradicts the atom, but the decision it records still explains something | Rewrite the text, keep the ID and its edges |
| Atom describes a thing that no longer exists anywhere, and nothing actionable survives it | Delete |
| Two atoms cover the same ground, one strictly poorer | Move the poorer one's unique content onto the survivor, then delete it |
| A to-do list whose items all shipped | Delete; a finished plan is not knowledge |
| Superseded by a later decision, but the lineage matters | Keep, retitle to say it was superseded, link forward |

Outdated is a repair, not a deletion. An atom whose text went stale still records why
a decision was made, and that reason usually survives the code that prompted it.

## 4a. Assign a verdict to the edges

Edges decide what a future search can reach, so they get judged too. Atlas writes both
halves of a link, so an edge only one atom holds was written before that and is
invisible from the side that is missing it.

| Evidence | Verdict |
|---|---|
| The atom's text was just rewritten | Re-read its edges against the new text before moving on; an edge that fitted the old claim may not fit this one |
| Edge held by one atom only, and the relationship is real | `atlas link` the pair again; it restores the missing half and leaves everything else alone |
| Edge held by one atom only, and neither atom explains the other | `atlas unlink` the pair, which clears both sides |
| Dangling link, target no longer exists | Repoint it at whatever replaced the target, or unlink it |
| Atom with no edges that a cluster in this project clearly belongs to | Link it to the one or two atoms a reader would want next, not to everything sharing a tag |
| Atom whose subject is owned by a sibling project | Link across; the audit lists candidates by shared tags and title overlap |
| Cross-project candidate whose overlap is only vocabulary | Leave it; a wrong edge costs more than a missing one, because it gets read |

An edge earns its place the same way an atom does: someone arriving at one end needs
the other. Shared tags are a hint to read, never the reason.

**Cross-project edges write into a project this pass did not scope.** Adding one edits
the sibling's store, so list those writes and get confirmation before making them, the
same as for deletions. Say which sibling project, and how many edges.

## 5. Repair

One atom, one invocation:

```bash
python3 "$SKILL_DIR/scripts/atlas_rewrite.py" \
  --id <org>/<project>/K-NNNNNN --replace-source 'old/path/=new/path/' --dry-run
```

Drop `--dry-run` once the printed diff is right. The script re-reads the atom and
exits non-zero if any field moved that was not asked to move.

When a rewrite changed the title, summary or details, it then prints the atom's edges.
Those are the edges most likely to have just gone stale — the text they were justified by
is the text you replaced. Judge them by the table above before leaving the atom.

For new prose, write it to a file and pass `--details-file`. The script calls atlas
through argv rather than a shell, so quotes, backticks and `$(...)` inside the text
are stored literally instead of being executed.

Other flags: `--title`, `--summary`, `--type`, `--confidence`, `--add-tag`,
`--remove-tag`, `--add-source`, `--remove-source`, `--add-link`, `--remove-link`.

The helper changes links through `atlas link` and `atlas unlink`. `update` accepts no
link fields; `create` does, through `--link`, which matters in step 7 rather than here.

Because those two commands write both atoms, an `--add-link` or `--remove-link` here
also edits the peer. The round-trip check covers the atom named by `--id` only, so
read the peer afterwards when it matters.

For a one-sided edge that needs its missing half, skip the helper — `atlas link` on the
pair is the whole repair, and it touches nothing else.

## 6. Prune

Deleting is the only irreversible step in this skill.

1. List the deletions with one line of reasoning each and get the user's confirmation.
   Do not delete on a general instruction to curate.
2. Where the store lives in a git repository, commit it first, so the removals arrive
   as a reviewable diff.
3. `atlas delete <id>`. It unlinks the atom from everything that referenced it and
   reports those atoms as `detached`, so no half-edges survive. Read that list before
   moving on: an atom several others leaned on is usually worth rewriting instead, and
   the peers that lost an edge may now need one to whatever replaced it.

## 7. Fill the gaps

Curation is not only subtraction. With the store's current shape in mind, look at what
the project actually does — recent commits, the modules the atoms never mention, the
conventions a newcomer would trip over — and record what is missing.

Hold new atoms to the same bar as `atlas-put`: reusable, non-obvious, stable,
actionable. Anything a reader would learn faster by opening the file is not an atom.
Prefer a handful of atoms that close real gaps over a sweep that documents the
repository back to itself.

A missing edge is a gap too, and a cheaper one to close than a missing atom. Create new
atoms with `--link`, so an atom recorded during curation never joins the isolated pile
the audit just listed, and give the isolated atoms the audit did find the one or two
edges a reader would actually follow.

## 8. Verify and report

Re-run the audit. Every repair should have cleared its flag, and nothing that was
clean should have become flagged.

Report per group: atoms repaired, atoms rewritten, atoms deleted with the reason,
atoms created, edges added and removed, and anything flagged but deliberately left
alone with why. Count the cross-project edges separately and name the sibling projects
they touched, because those writes landed outside the scope the pass declared. Name the
atoms whose flags survive the pass — a dead source you decided not to chase is a
finding, not a silence.
