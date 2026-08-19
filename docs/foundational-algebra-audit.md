# Foundational actor-template algebra audit

This audit is a design input, not a compatibility plan. It classifies the
current production templates before their composition surface is changed.

The audited forwarding defect has now been removed: `EventInput`, `RouteInput`,
and `forward_event_lane!` no longer exist. Structural `InjectEvent<Input,
Path>` evidence names `Here` or an exact `Inside<Path>` owner. The historical
names below describe the defect that motivated the replacement, not retained
APIs.

## Foundational pillars

Every complete actor definition has four distinct, statically known algebras:

1. **Protocol identity** names a stable address namespace and public message
   algebra. It remains usable when the actor changes behavior and when a
   different actor emits a delivery to it.
2. **Ingress algebra** is the concrete sum of user communications and local
   environment facts accepted by the current composed behavior.
3. **Behavior algebra** is the pure fold from current state and one ingress
   value to the next state and explicit `Actions`.
4. **Effect algebra** is the concrete product of communications, staged fresh
   creations, local observation/scheduling requests, and the next-behavior or
   termination decision.

Protocol is not behavior, ingress is not protocol, and an interpreter-facing
request is not the fact that later returns through an ingress lane.

The audit also exposes five cross-cutting pillars that connect those algebras
without merging them:

5. **Destination capability** distinguishes an established transferable
   `Recipient<P>`, a creator-local `ChildRecipient<P>`, and a local ingress
   destination. Equal address/message shapes do not imply equal identities.
6. **Causality and correlation** connect a scheduling/observation request to
   exactly the fact it may later produce. A nonce, timer ID, or generation is
   correlation data, not the destination capability itself.
7. **Namespace and freshness** distinguish globally established identity from
   creator-local routing and staged creation. Creation intent, committed
   installation, and replacement provenance remain separate values.
8. **Lifecycle and phase** distinguish initialization, active turns,
   behavior-selected phase change, behavior-selected termination, runtime
   termination fact, and supervision policy. They are not one status flag.
9. **Interpretation order** connects the pure `Actions` product to the runtime:
   commit fresh creations before dependent same-action services/sends, preserve
   order within each named lane, and do not invent ordering between independent
   lanes.

These pillars are orthogonal but not independent: their connections must be
represented by typed capabilities and named products rather than inferred from
addresses, IDs, nesting depth, or interpreter searches.

In particular, the effect algebra is indexed by the event algebra for local
runtime returns. `Behavior::Sends: SendsFor<Behavior::Event>` makes this
connection a compiler obligation. `InterpreterRequest::ReturnToEmitter` distinguishes
emitter-local continuations from child, parent, ancestor, and established
destinations. A wrapper reindexes only the former.

The audit found three wrappers that extended events while claiming literal
send equality: `Guardian`, `StopOnShutdown`, and `FinalizeOnShutdown`. They now
use `SendLayer<NoSends, Inner>`. `Stash` remains literally transparent
because it changes neither event nor effect algebra. Worker lifecycle reports were also
incorrectly described as local `Here` returns even though the emitting proxy
targets its parent relationship; those reports are now explicitly one-way
interpreter requests rather than false local ingress capabilities.

## Actor templates versus transition components

A concrete actor template owns a nominal public protocol, complete state, and
complete behavior/effect algebra. Reuse below that boundary should come from a
pure transition component with one stated law, not from inheritance, runtime
delegation, or an extra actor hop.

The audit identifies these genuine component families:

- one-shot, periodic, deadline, receive-timeout, lease, presence expiry,
  circuit reset, and restart backoff all use generation-safe timer ownership;
- watch, termination monitoring, child supervision, creation observation, and
  coordinated shutdown all use request-selected lifecycle observation;
- standalone `Supervisor`, composed `Supervise`, `BackoffSupervisor`,
  `BackoffSupervise`, `WorkerPool`, and `KeyedWorkerPool` share one
  stable-proxy fleet-ownership transition component; backoff forms additionally
  share one generation-safe pending-delay component;
