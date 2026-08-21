# Behavior adapter contract

An adapter drives one concrete `B: Behavior`. It owns execution and resource
capabilities while the behavior owns state and semantic decisions.

## Universal execution law

For each actor incarnation an adapter must:

1. consume `B` through `initialize` exactly once;
2. completely interpret initialization `Actions` before accepting mailbox
   events;
3. process at most one accepted `B::Event` at a time;
4. call `transition` exactly once for that event;
5. if the fold succeeds, interpret the returned `Actions` exactly once; and
6. commit its `Continue`, `Goto`, or `Stop` verdict exactly once.

A controlled fold error commits no partial actions. Cancellation or adapter
failure must not manufacture a successful semantic fact.

## Static boundary

The adapter is monomorphized over the complete behavior and its capabilities:

```text
B: Behavior
B::Sends: InterpretSends<Adapter, B::Event, Here>
B::Birth::Child: DispatchBirth<B::Protocol::Addr, Installer, Output, Error>
```

The concrete bounds vary with the host design, but every lane and child
alternative must have a compile-time implementation. A universal driver may
be generic; it may not erase events, effects, endpoints, futures, or child
types to achieve universality. `EndpointAddress` itself does not require every
endpoint to be `Send`. A thread-safe driver instead requires `Send` on the
concrete `Actions`, request, event, endpoint, or future that it actually moves
across its asynchronous boundary.

## Actions commitment

For one successful `Actions` value, the adapter interprets:

1. creations in vector order;
2. each creation's initialization effects before committing that child;
3. every named sends lane in its structural `InterpretSends` order; and
4. the next-behavior or termination verdict.

All creation attempts precede sends and interpreter requests from the same
action. This is a Bombay policy that makes create-then-send and
create-then-observe deterministic. It is not a general actor-model ordering
guarantee between independent actors.

Within `SendLayer`, inner effects are interpreted before wrapper-owned effects.
Within a vector lane, values retain vector order. The algebra deliberately
does not invent an order between independent product lanes beyond the product's
own `InterpretSends` implementation.

If interpretation fails, later effects are not consumed. The adapter reports
its concrete error; it may not reinterpret failure as actor termination,
successful creation, restart, or observation.

## Fresh creation

`Create<A, C>` contains a creator-local nonce, concrete child, and
`CreationKind`. The adapter must not derive an address from the nonce.

For each request it must:

- reject an already-bound nonce without disturbing the existing binding;
- allocate an address fresh with respect to the actor configuration;
- initialize the concrete child and interpret its initialization actions;
- install a runtime endpoint for `C::Protocol`;
- atomically commit the creator-local occurrence/nonce binding; and
- publish `EstablishedCreation<C::Protocol, Occurrence>::Installed` only after
  all preceding steps succeed.

Failure publishes the matching `CreationRejection` and no capability. An
allocation collision is retryable or rejected according to runtime policy; it
can never mean replacement. `CreationKind::ReplacementIncarnation` preserves
provenance but still requires fresh allocation.

`DispatchBirth` recursively selects the concrete child. One
`InstallBirth<Position, C, ...>` implementation is required per structural
occurrence. `Position` is interpreter navigation evidence, not hosting
identity.

## Destination interpretation

The three delivery paths have different obligations:

| Request | Adapter action |
|---|---|
| `Delivery<P>` | resolve `Recipient<P>::address()` under the runtime's logical-name policy, then deliver `P::Msg` |
| `ChildDelivery<P, O>` | resolve the current creator's committed `(O, nonce)` binding, then deliver |
| `EstablishedDelivery<P>` | transfer the exact endpoint and deliver directly |

The adapter must preserve `P` statically throughout. It may not coalesce two
protocols merely because their addresses or message layouts match.

`EndpointAddress` lets the runtime's own address newtype select the endpoint
family. `InterpretEstablished`, `InterpretEstablishedDelivery`, and the exact
observation/shutdown interpreter traits are public power-user boundaries. They
do not grant endpoint access outside the explicit transfer call, but they are
not sealed as exclusive runtime authority.

## Event injection

Interpreter-originated facts return through their declared
`ReturnsToEmitter<Input, Path>`. The adapter retains the corresponding
`Ingress<Input, Path>` or equivalent static constructor and enqueues exactly
the root `B::Event` it produces.

It must not inspect payload types to find a lane. Repeated fact types at
different wrapper depths are distinct because their structural paths are
distinct.

## Observation

Exact observation uses `ObservationId` as observer-local relationship
correlation and the supplied endpoint as incarnation identity. The adapter
must return a complete `EstablishedObservation<P>` fact for start, cancel,
rejection, or eventual stop. Duplicate live IDs and cancellation of a missing
relationship are explicit rejections.

Address-based `ObservePeer<A>` remains separate. A missing live address is not
proof that the requested incarnation stopped. Without a selected live
incarnation or authoritative retained terminal fact, the adapter returns its
own error rather than fabricating `PeerStopped`.

## Orderly shutdown

`ShutdownEstablished<B, Path>` supplies an exact endpoint and a typed
`Ingress<ShutdownRequested, Path>`. The adapter injects that event through the
normal mailbox boundary. It returns `EstablishedShutdownResolved<P>::Accepted`
only when the request is admitted, or the precise rejection otherwise.

Acceptance is not termination. Eventual termination is reported through the
observation relationship.

## Environment and liveness

Clock reads, timer queues, bounded-mailbox waiting, task scheduling, endpoint
storage, networking, persistence, and operating-system failures are adapter
concerns. They must enter or leave the fold only through declared typed events,
effects, and errors.

Backpressure may delay a concrete interpretation future. The adapter must not
silently drop or reorder accepted effects. Fairness, scheduling policy, and
resource limits are runtime policy and should be documented by the runtime,
not presented as laws of this crate.

## Conformance checklist

- initialization once and before mailbox events;
- one event at a time and one fold per accepted event;
- creations before dependent sends/requests;
- fresh allocation independent of nonce;
- rejected creation commits no binding or endpoint;
- static interpretation for every effect and child occurrence;
- exact endpoints bypass logical-name resolution;
- all returning facts use their declared structural ingress;
- no erased envelopes, registries, downcasts, or hidden side effects; and
- no success, restart, stop, or observation fact is inferred from failed
  mechanics.
