# Telemetry retention plan

1. Add metrics data types and update them for each recorded search, get, and feedback event.
2. Store aggregate data in `metrics.yaml`, keeping all-time counters and the last 90 daily buckets.
3. Trim `events.jsonl` after each append, preserving only whole newest entries within 10 MiB.
4. Add `atlas telemetry metrics` and make `atlas telemetry clear` remove both telemetry files.
5. Cover compaction, aggregate data, and clear behavior with tests and document the policy.

## Simplicity check

PASS. One small YAML aggregate replaces no data and adds no database, scheduler, or retention configuration.
