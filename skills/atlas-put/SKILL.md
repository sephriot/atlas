---
name: atlas-put
description: "Record new knowledge to Atlas long-term memory. Use after completing non-trivial work (bug fixes, features, refactors), discovering patterns or gotchas, making architectural decisions, or learning something reusable. Triggers on: `record this`, `save to Atlas`, `remember this pattern`, `add a gotcha`, or when suggesting knowledge worth preserving."
---

# Atlas Put

Capture reusable learnings in Atlas.

Never hand-edit the store. Create, update, link, and delete through the CLI, including a
one-word typo fix: the files carry identity and index state that a direct edit corrupts
silently.

## When Recording Is Required

Record when any of these is true:

1. A bug fix, feature, or refactor was completed.
2. A new gotcha or reusable pattern was discovered.
3. A meaningful technical decision was made.
4. An alternative approach was raised and left without a verdict. Record it as a `decision` whose title opens with `Open:` and whose details name who raised it and what would settle it. A verdict that lives only in a thread or a task list dies with the session; the next `atlas-get` is the only place a later session reliably looks.

Otherwise, a concrete skip reason in the final response is enough. Report the resulting atom ID, or that skip reason, under the `Atlas` response artifact either way.

## Context Check

Before recording, verify Atlas context:

1. **Run `atlas context`** to see current org/project
2. **If context is unknown** (`source: fallback` or missing org/project):
   - **Run `atlas projects`** to list known projects
   - **If a project name matches** the repo you are working on, rerun Atlas commands with `--org <org> --project <project>`
   - **If no matching project exists**, stop. Atlas refuses fallback-context writes; ask for explicit org/project context
3. **Check the `source` field**:
   - `env_vars`, `local_storage`, or `git_remote` = Good to proceed

Recording to the wrong project makes knowledge unfindable.

## Atom Types

| Type | Use for |
|------|---------|
| `note` | Verified facts, conventions, domain knowledge |
| `gotcha` | Warnings, pitfalls, "watch out for X" |
| `recipe` | Patterns, how-to guides, code snippets |
| `decision` | Architectural choices, why X over Y |

## Workflow

1. **Search first** - Avoid duplicates with `atlas search <query>` or pipe generated notes into `atlas search`
2. **Evaluate** - Is this reusable, non-obvious, stable, actionable?
3. **Create atom** with `atlas create`, or revise an existing atom with `atlas update <id>`, preferring stdin for long or already-generated text:
   - `--title`: Clear, searchable name (required)
   - `--type`: gotcha, recipe, decision, or note (required)
   - `--confidence`: high, medium, or low (required)
   - `--summary`: Short one-line summary; omit it to read summary from stdin
   - use `--details -` to read details from stdin, preserving markdown and avoiding shell quoting problems
   - `--tag`: Keywords for searchability (repeatable)
   - `--source`: Relevant file paths (repeatable)
   - `--link`: Atoms this one relates to (repeatable), per Linking below

**`atlas update` is a patch.** Omitted fields stay unchanged. Use
`--clear details`, `--clear tags`, `--clear sources`, or `--clear pitfalls` to
remove stored data intentionally. Update takes no link fields — it patches one atom,
and an edge belongs to two. Change edges on an existing atom with `atlas link` and
`atlas unlink`.

**An update reports the atom's edges back, so read them.** Changing what an atom says
can outdate why it was linked: an edge that pointed at the reason for the old claim may
have no bearing on the new one. Every update prints the atoms this one still references —
that list is a question, not a receipt. Keep the edges that still explain something and
`atlas unlink` the rest, in the same pass as the rewrite rather than leaving it for a
future audit.

## Correcting Retrieved Atoms

Correct an atom proactively when retrieval feedback and authoritative evidence show that it is misleading or stale. Do not wait for a separate curation task.

1. Submit the matching `atlas feedback` event first. Include the search ID and a short, non-sensitive note when available; feedback records the retrieval problem but does not alter the atom.
2. Read the full atom with `atlas get <id> --format json`. Feedback alone is not evidence for a correction.
3. Update only the fields the authoritative evidence changes. Preserve existing tags, sources, and links unless the correction requires changing them.
4. Read the atom again, confirm its revised claim and sources, and reassess every reported edge. Use `atlas unlink` for edges that no longer explain the atom.

## Linking

Pass `--link` on `create` and the atom arrives with its edges. That is the point:
recording and linking as one command, because linking as a second command is the step
that gets skipped. It is repeatable, and it fails without writing anything when a named
atom does not exist, so a rejected create is safe to retry.

An edge is mutual whichever command writes it — `--link`, or `atlas link` on two atoms
that already exist. One call, followable from either end, no direction to get right.

Give every new atom at least one edge when one exists: the gotcha the decision
avoids, the recipe this fix generalizes, the atom this one supersedes. An atom with
no edges is reachable only by guessing its keywords.

