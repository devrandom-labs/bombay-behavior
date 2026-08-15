# Hypotheses

Order by expected value and rotate repository domains.

## H01

- Status: validated with mixed outcomes; see E01-E04, E06/E06b, E09, and the
  rejected near-misses in `DEAD_ENDS.md`.
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
- Evidence: Whole-repository inventory found duplicated error authority,
  boolean blindness, sentinel state, and forgeable derived fields. It also
  found several similar-looking types whose distinct semantic authority
  falsified consolidation.

## H02

- Status: validated for one private construction (E04); broader speculative
  extraction was falsified or deferred.
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
- Evidence: E04 proved four mutually exclusive incarnation effects were one
  semantic sum. Other aliases and similarly shaped products retained distinct
  laws or were truthful aliases for otherwise verbose concrete types.
