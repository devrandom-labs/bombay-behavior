# Public surface inventory

This inventory supports [A13](repository-quality-audit.md). `#[doc(hidden)]`
changes Rustdoc display, not Rust visibility. The counts below are annotation
sites in source, so a grouped re-export and its original declaration each
count once. The current branch has 10 sites in `crates/behavior/src` and 50
in `crates/actors/src`; the earlier audit counted 25 and 89 before the
documentation visibility review.

## Hidden core declarations

| Owner | Annotated declarations | Contract owner | Visibility decision to review |
|---|---|---|---|
| `actor/creation.rs`, occurrence proof | `StructuralChildOccurrence`, `ChildCreationProduct`, `ChildOccurrenceResolution`, `ResolveChildOccurrenceDescriptor`, `BirthNodeAt`, `ChildOccurrenceProductAt` | Generated code obligation | The macro and structural child products implement these proofs. A visibility change needs compile-pass and forged-occurrence compile-fail witnesses. |
| `actor/creation.rs`, protocol projection | `BirthModeProtocols`, `BirthNodeProtocols`, `BirthNodeLogicalHosts` | Generated code obligation | These traits project closed birth and logical-host products; consumers can name the resulting associated types without constructing the proof nodes. |
| `actor/creation.rs`, creation staging | `ChildProduct::stage` | Sealed structural conversion | `Children::into_creates` calls this method to turn the closed heterogeneous product into an ordered `Creations` batch. Only `NoChildren` and `ChildCons` implement the sealed trait. The interpreter receives the resulting batch through the creation effect; it does not call `stage`. |

`ChildOccurrence::Resolution` and `DeclaredChildOccurrence` are now visible
because manually authored roles must name them. `BirthProtocolProduct` is
visible because generic logical-host owners constrain and append that public
product. Rustdoc lists both names and the caller suites exercise them. The
private `Recipient::new` no longer carries an ineffective documentation marker.
`CreationSettlement`, `CreationSettlements`, and `CreationsSettled` are now
visible because external callers name them when retaining or returning exact
child-creation custody. `InterpretCreations` is also visible: the real Bombay
runtime names it in the bound for its action interpreter. This is a source
witness for required port visibility, not yet an A17 integration verdict.
`CreateChild::into_parts` is visible because external creation-interpreter
tests and the inspected Bombay child host consume it to retain exact owned
creation parts across acceptance or rejection. The method did not gain a new
capability; its Rustdoc now describes the existing custody transfer.
`CreationCorrelation<P, Occurrence>` is visible because external `ActionItem`
implementations name it as an exact, non-authoritative creation prerequisite.
Its occurrence parameter and private fields keep equal numeric IDs at
different child positions distinct.
`CreationId::get` is also visible because the actor catalogue derives
shutdown and operation correlations from it, and external interpreter tests
inspect those exact values. Its Rustdoc now states that the numeric projection
is neither actor identity nor proof of a committed fresh child.

## Hidden actor declarations

