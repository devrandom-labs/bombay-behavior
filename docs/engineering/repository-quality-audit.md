# Remaining repository quality obligations

The former chronological audit is preserved at
[the source baseline](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/repository-quality-audit.md).
Its historical checkmarks are not evidence about another revision. Current
source and executed tests determine implementation and verification status.

The [developer experience PRD](developer-experience-prd.md) owns the new
simplification work; its [coverage appendix](developer-experience-evidence.md)
records source-backed findings across Behavior and Bombay. Do not restart the
retired audit as another open implementation campaign.

## Downstream interpreter acceptance

The former A17 obligation is retained: run a real downstream interpreter
against immutable released Behavior packages and record the revision, command
and result for each relevant trace:

- Initial and replacement creation, collision and namespace exhaustion.
- Initialization stop, controlled rejection, panic and partial effects.
- Activation and publication, including readiness arriving during shutdown.
- A rejected effect followed by an independent effect that must still execute.
- Source return after emitter closure and complete parent-to-root retirement.
- No successful restart report for failed replacement establishment.
- Exact assignment/proxy rejection, affine receipts and immutable endpoints.

Pure models, fabricated test endpoints, compilation and interpreter presence
cannot establish these runtime results. Do not repeat an already executed
witness for unchanged code solely to generate more audit prose; preserve its
immutable evidence and rerun when the affected contract changes.

## Complete custody requirements

The [interpreter ownership and startup PRD](creation-and-delivery-custody-prd.md)
retains T01–T24 and release/lockfile requirements. Before selecting work, compare
each relevant obligation with the actual owner and Bombay source. A historical
statement that an API is missing cannot establish that it is still missing.
FIFO's existing clone-and-retry policy is a distinct ownership decision; do not
silently replace it with a move-only retry claim or discard accepted work.

## Verification discipline

Keep complete action assertions, independent models, compile denials,
optimized replay, controlled fault injection and package-consumer tests.
Mutation viability and mutation detection remain separate evidence. Preserve
actual failed gate results and environmental limitations; counts from old
campaigns are not current coverage measurements.

The repository instructions own the required aggregate-drift, provenance and
change-budget checkpoints. The new PRD maps application syntax, public surface,
crate reuse, inference, diagnostics, performance and documentation obligations
to their actual owners. No production contract changes in this cleanup.
