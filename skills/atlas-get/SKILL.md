---
name: atlas-get
description: "Retrieve knowledge from Atlas long-term memory. Use when starting any task, answering questions about the codebase, or needing context about patterns, gotchas, decisions, or conventions. Triggers on: `what do we know about X`, `check Atlas for`, `search knowledge`, `find patterns`, `any gotchas`, or at the start of any non-trivial task."
---

# Atlas Get

Retrieve relevant knowledge from Atlas before taking action.

## Context Check

Before searching, verify Atlas context:

0. **If `atlas` is not installed** (`command not found`), stop here. Report `Atlas: unavailable — CLI not found` under the `Atlas` response artifact and proceed without the Retrieve phase. Do not treat a missing binary as a blocker, and never cite an atom ID you did not read.
1. **Run `atlas context`** to see current org/project
2. **If context is unknown** (`source: fallback` or missing org/project):
   - **Run `atlas projects`** to list known projects
   - **If a project name matches** the repo you are working on, rerun Atlas commands with `--org <org> --project <project>`
   - **If no matching project exists**, do not attach to a plausible-looking neighbour. Report `Atlas: no project for this repo` and proceed without Retrieve; guessing writes atoms where nobody will find them.
3. **Check the `source` field**:
   - `env_vars`, `local_storage`, or `git_remote` = Good to proceed

This prevents searching/recording in the wrong project (e.g., `global/tmp`).

## Workflow

1. **Extract keywords** from the task/question
2. **Search the current project** with those keywords using `atlas search <query>`; use `--scope <org>` only when cross-project knowledge is needed
3. **Read full atoms** with `atlas get <ID>` for each relevant hit
4. **Cite atoms** in your response using `[K-XXXXXX]` format

## CLI Commands

**Broad discovery:**
```bash
atlas search "error handling" --page-size 10
```

**Cross-project discovery:**
```bash
atlas search "error handling" --scope acme --page-size 10
```

**Type-specific:**
```bash
atlas search --type gotcha --tag api
```

**High-confidence only:**
```bash
atlas search "authentication" --confidence high
```

**Get full atom content:**
```bash
atlas get K-000012
```

`atlas get --format json` omits a field entirely when it is empty, so an absent `links` key means the atom has none — not that the read failed.

**List the store instead of searching it**, for an audit or to see what exists:
```bash
atlas atoms --scope <org>/<project> --type gotcha
atlas atoms --ids
```

`atlas atoms` returns summary fields only: no `details`, no `sources`, no `links`. Read the full atoms with `atlas get`, or a bulk review sees every atom as sourceless.

**Pipeline usage:**
```bash
printf '%s\n' "error handling" | atlas search --ids | xargs atlas get
```

## Output Format

Always report what was found:

```
**Atlas:**
- [K-000012] Error Handling: Applied retry pattern from this recipe
- [K-000045] API Gotcha: Avoided rate limit issue per this warning

**Knowledge Gaps:**
- No existing pattern for X
```

If nothing relevant found, state: "No relevant Atlas knowledge found for [keywords]."
