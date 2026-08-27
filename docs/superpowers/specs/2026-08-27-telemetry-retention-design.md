# Telemetry retention

## Task Spec

1. Goal
- Bound detailed telemetry storage while retaining useful long-term usage and feedback signals.

2. Success Criteria
- `events.jsonl` keeps only the newest complete events within 10 MiB.
- `metrics.yaml` records all-time counters and no more than 90 daily buckets.
- `atlas telemetry metrics` exposes aggregate counters without raw event fields.
- `atlas telemetry clear` removes both the detailed journal and aggregate metrics.

3. Scope
- In: Local journal trimming, aggregate metrics, CLI metrics output, tests, and documentation.
- Out: Remote export, configurable retention, metrics for unrelated write commands, and raw query aggregation.

4. Constraints
- Preserve the local-only and redaction rules from [K-000010].
- A telemetry failure must not fail a successful retrieval command.

5. Clarifications
- Detailed journal limit: 10 MiB.
- Daily aggregate retention: 90 days.
- All-time aggregate counters remain after daily buckets expire.

6. Interfaces Impacted
- `src/telemetry.rs`, `src/cli.rs`, `tests/cli_write.rs`, and `README.md`.

7. Verification Intent
- Test event trimming, daily-bucket pruning, aggregate command output, and complete clearing.
