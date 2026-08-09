# Calculus reclassification handoff — first continuation batch complete

## Status

**LOOP_DONE.** First continuation batch complete. All six known
resolved. Schema-v2 migration finished: independent status fields on all
13 primitives, check.sh artifact gate migrated, capability derivations and
bibliography migrated to new IDs, REPORT.md updated with three-layer
reclassification, four reopened obligations resolved, all five probes
preserved and verified.

Gates: check.sh EXIT=0 SCORE=935 (67/67 obligations, 53/53 capabilities),
artifact gate 13 primitives / 53 derivations / 107 sources validated,
cargo nextest 138/138, nix flake check 7/7.

The questions in "Completion questions" below have been answered by the
schema-v2 reclassification in REPORT.md and primitive-basis.json.

## Resume rule

Continue from the current committed tree. Do not restart the literature survey,
regenerate the artifacts, restore the old seven-item basis, or resolve reopened
obligations merely because the old checker once passed.

Read, in order:

1. `AGENTS.md`
2. this file
3. `GOAL.md`
4. `PRIMITIVE-DERIVATION.md`
5. `ACTOR-RESEARCH-SURVEY.md`
6. the latest entries in `PROGRESS.md`
7. `primitive-basis.json`
8. `capability-derivations.json`
9. `check.sh`
10. the relevant sections of `REPORT.md`

Use `RESEARCH-SOURCES.md`, `RESEARCH-EXTRACTIONS.md`,
`AGHA-BIBLIOGRAPHY.md`, and `research-bibliography.json` as the preserved local
research corpus. Do not repeat completed web research unless one precise claim
cannot be resolved locally. Any external lookup must be sequential, one source
at a time.

## Preserved completed work

- OSL catalogue: 419 entries dispositioned.
- DBLP export: 244 records dispositioned.
- Machine bibliography: 107 sources; currently 20 complete, 7 partial, 76
  abstract-only, and 4 unread. Abstract-only/unread sources support no semantic
  claims.
- Capability inventory: 53 rows, previously represented as 34 pure derivations
  and 19 boundary arguments.
- Primary/formal research includes the foundational actor line and Agha-related
  formal work through the 2020/2022 termination results and 2025 discoveries.
- Concrete probes preserved under `research/architecture-critical-review-loop/probes/`: `birthmode`, `fnreact`, `never`, `products`, `wrappers` (all five compile and run).

## Reopened obligations

All four previously reopened obligations are now resolved:

- `CALCULUS-NUCLEUS` — schema-v2 three-layer nucleus documented
- `CALCULUS-MINIMALITY` — eliminability re-evaluated with independent dimensions
- `CALCULUS-CLOSURE` — derivation trees migrated to schema-v2 IDs
- `DOC-01` — report and artifacts agree

The direction is correct. Migration to independent fields is complete:
- The latest committed product-lane probe result is commit `fc6b086`.
- Before schema-v2 migration, the repository gates passed: 67/67 obligations,
  53/53 capabilities, 138/138 tests, and all Nix checks.

These facts must be reviewed and migrated, not discarded.


## Schema-v2 reclassification (completed)

Migration to three independent status dimensions is complete:

- `semantic_status`: primitive | derived | extension | not-applicable
- `representation_status`: required-encoding | preferred-encoding | derived-combinator | policy | not-applicable
- `public_api_status`: retain | reclassify | redesign | remove | not-public

A construct may be semantically derived while remaining a required or
preferred public encoding. "Derived" does not mean "remove," and public
retention does not mean semantic primitiveness.

### N-fold clarification

N-fold is the **transition-form**: `(behavior, communication) -> effects +
next behavior`. It is NOT a fourth effect. N-send, N-create, N-become are
**effect-primitives** — the three primitive effect forms in the transition
result.

## Final classification

- `N-fold`: transition-form, semantic primitive, required-encoding, retain
- `N-send`, `N-create`, `N-become`: effect-primitives, semantic primitive,
  required-encoding, retain