**Reach into a sibling project when the work did.** An edge across projects is the
only cheap path from a project-scoped search into another repository's knowledge, and
links stay inside one org. Search `--scope <org>` before recording when any of these
holds:

- The change was driven by an API, schema, or convention another repository owns
- The cause sits in one repository and the symptom in another
- One fix, migration, or incident touched more than one repository

Name a cross-project target by its project: `--link backend/K-000031`, or
`atlas link K-000012 backend/K-000031`. A bare ID always means the current project.

Prefer a few useful edges. Linking everything that shares a tag is traversal noise.

## Stdin and Pipe Preference

Use shell pipelines as the default path when recording anything longer than a sentence, anything with markdown/code blocks, or anything produced by another command. Inline `--summary` is best for short summaries; stdin is best for details.

**Deliver details through a file, not a heredoc.** Write the details to a scratch file with the file-writing tool, then `cat <file> | atlas create ... --details -`. That pipeline is a plain two-command pipeline, so a host that checks command shapes can approve it from prefix rules alone; a heredoc cannot be analyzed statically and forces a manual approval every time.

**Inline arguments must be free of shell metacharacters.** `--title` and `--summary` are shell-quoted arguments: inside double quotes, `$(...)` and backticks still execute. An atom describing shell syntax will run that syntax while being recorded. Keep inline text plain, single-quote it when it contains no apostrophe, and move anything with `$`, backticks, or quotes into the details file.

`printf` is not a safer substitute for either rule: a double-quoted payload expands, and a single-quoted one breaks on the first apostrophe.

Atlas stdin behavior:
- `atlas search` with no query reads the query from stdin
- `atlas create` with no `--summary` reads stdin as the summary
- `atlas create --summary "..." --details -` reads piped stdin as `details`
- `atlas create --summary -` reads stdin as the summary explicitly
- Only one of `--summary -` or `--details -` should consume stdin
- Updates use `atlas update <atom-id>`, and the ID is the only identity — titles are not
- Updates accept only the fields being changed; `--details -` remains the explicit stdin path for updates

## CLI Commands

**Search for duplicates:**
```bash
atlas search "error handling"
```

**Search for duplicates from generated text:**
```bash
printf '%s\n' "$LEARNING" | atlas search --page-size 5
```

**Create new atom:**
```bash
atlas create \
  --title "API rate limits require exponential backoff" \
  --type gotcha \
  --confidence high \
  --summary "The external API enforces strict rate limits..." \
  --tag api --tag rate-limiting \
  --source src/api/client.rs
```

**Create summary from stdin:**
```bash
printf '%s\n' "External API calls must use exponential backoff after 429s." \
  | atlas create \
      --title "API rate limits require exponential backoff" \
      --type gotcha \
      --confidence high \
      --tag api --tag rate-limiting \
      --source src/api/client.rs
```

**Create details from stdin, with edges:**
```bash
cat notes.md | atlas create \
  --title "API rate limits require exponential backoff" \
  --type gotcha \
  --confidence high \
  --summary "The external API enforces strict rate limits..." \
  --details - \
  --tag api --tag rate-limiting \
  --link K-000012 --link backend/K-000031
```

**Capture command output as details:**
```bash
git show --stat --oneline HEAD | atlas create \
  --title "Recent migration shape" \
  --type note \
  --confidence medium \
  --summary "Migration changed the storage and CLI boundaries." \
  --details - \
  --tag migration
```

**Revise an existing atom:**
```bash
cat notes.md | atlas update K-000012 \
  --details -
```

**Link two atoms to each other:**
```bash
atlas link K-000012 K-000045
atlas link K-000012 backend/K-000031
```

**Remove an edge from both sides:**
```bash
atlas unlink K-000012 K-000045
```

**Delete an atom** — only when it is useless, not merely outdated. Prefer `atlas update`, because an atom whose text went stale still records why a decision was made:
```bash
atlas delete K-000012
```

Deletion also unlinks the atom from everything that referenced it and reports those atoms as `detached`, so no half-edges survive it. Read that list: an atom that several others leaned on is usually worth rewriting instead of removing. Where the store lives inside a git repository, commit it before deleting so the removal stays recoverable.

## Quality Criteria

Only record if knowledge is:
- **Reusable** - Applies beyond this specific instance
- **Non-obvious** - Not easily discoverable from code/docs
- **Stable** - Unlikely to change frequently
- **Actionable** - Helps make decisions or avoid mistakes
- **Source-backed** - A claim about a pre-existing system, event, provenance, or reachability identifies its supporting source. A constructed test proves code behavior for its input, not that the input existed in production. Record an unsupported claim as a conditional hypothesis and name the evidence still needed.

## Anti-Patterns

- Creating atoms for trivial/obvious information
- Duplicating existing knowledge
- Recording temporary workarounds as permanent
- Atoms too specific to be reusable

Creating nothing is better than creating low-value atoms.
