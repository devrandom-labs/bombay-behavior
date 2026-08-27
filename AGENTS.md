# Bombay Behavior: Agent Instructions

## Mission

This repository is the core actor-behavior primitive for the Bombay actor
stack. Treat it as foundational concurrency infrastructure, not as a general
application framework.

The design is grounded in Gul Agha's actor model and related actor research.
Preserve the semantic core: an actor processes one communication at a time and
the result of a transition is expressed as the ability to:

1. send communications to known recipients;
2. create fresh actors; and
3. designate the behavior used for the next communication.

In this codebase that boundary is the pure `Behavior` fold and its typed
`Actions` result (send, create, become). Scheduling, mailbox transport, clocks,
and execution belong to an interpreter. They must not leak into the behavior
algebra as hidden effects.

Research fidelity outranks convenience. Before changing actor semantics,
supervision, observation, creation, ordering, or lifecycle behavior, inspect the
relevant primary research and state the semantic law being implemented. Do not
use folklore, another framework's API, or familiarity with Erlang/Akka as a
substitute for the model. Clearly distinguish:

- a law required by the actor model or a cited source;
- a derived construction used by this library; and
- a deliberate Bombay policy choice.

If the research does not determine a behavior, document the choice instead of
presenting it as an Agha guarantee.

## Non-negotiable architecture

- Keep protocols concrete and statically known. Message types, event lanes,
  recipients, send products, birth capabilities, phases, errors, and composed
  behaviors must remain visible to Rust's type checker.
- Make illegal states and capabilities unrepresentable. Prefer associated
  types, generics, typestate, uninhabited types, and exhaustive enums over
  flags, sentinel values, validation, downcasts, or panic paths.
- Preserve `Actions` as the explicit effect boundary. A successful transition
  returns sends, fresh creations, and its next behavior/termination decision.
  Do not add ambient side channels or let combinators perform those effects.
- Preserve freshness. `Create` is a staged request to establish a fresh child
  and bind it to a nonce in the creating actor's child namespace. The nonce is
  a local routing and correlation key, not an actor identity or proof of
  freshness. A creation becomes an established birth only when the interpreter
  successfully installs a fresh actor and commits that binding. Replacement at
  an existing address is not a primitive; stable identity is a derived
  construction such as the existing proxy-based supervision design.
- Preserve protocol composition. Timing, watching, supervision, stashing, and
  finite-state behavior are typed event/effect transformations, not runtime
  queries or privileged global services hidden from the signature.
- Preserve initialization as part of the behavior contract. Initialization
  effects are interpreted before mailbox events and must compose with wrapper
  initialization effects in a defined order.
- Keep the core independent of a particular executor or transport. Runtime
  concerns may be interpreted at the boundary, but must not define the
  behavior semantics.

## Creation and lifecycle law

Bombay's creation leg is a typed, staged realization of actor creation, not a
literal transcription of Agha's `letactor`/`newadr`/`initbeh`. Retain the
following distinctions when documenting or changing it:

- The actor-model law is fresh allocation. Agha's allocation chooses an address
  fresh with respect to the actor configuration; behavior replacement through
  `become` is a different operation.
- Bombay derives a child route from a creator-local nonce known before
  interpretation. This lets an action refer to its requested child without
  performing allocation inside the pure fold.
- The interpreter must commit creation before interpreting same-action sends or
  local observation requests that depend on that child. This ordering and the
  packaging of allocation with initialization are deliberate Bombay policies,
  not general actor-model guarantees.
- A nonce collision must never mean replacement or overwrite. If freshness or
  binding cannot be established, the creation was not accepted; represent the
  failure explicitly and prefer a typed error over a production panic.
- Lifecycle provenance is semantic data, not an inference from allocation
  mechanics. Initial birth, later ordinary birth, and a replacement incarnation
  may all use fresh creation. Behavior must explicitly preserve any distinction
  the runtime is required to report through the typed effect path.
- A request or supervision decision is not a completed restart. The runtime may
  report `Restarted` only after successfully installing a creation that Behavior
  explicitly designated as a replacement incarnation. Installation failure
  must not produce a successful birth or restart diagnostic.

Describe `Actions` as Bombay's typed realization of the actor transition
effects—communications, fresh actor creation, and next behavior or
termination. Do not call the concrete Rust representation "exactly Agha's
effect triple": typed send products, termination, phases, initialization
effects, creator-local child routing, and interpretation ordering include
derived constructions and Bombay policy.