| Owner | Annotated declarations | Contract owner | Visibility decision to review |
|---|---|---|---|
| `atomic/{dynamic_supervisor,fifo_pool,fixed_supervisor,keyed_pool}/{event,requests}.rs` | `DynamicSupervisorEvent`, `DynamicSupervisorRequests`, `FifoEvent`, `FifoRequests`, `FixedSupervisorEvent`, `FixedSupervisorRequests`, `KeyedEvent`, `KeyedRequests` | Runtime port | Public associated event and send products let an interpreter carry complete typed lanes; the aggregate owns their transition semantics. |
| `atomic/fixed_supervisor/lifecycle.rs` | `FixedLifecycleRoute` | Generated code obligation | This sealed route proof is implemented for the finite fixed-supervision lifecycle forms. |
| `atomic/pool/mod.rs` | `CompletesAssignments` | Generated code obligation | The sealed completion capability belongs to generated pool workers and declared completion products. |
| `atomic/stable_proxy/{effects,protocol}.rs` and `atomic/mod.rs` | `ProxyEffects` and its re-export; `ProxyEvent` as a concrete associated event type | Structural event/effect products | The host and typed proxy effects must keep rejection custody and every ordered lane; application callers use the typed `Behavior` projections and event ingress. `ProxyDrain`, `WorkerStartResult`, and `ProxyOperation::creation` are now visible because callers match the concrete outcomes or inspect the exact creation correlation. |
| `atomic/worker/activation.rs` | `ActivationStartRejection`, `BeginActivation`, `BeginActivation::{new,target,worker,initialization,started,start_rejected,activate}`, `WorkerActivation`, `WorkerActivation::{worker,into_ready,into_rejection}` | Runtime port | The host settles activation only after the exact worker and initialization attempt are known. |
| `atomic/worker/initialization.rs` | `InitializationAttempt`, `ActivationPermit`, `ActivationPermit::{worker,initialization,target}`, `InitializeWorker`, `InitializeWorker::{target,worker,initialization,resolve}`, `WorkerInitializationOutcome`, `WorkerInitializationReport`, `WorkerInitializationFailure` | Runtime port | Initialization settlement carries exact worker custody and can authorize or reject later activation. |
| `atomic/worker/mod.rs` | `HostedInitialization` | Internal type alias | `WorkerRecovery::into_retirement` returns the actual `Actions` type through this alias. The exact worker and actions are now visible on `WorkerRecovery`; `WorkerAttempt` is a visible opaque correlation value. Whether the alias itself needs an external spelling requires a real host caller. |
| `lifecycle/shutdown_coordinator.rs` | `HeterogeneousShutdownItem`, `ChoiceSettlements`, `HeterogeneousShutdownChoiceSettlement` | Generated code obligation | The closed heterogeneous choice product supplies the typed settlement shape. |
| `protocol/mod.rs` | `ObserveCreation` | Runtime port | Observation must refer to the exact staged child creation and return its request on rejection. |
| `atomic/mod.rs`, `atomic/pool/mod.rs`, and `lifecycle/shutdown_coordinator.rs` | Grouped re-exports of the declarations above | Same as original declaration | The re-export annotations add no second capability; each name remains publicly reachable through its parent module. `CustomerDelivery` is now visible because external interpreters must name it; `FixedBuilder`, `FifoError`, and `KeyedError` are visible because applications name the inferred builder and aggregate errors. |

This table classifies ownership but does not by itself justify retaining each
public spelling. In particular, an associated type that mentions one of these
values is not proof that applications must name it. Closing A13 still requires
caller-facing compile witnesses, a trait-implementor inventory, and a
repeatable compile-cost comparison before changing visibility or bounds. The
branch-level cold comparison below measures aggregate compiler impact; it does
not replace a focused before/after comparison for a future individual bound.

`AssignWorker::target` and `CustomerDelivery` are now visible runtime ports.
The interpreter obtains a clone of the exact recipient capability through
`target`, then consumes `settle` to transfer the delivery while the request
retains its receipt. A rejected keyed outcome retains the original customer
route in the complete `CustomerDelivery` action. The former public `receipt`
and `into_parts` assembly methods remain atomic-module-only.

`ProxyDrain` and `WorkerStartResult` are visible domain outcome sums because
external supervisor and proxy callers match their concrete alternatives.
`ProxyOperation::creation` is visible for trusted interpreters selecting the
exact proxy; its numeric value remains creator-local correlation, not actor
identity or installation evidence. `ProxyEffects` and `ProxyEvent` remain
hidden structural products reachable through typed behavior projections and
event ingress; their documentation status grants no extra authority.

`WorkerRecovery` and `WorkerAttempt` are visible opaque custody and
correlation values. `WorkerRecovery::into_retirement` consumes the failed
worker and untouched initialization actions together. `WorkerAttempt::creation`
reveals only the creator-local correlation; both constructors and their
fields remain private. The `HostedInitialization` alias still has no public
crate-root spelling, so its external naming need remains open for the real
host witness.