- routing actors share typed destination/evidence components while retaining
  different admission, ordering, capacity, and rejection laws;
- request/reply catalogue actors share typed reply delivery but not necessarily
  the same state machine;
- `Features` is a truthful domain specialization of versioned
  `Configuration` rather than an independent transition implementation.

The target construction is therefore conceptually:

```text
concrete actor template
= nominal protocol owner
+ domain state/transition component
+ owned ingress components
+ named effect components
+ birth capability
```

This equation is a design decomposition, not a public tuple callers assemble.
The concrete template constructor supplies and infers the components. A
component becomes a separate actor only when independent concurrency,
addressability, and mailbox serialization are themselves part of the required
semantics.

Similarity is insufficient justification for abstraction. Buffering,
priority ordering, rate limiting, deduplication, and work assignment may all
retain collections, but their acceptance and ownership laws differ. They must
not be collapsed into a configurable mega-template whose flags recreate the
state machines at runtime.

## Source of the forwarding defect

`RouteInput<Input>` tries to express two incompatible implementations for a
generic wrapper:

- own `Input` at the current layer; and
- forward every otherwise accepted `Input` to the inner layer.

Those implementations overlap when the inner algebra also accepts `Input`.
The repository works around Rust coherence by enumerating a closed list of
unrelated inputs in `forward_event_lane!`. Consequently a new lifecycle fact
requires edits to timer, watch, shutdown, supervision, proxy, and coordinator
products that do not own that fact. Missing one edit silently breaks a valid
nesting order.

The scheduling and observation effects expose the same missing abstraction.
`ScheduleAfter`, `ScheduleAt`, `ObservePeer`, `ObserveChild`, and
`ObserveCreation` identify correlation data but not the typed ingress
destination that must receive the resulting fact. The interpreter is therefore
forced to rediscover a route from the final behavior type.

## Public-surface findings

The production actor crate currently contains 44 behavior implementations and
374 public structs, enums, aliases, and traits. Only the small wrapper and
lifecycle family needs compositional non-user ingress; most concrete catalogue
actors are pure `User<Addr, Message>` folds. A universal event-routing API is
therefore solving a narrow interpreter seam at the cost of coupling the whole
catalogue.

Current protocol identity has three spellings:

- `Protocol = Self` gives a concrete template a useful nominal identity with
  no additional user type;
- a distinct message enum used as `Protocol` also gives nominal identity, but
  conflates the message algebra with the owner of that algebra;
- a separate zero-state protocol product is necessary for recursive seams and
  for identities that must not name the implementing behavior.

These are not interchangeable style choices. Concrete templates should own a
nominal protocol automatically. Message enums remain messages. Separate
zero-state protocol products are reserved for recursion or an intentionally
shared protocol; they are not aliases added merely to shorten a signature.

`PhantomData` likewise has two different meanings. It is sound when it carries
nominal protocol/type evidence in a zero-state value. It is a development-
experience smell when a public constructor has no meaningful value from which
an essential child, destination, or reply protocol can be inferred. The latter
must be replaced by a typed topology, recipient, factory, or creation value—or
acknowledged as information the caller fundamentally must choose.

## Template classification

### Transparent behavior wrappers

These preserve `B::Protocol` and must preserve all inner ingress/effect lanes:

- `Stash` owns no environment lane and transforms user-message handling.
- `Deadline`, `OneShot`, `Periodic`, and `ReceiveTimeout` own keyed
  `TimerElapsed` lanes.
- `Watch` and `TerminationMonitor` own keyed `PeerStopped` lanes.
- `StopOnShutdown` and `FinalizeOnShutdown` own every shutdown request at
  their layer.
- `Guardian` is different: its policy is selected at construction. The direct
  form owns root shutdown; the coordinated form delegates it to the exact
  inner `Here` owner.
- `Supervisor` owns lifecycle facts for its configured child namespace and
  preserves facts it does not own.
- `BackoffSupervisor` and `BackoffSupervise` add the same keyed timer fold
  around standalone and composed supervision respectively.
