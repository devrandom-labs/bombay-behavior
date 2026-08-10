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

Test-only `unwrap`/`expect` is acceptable when it asserts test setup or an
expected successful transition. Production panics require a documented,
provably unreachable invariant or a genuine resource-exhaustion boundary;
prefer typed errors otherwise.

## Repository boundaries

- `crates/behavior`: the public algebra, concrete combinators, and minimal
  interpreter-facing contracts. Keep this surface small.
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
independently, and the full repository gates pass.
