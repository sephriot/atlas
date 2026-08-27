# Telemetry retention tasks

1. [X] T1: Add failing tests for journal trimming, daily pruning, aggregate output, and clear behavior. [blocks: T2]
- Files: `src/telemetry.rs`, `tests/cli_write.rs`
- Acceptance: New tests fail because metrics and bounded retention do not exist.

2. [X] T2: Implement bounded journal and aggregate metrics. [blocks: T3]
- Files: `src/telemetry.rs`
- Acceptance: Unit tests prove complete-line trimming and 90-day pruning.

3. [X] T3: Expose metrics and update documentation. [blocks: T4]
- Files: `src/cli.rs`, `README.md`
- Acceptance: Integration test reads counters and clear removes all telemetry data.

4. [X] T4: Run formatting, tests, Clippy, and diff checks.
- Files: `docs/superpowers/tasks/2026-08-27-telemetry-retention.md`
- Acceptance: All checks pass without warnings.
