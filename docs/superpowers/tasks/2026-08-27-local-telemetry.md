# Local telemetry tasks

1. [X] T1: Add failing end-to-end tests for default state, idempotent configuration, feedback, and journal clearing. [blocks: T2]
- Files: `tests/cli_write.rs`
- Acceptance: New tests fail because the telemetry commands and search correlation ID do not exist.

2. [X] T2: Add local telemetry storage and CLI commands. [blocks: T3]
- Files: `src/telemetry.rs`, `src/main.rs`, `src/cli.rs`
- Acceptance: Tests prove configuration changes and journal operations use the isolated storage root.

3. [X] T3: Instrument retrieval and add feedback persistence. [blocks: T4]
- Files: `src/cli.rs`, `src/telemetry.rs`
- Acceptance: Tests prove the journal has no raw search text and feedback refers to the search ID.

4. [X] T4: Document the local-only contract and run the full Rust checks.
- Files: `README.md`, `docs/superpowers/tasks/2026-08-27-local-telemetry.md`
- Acceptance: Documentation matches the implemented commands; formatting, tests, and Clippy pass.
