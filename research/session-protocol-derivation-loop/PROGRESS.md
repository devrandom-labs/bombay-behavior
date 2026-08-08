# Loop progress

## Prepared

- Fixed obligations and four honest outcomes established.
- No concrete protocol selected and no outcome preferred.
- Checker intentionally fails until all evidence and the final report exist.

## 2026-08-08 — campaign complete

- Concrete protocol: Supervised Worker Lifecycle (Starting/Running/Draining),
  direction-sensitive messages, actor-specific lifecycle concerns.
- Outcome: `application-local`. The existing FSM handles runtime phase
  sequencing correctly. Applications CAN build per-phase event types but
  cannot bridge them to the Behavior trait's single Event type. No reusable
  core construction is lawful because the obstruction is in the Behavior
  trait's design contract (single Event type enables universal wrapper
  composition), not in a missing primitive.
- Derivation file: `crates/behavior-testkit/tests/session_protocol_derivation.rs`
  — 7 tests, 5 derivation attempts, documented obstructions.
- All 15 obligations resolved with concrete evidence and validation.
- No production code changes from `prepared_at_commit`.
- Gates: check.sh exits 0, nextest 130/130 (123 existing + 7 new), nix flake
  check 7/7.