The P2 assignment custody witness narrowed two formerly public methods after
external caller tests proved the consuming `settle` operation. Compile-fail
fixtures reject receipt extraction and splitting the request with `E0624`,
while a second settlement fails with `E0382`. The corresponding real FIFO and
keyed caller suites, benchmark, and fuzz campaigns use actual exact-delivery
admission. This closes those two spellings only; proxy assembly remains open.

The P2 proxy owner settlement also made the previously doc-hidden
`ProxyOperationId` export unnecessary. External interpreters now receive the
complete control through `ProxyControlAdmission` and return the exact actor or
control; only the owner retains the ID. A dedicated external fixture rejects
the ID name with `E0603`, while the receipt-constructor fixture independently
rejects `new` with `E0599`. The ID remains available to private supervisor
state and receipt settlement through a crate-private re-export.

The exact assignment and proxy interpreter products are now visible in the
`atomic` Rustdoc index: `AssignWorker`, `AssignmentReceipt`, `ProxyControl`,
`ProxyOperation`, `ProxyInputReceipt`, and `ProxyInputResult`. These were
already public Rust names required by external `InterpretItem` and
`ProxyControlAdmission` implementations. Their fields and owner-only receipt
constructors remain private; hiding the item pages served no capability
boundary. Six actor annotation sites were removed, leaving 75.

The worker-preparation interpreter path is also visible: `PrepareWorkers`,
`PendingWorkerPreparation`, `WorkerPreparation`, and both request phases'
`source_and_role`, `accept`, and `reject` methods. External fixed/FIFO tests
and Bombay's inspected source already name this progression. The ticket,
constructors, fields, and owner settlement remain private. Nine more actor
annotation sites were removed, leaving 66.

`DiagnosticRoute`, `DiagnosticAction`, `DiagnosticAccepted`, and their four
existing constructors are now visible in Rustdoc. External actor and
interpreter-contract suites already name these products to settle routed and
route-free diagnostics, while the sealed route trait still limits its
implementors. Removing eight documentation annotations changes no Rust
visibility, constructor, or transition; 58 actor annotations remained at that
checkpoint, with 56 after exposing the customer-delivery ports.
The proxy outcome and correlation documentation repair leaves 54 sites.
The returned-worker custody repair leaves 50 actor sites.

`WorkQueue` now keeps worker-route cloning and equality on construction and
transition operations, where queue inspection and duplicate availability use
them. Its protocol identity accepts a lawful `ReplyRoute` without `PartialEq`.
The external protocol-only caller failed on the previous aggregate bound with
only `E0277` and passes after the bound move; the FIFO transition suite still
exercises the comparable route used by a running queue.

Twelve unwrapped catalogue actors now expose `BehaviorBase<Base = Self>` for
opaque domain types without inheriting bounds used only by construction or
transition: `Acknowledgements`, `Resolver`, `Configuration`, `Readiness`,
`Machine`, `Topic`, `PubSub`, `Presence`, `Lease`, `Barrier`, `Workflow`, and
`OrderGate`. The external caller test failed on their former `Clone`, `Eq`,
`Copy`, `PartialEq`, or `Ord` bounds and passes after removing only those
impl-level bounds. Their stored state, protocol, and transition contracts are
unchanged. `Router` remains structurally bound by its route and strategy
policy and needs a separate owner review.

`Deduplicator` and `OrderGate` now also expose protocol identity for an opaque
key. `Deduplicator` exposes its read-only base projection on the same terms.
The key comparison bounds remain on the operations that actually deduplicate
or order messages. A focused caller failed before the change only on these
three impl-level bounds and passed afterward; the routing transition suite
remained green.

`PriorityQueue` now also accepts an opaque priority type at protocol identity
and base projection. Its `Ord` proof remains required where construction and
transition use the heap. The external caller failed on the old aggregate
bound and passes after this move; the stable-priority selection trace remains
green.

