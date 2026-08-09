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
  and framework-doc literature excluded. An invented scaffold label was
  corrected to “Agha's later functional and algebraic actor work.”
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

## 2026-08-08 — reopened review resolved (rerun complete)

- All 53 capability rows now carry explicit `claim_classification`
  (checker-owned vocabulary) and `limitations` fields; the three reopened
  rows rewritten honestly.
- Dispositions changed: `security-capability` and `location-transparency`
  derived -> interpreter; `protocol-session` claim narrowed to finite-state
  sequencing, disposition unchanged. Checker-derived totals: existing=26,
  derived=8, interpreter=13, application=6, new-primitive=0, rejected=0.
- Verification record corrected: workspace nextest is 123/123 total
  (bombay-behavior 31 + bombay-behavior-testkit 92); the first report's 154
  double-counted.
- Six obligations re-resolved: RESEARCH-LABELS, SURVEY-TAXONOMY,
  SURVEY-BASIS, SURVEY-GAPS, DOC-01, VERIFY-01.
- REPORT.md appends a superseding conclusion: partial representation of
  protocol-session, security-capability, and location-transparency stated
  explicitly; check.sh described as a structural/ratchet/repository gate,
  not a proof of entailment.
- Gates: check.sh exits 0, SCORE 875, nextest 123/123, nix flake check 7/7.
- Result is qualified, not total closure; no primitive added because no
  concrete derivation failure proved an algebraic gap.

## 2026-08-09 — research campaign completed (RESEARCH-* + CALCULUS-* resolved)

- Sequential survey executed per ACTOR-RESEARCH-SURVEY.md: OSL catalogue (419
  entries) and DBLP XML export (244 records) completely dispositioned into
  AGHA-BIBLIOGRAPHY.md; 18 primary sources processed one at a time with
  per-source checkpoints in RESEARCH-EXTRACTIONS.md; access limitations
  recorded explicitly (Clinger 1981, Agha 1986 book, LNCS 2004 chapter,
  Talcott AFP 1996, Greif 1975, Baker-Hewitt 1977, Hewitt 1977, Honda line,
  Rebeca FI 2004, two paywalled 2019/2025 chapters).
- New finds beyond the prior run: Charalambides-Palmskog-Agha 2019 (types for
  progress), Paul et al 2021/2023 (failure-aware actor model), Plyukhin-Agha-
  Montesi 2025 (CRGC), De Koster-De Meuter 2025 (Isolated Turn taxonomy),
  DBLP-only 1986 items. None adds a transition primitive; two primary results
  actively confirm derived-over-primitive (2001 sync-constraint translation;
  SAL 2003/2004).
- REPORT.md gained the nine required sections: research method, Agha
  bibliography disposition, foundational comparison, post-2000 comparison,
  claim map, candidate primitive basis (seven constructs), primitive
  soundness (8 obligations), primitive eliminability (7 experiments),
  capability derivation trees (53 rows) with the qualified closure claim.
- evidence.json: RESEARCH-AGHA, SURVEY-SEARCH, RESEARCH-BIBLIOGRAPHY,
  RESEARCH-FORMALISMS, CALCULUS-NUCLEUS, CALCULUS-SOUNDNESS,
  CALCULUS-MINIMALITY, CALCULUS-CLOSURE resolved with decisions, evidence,
  validation.
- Production changes: none. One formatting fix in
  crates/behavior-testkit/tests/session_protocol_derivation_loop.rs
  (stray blank line; rustfmt) — required for nix flake check.
- Gates: check.sh EXIT=0, SCORE 935 (was 855), obligations 67/67,
  capabilities 53/53, nextest 123/123, nix flake check 7/7.
- Conclusion remains qualified: protocol-session, security-capability, and
  location-transparency stay partially represented; no primitive added
  because no concrete derivation failure demonstrated an algebraic gap.
