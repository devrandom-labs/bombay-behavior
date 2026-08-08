# Goal: derive and verify the pure actor-behavior algebra

## Objective

Perform an independent, skeptical derivation and verification pass over Bombay
Behavior. The semantic authority is premium primary actor-system research:
Hewitt's actor model; Agha's actor languages and semantics; Agha and
collaborators' later functional and algebraic actor work; formal lambda-calculus
foundations of actor computation; and other peer-reviewed original papers,
dissertations, or authoritative formal specifications discovered by the survey.
Generic reactor/event-loop work is explicitly not part of this authority.
`AGENTS.md` governs how that research may become code. Test every claim
against the current source, public algebra, tests, compile-time behavior, and
the primary literature. Preserve the semantic core, simplify machinery that
evidence proves unnecessary, and close genuine gaps in the reusable pure
actor-behavior algebra.

## Functional derivation discipline

The existing behavior combinatorics is the implementation lens. Begin with the
pure fold, then derive actor behaviors using typed sums for alternatives, typed
products for independent effects/capabilities, higher-order behavior
transformations, lawful nesting, and ordinary functional composition. Treat
initialization as the fold's explicit initial observation and termination as an
explicit result—not ambient runtime state. For each capability, write the
functional derivation before considering an API or implementation.

The preferred result of research is therefore a law or a derivation using the
existing algebra, followed by tests of identity, associativity/preservation,
wrapper order, and observable behavior. A named pattern from a paper or actor
framework is not by itself a reason to add a type. A primitive is admissible
only after concrete typed derivations fail and the report records the minimal
algebraic obstruction. Actorpass remains an interpreter of emitted values and
must not push runtime mechanics into the pure fold.

Behaviorpass is designed for actor systems in general. Actorpass is one
interpreter and consumer; its current needs are useful validation, not the
specification or the limit of the algebra. Search the web and primary sources
for the broadest defensible catalogue of actor-related behavior capabilities.
Attempt to express each pure capability by composition from the smallest
law-backed basis. Add a new primitive only when the survey and concrete
derivation attempts prove the capability cannot be expressed cleanly without
losing static safety, determinism, or composability.

This is not a feature campaign. Do not add capabilities or interpret runtime
work inside behavior. Do not edit an external backlog, actorpass, or neighboring
Bombay crates.

## Source of truth

- Research synthesis: `research/architecture-critical-review-loop/REPORT.md`
- Fixed obligations: `research/architecture-critical-review-loop/evidence.json`
- Actor-capability survey:
  `research/architecture-critical-review-loop/capability-matrix.json`
- Frozen ratchets: `research/architecture-critical-review-loop/baseline.json`
- Completion check: `research/architecture-critical-review-loop/check.sh`
- Run instructions: `research/architecture-critical-review-loop/RUNBOOK.md`
- Durable notes: `research/architecture-critical-review-loop/PROGRESS.md` and
  `research/architecture-critical-review-loop/ASSUMPTIONS.md`

Never delete an obligation, weaken the checker, falsify evidence, or edit the
baseline to raise the score. Resolve an item only with a concrete decision,
source/research evidence, and independent validation evidence.

“All possible actor behaviors” is not a claim that an infinite program space
can be enumerated. Operationally it means: perform a broad, documented,
repeatable literature search; cover every required taxonomy row; add newly
discovered categories to the matrix; and show that the remaining pure behavior
space is generated compositionally from a small basis rather than represented
by a primitive-per-use-case catalogue.

## Semantic method

For every proposed semantic change:

1. State the invariant first.
2. Classify it as actor-model law, Bombay-derived construction, or deliberate
   Bombay policy.
3. Inspect the relevant primary source; framework folklore and prior local
   architecture proposals are not evidence.
4. Give the derivation in the existing functional combinator algebra, including
   its sums, products, fold/transformation, identity, and preservation laws.
5. Express static invariants in public types and dynamic ordering/executor
   properties in focused tests.
6. Check every affected wrapper order and initialization/effect accumulation.
7. Make one small experiment, run focused tests, and retain or revert it.
8. Record dead ends with exact compiler/test evidence.
9. Commit each coherent retained batch separately.

For research, prefer peer-reviewed primary papers, original dissertations, and
authoritative formal specifications. Record full bibliographic identity or DOI,
search queries, sources considered, inclusions, exclusions, and uncertainty.
Blogs, summaries, framework APIs, and generic reactor/event-loop literature may
suggest search terms but cannot establish actor-model or behavior-algebra laws.

## Architecture traceability

The obligation inventory directly covers:

- pure `Behavior` fold and explicit `Actions` boundary: `CORE-*`;
- typed modules and protocol vocabulary: `MODULE-*`;
- every concrete transformation and mixed nesting: `COMPOSE-*`;
- freshness, nonce meaning, creation ordering, and provenance: `CREATE-*`;
- retained concrete structures: `RETAIN-*`;
- interpreter-owned boundary work: `BOUNDARY-*`;
- rejected dynamic/speculative escape hatches: `REJECT-*` and `STATIC-01`;
- public surface, generics, phantoms, complexity, panics, and ergonomics:
  `SURFACE-*`, `GEN-*`, `PHANTOM-01`, `COMPLEXITY-01`, `PANIC-01`, `ERGO-01`;
