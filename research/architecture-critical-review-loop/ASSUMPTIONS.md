# Assumptions and judgment calls

- Actorpass is treated as an out-of-repo interpreter: its obligations
  (freshness acceptance, collision errors, Restarted diagnostics) are
  specified here and validated through the testkit's pure drivers, not through
  actorpass integration tests, which are outside this repository.
- Armstrong's Erlang work (2003 thesis, 1996 book) is used as primary evidence
  for supervision/linking/restart vocabulary, labeled DERIVED/POLICY — never
  as actor-model law. The law layer comes only from Hewitt/Greif/Clinger/
  Agha and collaborators.
- The 'L-reActor' name could not be pinned to a DOI in this pass; the
  algebraic-actor line is covered by Agha-Thati (LNCS 2941) and
  Agha-Thati-Ziaei (OSL report). No adopted claim depends on the missing
  citation.
- Compile-fail coverage is scoped to one compile_fail doctest (erased effect
  seat) plus positive compile-time probes, because the phase/birth/capability
  negatives are uninhabited-type impossibilities rather than runtime
  rejections. A trybuild harness is a recorded future option, not a gap.
- Non-idempotent wrapper init under direct misuse (double-init,
  step-before-init) is retained as a documented interpreter/driver contract:
  the driver inits first, so these paths are unreachable in supported use.
- The Fsm mid-drain error asymmetry (dropped unprocessed batch vs Stop
  preserving it) is test-pinned existing behavior, retained per the audit
  charter rather than redesigned without a research mandate.
- Re-review judgment: security-capability and location-transparency are
  interpreter-boundary rows, not derivations, because the public types
  (Copy MailAddr, public Recipient constructors, public Address::birth)
  establish only protocol compatibility, never authenticity, secrecy, or
  unforgeability. Protocol-session stays derived as a finite-state
  combinator; Honda-style session typing is recorded as only partially
  represented, with no primitive proposed because no derivation failure
  proved an algebraic gap.
- The checker is treated as a structural/ratchet/repository gate: it
  validates presence, vocabulary, counts, and repository health, but not
  citation entailment or test relevance; those remain human review duties.