## Static-dispatch rule

There is no dynamic escape hatch in the core design.

Do not introduce:

- `dyn Trait`, trait objects, virtual dispatch, or erased boxed futures;
- `Any`, `TypeId`, downcasting, reflection, or type-name dispatch;
- an untyped/global message envelope or catch-all payload;
- runtime protocol registries, capability lookup, plugin lookup, or stringly
  typed routing;
- `unsafe` used to bypass a type or lifetime constraint;
- serialization as an internal substitute for typed protocol composition; or
- a runtime check for an invariant that can be encoded in a type.

This prohibition includes private implementation details and test helpers when
they would conceal a weakness in the public algebra. Heap allocation and
ordinary runtime state are not themselves forbidden; type erasure and runtime
substitution for compile-time proof are. Prefer monomorphized generics and
closed, exhaustive sum/product types. If those produce verbose types, add
truthful aliases or concrete wrapper types rather than erasing them.

Do not weaken bounds, add a default generic, widen visibility, or add a
convenience constructor merely to make invalid programs compile. Compiler
friction is often evidence that an invariant has not yet been modeled.

## Rust design discipline

Model the semantic domain before writing transition code. Rust's algebraic
data types are the default design language for this repository, not an
implementation detail to add after behavior has been encoded procedurally.

- Use `Option<T>` only when the domain is exactly “one `T` or absence.” Do not
  combine several `Option` values to encode mutually exclusive phases or
  correlated capabilities.
- Use `Result<T, E>` when an operation has a successful value or a typed,
  actionable failure. Do not represent rejection with a boolean, sentinel,
  empty collection, panic, log entry, or unrelated terminal outcome.
- Use an exhaustive `enum` (a sum type) when a value can be in one of several
  semantic states. Each variant must own exactly the data and capabilities
  valid in that state.
- Use a named `struct` (a product type) when several fields coexist. Give
  public effect lanes and protocol products semantic field names; do not expose
  positional `.inner`/`.own` chains whose meaning depends on nesting depth.
- Do not coordinate lifecycle with booleans such as `alive`, `pending`,
  `started`, or `restarting` when their combinations describe a state machine.
  Replace the combination with one exhaustive state enum so contradictory
  states are unrepresentable.
- Keep distinct facts distinct. In particular, a requested operation, an
  in-progress attempt, a committed success, a typed rejection, and a later
  retry are different states and must not share one flag or inferred counter.
- Never infer semantic provenance from sequence arithmetic, address reuse,
  timing, or adjacency when the provenance can be carried explicitly in a
  typed value.

Prefer functional transition structure: consume one current semantic state and
one typed event, compute the next semantic state plus explicit `Actions`, and
commit that pair once. Isolate transition functions by event or state where it
keeps each function total, short, and independently testable. Avoid partially
mutating several fields across a branch, using a temporary invalid state, or
relying on later code to restore an invariant. Mutation is permitted as an
implementation technique, but the code must still visibly implement a pure
state-transition law.

Follow established Rust ownership and type-design idioms. Prefer exhaustive
matching, ownership transfer, newtypes, associated types, and concrete generic
composition over flags, cloning to escape ownership, interior mutability,
index-based meaning, or runtime validation. A design that compiles is not
therefore idiomatic or accepted; its types must make the semantic law apparent
to a Rust reader.

Every new state, result, protocol, or effect product must pass a composability
review before implementation:

1. State the complete sum of phases and outcomes, including rejection, stale
   input, overlap, retry, and terminal cases.
2. Show which data belongs to each variant and which transitions are legal.
3. Check the type through every existing wrapper order and interpreter path.
4. Use named concrete products when multiple effect lanes travel together.
5. Prove that composition cannot drop, duplicate, reorder, reinterpret, or
   consume an inner lane unintentionally.
6. Reject the design if adding another wrapper would require callers to know a
   positional nesting path or synchronize correlated flags.

Do not begin with a compatibility patch and refactor toward the model later.
For semantic work, establish the truthful sum and product types first, then
implement the fold and update callers explicitly.

## Change method

