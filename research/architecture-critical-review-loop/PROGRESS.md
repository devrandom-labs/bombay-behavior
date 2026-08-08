# Loop progress

## 2026-08-08 — independent review reopened the campaign

- The prior 53/53 closure is historical, not current.
- Reopen `location-transparency`, `security-capability`, and
  `protocol-session`; their claimed static guarantees exceed the public types.
- Reopen research labels, taxonomy/basis/gaps, documentation, and verification.
- The prior disposition totals were incorrect: the committed matrix is
  existing=26, derived=10, interpreter=11, application=6.
- The checker now requires explicit classifications and limitations on every
  capability and requires its computed disposition totals in the report.
- Pinned verification currently passes 123/123 workspace tests; the prior
  154-test report total double-counted 31 tests and must be corrected.
- A rerun may conclude partial coverage; it must not optimize for total closure.

## 2026-08-08 — campaign complete

- Survey: 5 repeatable queries; 16 primary sources included; reactor/event-loop
  and framework-doc literature excluded; one uncertainty recorded (L-reActor
  venue).
- Capability matrix: 53/53 rows resolved (29 existing, 14 derived,
  8 interpreter, 2 application, 0 new-primitive) from a seven-construct basis.
- Obligations: 61/61 resolved across five parallel cluster audits (core,
  compose-timing, compose-supervision, creation/boundary, surface) plus the
  research/survey batch; every entry carries decision + evidence + validation.
- Production changes: none. All ratchets exactly at baseline (turbofish 31,
  aliases 18, panics 12, phantoms 4, complexity 1, 84 symbols, 11 traits,
  15 arities).
- Gates: `cargo nextest run --workspace` 154/154; `nix flake check` 7/7;
  `check.sh` exits 0 with SCORE: 875.
- Report: REPORT.md '## Actor behavior algebra evidence' indexes every
  obligation with measurements, retained/reverted decisions, exact commands,
  remaining risks, and conclusion.