A source scan after these batches found `Router` as the only catalogue
`Protocol` or `BehaviorBase` impl whose header still carries a copying,
comparison, or ordering bound. Its route equality is used for membership and
its current `RoutingStrategy<Route>` contract requires `Clone + PartialEq`.
The aggregate review retained those bounds: construction deduplicates by
route identity, Add/Remove compare exact members, and a route attempt clones
its policy candidate for rollback on rejection. A type-level router with an
uncomparable route would have no lawful construction or transition, so
removing only a repeated header bound would add no usable protocol contract.

## Public trait implementors

The source declares 78 top-level public traits: 55 in `behavior` and 23 in
`behavior-actors` (counted with `rg '^pub trait '`). The following inventory
accounts for every declaration. “Author” means an application defining its
own typed actor, effect, or child protocol; “interpreter” means the code that
settles an exact typed request. A structural proof may be public because a
generated or manually authored product must implement it, even if ordinary
applications should not mention it directly.

| Owner | Traits | Lawful implementors and reason for the port |
|---|---|---|
| `transition.rs` | `Protocol`, `Behavior` | Authors declare a stable typed destination and its pure transition; wrappers implement the same contract for composed actors. |
| `transition.rs` | `LogicalHostRequirements`, `BehaviorBase` | Blanket logical-host projection for any qualifying behavior; authored behaviors and wrappers expose the underlying base. |
| `transition.rs` | `BehaviorLayer` | A concrete construction closure or an authored construction type; the existing blanket closure implementation is the common case. |
| `user_event.rs` | `UserEvent`, `ComposedEvent`, `EventIngress`, `ChildInputIngress`, `InjectEvent`, `RecoverEvent` | Authored event sums and structural wrapper event products; these preserve a typed user-message lane and lossless event injection/recovery. |
| `actor/addressing.rs` | `Address`, `EndpointAddress` | Address-space authors and concrete address types; the latter provides the endpoint form used at the interpreter edge. |
| `actor/addressing.rs` | `InterpretEstablished` | Interpreters of an exact established recipient capability. |
| `actor/creation.rs` | `ChildRole`, `ChildOccurrence`, `ResolveChildOccurrence`, `EstablishChild`, `ChildCreationProduct`, `DispatchBirth` | Authors or generated role declarations establish exact child positions; structural birth products and interpreter adapters resolve and dispatch them. |
| `actor/creation.rs` | `ChildPosition`, `BirthNodeAppend`, `ChildOccurrenceResolution`, `ResolveChildOccurrenceDescriptor`, `BirthNodeAt`, `ChildOccurrenceShape`, `ChildOccurrenceProduct`, `ChildOccurrenceProductAt`, `ChildProduct` | Closed structural child-position and birth-product proofs; source and generated products implement these to retain exact occurrence and custody. |
| `actor/creation.rs` | `BirthMode`, `BirthProtocolAt`, `BirthProtocolProduct`, `BirthProtocols`, `BirthModeProtocols`, `BirthNodeProtocols`, `BirthNodeLogicalHosts` | Authored birth modes and structural projections enumerate child protocols and logical hosts without runtime lookup. |
| `effects/actions.rs` | `ActionSettlements`, `BehaviorSettlements`, `CreationSettlements`, `AppendSend` | Complete structural action/creation settlements and typed send-product append operations. |
| `effects/actions.rs` | `InterpretCreations` | Interpreter-bound traversal of one exact ordered creation product. |
| `effects/sending.rs` | `ClassifySettlement`, `ActionItem`, `SendSettlements`, `SendInput`, `SendEffects`, `LogicalDeliveryProtocols`, `SendsFor`, `SourceAction`, `SourceSettlementCustody`, `ReturnToEmitterFor`, `InterpreterRequest` | Authors and concrete structural products define the request, settlement, routing, and logical-destination equations. The `#[behavior]` macro derives the logical-host projection for named sends in field order; other authored custom sends products state it explicitly. A generated product is private by default and may be exported with `sends = pub { ... }` when its field types are public. |
| `effects/sending.rs` | `InterpretItem`, `InterpretSends`, `SourceAdmission` | Interpreters settle exact items; concrete send products traverse them; actor ingress admits an exact returned source action. |
| `activation.rs` | `Activate` | Blanket implementation for a behavior that can enter its consuming initialization path. |
| `atomic/diagnostic.rs`, `atomic/fixed_supervisor/lifecycle.rs`, `atomic/pool/mod.rs` | `DiagnosticRoute`, `FixedLifecycleRoute`, `CompletesAssignments` | Sealed diagnostic, fixed-lifecycle, and completion products; only the declared finite alternatives implement them. |
| `atomic/worker/mod.rs`, `atomic/worker/preparation.rs` | `ActivationPlan`, `WorkerSource` | An author supplies a concrete worker activation plan and a source that returns complete prepared or rejected worker custody. |
| `atomic/stable_proxy/operation.rs` | `ProxyControlAdmission` | A trusted interpreter of a statically selected proxy child implements this port. It receives a concrete `ProxyControl` and returns the exact admitted actor or the owned control with a reason; the operation ID stays with `ProxyOperation`. The actor suites provide local interpreters, and the isolated Bombay runtime has an implementation, but its unmerged source does not prove production integration. |
| `composition/delivery_route.rs` | `DeliveryRoute`, `DeliveryRouteFor` | Sealed exact route products and their owning behavior relationship. |
| `lifecycle/child_shutdown.rs` | `BeginShutdownPhases`, `DeclareShutdownPhase`, `FinishShutdownPhases`, `AssignAt`, `AllAssigned` | Structural shutdown-plan composition and the finite proof that every required child was assigned. |
| `lifecycle/shutdown_coordinator.rs` | `ShutdownTargetAt` | A typed child-position shutdown target in a heterogeneous plan. |
| `lifecycle/termination_monitor.rs`, `lifecycle/termination_propagation.rs` | `TerminationObservationTarget`, `TerminationTarget` | Exact recipient forms for observing and propagating termination. |
| `protocol/established.rs` | `InterpretEstablishedObservation`, `InterpretEstablishedShutdown` | Interpreters of exact established observation and shutdown requests. |
| `routing/router.rs` | `RoutingStrategy`, `RouteKey` | Authors select a concrete policy and define the application message key; the catalogue supplies round-robin, least-loaded, consistent-hash, and rendezvous strategies. |
| `stash.rs` | `StaticallyInfallible`, `StashStatus` | The former is sealed to infallible forms; the latter projects stashed-message status through multiple existing wrappers. |

