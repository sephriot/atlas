# Local telemetry

## Task Spec

1. Goal
- Record Atlas retrieval use and caller feedback locally without adding a network service or repository files.

2. Success Criteria
- `atlas telemetry status` reports telemetry enabled by default and the local journal path.
- Repeating `atlas telemetry enable` or `atlas telemetry disable` succeeds and leaves the requested state unchanged.
- Successful searches and reads append redacted events under `<atlas storage>/telemetry/`.
- `atlas feedback` records a verdict against a search result, and `atlas telemetry clear` removes the journal.

3. Scope
- In: Local configuration and JSONL journal, telemetry CLI commands, feedback command, search/get instrumentation, CLI integration tests, and README documentation.
- Out: Remote telemetry, raw query text, atom details, path capture, automatic feedback, retention limits, and analytics reports.

4. Constraints
- Telemetry is enabled by default but never leaves the local machine.
- Failed telemetry writes must not change a successful Atlas command into a failure.
- Configuration and events must stay outside repository-local `.atlas/` state when Atlas uses its default storage.

5. Assumptions
- `ATLAS_STORAGE` is the appropriate testable override for the telemetry root.
- Atom IDs are useful local correlation data and are acceptable to record.

6. Clarifications
- Telemetry starts enabled by default.
- `enable` and `disable` are idempotent.
- Feedback is a dedicated `atlas feedback` command. A search ID is optional so callers of `search --ids` can give result-only feedback.

7. Interfaces Impacted
- `src/cli.rs`: command surface and command dispatch.
- `src/main.rs`: telemetry module registration.
- `src/telemetry.rs`: local journal and state operations.
- `tests/cli_write.rs`: end-to-end command contracts.
- `README.md`: command and privacy documentation.

8. Verification Intent
- Use end-to-end CLI tests with an isolated `--storage` directory. Verify default state, idempotence, redaction, feedback persistence, and journal clearing.
