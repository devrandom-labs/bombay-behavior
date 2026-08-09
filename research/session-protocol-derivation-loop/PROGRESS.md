# Independent session validation progress

This log belongs only to the Resource Pool validation. Architecture-audit
milestones and the original Supervised Worker campaign are intentionally not
continued here.

## Prepared

- Fixed obligations and four honest outcomes established.
- Concrete protocol: Resource Pool (Initializing/Serving/Draining), distinct from
  previous campaign's Worker Lifecycle.
- No outcome preferred; derivation started fresh.

## 2026-08-08 — campaign complete

- Concrete protocol: Resource Pool actor with creation capability, peer watching,
  and supervised drain coordination. Direction-sensitive inbound/outbound
  messages.
- Outcome: `application-local`. The existing FSM handles runtime phase
  sequencing correctly. Applications CAN build per-phase event types but
  cannot bridge them to the Behavior trait's single Event type while
  preserving both phase safety AND wrapper composition.
- Derivation file: `crates/behavior-testkit/tests/session_protocol_derivation_loop.rs`
  — 8 tests, 6 derivation attempts, 5 documented obstructions.
- All 15 obligations resolved with concrete evidence and validation.
- No production code changes from `prepared_at_commit`.
- Protocol is distinct from previous campaign (Resource Pool vs Worker Lifecycle)
  — obstructions generalize across actor protocols.