Every change must be holistic. Never introduce a one-off or ad hoc addition
that solves only the immediate call site. Before editing, trace the concept
through the complete repository contract: public algebra and names, macros and
handwritten equivalents, wrapper composition in every relevant order,
interpreter realization, lifecycle and error semantics, examples and
documentation, source compatibility, and every applicable test layer. Reuse
existing semantic types and naming patterns wherever they remain truthful. If
the concept cannot be integrated coherently across those surfaces, stop and
redesign it instead of adding a local exception, compatibility shim, parallel
abstraction, or template-specific shortcut.

For every semantic change:

1. Write down the invariant or transition law before editing code.
2. Identify whether it is research-mandated, derived, or project policy.
3. Express the invariant in public types first, then implement the fold.
4. Keep each transition deterministic and inspectable: input state plus one
   typed event produces an explicit result and next state.
5. Check every existing composition order affected by the change. Wrapper
   nesting is part of the concrete protocol and must not silently drop,
   duplicate, reorder, or reinterpret an event or effect lane.
6. Avoid broad compatibility shims. If a sound design requires an API break,
   make the break explicit and update all callers and tests.
7. Before editing, inspect every interpreter that must realize the new
   contract. Do not publish an algebra whose success, rejection, ordering, or
   initialization semantics cannot be implemented end to end by the current
   runtime boundary.
8. Stop the implementation when the emerging code requires coordinated flags,
   nested positional effect access, inferred provenance, or short-circuiting
   that skips required effects. Return to the state and protocol design rather
   than repairing those smells incrementally.

Do not add speculative abstractions. Add a trait or combinator only when its
laws are clear, its composition is type-safe, and at least one concrete use
demonstrates why it belongs at the algebraic boundary.

## Compiler-friction checkpoint

The compiler is an invariant oracle and a veto, never a source of architecture
or a work queue. Compiler output may reject a candidate model; it may not
originate or justify a type, trait, bound, callback, wrapper, alias, marker,
event variant, route, associated type, constructor parameter, default generic,
visibility change, or structural path.

### Design-provenance gate

Every production edit that changes the static or semantic shape of the system
must have all of the following provenance recorded before the edit exists:

1. a stated actor-model, derived, or deliberate Bombay law;
2. the user-level syntax and complete observable transition that law requires;
3. a focused compile or pure-fold regression written in domain vocabulary,
   before production code, which fails for that law on the prior design; and
4. the existing lower-order behaviors, layers, event/effect products, and
   interpreter capabilities that the implementation will compose or delete.

“The compiler requires it,” “this bound makes the implementation type-check,”
and “the callers can be migrated” are never valid provenance. If a compiler
error appears to require new semantic surface not already named by the recorded
law and regression, stop the implementation and return to the model. Do not
edit around the error, even once.

Separate design from fallout. A stage that invents or changes an abstraction
may update only its focused law tests and the smallest end-to-end interpreter
witness. It must not bulk-migrate the catalogue. Mechanical migration is a
later, separately measured stage and may only apply the already-proven syntax;
it may introduce no new semantic type, bound, wrapper, policy, route, or test
fixture. If migration discovers one, the abstraction is unproven and the
design stage reopens.

At every checkpoint, audit the entire working-tree diff by provenance rather
than asking whether it compiles. For each new or reshaped production symbol,
point to its pre-edit law and regression. Remove any symbol or caller plumbing
whose only explanation is a compiler diagnostic. A green build with an
unproven symbol is a failed design; a temporarily failing build while an
invalid candidate is being removed is not a reason to preserve that candidate.

### No-op symptom prohibition

A general behavior or template API must never require a caller to provide a
no-op policy, callback, marker, route, alias, wrapper, event variant, or
discarded transition merely because the template's signature demands one.
This prohibition applies equally to production callers, examples, unit and
compile tests, model tests, properties, fuzz targets, benchmarks, macros, and
downstream interpreters.

The first legitimate caller that can only satisfy a new abstraction with a
no-op or placeholder invalidates that abstraction. Stop immediately: do not
fix a second caller, do not add a default or convenience constructor, and do
not hide the placeholder in a helper. Restore the user-level composition law
first, beginning with ordinary `BehaviorLayer` composition and the existing
event/effect products. A mandatory input is lawful only when absence is itself
an illegal semantic state proven by the public type and exercised by every
valid construction.

Mechanical migration may begin only after a focused pure-fold test proves the
new law, at least two unrelated real templates and two wrapper orders use it
without placeholders, and the change deletes or subsumes the repeated
machinery it replaces. A green compiler cannot waive this rule.

