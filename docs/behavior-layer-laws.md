# Behavior layer laws

This document is the acceptance contract for every same-actor behavior
composition in Bombay. It is deliberately a semantic review rule, not another
Rust trait hierarchy.

`BehaviorLayer<B>` has one narrow job: construct a fully concrete output
behavior from `B` without forcing a generic owner to name that output type.
The trait does not make an output lawful merely because the associated type
implements `Behavior`. The concrete output behavior owns and must prove the
event, effect, initialization, error, birth, and lifecycle transformation
described below.

## Provenance of the laws

Agha's actor calculus supplies the foundational boundary: an actor processes a
communication, may send communications, create actors, and designate its next
behavior. Actor configurations compose through their explicit receptionist
and external-name interfaces. Agha does not specify Bombay's same-mailbox Rust
wrapper order, typed event sums, typed effect products, initialization fold,
controlled errors, shutdown protocol, or structural child occurrences.

Consequently:

- single-communication processing, actor-owned behavior change, messaging to
  known actors, and fresh actor creation are actor-model laws;
- `Behavior`, `Actions`, `EventLayer`, `SendLayer`, initialization ordering,
  creator-local child routing, and structural occurrences are derived Bombay
  constructions; and
- wrapper precedence, error policy, shutdown policy, buffering, retries, and
  lifecycle reactions are deliberate Bombay policy choices owned by concrete
  behaviors.

References:

- Gul Agha et al., *A Foundation for Actor Computation*.
- Gul Agha, *Actors: A Model of Concurrent Computation in Distributed
  Systems*.

## Construction is not transition composition

For a layer value `L` and behavior `B`:

```text
L.layer(B) -> Output
```

is construction only. It must not initialize an actor, process an event,
allocate an address, interpret an effect, or acquire a runtime capability.
`Output: Behavior` defines the operational composition.

The blanket `Fn(B) -> Output` implementation exists only so ordinary concrete
constructors and closures can be reused by topology owners. It is not evidence
that two actor laws have been composed. A catalogue migration is complete only
when the resulting concrete behavior satisfies every applicable law below and
the former bespoke implementation has been deleted.

Do not add associated types, marker types, `*Layer` configuration wrappers, or
aliases merely to make a generic call nameable. A new concrete wrapper is
admissible only when it owns a distinct state or event/effect transformation.

## Enforcement hierarchy

No single checker can establish these laws. The repository uses four
complementary levels, in this order:

1. this semantic contract decides what the behavior must mean;
2. concrete Rust sums, products, capabilities, and compile-fail tests make
   representable invariants static;
3. compiler and Clippy hard failures reject unsafe code, suspicious constructs,
   ignored `must_use` results, mutation hidden in debug assertions, placeholders,
   and unexplained lint suppressions; and
4. pure-fold, model, exhaustive, property, fuzz, and interpreter-path tests
   establish the observable transition laws.

Broad style, complexity, and performance lint groups are review inputs rather
than repository gates. They must not compel aliases, wrappers, boxing, helper
traits, or reordered branches that obscure the actor algebra. Conversely, a
lint suppression is not evidence that a semantic law holds. Clippy cannot prove
effect conservation, error atomicity, lifecycle authority, initialization
order, or higher-order reuse; those require the types and independent tests
above.

## L1: stable public capability

A transparent layer preserves `B::Protocol` exactly. A layer may change the
public protocol only when protocol adaptation is its documented semantic law.
Changing the internal event algebra, send product, child topology, error,
phase, or state representation does not by itself create a new public
destination identity.

The address namespace is preserved. A logical recipient, established
recipient, creator-local child route, and established parent relationship keep
their original routing intent through composition. A layer must not replace
one with another to satisfy a bound.

## L2: closed and single-owned event algebra

The output event is the exhaustive sum of its owned inputs and the preserved
inner event algebra.

- Every accepted event has exactly one semantic owner.
- An inner event is injected once and delegated once.
- An owned event is either handled by the layer or deliberately transformed
  into one exact inner event.
- Equal payload representations from different sources or child occurrences
  remain distinguishable by their semantic source.
- No implementation searches by payload type, wrapper depth, type name, or a
  runtime registry.
- Adding an unrelated outer layer cannot change which existing law owns an
  input.

Public domain messages and private system inputs are members of the same
closed `Behavior::Event` sum, but only the domain-message lane belongs to
`Behavior::Protocol`.

Two ingress traits that differ only because Rust cannot prove their blanket
implementations disjoint do not establish two semantic input laws. Such a
split must be redesigned around explicit event ownership rather than exposed
as architecture.

## L3: one deterministic fold

One output event produces one `Result<Actions, Error>`. A layer does not run an
interpreter, spawn work, call a clock, or perform another actor turn.

For a delegated event, the inner fold is invoked exactly once. For an owned
event, the layer's documented transition is invoked exactly once. If an owned
transition also induces an inner transition, the order and combined state
commit are part of that layer's law and must be tested as one atomic fold.

Construction layers may be nested:

```text
b.layer(first).layer(second)
```

Nesting is observationally associative with the equivalent composed closure:
it must not change event ownership or action order merely because the caller
grouped construction differently. This is observational equivalence, not Rust
type equality.

## L4: effect conservation

The output `Actions` product retains every inner effect and every layer-owned
effect in semantically named lanes.

- Inner sends are not dropped, duplicated, consumed, or reinterpreted.
- The order within each inner lane is unchanged.
- Layer-owned sends occupy an owned lane; positional `.inner.inner` access is
  not a public semantic interface.
- Mapping sends preserves creations and the exact next-behavior verdict.
- Mapping the verdict preserves sends and creation order.
- A wrapper may consume an inner effect only when effect interception is its
  explicitly documented law and the replacement outcome is complete and
  observable.