- `ShutdownCoordinator` owns shutdown progress facts for its current plan and
  preserves stale or unrelated facts.

### Concrete actors with non-user ingress

- `Proxy` owns proxy commands, child termination, creation resolution,
  shutdown, and child-shutdown rejection.
- `DynamicSupervisor` owns its command protocol and the lifecycle facts for
  its dynamic proxy set. It is not currently a coordinated-shutdown state
  machine.
- `WorkerPool` and `KeyedWorkerPool` own their command protocols plus the
  supervision facts required by their fixed worker topology.
- `CircuitBreaker`, `Lease`, and `Presence` own their public messages plus one
  private keyed timer lane.

### Concrete user-protocol actors

These accept only `User<Addr, Message>` and therefore do not need an ingress
wrapper: `Machine`, `MessageAdapter`, `Task`, `Buffer`, `Router`,
`PriorityQueue`, `WorkQueue`, `Sequencer`, `Acknowledgements`, `Deduplicator`,
`RateLimiter`, `Correlator`, `OrderGate`, `Topic`, `Registry`, `Resolver`,
`PubSub`, `Configuration`, `Health`, `Readiness`, `Cache`, `Workflow`,
`Barrier`, and `Latch`.

## Complete production behavior inventory

| Domain | Template | Identity role | Ingress role | Effect/birth role |
|---|---|---|---|---|
| core | `Machine` | concrete nominal template | user only | no sends/births |
| composition | `MessageAdapter` | intentional protocol adapter | user only | one typed delivery |
| composition | `Stash` | preserves inner | preserves exact inner event | preserves inner effects/births |
| lifecycle | `Guardian` | preserves application identity | direct root owner or exact coordinated-inner owner | preserves inner effects/births |
| lifecycle | `StopOnShutdown` | preserves inner | owns shutdown | preserves inner effects/births |
| lifecycle | `FinalizeOnShutdown` | preserves inner | owns shutdown | preserves final sends/births, forces stop |
| lifecycle | `ShutdownCoordinator` | preserves inner | owns planned shutdown progress | adds typed child-shutdown requests |
| lifecycle | `TerminationMonitor` | preserves inner | owns one peer observation | adds peer observation |
| lifecycle | `Task` | concrete nominal template | user only | typed terminal reply, stop |
| timing | `Deadline` | preserves inner | owns one timer destination | adds absolute schedule |
| timing | `OneShot` | preserves inner | owns one timer destination | adds relative schedule |
| timing | `Periodic` | preserves inner | owns one recurring timer destination | adds relative schedules |
| timing | `ReceiveTimeout` | preserves inner | owns one inactivity timer destination | adds relative schedules |
| timing | `Lease` | concrete nominal template | user plus private timer | replies plus schedule |
| supervision | `Proxy` | stable proxy identity | commands plus exact worker lifecycle | worker observations/reports/births |
| supervision | `FixedFleetOwnership` | private transition component, no identity | exact configured-child lifecycle | proxy commands/observations/births |
| supervision | `Supervisor` | concrete nominal ownership-only protocol | exact configured-child lifecycle | shared fixed-fleet effects/births |
| supervision | `Supervise` | preserves inner application identity | inner ingress plus exact configured-child lifecycle | shared fixed-fleet effects/births plus inner effects |
| supervision | `BackoffSupervisor` | preserves standalone supervisor identity | supervisor ingress plus private timer | supervisor effects plus schedule |
| supervision | `BackoffSupervise` | preserves inner application identity | composed-supervision ingress plus private timer | composed-supervision effects plus schedule |
| supervision | `DynamicSupervisor` | concrete nominal template | commands plus exact dynamic-child lifecycle | proxy effects/births/outcomes |
| pool | `PoolCore` | private transition component, no fabricated behavior identity | user plus exact worker lifecycle | shared fixed-fleet effects/births plus assignments/outcomes |
| pool | `WorkerPool` | public nominal pool protocol | user plus exact worker lifecycle | supervised pool effects/births |
| pool | `KeyedWorkerPool` | public nominal keyed-pool protocol | user plus exact worker lifecycle | supervised pool effects/births |
| routing | `Buffer` | concrete nominal template | user only | released values and outcomes |
| routing | `Router` | concrete nominal template | user only | routed deliveries |
| routing | `PriorityQueue` | concrete nominal template | user only | target deliveries and outcomes |
| routing | `WorkQueue` | concrete nominal template | user only | worker deliveries and outcomes |
| routing | `Sequencer` | concrete nominal template | user only | ordered deliveries and outcomes |
| routing | `Acknowledgements` | concrete nominal template | user only | acknowledgement replies |
| routing | `Deduplicator` | concrete nominal template | user only | target deliveries and outcomes |
| routing | `RateLimiter` | concrete nominal template | user only | target deliveries and outcomes |
| routing | `Correlator` | concrete nominal template | user only | correlated replies |
| routing | `OrderGate` | concrete nominal template | user only | target deliveries and outcomes |
| routing | `CircuitBreaker` | concrete nominal template | user plus private timer | outcomes plus schedule |
| discovery | `Topic` | concrete nominal template | user only | publication deliveries |
| discovery | `Registry` | concrete nominal template | user only | lookup replies |
| discovery | `Resolver` | concrete nominal template | user only | resolution replies |
| discovery | `PubSub` | concrete nominal template | user only | destination deliveries |
| discovery | `Presence` | concrete nominal template | user plus private timer | replies plus schedules |
| operations | `Configuration` | concrete nominal template | user only | versioned replies |
| operations | `Features` | truthful specialization of `Configuration` | user only | versioned replies |
| operations | `Health` | concrete nominal template | user only | health reports |
| operations | `Readiness` | concrete nominal template | user only | readiness reports |
| persistence | `Cache` | concrete nominal template | user only | cache-result deliveries |
| workflow | `Workflow` | concrete nominal template | user only | workflow outcomes |
| workflow | `Barrier` | concrete nominal template | user only | release deliveries |
| workflow | `Latch` | concrete nominal template | user only | release deliveries |

