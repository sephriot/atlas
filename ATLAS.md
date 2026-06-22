# Atlas-Driven Agent Context

You are a knowledge-aware agent. `atlas` is your long-term memory for project context, patterns, gotchas, and decisions.

## Compliance Rules

1. **Retrieve before planning** - Search Atlas before forming an implementation plan.
2. **Read full atoms** - Search returns summaries; run `atlas get <id>` before applying a result.
3. **Record reusable learnings** - Use `atlas create` for new atoms and `atlas update --id <id>` for existing atoms.
4. **Cite atom IDs** - Reference atoms in reasoning with IDs such as `[K-000042]`.
5. **Do not edit `.atlas/` directly** - Use the CLI.

## CLI Commands

| Command | Purpose | When to use |
|---------|---------|-------------|
| `atlas search <query>` | Find atoms by query/tags | First action for any task |
| `atlas get <id>` | Retrieve full content | After search finds relevant hits |
| `atlas create ...` | Create a new atom | When learning something reusable |
| `atlas update --id <id> ...` | Update an existing atom | When revising known knowledge |
| `atlas atoms` | Browse by type/tags | Exploring known memory |
| `atlas delete <id>` | Remove obsolete atoms | Cleaning outdated knowledge |
| `atlas link <source> <target>` | Create directed link | Connecting related atoms |
| `atlas unlink <source> <target>` | Remove directed link | Disconnecting atoms |
| `atlas context` | Check current org/project | Verify context detection |
| `atlas projects` | List projects | Find known org/project names |
| `atlas instructions --client codex` | Print agent usage guidance | Bootstrapping an agent prompt |

Aliases: `find` for `search`, `read` for `get`, and `list` for `atoms`.

Updates are explicit by ID. Do not expect title matching: titles are mutable
metadata and may be duplicated.

## Context Verification

Before recording or applying project-specific knowledge:

1. Run `atlas context`.
2. If `source: fallback` is wrong, run `atlas projects`.
3. Rerun commands with explicit context:

```bash
atlas --org myorg --project myproj search "error handling"
```

Detection priority:

1. `--org/--project` or `ATLAS_ORG`/`ATLAS_PROJECT`
2. Repo-local `.atlas/config.yaml`
3. Git remote
4. Fallback `global/{directory_name}`

## Atom Types

| Type | Use for |
|------|---------|
| `note` | Verified facts, observations, domain knowledge |
| `gotcha` | Warnings, pitfalls, things to avoid |
| `recipe` | Patterns, how-to guides, code snippets |
| `decision` | Architectural choices, why X over Y |

## Mandatory Workflow

### Phase 1: Retrieval

1. Extract key concepts from the request.
2. Search with `atlas search "<keywords>"`.
3. Read useful hits with `atlas get <id>`.
4. Track gaps for possible recording later.

### Phase 2: Planning

1. Let retrieved atoms constrain the approach.
2. Flag conflicts between the request and known decisions/gotchas.
3. Cite atom IDs that inform the plan.

### Phase 3: Execution

1. Search again when uncertain.
2. Follow retrieved recipes and decisions.
3. Avoid retrieved gotchas.
4. Keep citations available for the final explanation.

### Phase 4: Consolidation

Create a new atom or update an existing atom by ID when you complete non-trivial work, discover a reusable pattern, hit an unexpected issue, or make an architectural decision.

```bash
atlas create \
  --title "API rate limits require exponential backoff" \
  --type gotcha \
  --confidence high \
  --summary "The external API enforces strict rate limits..." \
  --tag api --tag rate-limiting \
  --source src/api/client.rs
```

## Pipe-Friendly Patterns

```bash
# Search from stdin
printf '%s\n' "database migrations rollback" | atlas search

# Compose with Unix tools
atlas search "auth middleware" --ids | xargs atlas get

# Fetch IDs from stdin
printf '%s\n' K-000001 K-000002 | atlas get -

# Record stdin as details explicitly
cat notes.md | atlas create \
  --title "Resolver convention" \
  --type recipe \
  --confidence high \
  --summary "Resolvers validate input before storage calls." \
  --details - \
  --tag graphql
```

## Output Format

Always report knowledge context:

```text
Atlas Context:
- [K-000012] Error Handling: Applied retry pattern from this recipe
- [K-000045] API Gotcha: Avoided rate limit issue per this warning

Knowledge Gaps:
- No existing pattern for X
```

If nothing relevant is found, state: `No relevant Atlas knowledge found for <keywords>.`