The no-op rule is only a high-signal symptom check. Passing it does not satisfy
the design-provenance gate: compiler-driven surface can still be invalid even
when every caller performs non-empty work.

Trigger this checkpoint before fixing the second call site when two failures
share any of the following:

- the same missing bound, ingress path, occurrence proof, or associated type;
- the same template-specific constructor, alias, `With*` variant, builder, or
  policy marker;
- the need to count wrapper depth or spell `.inside()` differently because an
  otherwise ordinary `BehaviorLayer` was added;
- a new application event, type alias, or adapter whose only purpose is to
  satisfy machinery internal to a reusable template; or
- the same routing, lifecycle, availability, or hosting operation implemented
  independently by two higher-order templates.

When triggered, stop production and migration edits and perform this audit:

1. Cluster every matching compiler error and locate every occurrence across
   Behavior, actors, macros, testkit, interpreters, examples, and Bombay.
2. State the user-level composition syntax that should work without naming the
   resulting behavior type. Write the smallest compile-only or pure-fold test
   for that syntax before changing production code.
3. Identify the one semantic law and the existing lowest-order behavior or
   layer that should own it. Treat repeated generic plumbing as evidence of a
   missing composition law, not evidence that every caller needs an adapter.
4. List the aliases, `With*` variants, constructors, wrappers, marker types,
   and duplicated folds the design will delete. If it deletes none, presume
   the proposed abstraction is relocating complexity and redesign it.
5. Prove the same mechanism through at least two unrelated templates and two
   wrapper orders. A mechanism demonstrated only by proxy/supervision, only by
   pools, or only by one test fixture is still template-specific.
6. Inspect the interpreter boundary before accepting the algebra. Static
   metadata that no interpreter can consume is not an implemented feature.
7. Only after the public algebra and interpreter path are coherent may bulk
   call-site migration begin.

During this checkpoint, making more tests compile is not progress. Do not add
custom fixture events, aliases, explicit generic annotations, compatibility
constructors, or path-counting calls to preserve the suspect API. If such
edits have already begun, stop and separate or revert only those edits before
continuing; never use unrelated user changes as rollback collateral.

At the end of each compiler-fix batch, answer these questions explicitly:

- Did this batch remove a repeated concept, or merely teach more callers its
  current shape?
- Would adding one unrelated `BehaviorLayer` require another caller edit?
- Is any public name describing structural position (`WithParent`, `Inner`,
  `AtPath`) rather than a distinct state-transition law?
- Did test code gain more protocol/type plumbing than production code lost?

Any “yes” to the last three questions reopens the design checkpoint and blocks
further migration.

## Change containment and abstraction budget

Correct architecture does not justify unlimited code growth. Treat source
size, public surface, and reviewability as design constraints. A typed wrapper
that merely relocates complexity is not a successful composition.

Keep the requested blocker separate from later cleanup. Complete and verify
the blocker before editing production code for a broader audit or refactor. An
audit may add independent tests and identify later work, but it must not grow
the current production design unless a failing law independently proves that
the additional machinery is necessary.

Before the first production edit, record a change ledger containing:

- the exact blocker and the smallest end-to-end failing regression;
- expected files touched and expected production line delta;
- public types expected to be added and removed; and
- the existing folds, products, and compositions that will be reused or
  deleted.

The following are automatic stop thresholds for the cumulative task, not
targets to evade by splitting commits:

- more than 15 changed files;
- more than 500 net new production lines; or
- more than three new public types.

When any threshold is reached, stop before further production edits. Report
the current ledger and obtain explicit user authorization for the expanded
surface. Prior instructions to “finish,” “audit everything,” or “do it
holistically” do not waive this checkpoint.

Every new wrapper, builder, trait, or public product must answer all of these
questions before implementation:

1. What unique semantic state does it own?
2. What unique event or effect transformation does it implement?
3. Why can the existing concrete composition not express the law?
4. What existing production code or caller-side machinery does it delete?
5. Which concrete use demonstrates that the abstraction belongs here?

Reject an abstraction that only renames a nested type, forwards unchanged
events or effects, stores another wrapper, hides a structural path, or makes a
single example look shorter while increasing the total public surface. Start
with a compile-only or pure-fold test that attempts the desired syntax using
existing types. Add production machinery only after that test isolates the
precise compositional gap.