Aliases that merely give an established semantic specialization a domain name
(`Link`, `Reaper`, `LifecyclePublisher`, `TreeShutdown`, and `Features`) are
reviewed separately from aliases that conceal plumbing. The former may remain
when their laws and documentation are genuinely identical; the latter should
be removed rather than treated as new templates.

## Required composition laws

1. A layer declares only the ingress lanes it owns.
2. Adding a new semantic input never requires editing an unrelated layer.
3. Every effect that requests a later local fact carries a statically selected
   ingress destination; the interpreter does not search the behavior type.
4. Wrapping an actor maps every inner local destination through exactly one
   structural `Inside` step without changing its payload or owner.
5. Equal payload types at different layers remain distinct capabilities.
6. A keyed request selects its exact owner when it is emitted. The selected
   owner consumes matching facts and treats stale facts as documented; it does
   not search inner layers for another owner with the same payload type.
7. Ordinary layer ownership and root shutdown policy are different domains.
   Coordinated shutdown is configured as Guardian policy; otherwise Guardian
   owns direct normal stop.
8. Unsupported ingress has no construction capability and fails at compile
   time.
9. Transparent wrappers preserve protocol identity. Protocol adapters create a
   new nominal protocol intentionally.
10. User construction receives all otherwise-uninferrable types through
    meaningful protocol, recipient, topology, or creation values. A
    `PhantomData` field is valid as zero-state proof, but not as a substitute
    for missing semantic constructor input.

## Provenance of the laws

### Actor-model laws

- An actor processes one communication at a time.
- Its transition may send communications to known recipients, create fresh
  actors, and designate the behavior for the next communication.
- An actor address is the identity used by communication; it is not the current
  behavior implementation or the actor that happens to emit a later send.
- Fresh actor allocation and behavior replacement are distinct operations.

These laws justify the pure fold and explicit `Actions` boundary. They do not
dictate Rust event enums, timer IDs, shutdown precedence, or Bombay's local
child routes.

