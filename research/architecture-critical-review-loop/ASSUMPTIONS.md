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