The table identifies implementor *roles*, not proof that every spelling should
stay public. In particular, the structural rows still need external compile
witnesses before visibility can be reduced. `StashStatus` already has multiple
real wrapper implementations, so treating it as a redundant one-implementation
trait would be incorrect. The remaining A13 work includes caller diagnostics
and a focused compile-cost comparison for any further bound change.

## Cold actor-library compile comparison

On aarch64-darwin, the Nix shell at `1aeaed1` supplied Cargo 1.95.0 for both
revisions. Each run used a newly absent `CARGO_TARGET_DIR`, the same command
shape (`cargo check --manifest-path REV/Cargo.toml -p
bombay-behavior-actors --lib --locked --offline -q`), and ran sequentially.
The actor crate's Cargo fingerprint recorded the same feature list, profile,
compiler configuration, and dependency identities in both revisions.

| Revision | Cold run 1 | Cold run 2 | Actor `.rmeta` |
|---|---:|---:|---:|
| `main` at `435560c` | 184.54 s | 180.09 s | 27 MiB |
| Audit branch at `1aeaed1` | 4.46 s | 3.97 s | 5.8 MiB |

The measured command checks the actor library only, not tests, downstream
applications, or release builds. The revisions differ in many source files,
including macro and actor implementations; the observation cannot attribute
the difference to any one A13 bound or predict downstream compile time. It
does show that this branch's actor-library metadata and cold check cost did
not grow under this controlled comparison.
