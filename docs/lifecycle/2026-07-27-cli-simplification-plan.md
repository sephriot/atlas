# CLI Simplification Plan

1. Replace update's flag-only ID and five dedicated clearing flags with positional ID and repeatable typed `--clear`.
2. Remove inline link fields from create/update so the existing link/unlink commands own edge changes.
3. Move local-storage root selection to `enable-local --root`; resolve its org/project from detected non-fallback context.
4. Remove deprecated global JSON alias, retaining `--format json`.
5. Update tests, README, global skills, and portable copies.

Simplicity: PASS. Pair 1 is closest: `--clear` replaces five single-use switches with one explicit, typed control; it does not add a configuration layer. Pair 5 passes: every changed interface is an approved item in the spec.