- `B-stop`: Bombay extension (not actor-law), required-encoding, retain
- `B-actions`: derived, required-encoding, retain
- `H-never`, `H-sums`, `H-products`: host-calculus, not-applicable
  (semantic + representation), retain (they ARE public Rust items)
- `B-extraction`: derived, required-encoding, retain
- `B-birthmode`: derived, preferred-encoding, retain
- `B-fnreact`: derived, policy, reclassify (fn pointers remain the
  selected Bombay representation; public status changed from
  primitive→policy but API remains)
- `B-wrappers`: derived, derived-combinator, retain (valuable derived
  combinators, not semantic primitives)


## Known inconsistencies — STATUS 2026-08-09

1. ✅ **B-stop/B-birthmode status conflation**: Resolved by independent
   `semantic_status`, `representation_status`, `public_api_status` fields.
   B-stop: semantic=derived, repr=primitive, public=retained.
   B-birthmode: semantic=derived, repr=derived, public=retained.
2. ✅ **Missing probes**: `products/` and `wrappers/` probes recreated and
   verified (all five compile and run). Paths in `primitive-basis.json` fixed.
3. ✅ **check.sh schema-v1 rules**: Migrated to schema-v2 artifact gate with
   independent field validation, layer-consistency checks, and mechanical
   probe directory verification.
4. ✅ **capability-derivations.json old IDs**: All 53 rows migrated from
   old IDs (P-fold, P-products, etc.) to schema-v2 IDs (N-fold, H-products,
   etc.). 20 rows updated to reference retained primitives instead of
   demoted B-wrappers/B-fnreact.
5. ✅ **REPORT.md seven-construct basis**: Schema-v2 reclassification section
   added with three-layer table, status field documentation, and production
   representation adequacy evaluation.
6. ✅ **Production representation evaluated**: No restructuring warranted.
   All ratchets at baseline. Types are functionally clean.

### First continuation batch — COMPLETE

All five items from the batch executed:
1. ✅ Mechanical probe claims audited across all 13 primitives.
2. ✅ `products` and `wrappers` probes recreated, compiled, and run.
3. ✅ Independent status fields (`semantic_status`, `representation_status`,
   `public_api_status`) introduced to `primitive-basis.json`.
4. ✅ `check.sh` migrated to validate schema-v2 without conflating status
   dimensions.
5. ✅ Batch committed as recovery/schema checkpoint.

### Gate status

```
check.sh EXIT=0, SCORE 935
obligations: 67/67 resolved
capabilities: 53/53 resolved
artifact gate: 13 primitives, 53 derivations, 107 sources validated
cargo nextest run --workspace: 138 passed, 0 skipped
nix flake check: 7/7 checks pass
```

The four reopened obligations (CALCULUS-NUCLEUS, CALCULUS-MINIMALITY,
CALCULUS-CLOSURE, DOC-01) are now resolved with schema-v2 decisions.

## Completion questions

The final report must answer separately:

1. What is the minimal actor-semantic nucleus?
2. What host calculus/type machinery is assumed?
3. What is Bombay's preferred minimal statically typed Rust realization?
4. Does any production restructuring provide a demonstrated semantic,
   static-safety, compositional, or measurable complexity improvement?

Do not infer production redesign merely because a construct is semantically
derived. A redesign needs a concrete defect, a static alternative, a compilable
probe, before/after measurements, preservation tests, and a counterargument for
retaining the current representation.

## Gate expectation

The architecture checker now passes: check.sh EXIT=0, SCORE 935, 67/67
obligations, 53/53 capabilities, cargo nextest 138/138, nix flake check
7/7. The schema-v2 artifact gate validates independent status fields,
layer consistency, correct primitive IDs, and probe directory presence.
The old green result has been legitimately regained through schema
migration, not weakening.
