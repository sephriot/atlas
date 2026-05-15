# Atlas

CLI knowledge and memory manager for coding agents. Atlas stores small, searchable knowledge atoms under an `org/project` hierarchy so tools like Codex, Claude Code, and shell scripts can retrieve project memory with normal command-line workflows.

## Features

- **CLI-only workflow** - no server process or transport setup
- **Pipe-friendly commands** - search from stdin, emit IDs, fetch IDs from stdin
- **Automatic context detection** - uses `--org/--project`, repo-local `.atlas/`, git remote, then fallback
- **Hierarchical storage** - atoms live in `~/.atlas/orgs/{org}/{project}/atoms/`
- **Version-controlled storage** - store atoms in a repo-local `.atlas/` directory with `enable-local`
- **Cross-project links** - reference atoms across projects within the same org
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

# Search memory
atlas search "error handling" --page-size 10

# Fetch a full atom
atlas get sephriot/atlas/K-000001

# Record a short atom
atlas upsert \
  --title "API clients require explicit timeouts" \
  --type gotcha \
  --confidence high \
  --summary "Network clients should set explicit timeouts; defaults may hang." \
  --tag api --tag timeout \
  --source src/api/client.rs
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
  | atlas upsert --title "Parser snapshots" --type gotcha --confidence medium --tag parser

# Use stdin as details when --summary is set
cat notes.md | atlas upsert \
  --title "GraphQL resolver convention" \
  --type recipe \
  --confidence high \
  --summary "Resolvers validate input before storage calls." \
  --tag graphql --tag resolver
```

## Commands

| Command | Purpose |
|---------|---------|
| `search` / `find` | Search atoms by query, type, tags, confidence |
| `get` / `read` | Get one or more full atoms by ID |
| `atoms` / `list` | List atoms with optional filters |
| `upsert` / `record` | Create or update an atom |
| `delete` | Delete an atom |
| `link` | Create a directed link between atoms |
| `unlink` | Remove a directed link |
| `projects` | List all projects |
| `context` | Show detected org/project context |
| `instructions` | Print agent instructions for using Atlas |
| `enable-local` | Use repo-local `.atlas/` storage for a project |

Global options:

```bash
atlas --org my-company --project my-service search "error handling"
atlas --storage /path/to/knowledge atoms
atlas --format json search "auth"
atlas --json get K-000001
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

When fallback is wrong, pass explicit context:

```bash
atlas --org my-company --project my-service context
```

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
atlas enable-local --org my-company --project my-service
```

This creates:

```text
{repo}/.atlas/
├── index.yaml
└── atoms/
    └── K-XXXXXX.yaml

~/.atlas/orgs/{org}/{project} -> {repo}/.atlas/
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ATLAS_ORG` | Override organization |
| `ATLAS_PROJECT` | Override project |
| `ATLAS_PROJECT_ROOT` | Project root for local storage |
| `ATLAS_CWD` | Override working directory for git context detection |
| `ATLAS_STORAGE` | Override storage root, default `~/.atlas` |

## Development

```bash
cargo fmt --check
cargo test
cargo clippy -- -D warnings
```
