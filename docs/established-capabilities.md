# Established capabilities

This document defines the production contract for runtime-issued exact actor
capabilities. It does not claim that a downstream runtime has already adopted
the contract.

## Semantic classification

Actor-model laws:

- actors may communicate only through acquaintances they possess;
- actor names may be communicated; and
- creation allocates a fresh actor name.

Derived Bombay constructions:

- protocol-indexed endpoint families selected by a runtime-owned address type;
- opaque inert recipient capabilities;
- structural child occurrences and named effect products; and
- distinct protocol and concrete-behavior capability strengths.

Deliberate Bombay policies:

- a behavior stages creation with a `CreationId` that is non-reused within one
  statically declared child occurrence before allocation;
- all creations in one `Actions` value commit before dependent sends or
  interpreter requests from that value;
- successful creation reports an exact installed endpoint;
- observation and orderly shutdown have explicit correlation identities and
  complete accepted/rejected fact algebras; and
- public interpretation traits are power-user authority boundaries.

## Capability ladder

```text
logical name                     exact installed incarnation

Recipient<P>                    EstablishedRecipient<P>
    │ address lookup                 │ endpoint transferred directly
    ▼                                ▼
Delivery<P>                     EstablishedDelivery<P>

CreateChild<A, C> --establish--> EstablishedCreation<P, O>
          │                            │ protocol occurrence
          │                            ▼
          └───────────────────> EstablishedActor<C>
```

`Recipient<P>` is a typed logical name. It remains necessary where a stable
name, external address, discovery result, or transport address is the intended
semantics. Resolving it to a live endpoint is runtime work.

`EstablishedRecipient<P>` is an inert capability for one exact installed
incarnation. Its endpoint representation is
`<P::Addr as EndpointAddress>::Established<P>`. It exposes no endpoint
accessor and no direct send method. Transfer occurs through the public
`InterpretEstablished<P>` boundary, so this is a power-user API boundary, not
exclusive runtime authority.

`EstablishedActor<B>` additionally proves which concrete behavior was
installed. Most consumers should keep only `EstablishedRecipient<B::Protocol>`.
The stronger proof is appropriate when an operation depends on `B::Event`, as
orderly shutdown does.

## Why the address owns the endpoint family

`EndpointAddress` is implemented once by a runtime's address newtype:

```rust,ignore
impl EndpointAddress for RuntimeAddr {
    type Established<P> = RuntimeEndpoint<P>
    where
        P: Protocol<Addr = Self>;
}
```

This preserves static dispatch without imposing an associated endpoint or key
on every domain protocol. Each concrete `P`, including a generic
instantiation, produces a distinct `EstablishedRecipient<P>` capability type.
The runtime may reuse the same underlying endpoint representation across
projections without allowing those capability types to mix. There is no
hashing, `TypeId`, dynamic dispatch, erased storage, or manually authored
protocol key.

The associated endpoint must be `Clone`, because an established recipient is
transferable acquaintance evidence. It is deliberately not unconditionally
`Send`. Local interpreters may use closure-owned or otherwise thread-local
endpoints. `InterpretSends` requires the complete concrete delivery or request
to be `Send` when an asynchronous interpreter moves it, which naturally
requires both the endpoint and `P::Msg` to be sendable on that path. This keeps
executor policy out of inert capability construction and avoids propagating a
false `P::Msg: Send` obligation through local creation, observation, and
shutdown facts.

`MailAddr` intentionally implements only `Address`. It is a neutral logical
address used by pure examples and tests, not a commitment to a runtime endpoint
representation.

## Creation transaction

`CreateChild<A, C>` owns one creator-issued `CreationId`, concrete child `C`,
and explicit birth or replacement intent. The runtime interprets the ordered
creation batch and each routed child as a transaction:

```text
prepare one route for every creation without partial consumption
    -> allocate fresh address
    -> initialize child
    -> interpret initialization actions
    -> install exact endpoint
    -> commit creator-local (protocol occurrence, CreationId) binding
    -> publish Installed result
```

Route preparation rejection returns the complete untouched batch with
`ChildNamespaceExhausted`. After routing commits, any failed child step publishes
a rejected creation result and commits no binding. The owning
`ChildCreationOutcome` settlement separately retains the current child and exact
initialization value. The child rejection sum is:

- `Allocation(Exhausted | AddressAlreadyClaimed)`;
- `InitializationFailed`; or
- `EnvironmentFailed`.

The allocator's fresh address is independent of `CreationId`. `Address` has no
derivation operation. A `CreationSequence` retained by one child occurrence
makes reuse within that occurrence unrepresentable. Independent occurrences may
issue equal numeric IDs because the occurrence is part of every binding. Stable
identity and replacement are higher-level constructions; replacement never
means overwriting an address.

