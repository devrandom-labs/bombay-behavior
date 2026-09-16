# Bombay Behavior core type inventory

## Scope

This document inventories the types exported at the root of the
`bombay-behavior` crate (`behavior` in Rust source). The authority for the list
is `crates/behavior/src/lib.rs`; semantic descriptions are taken from the
defining modules in `crates/behavior/src`. It describes the current source
surface, not a proposed redesign.

The inventory includes public traits, structs, enums, and type aliases. It
also identifies the crate's public functions and `#[behavior]` macro because
they construct or consume the types. It excludes private module items,
test-only fixtures, and downstream types generated in a user's crate except
for a separate description of the macro-generated family.

Some root exports are marked `#[doc(hidden)]`. They remain listed because they
are publicly nameable implementation contracts, but ordinary behavior authors
should not treat them as the preferred API.

## Semantic center

The central relationship is:

```text
Protocol = address namespace × public message type

Behavior = protocol
         × complete event algebra
         × send product
         × birth algebra
         × phase menu
         × controlled error

BehaviorActed<B>
    = Result<
          Actions<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
          B::Error,
      >

Actions = sends
        × ordered staged fresh creations
        × (Continue | Goto(phase) | Stop(Stopped))
```

`Protocol` is stable public communication identity. `Behavior` is the concrete
stateful fold implementing that protocol. They are deliberately separate:
recipients and deliveries do not recursively carry a destination behavior's
sends, births, phases, or errors.

`Actions` is Bombay's typed realization of actor-transition effects. It is not
described as a literal Agha effect triple: the Rust surface additionally owns
typed send products, creator-local child routing, phases, controlled errors,
termination, initialization effects, and Bombay interpretation ordering.

## Protocol, behavior, and transition types

| Type | Kind | Semantic role |
|---|---|---|
| `Protocol` | Trait | Associates one stable public actor identity with `Addr: Address` and `Msg`. It is independent of transition implementation. |
| `MessageProtocol<A, M>` | Zero-state struct | Reusable nominal-free protocol signature for address type `A` and message type `M`. |
| `Behavior` | Trait | Pure initialized fold from one complete typed event to `BehaviorActed<Self>`. Associated types expose protocol, event, sends, phases, controlled error, and birth capability. |
| `BehaviorActed<B>` | Type alias | Exact controlled result type of behavior `B`: `Acted<BehaviorAddr<B>, B::Ph, B::Sends, B::Birth, B::Error>`. |
| `BehaviorAddr<B>` | Type alias | Projects the address namespace from `B::Protocol`. |
| `BehaviorMessage<B>` | Type alias | Projects the public message type from `B::Protocol`. |
| `InitializationTurn` | Non-constructible struct | Lifecycle-issued authority to invoke `Behavior::init` exactly at the initialization boundary. |
| `ActiveTurn` | Non-constructible struct | Lifecycle- or composition-issued authority to invoke one active transition. |
| `BehaviorLayer<B>` | Trait | Statically constructs one fully concrete behavior from another; closures implement it without trait objects or effects. |
| `BehaviorBase` | Trait | Projects a composed wrapper to its authored base behavior without exposing wrapper depth. |
| `LogicalHostRequirements` | Trait | Derives the ordered, duplicate-preserving logical protocol hosts required by a behavior's sends and transitive births. It is static evidence, not allocation. |

The associated type equation of `Behavior` is:

```text
Behavior {
    Protocol: Protocol
    Event: UserEvent<Addr = BehaviorAddr<Self>,
                     Message = BehaviorMessage<Self>>
    Sends: SendEffects + SendsFor<Event>
    Ph
    Error
    Birth: BirthMode
}
```

## Next-state and action types

| Type | Kind | Semantic role |
|---|---|---|
| `Never` | Uninhabited enum | Proves absence. It is used for no phase transitions, impossible events, and empty closed sums. |
| `Stopped` | Unit struct | Payload-free normal behavior-termination marker. Lifecycle provenance belongs in typed protocols, not this seat. |
| `Step<Ph, R>` | Enum | Exhaustive next verdict: `Continue`, `Goto(Ph)`, or `Stop(R)`. |
| `Become<Ph>` | Type alias | Behavior next verdict, fixed to `Step<Ph, Stopped>`. |
| `Actions<A, Ph, Sends, Birth>` | Struct | Explicit transition product with named `sends`, ordered `creates`, and `become_` legs. The interpreter commits creations before same-action sends that may depend on them. |
| `Acted<A, Ph, Sends, Birth, E>` | Type alias | `Result<Actions<A, Ph, Sends, Birth>, E>`. |
| `AppendSend<Input, Path>` | Trait | Appends one input to a statically selected send lane while preserving creation and next-state legs. |

