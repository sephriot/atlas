# Atlas

CLI knowledge and memory manager for coding agents. Atlas stores small, searchable knowledge atoms under an `org/project` hierarchy so tools like Codex, Claude Code, and shell scripts can retrieve project memory with normal command-line workflows.

## Features

- **CLI-only workflow** - no server process or transport setup
- **Pipe-friendly commands** - search from stdin, emit IDs, fetch IDs from stdin
- **Automatic context detection** - uses `--org/--project`, repo-local `.atlas/`, git remote, then fallback
- **Hierarchical storage** - atoms live in `~/.atlas/orgs/{org}/{project}/atoms/`
- **Version-controlled storage** - store atoms in a repo-local `.atlas/` directory with `enable-local`
- **Mutual cross-project links** - linked atoms reference each other, across projects within the same org
- **Simple atom model** - 4 types: note, gotcha, recipe, decision

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

The binary is named `atlas`.

## Quick Start

```bash
# Verify context
atlas context

# Search the org's memory, current project ranked first
atlas search "error handling" --page-size 10

# Fetch a full atom
atlas get sephriot/atlas/K-000001

# Create a short atom, linked to what it relates to
atlas create \
  --title "API clients require explicit timeouts" \
  --type gotcha \
  --confidence high \
  --summary "Network clients should set explicit timeouts; defaults may hang." \
  --tag api --tag timeout \
  --source src/api/client.rs \
  --link K-000042 --link backend/K-000031
```

## Pipe Workflows

```bash
# Search using piped text as the query
printf '%s\n' "database migration rollback" | atlas search

# Emit only IDs for shell composition
atlas search "auth middleware" --ids

# Fetch search results through xargs
atlas search "auth middleware" --ids | xargs atlas get

# Fetch IDs supplied on stdin
printf '%s\n' K-000001 K-000002 | atlas get -

# Use stdin as the summary when --summary is omitted
printf '%s\n' "Remember to regenerate snapshots after parser changes." \
  | atlas create --title "Parser snapshots" --type gotcha --confidence medium --tag parser

# Use stdin as details explicitly
cat notes.md | atlas create \
  --title "GraphQL resolver convention" \
  --type recipe \
  --confidence high \
  --summary "Resolvers validate input before storage calls." \
  --details - \
  --tag graphql --tag resolver
```

## Commands

| Command | Purpose |
|---------|---------|
| `search` / `find` | Search atoms by query, type, tags, confidence |
| `get` / `read` | Get one or more full atoms by ID |
| `atoms` / `list` | List atoms with optional filters |
| `create` | Create a new atom |
| `update` | Update an existing atom by ID |
| `delete` | Delete an atom and unlink it from everything that referenced it |
| `link` | Link two atoms so each one references the other |
| `unlink` | Remove the references two atoms hold to each other |
| `projects` | List all projects |
| `context` | Show detected org/project context |
| `instructions` | Print agent instructions for using Atlas |
| `enable-local` | Use repo-local `.atlas/` storage for a project |
| `telemetry` | Manage local telemetry collection |
| `feedback` | Record caller feedback about a search result |

Updates are explicit: use `atlas update <atom-id> ...`. Atlas does not match
atoms by title because titles are mutable metadata, not stable identity.

Updates are patches: fields omitted from the command stay unchanged. Use
`--clear details`, `--clear tags`, `--clear sources`, or `--clear pitfalls` to
remove stored data. Use `atlas link` and `atlas unlink` to change atom links.

Links are mutual. `atlas link` writes the reference on both atoms, so either one
leads to the other; `atlas unlink` removes both. Re-running `link` on a pair that
only references each other one way restores the missing half. `delete` unlinks
the atom from every atom that referenced it and reports which ones it detached.

`create --link <atom-id>` records an atom together with its edges, repeatable, and
fails without writing anything if a named atom does not exist. `update` takes no
link fields: it patches one atom, and an edge belongs to two. It does report the
atom's edges back, because rewriting what an atom says can outdate why it was
linked — review that list and `atlas unlink` what no longer holds.

Global options:

```bash
atlas --org my-company --project my-service search "error handling"
atlas --storage /path/to/knowledge atoms
atlas --format json search "auth"
```

## Atom Model

```yaml
id: K-000001
title: "Error handling pattern"
type: recipe
confidence: high
summary: "Brief explanation"
details: "Extended content"
pitfalls: ["Watch out..."]
tags: [rust, error-handling]
sources: [src/lib.rs]
links: [K-000042, api/K-000010]
updated_at: 2026-01-28
```

| Type | Purpose |
|------|---------|
| `note` | Facts, observations, general knowledge |
| `gotcha` | Warnings, pitfalls, things to avoid |
| `recipe` | How-to guides, patterns, code snippets |
| `decision` | Architectural rationale, why X over Y |

## Context Detection

Atlas detects the current project context in this order:

| Priority | Source | Description |
|----------|--------|-------------|
| 1 | CLI/env | `--org` and `--project`, or `ATLAS_ORG` and `ATLAS_PROJECT` |
| 2 | Local storage | `.atlas/config.yaml` in the current working directory |
| 3 | Git remote | Parses `git remote get-url origin` |
| 4 | Fallback | Uses `global/{directory_name}` |

Supported git URL formats include:

- `git@github.com:org/project.git`
- `https://github.com/org/project.git`
- `git@gitlab.com:org/project.git`

Fallback context is read-only. Pass explicit context before creating, updating,
deleting, linking, or unlinking atoms:

```bash
atlas --org my-company --project my-service context
```

Search covers the whole org by default and ranks the current project first, so a
sibling repository's knowledge is reachable without asking for it. Narrow to one
project when the cross-project results are noise:

```bash
atlas search "error handling" --scope my-company/my-service
```

`atlas atoms` is the opposite: an inventory of the current project unless a scope
names another project, or an org whose projects should all be listed.

## Storage

Default central storage:

```text
~/.atlas/
└── orgs/
    └── {org}/
        └── {project}/
            ├── index.yaml
            └── atoms/
                └── K-XXXXXX.yaml
```

Repo-local storage:

```bash
atlas enable-local

# Or choose a different repo root and explicit context
atlas --org my-company --project my-service enable-local --root /path/to/repo
```

This creates:

```text
{repo}/.atlas/
├── index.yaml
└── atoms/
    └── K-XXXXXX.yaml

~/.atlas/orgs/{org}/{project} -> {repo}/.atlas/
```

## Telemetry

Atlas records local telemetry by default under `~/.atlas/telemetry/`. It never sends this data to a remote service. When `--storage` or `ATLAS_STORAGE` is set, telemetry uses that storage root instead.

Events record search result counts and IDs, atom IDs read with `get`, and explicit feedback. They do not record raw search text, atom contents, source paths, or stdin input. A feedback note is stored only when the caller supplies one.

```bash
# Check the current state and journal location
atlas telemetry status

# These commands are idempotent
atlas telemetry enable
atlas telemetry disable

# Remove the local event journal
atlas telemetry clear

# Record feedback using the search_id returned by `atlas search`
atlas feedback S-123 --result sephriot/atlas/K-000001 --verdict helpful

# `--ids` searches do not return a search_id, so feedback may refer to an atom alone
atlas feedback --result sephriot/atlas/K-000001 --verdict misleading
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ATLAS_ORG` | Override organization |
| `ATLAS_PROJECT` | Override project |
| `ATLAS_CWD` | Override working directory for git context detection |
| `ATLAS_STORAGE` | Override storage root, default `~/.atlas` |

## Development

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```