A static runtime can derive its creator-local binding product with
`ChildOccurrenceProduct` and a runtime-owned `ChildOccurrenceShape`. The sealed
product retains each direct behavior leaf together with its existing structural
occurrence; the runtime decides what endpoint and nonce storage that leaf
contains. It adds no runtime value or capability to `Behavior`, and it does not flatten
descendant actors into the current creator's namespace.

Effects retain their nominal occurrence rather than the running wrapper type.
`ResolveChildOccurrence<O>` maps that occurrence back to the exact direct child
and position for the concrete emitter. Generated roles pass transparently
through wrappers only when `Behavior::Protocol` and `Behavior::Birth` are both
unchanged. Raw structural positions always refer to the emitter's current
birth node, including a topology-changing wrapper's proxy child. No endpoint,
binding, or runtime lookup is performed by this type-level resolution.

`EstablishedCreation<P, O>` deliberately excludes the parent behavior and
concrete child behavior from its identity. Ordinary consumers need only the
canonical protocol and occurrence. `into_actor::<Parent>()` reintroduces the
concrete child proof only where `O: ChildRole<Parent>` establishes it.

This separation is important for generic composition: a wrapper, destination,
payload, or application state type is never required to implement `Behavior`
merely to satisfy an indexing trick.

## Same-action local delivery

`ChildDelivery<P, O>` carries only the child protocol, structural occurrence,
nonce, and message. The active parent instance supplies the creator namespace
as interpreter context. The interpreter resolves its committed local binding;
it does not derive an address or consult an application-wide protocol table.

If the corresponding creation was rejected or no binding exists, delivery
must fail through the interpreter's typed error. It may not be silently
dropped, redirected, or resolved to an older incarnation.

## Exact observation

`ObserveEstablishedCreation<P, O>` requests the committed result of a staged
creation. It returns `EstablishedCreation<P, O>` to the emitting behavior.

`ObserveEstablished<P>` starts observation of an exact endpoint under a fresh
observer-local `ObservationId`. `CancelObservation<P>` cancels that exact
relationship. `EstablishedObservation<P>` exhaustively reports:

- `Started`;
- `Cancelled`;
- `Rejected { operation, reason }`; or
- `Stopped { outcome, at }`.

Duplicate IDs and cancellation of an absent relationship are typed
rejections. Cancellation does not retract a stop fact already admitted to the
mailbox.

Legacy address-based `ObservePeer<A>` remains a different operation. It asks a
runtime to select an incarnation by logical address and therefore still needs
name-resolution policy. It must not be described as equivalent to exact
capability observation. Its total action settlement accepts with unit after the
relationship is established, or returns the complete request with
`PeerObservationRejection::UnknownAddress` when neither a live incarnation nor
authoritative retained termination can be selected. It has no prerequisite;
missing interpreter support is corruption rather than unknown-address
rejection.

## Exact orderly shutdown

`ShutdownEstablished<B, TargetPath>` combines:

- `EstablishedActor<B>`;
- an observer-local `ShutdownId`; and
- `Ingress<ShutdownRequested, TargetPath>` proving where shutdown enters
  `B::Event`.

The interpreter receives the exact endpoint and typed ingress. The request
therefore remains an explicit event/effect transformation, not an ambient
mailbox or runtime shutdown side channel. Immediate resolution is either
`Accepted` or a typed `AlreadyStopping`/`AlreadyStopped` rejection. Later
termination remains a separate observation fact.

`ShutdownEstablished` is an `ActionItem` under the repository's single total
settlement law. Acceptance consumes the request and leaves its `ShutdownId`.
Either lawful rejection returns the complete original request and exact actor
capability. `InterpretEstablishedShutdown` can return only
`Result<(), ShutdownRejection>`; it no longer selects an arbitrary output type.
The request retains its original capability until acceptance and supplies a
clone to the concrete admission port so rejection remains ownership-complete.

## Runtime obligations

A conforming interpreter needs only capabilities justified by the values it
drives:

- one concrete endpoint family for its address namespace;
- fresh allocation independent of occurrence-local creation IDs;
- creator-instance protocol-occurrence/`CreationId` bindings for local child
  effects, which may be derived statically from the sealed direct-child
  occurrence product;
- exact endpoint delivery, observation, and typed shutdown interpretation;
- logical-address resolution only for retained `Recipient<P>` paths; and
- creation-before-dependent-effects ordering.

This contract can remove application-wide per-protocol actor spaces from exact
internal paths. It does not make logical address resolution disappear where
`Recipient<P>` is intentionally used.

## Composition invariants

- `P` remains the only protocol identity.
- Structural occurrences are navigation evidence only.
- Wrapping an emitter does not reindex an established or child destination.
- Rejected creation carries no endpoint capability.
- Exact capabilities are transferred only through `Actions` and explicit
  interpretation boundaries; they perform no ambient effect themselves.
- Creation, observation, and shutdown correlation IDs are not actor identity.
- No wrapper or domain generic is forced to implement `Behavior` unless an
  operation genuinely requires its concrete event or birth algebra.