`Actions` constructors (`cont`, `stop`, `goto`, `send`, and `create`) construct
values only. They do not interpret sends, allocate actors, change runtime
state, or perform lifecycle work.

## Addressing and installed-capability types

| Type | Kind | Semantic role |
|---|---|---|
| `Address` | Trait | Defines a pure logical address namespace and its creator-local `Nonce`. A nonce is correlation, not address or freshness proof. |
| `MailAddr` | Newtype struct | Built-in `u64` logical address whose nonce is also `u64`. |
| `EndpointAddress` | Trait | Runtime-owned projection from a logical address namespace and protocol to an exact endpoint representation. |
| `Recipient<P>` | Struct | Pure logical destination for protocol `P`; proves protocol/address/message agreement but not installation. |
| `EstablishedRecipient<P>` | Struct | Inert runtime-issued capability for one exact installed incarnation of protocol `P`. Its endpoint is not directly exposed. |
| `InterpretEstablished<P>` | Trait | Explicit power-user boundary that consumes an established recipient's exact endpoint. |
| `EstablishedActor<B>` | Struct | Exact installed capability that preserves both the public recipient and the concrete installed behavior type `B`. |
| `Delivery<P>` | Struct | Pure logical communication containing `Recipient<P>` and `P::Msg`. |
| `EstablishedDelivery<P>` | Struct | Pure communication to one exact `EstablishedRecipient<P>` without logical-address resolution. |

The capability strength increases from logical naming to installed evidence:

```text
Recipient<P>
    logical protocol destination

EstablishedRecipient<P>
    exact installed endpoint for P

EstablishedActor<B>
    exact installed endpoint for B::Protocol
    + static proof of concrete behavior B
```

None of these values has an ambient send, shutdown, allocation, or lookup
method. Effects cross explicit interpreter traits.

## Event and ingress types

| Type | Kind | Semantic role |
|---|---|---|
| `User<A, M>` | Struct | Base public user-message event with `from: A` and `message: M`. |
| `UserEvent` | Trait | Constructs and extracts the public `User` lane through a complete composed event algebra. |
| `EventLayer<Owned, Inner>` | Enum | Concrete coproduct `Owned(Owned) | Inner(Inner)` for adding one typed event lane around another. |
| `ComposedEvent` | Trait | Identifies an event algebra's inner event and its structure-preserving injection. |
| `EventIngress<Source, Input>` | Trait | Owner-selected construction of one input lane without caller-visible wrapper paths. |
| `ChildInputIngress<Source, Input>` | Trait | Constructs a private parent-to-child input in the concrete child's event algebra. |
| `InjectEvent<Input, Path>` | Trait | Low-level path-indexed injection into a structural event coproduct. |
| `Ingress<Input, Path>` | Zero-state struct | Address-free capability selecting one exact interpreter-return ingress member. |
| `Here` | Unit struct | Compile-time path selecting the current event or send layer. |
| `Inside<Path>` | Zero-state struct | Compile-time path selecting an inner layer. |

`Here` and `Inside<Path>` are structural proof types, not runtime routing
addresses. Ordinary composed templates should prefer semantic
`EventIngress<Source, Input>` implementations so callers do not count wrapper
depth.

## Send products and interpretation types

### Send-product algebra

| Type | Kind | Semantic role |
|---|---|---|
| `SendEffects` | Trait | Closed value algebra with `empty`, ordered `append`, and statically selected lane emission. |
| `SendsFor<Event>` | Marker trait | Proves that a send product's returning interpreter requests are lawful for the exact complete event algebra. |
| `SendInput<Input, Path>` | Trait | Selects one request lane at compile time and emits into it. |
| `Own` | Uninhabited marker enum | Selects a named send product's own semantic lane. |
| `NoSends` | Unit struct | Named empty send product. |
| `SendLayer<Owned, Inner>` | Struct | Named product of wrapper-owned and inner send effects. Interpretation preserves inner-to-outer authored order. |
| `LogicalDeliveryProtocols` | Trait | Projects intentional logical `Delivery<P>` destinations from a send product while preserving order and duplicates. |
| `InterpreterRequest` | Trait | Declares whether a runtime-local request later returns a typed event to its emitter. |
| `InterpreterRequests<M>` | Struct | Ordered send lane of runtime-local requests with no actor address. |
| `NoReturnToEmitter` | Uninhabited enum | Declares that an interpreter request produces no later local event. |
| `ReturnsToEmitter<Input, Path>` | Zero-state struct | Declares a later local event of `Input` at a compile-time event path. |
| `ReportToParent<R>` | Struct | Transfers an owned report through the established creator/child relationship; the interpreter attaches the exact occurrence-local creation ID. |

