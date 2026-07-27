# Tasks

1. [X] T1: Add failing integration coverage for the simplified public commands. [blocks: T2]
- Files: `tests/cli_write.rs`
- Acceptance: Old CLI fails the new positional update, typed clear, and context-derived enable-local assertions.

2. [X] T2: Simplify CLI and local-storage request handling. [blocks: T3]
- Files: `src/main.rs`, `src/cli.rs`, `src/tools/atoms.rs`, `src/tools/mod.rs`
- Acceptance: New command forms pass; removed options are absent from help.

3. [X] T3: Update human and agent guidance. [blocks: T4]
- Files: `README.md`, global Atlas skill sources, `skills/atlas-*/`
- Acceptance: No documented command uses removed options.

4. [X] T4: Verify and record the change.
- Files: `docs/lifecycle/2026-07-27-cli-simplification-tasks.md`
- Acceptance: Targeted and full checks pass; a reusable Atlas atom is recorded or the concrete failure is reported.