### Derived Bombay constructions

- `Protocol` is nominal compile-time evidence for an address/message
  signature.
- `Recipient<P>` and `Delivery<P>` are pure typed realizations of known actor
  destinations.
- `EventLayer<Owned, Inner>` is a concrete coproduct for composed ingress.
- A local ingress destination is the non-addressed counterpart of a recipient:
  it identifies exactly where a causally produced interpreter fact re-enters
  the composed fold.
- Creator-local child recipients, typed creation correlation, proxy-stable
  identity, closed child sums, and named send products are library
  constructions preserving the static laws.

### Deliberate Bombay policy

- Initialization effects precede mailbox events and wrappers preserve their
  defined order.
- Creations are committed before dependent same-action services and sends.
- A interpreter request selects the ingress destination of its eventual fact;
  facts are not broadcast through wrapper layers.
- Stale facts delivered to their selected keyed owner are inert unless that
  template documents a typed rejection.
- Ordinary event layers are outer-owned. Coordinated shutdown, when configured,
  is part of the guardian root policy rather than an accidental duplicate
  shutdown owner; otherwise guardian owns direct normal stop.
- No ordering is inferred between independent named effect lanes.

Structural ingress is not presented as a primitive from Agha; it is Bombay's
typed realization of interpreter-originated communication into a composed
behavior.

Stable proxy reports are parent-directed communications rather than returns to
the emitting proxy. The parent therefore supplies a `ProxyParentIngress` when
constructing the proxy. Its two fields preserve the correlated
`WorkerStopped` and `WorkerCreationResolved` owners as one capability product.
Wrapping a parent lifts both fields together (`Here` to `Inside<Here>`), while
the proxy's public protocol identity remains unchanged. This is a Bombay
derived realization of acquaintance passing: the actor-model locality law
requires the proxy to know its reporting destination, but does not prescribe
Rust event paths or a distinguished parent relationship.

## Status of the reported shutdown gap

`StopOnShutdown<DynamicSupervisor<...>>` and
`FinalizeOnShutdown<DynamicSupervisor<...>>` must compile because those
wrappers own the shutdown request they add. Adding `ShutdownRequested` to
`DynamicSupervisorEvent` merely to satisfy an inner-routing bound would be
false: it would claim a coordinated shutdown protocol without defining its
state machine. The former `Guardian<ShutdownCoordinator<...>>` construction
exposed duplicate ownership because the root policy was split across two
wrappers. Guardian now selects either coordinated shutdown or direct normal
stop as one exhaustive policy at construction.

The actor-model requirement here is that behavior transitions remain pure and
effects explicit. Structural ingress destinations, creator-local observation
correlation, and shutdown ownership precedence are derived Bombay
constructions/policies and must be documented as such.

## State-product findings outside ingress composition

The broader sum/product pass found additional issues that must not be hidden by
the ingress refactor:

- `IncarnationStopEffects` uses two `Option` fields although only three of the
  four combinations are semantic. It needs an exhaustive `Ignored | Stopped |
  Restarting` sum.
- Pool affinity uses `Option<Nonce>` to distinguish ordinary and affinity-bound
  jobs. That is a semantic routing choice and should be a named sum; the
  independent optional interruption fact may remain an `Option` because its
  domain really is fact-or-absence.
- Internal boolean completion in `CircuitBreaker` is derived from two
  exhaustive message variants. It should remain a sum through the helper
  boundary rather than becoming a boolean.
- Production `expect` calls in the breaker counter and full-buffer eviction are
  locally argued invariants but still expose state representations that permit
  the impossible case. They require either a truthful non-empty/advanced state
  product or a total branch that preserves the controlled error contract.
- Alias review must distinguish semantic specialization from a second name for
  plumbing. In particular, an alias is not a new protocol identity or actor
  template.

These are algebraic cleanup items, but they do not justify merging independent
domains. The ingress change must not become a pretext for rewriting unrelated
state machines without a stated law and regression model.