The normal structural interpretation order is inner effects before
outer-owned effects. A different order requires a named product, a stated
reason, and complete order tests; it cannot arise accidentally from field
layout.

## L5: initialization conservation

Construction performs no initialization. Output initialization invokes the
inner initialization fold exactly once unless construction or outer
validation rejects before an actor definition exists.

For successful transparent composition:

1. inner initialization effects retain their exact values and order;
2. outer initialization effects are added in the layer's named send and birth
   lanes;
3. the declared structural order is inner before outer; and
4. the combined initialization `become` decision is explicit.

If initialization fails, no partial `Actions` exists. Validation or staged
state changes performed before delegation must not leave the output in a
partially initialized state.

Every pair of initialization-owning layers must be tested in both wrapper
orders. Commutativity must never be assumed.

## L6: controlled-error atomicity

An output error is a truthful exhaustive sum of inner failures and failures
owned by the layer. It must retain the rejected input and any other owned value
needed for recovery.

- Delegated errors preserve the inner cause without reclassification.
- Returning `Err` emits no partial effects.
- State needed by a retry remains unchanged unless the documented error law
  says ownership was consumed.
- A layer cannot mutate one submachine and then discover that another
  submachine rejects the same transition.
- Expected availability, capacity, overlap, stale-input, and shutdown outcomes
  do not become opaque actor crashes after mailbox admission. They are
  successful typed responses or reports when a customer can recover.

Compiler difficulty coordinating two mutable folds is evidence that their
joint transition has not yet been modeled atomically; it is not permission to
add a wrapper or weaken an error bound.

## L7: birth conservation and freshness

A transparent layer preserves the complete inner birth algebra. A
topology-owning layer appends its children as distinct structural occurrences.

- Inner creations retain order, concrete child behavior, nonce, and
  `CreationKind`.
- An inner child is never reinterpreted as a layer-owned child.
- Duplicate protocols remain distinct occurrences.
- A nonce remains correlation data, not an address or freshness proof.
- Same-action creation dependencies retain Bombay's creation-before-send
  interpretation policy.
- Replacement provenance is carried explicitly and cannot be inferred from
  address or sequence reuse.

A layer that changes topology must expose the changed birth algebra. It may not
claim transparent topology merely so an old role or interpreter continues to
compile.

## L8: next behavior and lifecycle authority

The inner `Continue`, phase change, or termination verdict is preserved unless
the layer owns a documented lifecycle law that changes it.

Exactly one layer owns each lifecycle fact. Observation, shutdown,
replacement, timeout, retry, and termination facts retain their source and
generation. Duplicate, stale, contradictory, and late facts have exhaustive
outcomes. Shutdown is defined in every intermediate state.

An outer lifecycle layer may suppress an inner verdict only when its state sum
explicitly represents why and what later event completes the decision. It may
not use a boolean, inferred ordering, or a later cleanup branch to repair a
temporarily invalid combination.

Wrapper order is policy. For example, `StopOnShutdown<Stash<B>>` and
`Stash<StopOnShutdown<B>>` are not presumed equivalent; each applicable order
must either have a documented law and tests or be statically unavailable.

## L9: higher-order reuse

A higher-order actor may retain a separate implementation only for a genuinely
joint law that cannot be expressed by composing existing concrete behaviors.
The review must name the correlated state and prove why separate actors or
ordinary same-actor layers would alter the semantics.

It is not sufficient that two implementations call the same private helper.
If a higher-order template merely selects, parameterizes, or translates an
existing behavior, it must be ordinary typed composition and the duplicate
fold must be deleted.

Examples of required review:

- keyed affinity should be a layer over the ordinary pool unless key binding
  is inseparable from the pool's assignment/termination commit;
- fixed supervision may own one shared fleet state machine, but pools must not
  copy its restart, installation, or shutdown transitions; and
- a queue inside a pool is reusable only when its ownership, admission,
  completion, and interruption laws are actually the same—not merely because
  both use FIFO storage.

## L10: static metadata follows the real algebra

Protocol hosting, interpreter obligations, and birth requirements are
structural projections of the concrete output behavior. They must not be
owner-authored duplicate lists, runtime registries, or aliases that repeat an
already inferable composed type.

Static proof traits may expose an associated product when a generic framework
must consume the product. Such a trait is metadata, not an operational layer,
and its tests must use the real owner/interpreter path rather than a synthetic
lookalike proof.

## Required evidence

A concrete layer is not accepted by a compile-only example. Its evidence must
cover every applicable item:

1. construction without naming the output behavior type;
2. complete initialization `Actions` and order;
3. every owned event and a delegated inner event;
4. complete sends, births, and next verdict;
5. inner and owned controlled errors with state conservation;
6. both relevant wrapper orders;
7. duplicate, stale, contradictory, and shutdown lifecycle facts;
8. a complete outer interpreter path for every new effect capability;
9. an independent model or exhaustive trace for stateful laws; and
10. restoration or simulation of the original defect proving that the
    regression fails for the intended reason.

Assertions observe a completed transition. They never perform the transition,
move required state, or call a required function inside the assertion.

## Rejection checklist

Reject a proposed layer or helper if any answer is “no”:

1. Does it own a distinct state or event/effect transformation?
2. Is every event owned exactly once?
3. Are inner effects, births, errors, and verdicts conserved?
4. Is initialization order explicit?
5. Is failure atomic across every coordinated state machine?
6. Does it work through every relevant existing wrapper order?
7. Does it delete more bespoke caller machinery than it adds?
8. Can a user construct the composition without naming its output type?
9. Do tests exercise the public behavior law rather than its helper types?

A compiler error is diagnostic evidence for this review. Passing the compiler
is the final representation check, not the design criterion.
