---
name: atlas-record
description: "Record new knowledge to Atlas long-term memory. Use after completing non-trivial work (bug fixes, features, refactors), discovering patterns or gotchas, making architectural decisions, or learning something reusable. Triggers on: `record this`, `save to Atlas`, `remember this pattern`, `add a gotcha`, or when suggesting knowledge worth preserving."
---

# Atlas Record

Capture reusable learnings in Atlas.

## Context Check

Before recording, verify Atlas context:

1. **Run `atlas context`** to see current org/project
2. **If context is unknown** (`source: fallback` or missing org/project):
   - **Run `atlas projects`** to list known projects
   - **If a project name matches** the repo you are working on, rerun Atlas commands with `--org <org> --project <project>`
   - **If no matching project exists**, infer org/project from the file path and pass them with `--org <org> --project <project>`
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
3. **Create atom** with `atlas upsert`, preferring stdin for long or already-generated text:
   - `--title`: Clear, searchable name
   - `--type`: gotcha, recipe, decision, or note
   - `--summary`: Short one-line summary; omit it to read summary from stdin
   - stdin with `--summary` set becomes `details`, preserving markdown and avoiding shell quoting problems
   - `--confidence`: high, medium, or low
   - `--tag`: Keywords for searchability (repeatable)
   - `--source`: Relevant file paths (repeatable)
4. **Link related atoms** with `atlas link <SOURCE> <TARGET>` if applicable

## Stdin and Pipe Preference

Use shell pipelines as the default path when recording anything longer than a sentence, anything with markdown/code blocks, or anything produced by another command. Inline `--summary` is best for short summaries; stdin is best for details.

Atlas stdin behavior:
- `atlas search` with no query reads the query from stdin
- `atlas upsert` with no `--summary` reads stdin as the summary
- `atlas upsert --summary "..."` reads piped stdin as `details`
- `atlas upsert --summary -` reads stdin as the summary explicitly
- Only one of `--summary -` or `--details -` should consume stdin

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
atlas upsert \
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
  | atlas upsert \
      --title "API rate limits require exponential backoff" \
      --type gotcha \
      --confidence high \
      --tag api --tag rate-limiting \
      --source src/api/client.rs
```

**Create details from stdin:**
```bash
cat notes.md | atlas upsert \
  --title "API rate limits require exponential backoff" \
  --type gotcha \
  --confidence high \
  --summary "The external API enforces strict rate limits..." \
  --tag api --tag rate-limiting
```

**Capture command output as details:**
```bash
git show --stat --oneline HEAD | atlas upsert \
  --title "Recent migration shape" \
  --type note \
  --confidence medium \
  --summary "Migration changed the storage and CLI boundaries." \
  --tag migration
```

**Link related atoms:**
```bash
atlas link K-000012 K-000045
```

## Quality Criteria

Only record if knowledge is:
- **Reusable** - Applies beyond this specific instance
- **Non-obvious** - Not easily discoverable from code/docs
- **Stable** - Unlikely to change frequently
- **Actionable** - Helps make decisions or avoid mistakes

## Anti-Patterns

- Creating atoms for trivial/obvious information
- Duplicating existing knowledge
- Recording temporary workarounds as permanent
- Atoms too specific to be reusable

Creating nothing is better than creating low-value atoms.
