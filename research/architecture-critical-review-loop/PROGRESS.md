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

## 2026-08-09 — access limitations narrowed (second pass)

- Clinger 1981: fetched from DSpace after rate-limit lifted; pure image scan
  read partially via OCR (poppler + tesseract through nix). Extracted the
  behavior domain equation F ≅ [M → (F × P(A × M))] — the 1981 denotational
  statement of Bombay's fold shape — plus fairness/unbounded nondeterminism
  and the acquaintance/creation locality laws. (7a429ef)
- Rebeca FI 2004: full text via CiteSeerX link on rebeca-lang.org; rebecs,
  atomic message servers, known rebecs, weak-simulation compositional
  verification. (14d7474)
- System-A session types: full arXiv text (1208.4632); global types →
  projection → conformance; no delegation. (278f252)
- De Koster-De Meuter 2025: paper paywalled, but the complete PLT Redex
  models were read from the authors' public GitLab — Classic Actors =
  exactly spawn/send/become, fresh spawn, same-address become, mailbox-scan
  selective receive. Independent mechanized confirmation of the nucleus.
  (976a402)
- Greif 1975: located on DSpace (handle 1721.1/57710), read via OCR text
  layer — per-process total orders, system partial order, arrival events,
  no global clock. (611a503)
- Hewitt 1977: read via the AI Memo 410 author-version scan (Papers We Love
  mirror) — action-on-message + finite asymmetric acquaintances;
  request-and-reply as pattern. (aecf723)
- Baker-Hewitt 1977: read via MIT Working Paper 134 draft on DSpace —
  receipt-only events, Law of Discreteness, finite immediate successors,
  single predecessor, unique initial event. (352719b)
- Attempted and recorded as genuinely inaccessible: Agha 1986 book
  (archive.org borrow-only, 401/403 on text and search-inside);
  Charalambides 2018 thesis (IDEALS 403/404, Wayback SPA shell); Talcott
  AFP 1996 notes (blackforest.stanford.edu dead, Wayback 429). All three
  remain covered by fully-read subsuming sources.
- Gates after every batch: check.sh EXIT=0, SCORE 935, 67/67, 53/53.

## 2026-08-09 — machine-readable artifacts + mechanical eliminability probes

- New artifacts per the redirected objective: primitive-basis.json (7
  candidates with formation rules, semantics, classifications, laws,
  soundness records, eliminability attempts with exact obstruction kinds),
  research-bibliography.json (107 dispositioned sources: 18 complete,
  7 partial, 78 abstract-only with zero claims, 4 unread with zero claims;
  excluded groups E1-E5), capability-derivations.json (53 rows: 34 pure
  with derivation trees referencing primitive IDs, 19 boundary with
  explicit boundary arguments). (54da5f4)
- check.sh gained the artifact gate: rejects retained primitives without
  soundness+eliminability records, derived forms without derivations from
  retained primitives, pure capabilities without derivation trees, unknown
  primitive references, semantic claims resting on unread/abstract-only
  sources, missing core Agha records, and incomplete matrix coverage.
- Mechanical elimination probes executed and preserved in probes/:
  P-fnreact (generic-seat compiles but breaks the arity ratchet; closed
  enum forecloses user policies; dyn banned — retained), P-never (unit-seat
  substitution compiles and admits Err(())/Goto(()) — retained), P-birthmode
  (marker traits cannot name the composite child type; the fix reintroduces
  the associated type — retained). (8094e31, 99a4e5a, 87b3d53)
- Full reads added: Plyukhin-Agha termination (safety Cor. 6.11, liveness
  Thm 6.12), Paul-Agha FAM (available/failed states, predicate fairness,
  Athena-checked Theorem 1), Kumar-Sen-Meseguer-Agha FMOODS 2003. (480e8fe,
  205aac6, 07a5c72)
- Attempted and unreachable: CRGC 2025 (ACM/Montesi 403, Zenodo artifact is
  a 3.4 GB VM), AGERE 2018 GC (record only), receptionists 1984 (no open
  copy). All remain abstract-only with zero claims, gate-compliant.
- Gates: check.sh EXIT=0, SCORE 935, artifact gate validates 7 primitives /
  53 derivations / 107 sources.