The crate supplies `SendEffects`/interpretation implementations for selected
`Vec<T>` lanes, including logical deliveries, established deliveries,
creator-local child deliveries and inputs, and `Vec<Never>`. A general
`Vec<T>` is a send value container, but only statically supported effect kinds
receive interpreter meaning.

### Interpreter capabilities

| Type | Kind | Semantic role |
|---|---|---|
| `ActionItem` | Trait | Fixes one action item's accepted receipt, rejection, and prerequisite types for every runtime. |
| `ItemSettlement<Item, Accepted, Rejection, Prerequisite>` | Enum | Conserves one attempted item across acceptance, rejection, prerequisite blocking, and interpreter corruption. |
| `SettledItem<Item, Settlement>` | Enum | Distinguishes an attempted item from an untouched item after earlier corruption. |
| `Interpretation<Settlement>` | Enum | Owns the complete product after successful traversal or corruption. |
| `SendSettlements` | Trait | Projects one concrete sends product to its runtime-independent settlement product. |
| `ActionSettlements` | Trait | Projects one concrete `Actions` product to its complete creation-and-send settlement. |
| `BehaviorSettlements` | Trait | Blanket projection from a concrete behavior to that exact action-settlement type without restating internal birth or send bounds. |
| `InterpretItem<Item, RootEvent, Path>` | Trait | Lets one concrete runtime attempt exactly one statically selected action item. |
| `InterpretSends<Interpreter, RootEvent, Path>` | Trait | Exhaustively interprets one complete send product in structural order. |
| `SourceSettlementCustody<Host, RootEvent>` | Trait | Offers at most one emitter-return settlement input in declared order while retaining the exact residual product. |
| `SourceCustody<Residual>` | Enum | Distinguishes an exhausted product, exactly one admitted input, and closed admission with complete residual ownership. |

These are statically dispatched interpreter obligations. A composite send
product cannot silently omit a lane: the concrete interpreter must implement
every required capability or fail to compile. Source custody uses the same
declared order, but returns after one successful admission so the runtime can
process that input and all transitive effects before offering the next one.

## Staged creation and lifecycle result types

### Creation values and outcomes

| Type | Kind | Semantic role |
|---|---|---|
| `CreationId` | Newtype struct | Opaque correlation within one statically declared child occurrence; it is not an address, route, identity, or provenance. |
| `CreationSequence` | Struct | Checked source of non-reused IDs. Retaining one per occurrence is sufficient; one owner may share a sequence across several occurrences for a stronger guarantee. |
| `CreationKind` | Enum | Behavior-owned intent: ordinary `Birth` or fresh `Replacement { previous }`. |
| `CreateChild<A, New>` | Struct | Pure staged request containing a creation ID, owned child behavior, and creation intent. |
| `Creations<Item>` | Struct | One ordered creation batch. Runtime route preparation accepts the whole batch or returns it untouched. |
| `RoutedCreation<A, New>` | Hidden struct | Interpreter-private pairing of a complete creation with its selected runtime route. |
| `AllocationRejection` | Enum | Typed fresh-address failures: exhaustion or already-claimed proposed address. |
| `ChildNamespaceExhausted` | Struct | The interpreter cannot route the complete declared creation batch without partial route consumption. |
| `CreationRejection` | Enum | Complete rejected-child reasons after routing: allocation, initialization, or environment/commit failure. |
| `EstablishedCreation<P, Occurrence>` | Enum | Exact `Installed` or `Rejected` result for one child-protocol occurrence, retaining its creation ID and intent. |
| `ObserveCreation<P, Occurrence>` | Struct | Same-action request for the exact protocol/occurrence creation result; returns `CreationResolved<P::Addr>` and depends on `CreationCorrelation<P, Occurrence>`. |
| `ChildDelivery<P, Occurrence>` | Struct | Same-action public-protocol delivery to a declared creator-local child occurrence. |
| `ChildInput<Child, Source, Input, Occurrence>` | Struct | Private typed input to a concrete declared child event lane. |
| `ChildReport<R>` | Struct | Parent event payload containing the interpreter-attached creation ID and owned report. |

