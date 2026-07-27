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
against the atoms that exist, then reports three groups: atoms with dead sources or
dangling links, atoms with no checkable source, and duplicate candidates.

A dead source path is mechanical evidence that the atom describes a world that no
longer exists. It is the one signal here that needs no interpretation — start there,
and treat the other two groups as reading lists rather than verdicts.

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

## 5. Repair

One atom, one invocation:

```bash
python3 "$SKILL_DIR/scripts/atlas_rewrite.py" \
  --id <org>/<project>/K-NNNNNN --replace-source 'old/path/=new/path/' --dry-run
```

Drop `--dry-run` once the printed diff is right. The script re-reads the atom and
exits non-zero if any field moved that was not asked to move.

For new prose, write it to a file and pass `--details-file`. The script calls atlas
through argv rather than a shell, so quotes, backticks and `$(...)` inside the text
are stored literally instead of being executed.

Other flags: `--title`, `--summary`, `--type`, `--confidence`, `--add-tag`,
`--remove-tag`, `--add-source`, `--remove-source`, `--add-link`, `--remove-link`.

The helper changes links through `atlas link` and `atlas unlink`; create and update do not accept inline link fields.

## 6. Prune

Deleting is the only irreversible step in this skill.

1. List the deletions with one line of reasoning each and get the user's confirmation.
   Do not delete on a general instruction to curate.
2. Where the store lives in a git repository, commit it first, so the removals arrive
   as a reviewable diff.
3. Find every atom pointing at the doomed one and repoint or drop that edge before
   deleting. Match the fully-qualified `<org>/<project>/K-NNNNNN` — another project's
   `K-000012` is a different atom with the same bare ID.
4. `atlas delete <id>`. Atlas rejects remaining inbound links; do not use `--force`
   until they have been intentionally resolved.

## 7. Fill the gaps

Curation is not only subtraction. With the store's current shape in mind, look at what
the project actually does — recent commits, the modules the atoms never mention, the
conventions a newcomer would trip over — and record what is missing.

Hold new atoms to the same bar as `atlas-put`: reusable, non-obvious, stable,
actionable. Anything a reader would learn faster by opening the file is not an atom.
Prefer a handful of atoms that close real gaps over a sweep that documents the
repository back to itself.

## 8. Verify and report

Re-run the audit. Every repair should have cleared its flag, and nothing that was
clean should have become flagged.

Report per group: atoms repaired, atoms rewritten, atoms deleted with the reason,
atoms created, and anything flagged but deliberately left alone with why. Name the
atoms whose flags survive the pass — a dead source you decided not to chase is a
finding, not a silence.
