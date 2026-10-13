# Engineering requirements

Current implementation is established by source and executable tests. These
records specify proposed changes and unresolved acceptance obligations; they
are not additional API implementations or passing-test certificates.

- [Developer experience PRD](developer-experience-prd.md): one canonical
  application path, ownership by layer, required simplifications and acceptance.
- [Source coverage and documentation disposition](developer-experience-evidence.md):
  pinned evidence, complete requirement coverage and removed records.
- [Interpreter ownership and startup requirements](creation-and-delivery-custody-prd.md):
  complete ownership and T01–T24 acceptance traces. Reverify against selected code.
- [Remaining quality obligations](repository-quality-audit.md): downstream
  interpreter evidence and the limits of owner-only tests.
- [Atomic feature requirements](atomic-actor-features.md): requirement identities,
  not current implementation status.
- [Atomic ownership map](atomic-actor-architecture.md),
  [component authoring](atomic-actor-devx.md), and
  [verification requirements](atomic-actor-verification.md).

Completed research campaigns and obsolete source inventories were removed from
the working tree. Their immutable locations are recorded in the disposition
appendix; Git preserves their full text. New evidence belongs in the active
feature PRD, not another repository-wide chronological ledger.