At each logical checkpoint, measure the complete working tree, including
untracked files, and report:

```text
production: +A / -B / net C
tests:      +A / -B / net C
public API: +N types / -M types
```

Do not describe a change as cleanup, consolidation, or code reduction when its
production delta is net-positive. Separate new capability code from deletion
work so each can be judged honestly. Work in independently reviewable stages;
do not combine the blocker, a catalogue-wide redesign, wrapper cleanup, and a
test expansion into one undifferentiated patch.

## Testability standard

Extreme testability is a design constraint. Behavior logic must be runnable as
a pure, deterministic fold without spawning tasks, sleeping, opening sockets,
or depending on wall-clock time. Represent time and lifecycle observations as
typed inputs. Assert on complete `Actions` values or explicit traces.

Tests for a semantic change should cover all applicable layers:

- **Example/unit tests:** the intended transition and each boundary case.
- **Composition tests:** relevant wrapper orders, event routing, initialization
  ordering, and effect accumulation.
- **Independent model tests:** compare stateful features against a small model
  written from the documented contract, never copied from implementation
  branches.
- **Exhaustive tests:** enumerate tractable small state spaces, especially
  ordering, lifecycle, budget, and boundary conditions.
- **Property tests:** generate longer sequences and assert invariants after
  every step, not only at the end.
- **Fuzz tests:** extend the corresponding target for parsable or stateful
  transition surfaces.
- **Compile-fail tests:** when safety means that an invalid protocol,
  capability, birth, phase, or composition must not compile, test that fact at
  compile time. A runtime rejection test is not an adequate substitute.

Regression tests must fail for the original bug for the intended reason. When
testing a model, use independent vocabulary and structure so the test cannot
reproduce the same implementation error. Use paused or explicitly advanced
time for timer semantics; do not write timing-sensitive sleeps.

Before broadening a semantic change, run its focused regressions in both debug
and optimized builds. Assertions must be observational only: never place a
state transition, mutation, ownership transfer, or required function call
inside `assert!`, `debug_assert!`, or their equality variants. Treat
`clippy::debug_assert_with_mut_call` and `clippy::let_underscore_must_use` as
denied. For lifecycle and generation laws, explicitly replay the same fact in
an optimized test and prove that it cannot be accepted twice.

A regression is not accepted merely because it passes after the fix. Restore
or simulate the original defect and establish that the test fails for the
intended law. Tests must assert complete `Actions` lanes or an independent
trace; repeated assertions of the same field, discarded `Actions`, predicted
nonces, or models copied from implementation branches are invalid evidence.

Test-only `unwrap`/`expect` is acceptable when it asserts test setup or an
expected successful transition. Production panics require a documented,
provably unreachable invariant or a genuine resource-exhaustion boundary;
prefer typed errors otherwise.

## Repository boundaries

- `crates/behavior`: the public behavior primitives and minimal
  interpreter-facing contracts. Keep this surface small and independent of
  reusable actor implementations.
- `crates/actors`: reusable actors, concrete protocol transformations, and
  composition helpers built on `bombay-behavior`. It may depend on
  `crates/behavior`; the reverse dependency is forbidden.
- `crates/behavior-macros`: syntax generation only. Macros must emit the same
  concrete types a careful user could write and must not hide dynamic behavior.
- `crates/behavior-testkit`: independent models, adversarial suites,
  exhaustive checks, properties, fuzz targets, and benchmarks. It must not
  become a back door for production semantics.

Public API changes require rustdoc that states semantics, invariants, errors,
and panic conditions. Comments should explain laws and non-obvious constraints,
not restate syntax.

## Verification gates

Use the pinned toolchain and keep the workspace warning-clean. During
development, run the narrowest relevant tests first. Before considering a
change complete, run:

```sh
cargo nextest run --workspace
nix flake check
```

`nix flake check` is the authoritative repository gate and covers the build,
workspace tests, documentation, Rust formatting, TOML formatting, dependency
audit, and dependency policy. Run relevant fuzz targets for changes to
state-machine or protocol sequence handling; run `cargo bench` only when
evaluating performance, since benchmarks are not nextest binaries.

A change is not complete merely because happy-path runtime tests pass. It is
complete when the type surface preserves the stated invariants, invalid uses
fail to compile where appropriate, the transition laws are tested
independently, the final change ledger reports the complete tracked and
untracked delta, and the full repository gates pass.