- independent model and compile-fail quality: `TEST-*`;
- research fidelity and final synthesis: `RESEARCH-*`, `DOC-01`, `VERIFY-01`.
- actor-system capability closure: `SURVEY-*` plus every row in
  `capability-matrix.json`.

## Hard ratchets

The checker derives these from the current tree and compares them with the
frozen baseline. Each may decrease but never increase:

- public algebra symbols and public traits;
- generic arities of the selected core/wrapper types;
- phantom fields and type-complexity allowances;
- production `panic!`/`.expect(` sites;
- test turbofish expressions and top-level helper type aliases.

`dyn`, `Any`, `TypeId`, and `unsafe` remain zero-tolerance. Tests and examples
may become simpler but never require more turbofishes, aliases, explicit type
annotations, or protocol ceremony. Do not move complexity out of production
signatures and into test helpers.

## Milestones

1. Conduct a broad web/primary-source survey and resolve the complete capability
   matrix. Expand it whenever a genuinely distinct actor capability is found.
2. Verify Hewitt, Agha, Agha's later functional and algebraic actor work, formal
   lambda-calculus actor foundations, and further premium primary sources
   discovered by the survey; label every semantic claim
   law/derivation/policy.
3. Trace the pure fold, initialization, complete `Actions`, and interpreter
   boundary without assuming implementation names prove semantics.
4. Audit every wrapper independently and in supported mixed orders; prove no
   lane is dropped, duplicated, reordered, relabeled, or reinterpreted.
5. Audit staged creation and lifecycle provenance from behavior designation to
   successful interpreter installation, explicitly separating nonce from
   identity/freshness.
6. Derive every pure surveyed capability from the existing basis. For each
   derivation record its algebraic signature, required sums/products, identity
   and preservation laws, initialization/error/termination behavior, and at
   least one concrete composition test. If derivation fails, record the exact
   obstruction before proposing a minimal primitive.
7. Challenge every retained structure and rejected direction using concrete
   counterfactual experiments where useful.
8. Audit the complete public/type surface, diagnostics, panic contracts, and
   application ergonomics.
9. Add `## Actor behavior algebra evidence` to `REPORT.md` with
   all obligation IDs, before/after measurements, retained/reverted decisions,
   exact commands, remaining risks, and an honest conclusion.

## Off limits

- New actor features or runtime capabilities.
- Domain-specific behavior catalogues when a lawful generic composition
  expresses the same capability.
- Mailboxes, scheduling, clocks, routing, handles, allocation, or I/O in folds.
- Dynamic dispatch, erased futures, `Any`, reflection, registries, universal
  envelopes, serialization-based internal protocols, or `unsafe` escapes.
- Weakening static capability, phase, error, birth, or composition constraints.
- Treating nonce as identity/freshness or replacement as allocation overwrite.
- Broad compatibility shims, speculative abstractions, or grand rewrites.
- Dependency/manifests, CI, repository instructions, release configuration, or
  unrelated documentation.

Pure, reusable, research-backed behavior primitives are in scope when—and only
when—the capability matrix demonstrates a real algebraic gap. Runtime
realization remains out of scope.

## Completion

Run `research/architecture-critical-review-loop/check.sh`. It prints
`SCORE: N` and remains nonzero until every fixed obligation is valid, the final
research report accounts for every ID, all ratchets hold, and the repository's
full verification gates pass. Never claim `LOOP_DONE` while it fails.

## Reopened review obligations (2026-08-08)

The first closure report did not survive independent review. Preserve that run
as history, but do not repeat its conclusion until all of these are resolved:

- Reclassify `protocol-session`: the current `Fsm` has `Move::Goto(P)` and a
  single message type in every phase. A closed enum alone does not make invalid
  transitions, out-of-phase messages, or session duality unrepresentable.
- Reclassify `security-capability`: typed recipients preserve protocol
  compatibility, but public/copyable `MailAddr`, `Recipient::global`,
  `Recipient::child`, and `Address::birth` do not prove address authenticity,
  secrecy, possession authority, or unforgeability.
- Narrow `location-transparency`: the fold has no location-resolution service,
  but `Address` exposes `birth`, concrete addresses may expose representation,
  and `Debug` may reveal routes. Do not claim equality is the only observable
  operation or that the algebra prevents forging.
- Recompute all disposition totals from the matrix. The prior report's
  29/14/8/2 totals disagree with the actual 26/10/11/6 matrix and with the
  contradictory `SURVEY-BASIS` prose.
- Give every resolved capability explicit `claim_classification` and
  `limitations` fields. The checker must validate their vocabulary and
  presence; string length is not semantic evidence.
- Describe `check.sh` truthfully as a structural/ratchet/repository gate. It
  cannot establish that a citation entails a claim or that a test validates a
  derivation; those remain review obligations.

The next final report must include the checker-derived disposition line exactly
as printed, explain every changed disposition, and state whether any surveyed
capability remains only partially represented. A qualified result is valid;
unsupported total closure is not.
