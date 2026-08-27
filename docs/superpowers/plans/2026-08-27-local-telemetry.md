# Local telemetry plan

1. Add a local telemetry module that reads an enabled-by-default configuration and appends JSONL events below the configured Atlas storage root.
2. Add `telemetry status`, `enable`, `disable`, and `clear` commands. Keep state changes idempotent.
3. Add `feedback` with an optional search ID so both normal search output and `search --ids` callers can submit feedback.
4. Instrument successful `search` and `get` operations. Ignore journal failures in these retrieval paths so telemetry cannot break knowledge access.
5. Add end-to-end command tests and document the data boundary: local-only, no raw query text, contents, paths, or stdin input.

## Simplicity check

PASS. The design adds one module and two command surfaces. It does not add a service, queue, database, export path, retention policy, or telemetry for unrelated write commands.
