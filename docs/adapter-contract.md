# Behavior Adapter Contract

This contract uses the orthogonal roles defined in
[Protocol, ingress, behavior, and effect algebras](protocol-algebra.md). An
adapter drives a `Behavior`; ordinary delivery addresses its projected
`Behavior::Protocol`.

This document defines the complete runtime-neutral contract for driving a
concrete Bombay behavior. It does not define a runtime, executor, mailbox,
transport, or capability registry. Bombay's Driver and an independent adapter
are both implementations of this same boundary.

## Universal execution law

An adapter accepts one exact, closed behavior `B` and an environment capable of
producing `B::Event` and interpreting every lane of `B::Sends` and `B::Birth`:

```text
own one B
    -> initialize B exactly once
    -> commit the complete initialization Actions
    -> if terminal, retire
    -> otherwise repeat:
        obtain one B::Event
        -> fold it exactly once
        -> commit the complete successful Actions exactly once
        -> if terminal, retire
```

The algorithm is identical for `Machine`, `Supervisor`, `Deadline`, routing
templates, persistence-derived templates, nominal domain behaviors, and any
composition of them. An adapter must never inspect a wrapper type or select a
template-specific loop.

## Static boundary

For a concrete `B: Behavior<Ph = Never>`, the adapter must statically supply:

- an ordered source of the exact closed `B::Event` sum;
- an interpreter for every named lane in `B::Sends`;
- fresh creation for `<B::Birth as BirthMode>::Child`; a child may be one
  concrete behavior or a closed recursive `ChildChoice` whose exhaustive
  `DispatchBirth` implementation requires one concrete `InstallBirth` adapter
  per alternative;
- `Send` futures from concrete installation and exhaustive dispatch, preserving
  the recursive driver's eligibility for a thread-safe executor spawn;
- exact behavior and capability errors;
- incarnation-local retirement.

Missing capability support is a compile-time failure. Dynamic capability maps,
`Any`, downcasting, type-name dispatch, erased messages, and string routing are
not valid adapter mechanisms.

## Activation

`Activate::initialize` consumes a concrete definition and returns:

```rust,ignore
Initialized {
    behavior: Active<B>,
    actions: Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
}
```

The complete initialization actions must be committed before the event source
can expose the actor for delivery. A failed initialization consumes the
definition and produces no successful actions. `Active<B>` cannot initialize
again.

## Action commitment

One successful fold yields one indivisible decision value. The adapter receives
that complete value exactly once. This does not make external effects
transactional.

Commitment obeys these laws:

1. creations are attempted in vector order before same-action sends;
2. a nonce collision is a typed rejection, never replacement;
3. creation observations describe only creations in that exact action value;
4. order within every named send lane is preserved;
5. no payload is dropped, duplicated, or reconstructed;
6. a successfully committed prefix remains factual after a later failure;
7. the adapter performs no implicit retry or rollback;
8. final actions are committed before terminal retirement;
9. delivery admission does not claim recipient processing or business success.

For a heterogeneous creation sum, alternative dispatch occurs inside the same ordered
creation loop. It does not create another nonce namespace: collision checks,
`ObserveCreation`, and `ObserveChild` correlation all use the original
creator-local nonce and provenance. A successful arm installs the contained
concrete behavior and binds its declared `Behavior::Protocol`; the creation
choice sum is neither an actor nor a public protocol.

## Named products

Behavior Actors products expose semantic fields specifically so adapters can
compose interpreters without positional nesting knowledge. For example:

```rust,ignore
let DeadlineSends { behavior, schedules } = sends;
interpret_behavior(behavior)?;
interpret_schedules(schedules)?;
```

```rust,ignore
let SupervisorSends {
    behavior,
    child_observations,
    replacement_commands,
    failure_reports,
} = sends;
```

An adapter may define its own local, statically dispatched interpretation
traits for these public send products. Apart from the minimal `InstallBirth` /
`DispatchBirth` completeness contract needed to install either a direct child
or a heterogeneous creation sum without erasure, the behavior crates do not
prescribe one runtime trait or error sum. Public products must retain named owned fields so such
implementations require neither tuple positions nor wrapper inspection.

## Event injection

Runtime facts return as later typed events. `InjectEvent<Input, Path>` proves
both that the concrete closed event algebra accepts an input and which layer
owns it. `Here` selects the current layer; `Inside<Path>` selects the same
capability below exactly one outer layer. `EventLayer<Owned, Inner>` is the
canonical structural coproduct for a wrapper with one owned lane. Templates
with several related owned facts use a named exhaustive event sum and obey the
same path law.

There is no payload search or fallthrough. The request selects its owner when
emitted. A stale result is inert at that owner; it is never offered to an inner
layer merely because that layer accepts the same Rust payload type. Named
effect products preserve the dual structure: a wrapper's own service lane
returns through `Here`, while a request in its `behavior` product remains
owned by the inner layer and returns through `Inside<_>`.

Timer, creation, lifecycle, and observation callbacks must enqueue their
structurally selected typed input; they must not synchronously re-enter the
behavior fold.

## Error and cancellation law

Behavior failure and environment failure remain distinct concrete error types.
An adapter may compose capability failures into its own closed `thiserror` sum,
but must not erase or stringify them.

Cancellation drops the values owned by the cancelled future. It cannot claim
that asynchronous retirement or an uncertain external effect completed. Panic
or cancellation must not permit later polling of the consumed execution.

## Nameability

Local construction relies on inference:

```rust,ignore
let behavior = Deadline::new(Stash::new(worker, route), timer, when, react);
run(behavior, environment);
```

Adapter entry points should be generic over the concrete behavior:

```rust,ignore
fn run<B, E>(behavior: B, environment: E)
where
    B: Behavior<Ph = Never>,
    E: EnvironmentFor<B>,
{
    // Activate and drive this exact B.
}
```

This preserves the exact event, sends, phase, error, birth, initialization,
and transition contracts while allowing callers to rely on inference. A rare
component-internal storage boundary may use an ordinary Rust alias or newtype;
the catalogue does not define another macro or erased adapter type for it.

## Conformance checklist

`crates/actors/tests/runtime_contracts.rs` is the compile-time template
manifest for interpreter-originated lanes. It enumerates every timer,
observation, creation, parent-report, and shutdown request/fact pair and fails
to compile when a concrete template emits a request whose returned fact cannot
enter the owning event sum, or when `ShutdownChild<C>` names a child protocol
that cannot accept `ShutdownRequested`.

An adapter is conforming only when tests kill each of these inversions:

| Law | Required negative proof |
|---|---|
| Initialization once | missing or duplicate initialization fails |
| Initialization precedence | ingress before initialization commitment fails |
| One event, one fold | skipped or duplicate fold fails |
| One decision, one commit | dropped or duplicate action commitment fails |
| No prefetch | requesting the next event before commitment fails |
| No re-entry | folding while commitment is pending fails |
| Complete output | projecting or dropping a named lane fails |
| Lane order | reordering one lane fails |
| Creation precedence | sending before completing creation attempts fails |
| Creation-result scope | cross-action or reordered results fail |
| Terminal fusion | work after stop, failure, or input closure fails |
| Exact errors | behavior or capability error erasure fails |
| No retry | a second uncertain commitment attempt fails |
| No rollback | reconstructing predecessor state fails |
| Honest completion | external delivery completion claims fail |
| Retirement | missing or duplicate ordinary retirement fails |

The behavior testkit supplies deterministic folds, model traces, exhaustive
sequences, properties, and fuzz targets for the template side of these laws.
An execution adapter must additionally test its asynchronous event source,
commitment, cancellation, and retirement implementation.