Creation is staged, not performed by `CreateChild`. Behavior issues a
`CreationId` and emits the child in the same transition. Exact correlation is
the static child occurrence together with that ID. Independent layers may use
equal numeric IDs for distinct occurrences; reuse within one occurrence remains
invalid. The interpreter privately selects routes for the complete `Creations`
batch before it hosts any child. A creation becomes established only after fresh
allocation, initialization, installation, commit, and binding. Behavior never
constructs or stores a runtime route.

### Heterogeneous creation products

| Type | Kind | Semantic role |
|---|---|---|
| `Children<A, Product>` | Struct | Builder for a pure ordered heterogeneous product of staged direct-child creations. |
| `NoChildren` | Unit struct | Empty heterogeneous creation product. |
| `ChildCons<A, C, Earlier>` | Struct | One creation appended to an earlier heterogeneous product. |
| `ChildProduct<A>` | Trait | Sealed recursive conversion from `Children`'s product into one ordered `Creations` batch over a closed child choice. |

Each `Children::child` or `Children::create` call adds one new structural child
occurrence, so conversion into `Creations` is total. Equal IDs in distinct
occurrences remain distinct correlations. Runtime route exhaustion returns the
entire batch through `ChildNamespaceExhausted`.

## Closed birth and topology type algebra

### Birth capability and child sums

| Type | Kind | Semantic role |
|---|---|---|
| `BirthMode` | Trait | Associates a behavior with its closed child type algebra. |
| `NoBirths` | Unit struct | Birth mode whose child algebra is `Never`. |
| `Births<C>` | Zero-state struct | Birth mode admitting closed child algebra `C`. |
| `ChildChoice<Head, Tail>` | Enum | Closed recursive heterogeneous sum of concrete child behaviors. |
| `ChildHead` | Unit struct | Structural position selecting a sum's head. |
| `ChildTail<Position>` | Zero-state struct | Structural position selecting inside a sum's tail. |
| `ChildPosition<Children, Child>` | Sealed proof trait | Proves that an exact child behavior occupies an exact structural position. |
| `BirthNodeAppend<Tail>` | Sealed composition trait | Appends one closed direct-child algebra after another while preserving existing positions and creation order. |
| `BirthNodeAt<Position>` | Hidden sealed trait | Inverse projection from a structural position to its child type. |

`ChildChoice` is a creation-only sum, not a message envelope, behavior trait
object, registry, or runtime dispatcher.

### Nominal child roles and occurrence resolution

| Type | Kind | Semantic role |
|---|---|---|
| `ChildRole<Parent>` | Trait | Authored proof that one nominal role names one exact direct child and structural position of `Parent`. |
| `ChildOccurrence<Parent>` | Trait | Declares the sealed descriptor used to resolve one nominal or raw structural occurrence. |
| `DeclaredChildOccurrence` | Hidden unit struct | Descriptor for an authored nominal child role. |
| `StructuralChildOccurrence<Position>` | Hidden zero-state struct | Descriptor for a raw structural child position. |
| `ChildOccurrenceResolution<Parent, Occurrence>` | Hidden sealed trait | Restricts which descriptor may resolve a given occurrence. |
| `ResolveChildOccurrence<Occurrence>` | Sealed trait | Resolves an occurrence against the concrete emitter, following topology-transparent `BehaviorBase` wrappers lawfully. |
| `ResolvedChild<Emitter, Occurrence>` | Type alias | Projects the exact resolved child behavior. |
| `ResolvedChildPosition<Emitter, Occurrence>` | Type alias | Projects the exact resolved structural birth position. |
| `RoleChild<Parent, Role>` | Type alias | Projects the child behavior selected by one nominal role. |
| `RoleProtocol<Parent, Role>` | Type alias | Projects the canonical protocol of a role-selected child. |

Nominal roles distinguish duplicate occurrences even when they contain the
same child behavior. A role and its structural position are topology evidence,
not protocol identity, actor identity, or runtime lookup keys.

### Interpreter dispatch and child occurrence products

