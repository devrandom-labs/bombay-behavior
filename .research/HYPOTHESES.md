# Hypotheses

Order by expected value and rotate repository domains.

## H01

- Status: proposed
- Threatened invariant or cost: Loose or duplicated types may allow semantic
  drift or impose redundant public type specification.
- Workload: Complete public and internal type inventory across both scoped
  crates and their composition tests.
- Proposed change: None until concrete duplicate semantic authority is found.
- Falsifier: Similar-looking types encode different laws, provenance, ownership,
  or capability boundaries; consolidation would weaken static meaning.
- Measurements: Public-item/type counts, affected call sites, compiler
  diagnostics, full action traces, compile time, and complete verification.
- Rollback boundary: One candidate type consolidation per experiment commit.
- Evidence: Operator observation only; unvalidated.

## H02

- Status: proposed
- Threatened invariant or cost: Repeated concrete effect/protocol products may
  obscure named lane semantics through composition.
- Workload: All wrapper orders and named effect products in both scoped crates.
- Proposed change: Extract only the smallest named product or construction
  after demonstrated semantic recurrence.
- Falsifier: The recurrence is syntactic, or extraction hides protocols,
  reorders effects, introduces positional access, or requires erased types.
- Measurements: Composition trace equivalence, initialization order, sends,
  creations, become/termination propagation, and compiler-enforced invalid use.
- Rollback boundary: One extracted construction and its direct callers.
- Evidence: Operator observation plus repository's concrete-first extraction
  rule; unvalidated against current code.
