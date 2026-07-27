# Task Spec

## 1. Goal

Reduce Atlas CLI parameter bloat while preserving the workflows coding agents use most.

## 2. Success Criteria

- `atlas update K-000001 --summary "..."` updates the named atom without `--id`.
- `atlas update K-000001 --clear details --clear tags` clears the named fields.
- Atom links are created and removed only through `atlas link` and `atlas unlink`.
- `atlas enable-local` takes its org/project from normal context detection and rejects fallback context.
- Global help no longer exposes `--json` or `--project-root`; `enable-local --root` remains available.
- CLI help, README, global skills, and portable skill copies describe only the new interface.

## 3. Scope

- In: CLI argument definitions and conversion, local-storage context plumbing, integration tests, README, Atlas skills, and their checked-in portable copies.
- Out: atom storage format, search/list filter semantics, output formats, client instruction variants, and the user-owned `.gitignore` change.

## 4. Constraints

- Breaking CLI changes are allowed: the user explicitly approved no compatibility layer in the prior request.
- Keep explicit write-context safety and inbound-link deletion protection.
- Update global skill sources; sync their portable copies without disclosing local source paths in public documentation.

## 5. Assumptions

- `type`, `confidence`, tags, sources, and pitfalls remain useful atom metadata rather than parameter bloat.
- Pagination and filtering remain useful read-side controls.

## 6. Clarifications

- No unresolved clarification. User confirmed the proposed scope on 2026-07-27.

## 7. Interfaces Impacted

- `src/main.rs`: global command-line options.
- `src/cli.rs`: command arguments and request conversion.
- `src/tools/atoms.rs`: local-storage request/context boundary.
- `tests/cli_write.rs`: externally observable command behavior.
- `README.md`, `skills/atlas-*/SKILL.md`, and global Atlas skill sources: command examples and guidance.

## 8. Verification Intent

- Add integration tests before implementation, observe them fail on the old interface, then run the full Rust test suite, formatter, linter, help output, and skill-copy synchronization diff.
