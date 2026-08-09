# Partial calculus reclassification handoff

## Status

**PARTIAL — DO NOT CLAIM `LOOP_DONE`.**

The broad literature survey and the original artifact campaign completed, but
an independent review found that its seven-item basis conflated three layers:
actor-semantic primitives, host type-calculus machinery, and Bombay/Rust
representations. A schema-v2 reclassification is in progress.

The latest fully passing 67/67 result is historical evidence for the old
classification. It does not validate the unfinished schema-v2 conclusion.

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
- Concrete probes preserved under `probes/`: `fnreact`, `never`, and
  `birthmode`.
- The latest committed product-lane probe result is commit `fc6b086`.
- Before schema-v2 migration, the repository gates passed: 67/67 obligations,
  53/53 capabilities, 138/138 tests, and all Nix checks.

These facts must be reviewed and migrated, not discarded.

## Reopened obligations

The following entries in `evidence.json` are intentionally pending:

- `CALCULUS-NUCLEUS`
- `CALCULUS-MINIMALITY`
- `CALCULUS-CLOSURE`
- `DOC-01`

Keep them pending until schema-v2, capability derivations, checker rules, and
the report agree.

## Schema-v2 direction

`primitive-basis.json` now begins separating:

1. actor-semantic nucleus;
2. host calculus/type machinery; and
3. Bombay typed encodings, extensions, combinators, and policy.

The direction is correct but the migration is incomplete. In particular, one
status currently carries too many meanings. Replace it with independent fields:

- `layer`
- `semantic_status`
- `representation_status`
- `public_api_status`
- `semantic_primitive`
- `representation_primitive`
- `derivable_from`
- classified `retention_constraints`
- `evidence`
- `limitations`

A construct may be semantically derived while remaining a required or preferred
public encoding. “Derived” does not mean “remove,” and public retention does not
mean semantic primitiveness.

## Current provisional classification

- `N-fold`, `N-send`, `N-create`, `N-become`: actor-nucleus candidates. Clarify
  whether the fold is the transition form containing the three effects rather
  than a fourth independent actor effect.
- `H-never`, `H-sums`, `H-products`: host type-calculus machinery, never actor
  primitives.
- `B-actions`: Bombay's typed realization of the transition effects plus Bombay
  extensions; not automatically an actor primitive.
- `B-stop`: actor-level derived sink plus Bombay interpreter-visible lifecycle
  extension. Public retention and semantic derivability must be represented
  separately.
- `B-extraction`: Bombay's Rust discipline over host sums; representation-level
  necessity remains to be tested honestly.
- `B-birthmode`: semantically derived, likely retained as Bombay's canonical
  static creation-capability encoding.
- `B-fnreact`: demoted. Generic reaction objects compile, so function pointers
  are representation policy, not semantic primitives.
- `B-wrappers`: demoted as primitives. They are valuable derived higher-order
  combinators with tested composition laws.

## Known inconsistencies to fix first

1. `B-stop` and `B-birthmode` are provisionally public-retained while their
   eliminability verdict is `derived`. The old checker treats this as a
   contradiction because it lacks independent status dimensions.
2. `primitive-basis.json` refers to executed `probes/products/` and
   `probes/wrappers/`, but those directories are not currently preserved. Do
   not repeat the execution claim without recreating faithful minimal artifacts
   and rerunning them. Otherwise downgrade the evidence to conceptual/type-level.
3. `check.sh` still implements the schema-v1 retained/derived rules.
4. `capability-derivations.json` still needs migration from old primitive IDs
   and must distinguish semantic, host-calculus, and Bombay references.
5. `REPORT.md` and resolved evidence decisions still describe the superseded
   seven-item basis.
6. The production representation has not yet been evaluated construct by
   construct. No production rewrite is authorized by the reclassification
   alone.

## First continuation batch

Do only this batch before further synthesis:

1. Audit every `mechanical_probe` claim.
2. Recreate and preserve faithful `products` and `wrappers` probes, or downgrade
   their claims.
3. Introduce the independent status fields.
4. Migrate `check.sh` to validate schema-v2 without resolving obligations.
5. Commit the batch as a recovery/schema checkpoint.

Then migrate capability derivations and the report in separate commits.

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

The architecture checker is expected to fail or stop early while the reopened
obligations remain pending. That is correct. Do not weaken it to regain the old
green result.
