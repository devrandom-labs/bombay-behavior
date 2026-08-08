# Goal: derive or reject phase-indexed actor protocol typing

## Objective

Determine whether Bombay Behavior needs a reusable phase-indexed protocol
construction beyond the existing `Fsm`. Begin from one concrete actor protocol
and invalid Rust programs that ought not compile. Attempt a derivation from the
existing static algebra before proposing any production type.

This is a derivation and falsification campaign, not a feature campaign. A
successful result may conclude that the existing algebra is sufficient or that
the requirement belongs to an application. Do not optimize for adding code.

## Semantic classification

State the classification before every retained claim or change:

- **LAW:** a requirement established by cited primary research;
- **BOMBAY-DERIVED:** a construction from the existing algebra;
- **BOMBAY-POLICY:** a deliberate project choice not guaranteed by the actor
  model; or
- **APPLICATION:** protocol-specific machinery outside the reusable algebra.

Honda-style session duality is not an actor-model law. `Fsm` currently provides
runtime finite-state sequencing: `Move::Goto(P)` accepts any `P`, one `M` is
available in every phase, and no duality relation is encoded. Do not describe
it as session typing.

## Required concrete case

Choose one small, realistic Bombay actor protocol with at least three phases
and direction-sensitive messages. Document why it belongs in an actor behavior
study rather than being a generic channel example. For that case, specify:

1. valid traces;
2. invalid phase transitions;
3. messages invalid in each phase;
4. termination and error behavior;
5. initialization effects;
6. send, creation, and become preservation; and
7. which failures must be compile-time failures rather than runtime decisions.

Do not select a case merely because it makes a preferred encoding convenient.

## Derivation order

Before any experiment, set `evidence.json.prepared_at_commit` to the commit that
contains this completed scaffold and verify that the worktree is clean. This is
the production-code reversion baseline; never advance it during the loop.

Attempt these in order and record concrete Rust evidence for each:

1. Existing `Fsm` plus closed message/phase enums.
2. Existing `Behavior`, associated types, concrete event sums, products,
   typestate, uninhabited types, and ordinary generic wrappers.
3. A concrete application-local behavior using only public API.
4. Only after those attempts fail, a minimal reusable public construction.

For every attempt record the exact type signature, what compiles, what should
not compile, composition behavior, and the precise obstruction. Verbose types
are not by themselves an obstruction; use truthful aliases before considering
new abstraction.

## Static requirements

Any claimed static improvement must be demonstrated with compile-fail tests.
At minimum test:

- a message sent or handled in an invalid phase;
- an invalid phase transition;
- a protocol endpoint with the wrong direction/role, if duality is claimed;
- loss or substitution of a send/create/error/termination seat; and
- an invalid composition with an existing wrapper, if the type rejects it by
  design.

Positive probes must show the corresponding valid programs compile. Runtime
rejection does not satisfy a compile-time claim.

## Composition requirements

Any retained reusable construction must preserve the complete `Actions`
boundary and be checked with initialization, `At`, `ReceiveTimeout`, watching,
supervision, shutdown/finalization, stashing, and creation capability. Check
both relevant wrapper orders. It must not introduce dynamic dispatch,
type-erasure, runtime registries, universal envelopes, serialization, `unsafe`,
or ambient effects.

## Allowed outcomes

The final report must choose exactly one:

1. `existing-algebra-sufficient` — give the concrete derivation and tests;
2. `application-local` — explain why no reusable core construction is lawful;
3. `minimal-core-gap` — demonstrate a failed public-API derivation and specify
   the smallest reusable type-level construction; or
4. `insufficient-evidence` — record what remains unknown without guessing.

Outcome 3 permits retained production changes only when the same loop provides
a concrete caller, public rustdoc laws, positive tests, independent behavior
tests, and compile-fail tests. Otherwise leave the design as a proposal for a
separate implementation loop.

## Evidence and report

Sources of truth:

- `evidence.json`: fixed obligations and their status;
- `REPORT.md`: derivations, failed experiments, and final decision;
- `ASSUMPTIONS.md`: uncertainties and judgment calls;
- `PROGRESS.md`: durable iteration notes; and
- `check.sh`: structural, ratchet, and repository gate.

The checker cannot establish citation entailment or architectural truth. Those
remain explicit human-review duties.

## Repository constraints

All repository `AGENTS.md` instructions apply. Preserve static dispatch and the
pure `Actions` effect boundary. Do not modify manifests, CI, release files,
changelogs, or repository instructions. Do not edit actorpass or neighboring
repositories.

Do not retain production changes merely because they pass tests. If the final
outcome is not `minimal-core-gap`, revert all production experiments while
preserving their evidence in the report.

## Completion

Resolve every obligation in `evidence.json`, append `## Final decision` to
`REPORT.md`, select exactly one allowed outcome, account for every experiment,
and run `research/session-protocol-derivation-loop/check.sh`.

Never claim `LOOP_DONE` while the checker fails.
