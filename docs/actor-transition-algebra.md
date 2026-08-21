# Actor transition algebra

This is the canonical semantic map for Bombay Behavior.

## Laws and project constructions

The actor-model laws preserved here are:

1. an actor processes one accepted communication at a time;
2. a transition may communicate with known recipients, create fresh actors,
   and designate the behavior used for the next communication; and
3. a newly allocated actor address is fresh with respect to the actor
   configuration.

Bombay derives a concrete typed construction from those laws:

```text
Behavior::transition(ActiveTurn, Behavior::Event)
    -> Result<Actions<Addr, Phase, Sends, Birth>, Error>
```

`Actions` is the only effect boundary. Typed send products, closed creation
products, initialization effects, phases, explicit termination, structural
event paths, creator-local child routing, and interpretation order are Bombay
constructions and policies, not claims about the surface syntax of Agha's
formal calculus.

## Four orthogonal roles

| Role | Owns |
|---|---|
| `Protocol` | canonical destination identity, address namespace, message type |
| `Behavior::Event` | every public and interpreter-originated event accepted by a concrete behavior |
| `Behavior` | state, initialization, and the pure event fold |
| `Actions` | all communications, staged fresh creations, and the next-state verdict |

A protocol is not a behavior. It has no state, initialization, internal event
lanes, effects, errors, phases, or birth capabilities. A behavior is not a
protocol supertrait: senders need only the destination's public signature, not
proof of its current implementation.

A nominal actor may implement both traits and use `Behavior::Protocol = Self`.
Transparent wrappers preserve the inner protocol while changing the concrete
behavior and event/effect algebra.

## Destination evidence

| Type | Evidence | Requires address lookup? |
|---|---|---|
| `Recipient<P>` | logical address with canonical protocol `P` | yes |
| `Delivery<P>` | logical recipient plus `P::Msg` | yes |
| `ChildRoute<C, O>` | staged route for concrete child `C` at occurrence `O` | no address exists yet |
| `ChildDelivery<P, O>` | message to a committed local occurrence/nonce binding | no protocol-wide lookup |
| `EstablishedRecipient<P>` | exact installed endpoint for `P` | no |
| `EstablishedDelivery<P>` | exact endpoint plus `P::Msg` | no |
| `EstablishedActor<B>` | exact endpoint plus proof of installed concrete behavior `B` | no |

`P` is always canonical protocol identity. `O` is structural occurrence
evidence used to navigate duplicate child declarations. It is not a key,
address, or second identity.

The runtime selects the endpoint representation through
`EndpointAddress::Established<P>`. This makes the endpoint family a property
of the runtime-owned address namespace. Generic application types and protocol
owners do not implement key traits or carry runtime types. The inert endpoint
family requires `Clone`, not `Send`. `InterpretSends` and concrete request
interpretation require `Send` when an effect actually crosses an asynchronous
executor boundary.

## Fresh creation

`Create<A, C>` stages a concrete child behavior, creator-local nonce, and
`CreationKind`. Staging allocates nothing. The nonce cannot be converted to an
address and proves neither identity nor freshness.

The interpreter performs, in order:

1. fresh address allocation;
2. child initialization;
3. initialization-effect interpretation;
4. endpoint installation; and
5. creator-local binding commit.

Only the committed path may produce
`EstablishedCreation<P, Occurrence>::Installed`. Every failure produces
`Rejected` with `CreationRejection`; rejection contains no endpoint and binds
nothing. A nonce collision is rejection, never replacement or overwrite.

`CreationKind::ReplacementIncarnation` records Behavior-authored provenance.
It is still fresh allocation. A runtime may report a restart only after the
corresponding replacement creation commits successfully.

Creation facts are indexed by canonical child protocol and structural
occurrence—not by an entire parent behavior. A fact can be strengthened to
`EstablishedActor<RoleChild<Parent, Occurrence>>` only at the topology boundary
where `ChildRole<Parent>` genuinely proves the concrete installed behavior.
This avoids forcing arbitrary consumers or wrappers to pretend to be actors.

## Event and effect composition

`EventLayer<Owned, Inner>` forms a closed event sum. `Here` identifies the
current owner and `Inside<Path>` identifies an inner owner. `InjectEvent`
constructs exactly the selected lane; it never searches by payload type.

`SendLayer<Owned, Inner>` is the corresponding named effect product.
`SendsFor<Event>` proves that every interpreter request returning to its
emitter has a valid structural ingress. `InterpretSends` visits inner effects
before wrapper-owned effects, preserving initialization and delegated
transition order. Independent named lanes retain their own order; no global
order is invented between unrelated lanes.

Composition must preserve these invariants:

- every accepted event produces one result;
- mapping sends preserves creations and the exact next verdict;
- mapping the next verdict preserves sends and creation order;
- wrapping cannot drop, duplicate, reinterpret, or consume an inner lane;
- duplicate structural occurrences remain distinct;
- established, child, and external destinations are not reindexed merely
  because the emitting behavior is wrapped; and
- controlled failure emits no partial effects.

## Static interpretation

`InterpretSends`, `InterpretRequest`, `InterpretDelivery`,
`InterpretChildDelivery`, and `InterpretEstablishedDelivery` are
monomorphized capabilities. Closed child sums use `DispatchBirth` and one
`InstallBirth<Position, Child, ...>` implementation per concrete occurrence.
Missing support fails to compile.

`FoldBirthNode` is the structural dual needed by runtimes that retain
creator-local state per direct child occurrence. Behavior owns the closed
`Behavior` leaf / `ChildChoice` / `Never` recursion and supplies the same
`ChildHead` / `ChildTail<_>` navigation evidence used by installation. A
runtime-owned `BirthNodeMapper` supplies only the mapped storage constructor.
This is a type-level derived construction, not an actor operation: it performs
no fold transition, allocation, binding, lookup, or effect interpretation.
Transitive descendants remain owned by the concrete child actors that may
create them.

No core path uses trait objects, runtime protocol registries, reflection,
downcasting, type-name dispatch, serialization, or unsafe type escape hatches.

## Code map

- `crates/behavior/src/transition.rs`: protocol and behavior contracts
- `crates/behavior/src/effects/`: actions and static send interpretation
- `crates/behavior/src/actor/addressing.rs`: logical and established recipients
- `crates/behavior/src/actor/creation.rs`: staged creation and structural dispatch
- `crates/actors/src/protocol/established.rs`: exact creation, observation, and shutdown protocols
- `crates/actors/src/requirements.rs`: occurrence-preserving installation requirements