| Type | Kind | Semantic role |
|---|---|---|
| `ChildCreationOutcome<C, Occurrence>` | Enum | Created child, initialization rejection retaining child/error, or host rejection retaining child/uninterpreted initialization actions/reason. |
| `EstablishChild<Occurrence, C>` | Trait | Concrete interpreter ownership port returning the fixed `ChildCreationOutcome` result for `C` at one exact occurrence. |
| `ChildCreationProduct<A, Occurrence>` | Hidden trait | Runtime-independent result product for a closed creation-only child sum. |
| `DispatchBirth<A, Host>` | Trait | Exhaustive static dispatch over one closed creation-only child sum. |
| `ChildOccurrenceShape` | Trait | Downstream type constructor defining empty and per-child representations for a direct-child occurrence product. |
| `ChildOccurrenceProduct<Shape>` | Sealed trait | Selects a shape-owned static representation for a closed direct-child algebra. |
| `ChildOccurrences<Children, Shape>` | Type alias | Occurrence-preserving representation selected by one child shape. |

These types let an interpreter prove support for every child alternative at
compile time. They do not perform runtime protocol lookup or erase the child
behavior type.

### Protocol and host projections

| Type | Kind | Semantic role |
|---|---|---|
| `NoBirthProtocols` | Unit struct | Empty projected protocol product. |
| `BirthProtocol<P, Tail>` | Zero-state struct | One protocol occurrence followed by a remaining projected product. |
| `BirthProtocolHead` | Unit struct | Structural position selecting the current projected protocol. |
| `BirthProtocolTail<Position>` | Zero-state struct | Structural position selecting inside the remaining protocol projection. |
| `BirthProtocolAt<P, Position>` | Marker trait | Static membership evidence for one protocol occurrence. |
| `BirthProtocolProduct` | Hidden trait | Closed append operation over protocol products. |
| `BirthProtocols` | Trait | Projects a behavior's own protocol and every protocol reachable through transitive births. |
| `BirthNodeProtocols` | Hidden trait | Recursively projects protocols from one closed birth node. |
| `BirthNodeLogicalHosts` | Hidden trait | Recursively projects logical host requirements from one closed birth node. |

Protocol products preserve repeated occurrences. They are static evidence and
perform no allocation, hosting, normalization, or runtime lookup.

## Finite sequence observation

The Behavior crate exposes no mailbox reducer or finite-stream result. Its
contract ends at one initialization or event transition and the resulting
`Actions`. Runtime scheduling belongs to Bombay. Repository tests use
`behavior_testkit::drive`, whose `Trace` preserves the final active behavior,
ordered sends and creations, initialization-inclusive transition count,
pending mailbox suffix, and an exhaustive `DriveDisposition` distinguishing a
drained mailbox from `BehaviorStopped(Stopped)`.

## Public companion operations

| Item | Kind | Role |
|---|---|---|
| `initialize` | Hidden function | Canonical wrapper boundary invoking one inner initialization fold and returning its complete actions unchanged. |
| `delegate_transition` | Hidden function | Canonical wrapper boundary invoking one inner event fold and returning its complete actions unchanged. |
| `behavior` | Attribute macro | Generates the mechanical `Protocol`, `Behavior`, `BehaviorBase`, closed send product, and closed birth-role wiring for an inherent impl. |

For an actor named `Actor`, the macro may generate these concrete types when
the corresponding declarations exist:

```text
ActorSends
ActorSends<Field>                 one uninhabited selector per send lane
ActorActions                     fluent action-extension trait
ActorChildren                    closed ChildChoice alias
ActorChildren<Field>             one nominal role type per child declaration
ActorChild                       namespace containing role values
```

The exact generated Rust names preserve the actor and authored field names.
The macro does not create actors, interpret effects, introduce a dynamic
envelope, or grant capabilities absent from the handwritten core types.

## Boundary summary

The crate intentionally stops at the pure algebra and minimal typed
interpreter contracts:

- `Behavior` consumes one event and returns `Actions`.
- `Actions` owns sends, staged fresh creations, and next behavior or stop.
- event, send, and birth products are concrete closed sums/products;
- recipients distinguish logical identity from exact installed capability;
- creation IDs remain creator-local correlation rather than actor identity;
- interpreter traits realize typed effects without dynamic dispatch; and
- reducers observe folds without becoming a scheduler or runtime.

Scheduling, mailbox transport, clocks, endpoint allocation, installation,
effect settlement, and actor execution remain interpreter responsibilities.
