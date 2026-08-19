# Bombay behavior-template catalogue

This document is the design catalogue for reusable actor-behavior templates in
the Bombay stack. A catalogue entry is passive: it is a concrete, typed fold
which can be instantiated with an application domain and driven by Bombay. It
is not an independently running service or a second runtime. The document
records canonical names, template composition, required interpreter
capabilities, ownership, and where an implementation crate may supply an
algorithm or facility without defining Bombay semantics.

The common execution contract is defined in the
[Universal Behavior Driver](driver.md). Every catalogue entry must use that
one Driver law; a template is not admitted if it requires a private execution
loop.
Public destination identity, internal events, behavior state/fold, and effects
follow the orthogonal model in
[Protocol, ingress, behavior, and effect algebras](protocol-algebra.md). Catalogue
wrappers may extend events and effects but must preserve the wrapped public
protocol unless their stated purpose is message adaptation.
Actor roles requiring clocks, tasks, address authority, durability, transport,
operating-system resources, or other interpreted effects are tracked in the
[Runtime-backed actor capability record](runtime-backed-actors.md). That record
is the normative template-to-runtime backlog; a prospective catalogue name is
not a usable framework actor merely because its pure policy can be written.
The sibling repositories are one owned architecture; their boundaries and
dependency direction are summarized in
[Bombay ecosystem ownership](ecosystem.md).
The two implemented cross-family wrapper orders and their exact public
construction/error laws are documented in
[Proven composition recipes](composition-recipes.md).

The catalogue is prospective. An entry says that a reusable behavior template
can be derived from the existing algebra; it does not mean that the template is
already implemented. Only the implementation inventory below claims current
coverage. Add a template to the public API only when its laws, concrete use,
runtime realization, and composition tests justify it.

## Construction and ownership law

The construction rule is a derived Bombay architecture law:

```text
running actor
    = catalogue behavior template instantiated with a concrete domain
    + zero or more existing catalogue templates
    + Bombay runtime capabilities
    + repeated interpretation by bombay-engine::Driver
```

More precisely, the template consumes its current state and one typed event and
returns `Actions`: typed communications, staged fresh creations, and its next
behavior or termination decision. The driver supplies a steady stream of
events and interprets each returned action before advancing the actor:

```text
(current template state, next typed event)
                    |
                    v
             pure Behavior fold
                    |
                    v
       Actions { send, create, next }
                    |
                    v
       Bombay capability interpretation
                    |
                    +------> next typed event
```

The actor-model law is that an actor processes one communication at a time and
may send, create, and designate its next behavior. Packaging the fold,
capability adapters, and driver this way is a Bombay construction. The precise
commit ordering, initialization ordering, typed lanes, and lifecycle reports
are deliberate Bombay policies documented by the relevant contracts.

The terms have strict ownership boundaries:

- `bombay-behavior` owns only the pure transition algebra: a statically known
  event is folded into typed communications, staged fresh creations, and the
  next behavior or termination decision.
- `bombay-behavior-actors` owns reusable, passive templates and compositions
  expressed entirely through that algebra. It may use a crate for a collection
  or pure algorithm when doing so does not import hidden effects or foreign
  actor semantics.
- `bombay-engine::Driver` owns the common driving law: accept the next event,
  invoke the fold once, and hand its explicit result to capability
  interpretation. A template never runs its own loop.
- The [Bombay repository](https://github.com/devrandom-labs/bombay) owns the
  actor runtime interpreter: mailboxes, scheduling, clocks, installation, and
  the typed adapter boundary for persistence, transport, discovery, telemetry,
  and other effects. Mnesis owns durability semantics and stores; CESR/KERI
  owns its representation and identity protocols; focused Bombay siblings own
  their mechanisms. A template may expose a typed protocol for one of these
  capabilities, but it does not perform the capability itself.
- Applications own domain protocols and domain state. Catalogue templates wrap,
  route, supervise, persist, or otherwise compose those concrete protocols.

An external crate is an implementation component, never the semantic source of
truth. Its output must enter behavior as a typed event and its requested work
must leave through `Actions`. Serialization must not replace typed internal
protocol composition, and external identity generators must not be treated as
proof of fresh actor allocation.

### Required contract for each implementation

Before work begins on a catalogue entry, its design must state:

| Contract field | Required content |
|---|---|
| Template | Canonical Bombay role name and generic parameters |
| Domain slot | The concrete application protocol, behavior, state, or policy inserted by the user |
| State sum | Every valid phase, including rejection, stale input, retry, and termination where applicable |
| Event sum | Domain communications and typed runtime observations accepted by the fold |
| Action product | Every send, fresh creation, report, and next-behavior lane produced by a transition |
| Required capabilities | Timer, observation, address, mailbox, storage, transport, or other interpreter support |
| Current realization | The Bombay crate and adapter that can realize each capability, or an explicit gap |
| Policy | Decisions owned by the template rather than guaranteed by actor research or delegated to the runtime |
| Composition law | Wrapper orders supported and proof that no event or effect lane is lost, duplicated, reordered, or reinterpreted |

The implementation classifications used below are:

- **behavior template**: implement in `bombay-behavior-actors`;
- **derived template**: compose existing templates without new runtime
  machinery;
- **capability-backed template**: add a pure fold over an already implemented
  Bombay capability;
- **application template**: reusable structure whose protocol remains a user
  domain decision;
- **runtime capability**: interpreter machinery, not an actor-template entry;
- **subsystem gap**: requires runtime capability Bombay does not yet provide.

## Module ownership and Bombay façade map

This table is normative for the source layout. A row names one primary owner;
supporting types remain beside that owner unless a repeated semantic
construction has been demonstrated in at least two families. “Capability”
means a typed requirement interpreted outside this crate, never an ambient
service used by the fold.

| Catalogue family | Actors module | Owned templates and compositions | Owned state, protocol, products, and errors | Shared construction dependencies | Runtime capability dependencies | Bombay façade |
|---|---|---|---|---|---|---|
| Fundamental composition | `composition` | direct `Activate`, `Active`, public concrete wrapper constructors, `Machine`, protocol forwarding, stash | initialization typestates, machine moves, stash route/status, ordered heterogeneous creation products | event injection, named-lane forwarding, exhaustive `DispatchBirth` | none beyond the universal Driver and concrete `InstallBirth` implementations | `bombay::behavior::{Behavior, Actions, Children, Births, Activate, Machine}` and `bombay::actors` wrapper types |
| Lifecycle | `lifecycle` | watch/link policy, shutdown, lifecycle monitor compositions | observation and shutdown events, reactions, terminal classifications | initialization accumulation, observation forwarding | Observe, child installation, terminal publication | `bombay::actors::{Watch, Task, Guardian, ShutdownCoordinator}` as each concrete template ships |
| Supervision | `supervision` | `Proxy`, `Supervisor`, worker pools, restart policies | incarnation/fleet/budget sums, replacement protocol, `ChildTopology`, `RestartConfiguration`, `PoolConfiguration`, named supervision and pool products, typed errors | creation-result correlation, recipient membership | fresh installation, Observe, Timers for backoff | `bombay::actors::{Proxy, Supervisor, Pool, KeyedPool}` plus semantic configurations |
| Routing and delivery | `routing` | routers and strategies, work admission, correlation, ordering, retention and delivery-policy compositions | recipient membership, pending/attempt/order states, delivery outcomes and named delivery products | bounded FIFO, keyed pending correlation, bounded retention | Address and Communication; Timers for timed policies | `bombay::actors::routing::*` selected collision-free role exports |
| Discovery and messaging fabric | `discovery` | registry, resolver, receptionist, topics and typed hubs | binding/subscription/presence sums, lookup and conflict errors | recipient and subscription membership | Address; discovery observations from System adapters | `bombay::actors::discovery::*` |
| Time-derived behavior | `time` | deadline, receive timeout, one-shot, periodic, retry schedule, heartbeat, timed lease/passivation | timer generations, schedule lifecycle sums, named schedule products | timer-generation lifecycle | Timers and, where stated, Observe | `bombay::actors::time::*` |
| Persistence-derived behavior | `persistence` | only host-side typed folds whose Mnesis adapter exists; cache as a pure policy | recovery/command/commit/checkpoint outcome sums and named request products | keyed pending correlation, initialization phases | Mnesis through `mnesis-bombay`; Entity for stable local hosts | `bombay::actors::persistence::*` only after adapter completion |
| Workflow and streams | `workflow` | process, coordination, barrier/latch/rendezvous, batch and demand compositions | generation, participant, demand and compensation sums; named participant products | keyed pending correlation, bounded FIFO, recipient membership | Timers and Mnesis only when the concrete composition requires them | `bombay::actors::workflow::*` |
| Cluster behavior | `cluster` | pure policies over typed membership/placement evidence only | membership evidence, suspicion, placement and handoff sums | recipient membership, timer-generation lifecycle | future cluster membership, transport, fencing/leadership adapters | no ordinary-user exposure until a complete System path exists |
| Operational boundary behavior | `operations` | health/readiness/configuration/feature and typed observation policies | component status, version, rejection and export-request products | subscription membership | capability-specific System exporters and gateways | `bombay::actors::operations::*`; adapters remain private |

The top-level façade deliberately re-exports authored domain algebra through
`bombay::behavior`, reusable roles through `bombay::actors`, and runtime
construction through `bombay::{System, Actor, ActorRef, Handle}`. Component
module paths, environment implementations, `RuntimeEffects`, mailbox anchors,
child leases, timer queues, observation spaces, address registration leases,
Mnesis stores, and transport/codec adapters remain framework-extension or
private implementation surface. This is a proposed integration map: the
Bombay repository currently re-exports the foundational `behavior` crate but
does not yet depend on or re-export the split `bombay-behavior-actors` crate.

The future `System` construction boundary must accept an inferred concrete
behavior value (`spawn<B: Behavior>(definition: B)` schematically). Ordinary
users must not name types such as
`Deadline<Stash<Machine<...>>>` merely to start an actor. Local variables and
generic spawn calls infer that type from the wrapper chain. An explicit nested
type is appropriate only at an advanced component-extension boundary that
stores the value in a named field, exposes it in a function signature, or
defines a reaction whose parameter is that exact inner composition. Bombay may
offer nominal macros for those advanced boundaries, but must not erase the
protocol or require such a name for normal construction.

### Complete catalogue classification

The audit below assigns every catalogue noun to exactly one required ownership
category. “Concrete” means that the role owns a reusable state/protocol law in
this crate; it does not claim implementation status. Status remains recorded in
the implementation inventory, so a prospective row cannot be mistaken for a
shipped API.

| Required category | Catalogue entries | Consequence |
|---|---|---|
| **1. Concrete reusable behavior template** | `Machine`, `Proxy`, `Supervisor`, `Pool`, `KeyedPool`, `Task`, `Watch`, `Router`, `RoundRobin`, `Broadcast`, `LeastLoaded`, `ConsistentHash`, `RendezvousHash`, `WorkQueue`, `PriorityQueue`, `Correlator`, `Acknowledgements`, `Deduplicator`, `Sequencer`, `OrderGate`, `Stash`, `Buffer`, `CircuitBreaker`, `RateLimiter`, `Registry`, `Resolver`, `Topic`, `PubSub`, `Presence`, `Deadline`, `ReceiveTimeout`, `OneShot`, `Periodic`, `Lease`, `Cache`, `Workflow`, `Barrier`, `Latch`, `Health`, `Readiness`, `Configuration` | Implement here only as passive folds with complete concrete sums and named products. Runtime mechanisms named by a fold remain outside it. |
| **2. Composition or named specialization** | `Stateful`, `Server`, `Protocol`, `Handler`, `Forwarder`, `Adapter`, `Spawner`, `Guardian`, `DynamicSupervisor`, `BackoffSupervise`, `RestartLimiter`, `Link`, `LifecyclePublisher`, `TerminationMonitor`, `Reaper`, `ShutdownCoordinator`, `TreeShutdown`, `ScatterGather`, `Dispatcher`, `LoadBalancer`, `Broker`, `Aggregator`, `DeadLetters`, `Retry`, `Debouncer`, `Throttler`, `Bulkhead`, `Receptionist`, `Directory`, `EventBus`, `ObserverHub`, `RetrySchedule`, `Heartbeat`, `IdlePassivation`, `Process`, `Batch`, `Rendezvous`, `Source`, `Processor`, `Sink`, `FanOut`, `FanIn`, `Saga`, `Orchestrator`, `Diagnostics`, `Features`, `NodeGuardian`, `Downing`, `SplitBrain`, `Shards`, `ShardRegion`, `Placement`, `Rebalance`, `Activation`, `EntityPassivation`, `Replicator` | Represent the formula over concrete templates and expose a distinct name only when it preserves every lane and adds a demonstrated semantic contract; a catalogue noun is not sufficient reason for a wrapper type. |
| **3. Runtime capability supplied by an owned Bombay component** | `Journal`, `Snapshots`, `Checkpoint`, `DiscoveryBridge`, `Metrics`, `Trace`, `Audit`, `Resources`, `Ingress`, `Egress` | Reuse Mnesis or a Bombay Environment adapter. A host-side policy may later get a distinct template name, but these capability nouns are not copied into Actors. |
| **4. Environment interpreter or Bombay System responsibility** | `Random`, `Reminder`, `EventSourced`, `DurableState`, `Recovery`, `Outbox`, `Inbox`, `Projection`, `Replicated`, `Membership`, `FailureDetector`, `Singleton`, `ShardProxy`, `Gateway` | No shipped template until the typed capability and end-to-end Driver path exist. |
| **5. Domain-specific behavior, not a catalogue implementation** | `Worker`, `Authorizer` | Bombay supplies composition slots and typed boundaries; the application owns these decisions and protocols. |

### Implemented module and verification ledger

This ledger is the synchronized source-to-façade map for shipped catalogue
roles. “Model/property” means an independent testkit vocabulary is compared
after every generated operation; “composition” means complete action products
are checked through wrapper initialization or transition routing. Items absent
from this ledger remain prospective even when named in the catalogue.

| Family / owning module | Implemented role | State and protocol owner | Named effects / errors | Runtime capability | Current verification | Proposed Bombay exposure |
|---|---|---|---|---|---|---|
| `composition` | `Activate` / `Active` / concrete wrapper constructors | direct consuming initialization and explicit construction over routed concrete event sums | `Initialized`; concrete nested errors | universal Driver only | unit, composition, compile-fail, properties, adapter-contract tests | `bombay::behavior::{Activate, Active}` and `bombay::actors` wrapper types |
| `composition` | `MessageAdapter` | one function-pointer mapping from an input protocol to a concrete destination protocol | exactly one ordinary typed delivery; no custom effect lane | universal Driver and Communication | unit, concrete recursive supervisor/pool roots, recursive compile-time matrix for all reply templates | `bombay::actors::MessageAdapter` |
| `composition` | `supervised_backoff` / `coordinated_terminal_application` | exact existing concrete stacks with one owner-defined wrapper order and every policy input explicit | existing named lanes and errors only; no recipe behavior or aggregate error | existing supervision, lifecycle, and timer templates | exact-type assertions and differential initialization/transition traces | `bombay::actors::{supervised_backoff, coordinated_terminal_application}` |
| `composition` | `Machine` | `Move` and user state/event types | domain behavior actions/errors unchanged | universal Driver only | unit, exhaustive FSM, properties, fuzz | `bombay::behavior::{Machine, Move}` |
| `composition` | `Stash` | `StashRoute`, retained FIFO | `StashStatus`; inner products preserved | universal Driver only | unit, model, exhaustive, properties, fuzz | `bombay::actors::Stash` |
| `lifecycle` | `Guardian` | application/subtree boundary over the wrapped initialization and event contract | inner products preserved; normal shutdown adds no effects | universal Driver activation, shutdown delivery and retirement | unit, error-path, composition-order, compile-fail | `bombay::actors::Guardian` |
| `lifecycle` | `TerminationMonitor` | one exact peer observation with explicit awaiting-or-consumed phase | `SendLayer<InterpreterRequests<ObservePeer<_>>, _>`; complete reaction actions preserved | Observe | unit over matching, unrelated, duplicate and rejected reactions | `bombay::actors::{TerminationMonitor, TerminationObservation}` |
| `lifecycle` | `PropagateTermination` | one statically selected child or peer terminal fact with explicit discharge-or-propagate phase | named observation and exact `ReportTerminalOutcome` lanes; inner products preserved | Observe plus terminal publication | exhaustive terminal variants, independent sequence model/property, duplicate/unmatched facts, initialization, peer/child targets, compile-fail target identity, interpreter multiplicity and reference application | `bombay::actors::{PropagateTermination, ChildTermination, PeerTermination}` |
| `lifecycle` | `Watch` / `Link` specialization | `WatchEvent`, exact peer lifecycle protocol; reciprocity is two endpoint compositions | one structural observation lane, `LinkReaction` | Observe | unit, composition, lifecycle model | `bombay::actors::{Watch, Link, LinkReaction}` |
| `lifecycle` | `ShutdownCoordinator` / `TreeShutdown` | validated phases or acyclic dependency topology and exhaustive running/stopping/completed state | structural child-shutdown request lane, typed plan/tree/rejection errors | child shutdown request and exact `ChildStopped` facts | unit over validation, all phases, duplicates, stale facts, empty topology and atomic rejection | `bombay::actors::{ShutdownCoordinator, TreeShutdown, ShutdownTree}` |
| `lifecycle` | shutdown policies | internal `ShutdownEvent`, request ownership | `ShutdownReaction`; public protocol and inner lanes preserved | System shutdown delivery | unit, independent model, composition | `bombay::actors::{StopOnShutdown, FinalizeOnShutdown}` |
| `lifecycle::task` | `Task` | `TaskState`, `TaskMessage`, `TaskResult` | typed `TaskError`, terminal result delivery | Observe terminal publication | unit including completion/cancellation/post-terminal input | `bombay::actors::Task` |
| `supervision` | `Proxy` | explicit incarnation and replacement provenance | `ProxySends`, `ProxyError` | fresh installation, Observe | unit, model, exhaustive, properties, fuzz | `bombay::actors::Proxy` |
| `supervision` | `Supervisor` / `Supervise` | one shared fleet/incarnation/restart-budget ownership sum; standalone nominal protocol or composition around a genuinely child-creating application behavior | `SupervisorSends`, `SupervisorError` / `SuperviseError`, `SupervisionFailure` | fresh installation, Observe | unit, model, exhaustive, properties, fuzz, adapter contract, standalone/composed parity | `bombay::actors::{Supervisor, Supervise, ChildTopology, RestartConfiguration}` |
| `supervision` | `BackoffSupervisor` / `BackoffSupervise` | one shared checked-attempt, timer-generation and pending-replacement-batch fold over standalone or composed fixed supervision | `BackoffSupervisorSends`, typed configuration/overflow/collision errors | Timers, fresh installation, Observe | unit over policy bounds, delayed release, stale/colliding timers, repeated failures and standalone parity | `bombay::actors::{BackoffSupervisor, BackoffSupervise, Backoff}` |
| `supervision` | `DynamicSupervisor` | explicit installing/available/stopping/replacing/retired stable-proxy slots and typed management protocol | `DynamicSupervisorSends`, ownership-preserving admission and realization outcomes | fresh installation, child shutdown, Observe | unit over admission, duplicate ownership, committed creation, replacement and terminal facts | `bombay::actors::{DynamicSupervisor, DynamicChildPhase}` |
| `supervision` | `WorkerPool` / `KeyedWorkerPool` | assignment, worker phase, interruption policy, keyed events, `PoolConfiguration` | `PoolActions`, `PoolSends`, `PoolError`, `PoolRejection` | fresh installation, Observe | unit, independent ownership model, properties, fuzz, adapter contract | `bombay::actors::{Pool, KeyedPool, PoolConfiguration}` |
| `routing::router` | `Router<RoundRobin>` / `Router<Broadcast>` / `Router<LeastLoaded<_>>` / keyed consistent and rendezvous hash policies | ordered recipient membership; strategy-indexed observations; versioned load or stable-token evidence | ordinary typed deliveries; `RouterError` returns unroutable payload or typed policy rejection | Address and Communication; System supplies load/token evidence | unit over rotation, broadcast, load boundaries, stable hash determinism, evidence rejection, and removal remapping law | `bombay::actors::{Router, RoundRobin, Broadcast, LeastLoaded, ConsistentHash, RendezvousHash}` |
| `routing::correlator` | `Correlator` | `CorrelationState`, `CorrelatorMessage` | `CorrelationResult`, `CorrelatorError` | ordinary typed delivery | unit over resolve/cancel/unknown/stale paths | `bombay::actors::Correlator` |
| `routing::acknowledgements` | `Acknowledgements` | `AcknowledgementState`, record and operation sums | `AcknowledgementOutcome`, `AcknowledgementError` | ordinary typed delivery | unit over normalization, completion, cancellation, stale input | `bombay::actors::Acknowledgements` |
| `routing::buffer` | `Buffer` | `BufferState`, `OverflowPolicy`, `BufferMessage` | `BufferSends`, `BufferOutcome`, `BufferConfigError` | Communication for physical delivery/backpressure | exhaustive overflow and FIFO unit cases | `bombay::actors::Buffer` |
| `routing::sequencer` | `Sequencer` | `Sequence`, `SequencerState`, offer protocol | `DeliveryOutcomes`, ownership-preserving outcomes | ordinary typed delivery | unit, independent model/property sequences | `bombay::actors::Sequencer` |
| `routing::order_gate` | `OrderGate` | monotonic watermark plus `BTreeMap` holds | `DeliveryOutcomes`, ownership-preserving outcomes | ordinary typed delivery | unit, independent model/property sequences | `bombay::actors::OrderGate` |
| `routing::deduplicator` | `Deduplicator` | bounded FIFO retained-key window | `DeliveryOutcomes`, outcomes, config error | ordinary typed delivery | unit, independent model/property sequences | `bombay::actors::Deduplicator` |
| `routing::work_queue` | `WorkQueue` | bounded FIFO waiting values and one-use available-worker capabilities | `WorkQueueSends`, ownership-preserving admission outcomes | Communication for physical delivery/backpressure | unit over worker/value FIFO and zero-capacity rejection | `bombay::actors::WorkQueue` |
| `routing::priority_queue` | `PriorityQueue` | positive capacity and explicit active-or-exhausted stable-order phase | `DeliveryOutcomes`, ownership-preserving full/exhaustion outcomes, config error | Communication for physical delivery/backpressure | unit over priority order, FIFO ties, full/empty and token exhaustion | `bombay::actors::PriorityQueue` |
| `routing::circuit_breaker` | `CircuitBreaker` | exhaustive closed idle/awaiting, open, probing available/awaiting, and exhausted sums with explicit attempt and timer generations | `BreakerSends`, `BreakerOutcome`, typed configuration and admission rejection sums | Timers for reset evidence; protected operations remain domain behavior | unit over threshold opening, denial, matching reset, single probe, stale attempts and stale timers | `bombay::actors::CircuitBreaker` |
| `routing::rate_limiter` | `RateLimiter` | positive capacity and explicit available-token product | `DeliveryOutcomes`, ownership-preserving capacity/availability outcomes, config error | typed refill events from Timers/System | unit over admission, both rejection classes, saturating refill/overflow | `bombay::actors::RateLimiter` |
| `discovery::registry` | `Registry` | ordered typed bindings and mutation/lookup protocol | `RegistryResult`, `RegistryError` | Address for endpoint realization | unit over conflicts, stale unbind, found/missing | `bombay::actors::Registry` |
| `discovery::resolver` | `Resolver` | immutable unique typed bindings and read-only lookup protocol | `Resolution`, borrowed-definition `ResolverConfigError` | Address for endpoint realization | unit over duplicate definition, found/missing, immutable protocol | `bombay::actors::Resolver` |
| `discovery::topic` | `Topic` | ordered subscription membership and publication protocol | publication deliveries, `TopicError` returns undelivered value | Communication | unit over idempotence/order/empty publication | `bombay::actors::Topic` |
| `discovery::pub_sub` | `PubSub` | keyed retained topic membership and publication protocol | ordered publication deliveries; `PubSubError` returns undelivered value | Communication | unit over idempotence/order/known-empty/unknown publication | `bombay::actors::PubSub` |
| `discovery::presence` | `Presence` | versioned present-or-expired participant sum with explicit timer generation and retained tombstones | `PresenceSends`, `PresenceOutcome`, typed stale/conflict/collision/exhaustion errors | Timers for expiry evidence | unit over refresh, idempotence, expiry, stale evidence, live timer collision and generation exhaustion | `bombay::actors::Presence` |
| `time` | `Deadline` / `ReceiveTimeout` | explicit timer generations and closed timed event sums | structural timer-request lane; inner errors | Timers | unit, composition, independent model, properties, fuzz | `bombay::actors::time::{Deadline, ReceiveTimeout}` |
| `time` | `OneShot` / `Periodic` | generation-safe timer leases and timed event sums | structural timer-request lane; inner errors | Timers | unit over initialization, wrong/stale generation, rearming | `bombay::actors::time::{OneShot, Periodic}` |
| `time::lease` | `Lease` | explicit vacant, held-with-provenance, or exhausted ownership sum | `LeaseSends`, concrete acquisition/renewal/release/expiry outcomes and rejections | Timers schedule lane; cancellation adapter gap documented | unit over acquire/renew/release, wrong holder, stale expiry, matching expiry, generation exhaustion | `bombay::actors::time::Lease` |
| `persistence::cache` | `Cache` | positive capacity and deterministic recency state | ownership-returning `CacheResult`, `CacheConfigError` | none; not a durability subsystem | unit over hit/replacement/eviction/removal/zero capacity | `bombay::actors::Cache` |
| `workflow::latch` | `Latch` | `LatchState::{Counting, Released}` | typed `LatchReleased` delivery | ordinary typed delivery | unit over zero, countdown, duplicate terminal input | `bombay::actors::Latch` |
| `workflow::barrier` | `Barrier` | explicit generations, fixed membership and exhaustion sum | `BarrierReleased`, `BarrierError`, config error | ordinary typed delivery | unit/exhaustive boundaries over stale/future/duplicate/exhaustion | `bombay::actors::Barrier` |
| `workflow::coordinator` | `Workflow` | validated finite DAG and ready/running/succeeded/failed/cancelled sums with per-step blocked/active/completed state | `WorkflowOutcome`, `WorkflowRejection`, exhaustive graph configuration errors | participant execution is domain/System work; Mnesis owns durable saga state | unit over graph rejection, diamond activation order, blocked failure and duplicate completion | `bombay::actors::Workflow` |
| `operations::health` | `Health` | versioned present/tombstone component sum | `HealthReport`, `HealthError` | System health exporter only | unit over stale/conflict/removal/worst status | `bombay::actors::Health` |
| `operations::readiness` | `Readiness` | fixed dependencies and `Unknown | Observed` evidence sum | `ReadinessReport`, `ReadinessError` | System readiness exporter only | unit over all-ready/unknown/stale/conflict/empty set | `bombay::actors::Readiness` |
| `operations::configuration` | `Configuration` | `Unconfigured | Configured { version, value }` | ownership-preserving `ConfigurationError`, typed query | external source adapter only | unit over initial/query/idempotent/stale/conflict | `bombay::actors::Configuration` |
| `operations::features` | `Features = Configuration<FeatureSet<_>>` | duplicate-free explicit `FeatureStatus` product | inherits configuration products/errors | external feature source adapter only | normalization unit and compile-time universal-Driver contract | `bombay::actors::Features` |

## Naming law

Catalogue types use role nouns and omit a redundant `Actor` suffix:

- `Registry`, not `RegistryActor`;
- `Router<RoundRobin>`, not `RoundRobinRouterActor`;
- `Retry<ExponentialBackoff>`, not `RetryBehaviorWrapper`;
- `Supervisor`, not `SupervisorManager`;
- `Shards`, not `ShardManagementSystem`.

The role nouns have distinct meanings:

| Name | Meaning |
|---|---|
| `Manager` | Owns and administers a concrete collection of resources. |
| `Coordinator` | Participates in and advances a multi-party protocol. |
| `Supervisor` | Observes descendants and makes typed lifecycle decisions. |
| `Registry` | Binds typed names or keys to recipients. |
| `Directory` | Resolves placement or location, potentially across nodes. |
| `Broker` | Correlates requests with replies. |
| `Router` | Selects one or more recipients. |
| `Dispatcher` | Classifies communications and delegates by protocol. |
| `Proxy` | Preserves a stable externally visible endpoint. |
| `Adapter` | Transforms one statically known protocol into another. |
| `Monitor` | Observes and reports without owning recovery policy. |
| `Guardian` | Owns the lifecycle boundary of a subtree or system. |

Avoid vague architectural names such as `Core`, `Base`, `Common`, `Util`,
`Engine`, and `Service`. Strategy types use their actual policy names, such as
`RoundRobin`, `Broadcast`, `LeastLoaded`, `ConsistentHash`, and
`RendezvousHash`.

In the prospective tables below, a canonical type names the template produced,
not a process that already runs. “Composition” describes its pure fold and
policy. **Actors** means `bombay-behavior-actors`, **Bombay runtime** means the interpreter in the
[Bombay repository](https://github.com/devrandom-labs/bombay), and
**Application** means a user domain. “None” in the dependency column means that
the standard library and Bombay algebra are sufficient. The dependency column
identifies either a pure implementation component for the template owner or an
adapter dependency for the runtime owner; it never authorizes effects inside a
behavior fold. Consult the implementation inventory before treating a row as
shipped.

## Fundamental compositions

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Stateful<S>` | State plus a total behavior transition | Actors | None |
| `Machine<S>` | Finite state plus typed events | Actors | None |
| `Server<P>` | Request protocol plus stateful behavior | Actors | None |
| `Protocol<P>` | Typestate protocol phases | Actors | None |
| `Handler<E>` | Event lane plus behavior | Actors | None |
| `Forwarder<P>` | Recipient plus forwarding behavior | Actors | None |
| `MessageAdapter<In, Destination>` | A nominal actor protocol that maps one public input message algebra to one destination protocol and emits one ordinary typed delivery | Actors | None |
| `Spawner<C>` | Behavior plus staged fresh child creation | Actors | None |
| `Proxy<P>` | Stable endpoint plus current recipient | Actors | None |

These are direct algebraic constructions. Bombay supplies their one Driver,
mailbox, lifecycle, identity, scheduling, and dispatch architecture; a template
must not import a second implementation of those semantics.

## Lifecycle and supervision

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Guardian<C>` | Child ownership plus shutdown policy | Actors | None |
| `Supervisor<A, C>` | Standalone observation, recovery policy and stable-proxy fleet ownership | Actors | None |
| `Supervise<B, C>` | The same ownership fold composed with a genuinely child-creating application behavior | Actors | None |
| `DynamicSupervisor<A, C, Reply>` | Explicit typed dynamic stable-child set with command admission separated from runtime realization | Actors | None |
| `BackoffSupervisor<A, C>` / `BackoffSupervise<B, C>` | Standalone or composed fixed supervision plus the same timer/backoff fold | Actors | None; define policy locally |
| `RestartLimiter<W>` | Restart history plus a typed observation window | Actors | None |
| `Worker<P>` | Concrete application protocol implementation | Application | None |
| `Pool<W, R>` | Workers plus routing policy | Actors | Optional `indexmap` |
| `KeyedPool<K, W>` | Keyed children plus routing | Actors | Optional `indexmap` |
| `Task<R>` | One-result child lifecycle | Actors | None |
| `Watch<T>` | Observation request plus lifecycle events | Actors | None |
| `Link<T>` | Named `Watch<T>` specialization; mutual policy is the same composition installed at both endpoints | Actors | None |
| `LifecyclePublisher<T>` | Lifecycle events plus subscribers | Actors | None |
| `TerminationMonitor<T>` | Watch plus termination classification | Actors | None |
| `Reaper<T>` | Monitor plus cleanup communications | Actors | None |
| `ShutdownCoordinator<B, C>` | Homogeneous typed-child shutdown phases plus acknowledgements | Actors | None |
| `HeterogeneousShutdownCoordinator<B, T>` | Arbitrary closed typed-child shutdown phases with ordered static request dispatch | Actors | None |
| `TreeShutdown<B, C>` | Homogeneous typed-child topology plus ordered shutdown | Actors | `petgraph` only for a dependency graph |

Backoff is small and semantically observable, so Bombay should define its own
exhaustive policy instead of importing the assumptions of an asynchronous
retry library:

```rust,ignore
enum Backoff {
    Constant {
        delay: Duration,
    },
    Linear {
        initial: Duration,
        step: Duration,
        maximum: Duration,
    },
    Exponential {
        initial: Duration,
        factor: NonZeroU32,
        maximum: Duration,
    },
}
```

The behavior computes the next requested delay. The Bombay runtime owns the clock and
reports the resulting typed timer event.

## Routing and delivery

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Router<R>` | Recipient set plus selection strategy | Actors | Collection only when justified |
| `RoundRobin` | Deterministic rotating selection | Actors | None |
| `Broadcast` | Selection of every eligible recipient | Actors | None |
| `Random` | Selection policy plus explicit entropy observation | Actors / Bombay runtime | Runtime RNG only |
| `LeastLoaded` | Selection policy plus typed load observations | Actors | None |
| `ConsistentHash<K>` | Stable ring plus recipient membership | Actors | Prefer a reviewed local algorithm |
| `RendezvousHash<K>` | Highest-random-weight selection | Actors | Prefer a reviewed local algorithm |
| `ScatterGather<R>` | Broadcast plus correlation plus deadline | Actors | None |
| `Dispatcher<C>` | Typed classification plus delegation | Actors | None |
| `LoadBalancer<R>` | Router plus load observations | Actors | None |
| `WorkQueue<W>` | FIFO queue plus worker availability | Actors | `VecDeque` |
| `PriorityQueue<W, P>` | Typed priority plus queue | Actors | Optional `priority-queue` |
| `Broker<Req, Res>` | Request identifiers plus recipients plus correlation | Actors | None |
| `Correlator<K, V>` | Pending keys plus reply matching | Actors | Optional `indexmap` |
| `Aggregator<K, V>` | Correlation plus a completion policy | Actors | None |
| `Acknowledgements<K>` | Delivery state plus acknowledgement events | Actors | None |
| `Retry<P>` | Attempt state plus timer effects plus retry policy | Actors | None |
| `Deduplicator<K>` | Seen keys plus explicit retention policy | Actors | Optional `lru` |
| `Sequencer<K>` | Expected sequence plus buffered inputs | Actors | `BTreeMap` |
| `OrderGate<K>` | Ordering rule plus typed release | Actors | `BTreeMap` |
| `DeadLetters<P>` | Rejected or unroutable communications | Actors | None |
| `Stash<P>` | Deferred communications plus release policy | Actors | None; existing implementation |
| `Buffer<P>` | Bounded FIFO plus exhaustive overflow policy | Actors | `VecDeque` |
| `CircuitBreaker<P>` | Exhaustive closed, open, and probing states | Actors | None |
| `RateLimiter<P>` | Explicit tokens plus typed clock events | Actors | None |
| `Debouncer<P>` | Latest input plus timer effects | Actors | None |
| `Throttler<P>` | Queue plus explicit release events | Actors | None |
| `Bulkhead<P>` | Bounded permits plus queue | Actors | None |

Use [`indexmap`](https://docs.rs/indexmap/latest/indexmap/) only when stable
insertion order is observable or required for deterministic policy. Use
[`lru`](https://docs.rs/lru/latest/lru/) for explicitly bounded retention. Use
[`priority-queue`](https://docs.rs/priority-queue/latest/priority_queue/) only
when an item's priority must change after insertion. Otherwise prefer
`VecDeque`, `BTreeMap`, `BinaryHeap`, and `HashMap` from the standard library.

The currently surveyed consistent-hash and rendezvous-hash crates are small but
comparatively old and have unclear maintenance or MSRV commitments. Both
algorithms are compact enough that a local, documented implementation with an
independent model and property tests is safer than inheriting hidden policy.

## Discovery and messaging fabric

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Registry<K, P>` | Typed key bindings plus recipients | Actors | Optional `indexmap` |
| `Receptionist<P>` | Registry plus subscriptions | Actors | None |
| `Resolver<K, P>` | Lookup protocol plus typed failures | Actors | None |
| `Directory<K, P>` | Resolver plus placement or location data | Actors | None |
| `Topic<P>` | Subscribers plus publication behavior | Actors | Optional `indexmap` |
| `PubSub<P>` | Topics plus subscriptions | Actors | None |
| `EventBus<E>` | Closed event sum plus subscribers | Actors | None |
| `ObserverHub<E>` | Statically typed observation lanes | Actors | None |
| `Presence<K>` | Membership announcements plus expiry events | Actors | None |
| `DiscoveryBridge<P>` | External discovery observations into a typed protocol | Bombay runtime | Adapter-specific |

Discovery adapters may use
[`hickory-resolver`](https://docs.rs/hickory-resolver/latest/hickory_resolver/)
for DNS, [`mdns-sd`](https://docs.rs/mdns-sd/latest/mdns_sd/) for local-network
discovery, [`kube`](https://docs.rs/kube/latest/kube/) for Kubernetes, or
[`libp2p`](https://docs.rs/libp2p/latest/libp2p/) when Bombay deliberately
adopts peer-to-peer discovery. None belongs in the behavior-template crate.

## Time-derived templates

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Deadline<P>` | Inner behavior plus an absolute deadline event | Actors | None |
| `ReceiveTimeout<P>` | Activity observations plus a timer request | Actors | None |
| `OneShot<P>` | Timer request plus one typed event | Actors | None |
| `Periodic<P>` | Repeated typed timer observations | Actors | None |
| `Reminder<K, P>` | Named durable schedule protocol | Actors / Bombay runtime | Storage adapter |
| `RetrySchedule<P>` | Retry state plus timers | Actors | None |
| `Heartbeat<P>` | Periodic send plus peer observation | Actors | None |
| `Lease<K>` | Ownership state plus expiry events | Actors | None |
| `IdlePassivation<P>` | Inactivity observations plus lifecycle decision | Actors | None |

Clock ownership, sleeping, and timer execution remain in the Bombay runtime and
reuse `bombay-timers`. No template or interpreter should introduce a second
timer queue.

## Persistence-derived templates

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `EventSourced<S, E>` | Mnesis aggregate execution plus Bombay entity host | Actors / `mnesis-bombay` | Mnesis repository |
| `DurableState<S>` | Behavior policy plus versioned Mnesis state boundary | Actors / `mnesis-bombay` | Mnesis snapshot/state capability |
| `Recovery<S>` | Mnesis replay plus typed actor initialization phase | Actors / `mnesis-bombay` | Mnesis repository |
| `Journal<E>` | Append and read protocol | Mnesis | `mnesis-store` adapter |
| `Snapshots<S>` | Snapshot protocol plus recovery boundary | Mnesis | Mnesis `SnapshotStore` adapter |
| `Outbox<M>` | Durable publication state plus committed-log delivery | Actors / `mnesis-bombay` | Mnesis store capability; integration incomplete |
| `Inbox<K, M>` | Durable deduplication state | Mnesis / `mnesis-bombay` | Atomic inbox capability not yet shipped |
| `Projection<E, S>` | Mnesis projection fold plus subscription, checkpoint, and Bombay host | Actors / `mnesis-bombay` | Mnesis projection and snapshot capabilities |
| `Checkpoint<K>` | Mnesis position plus durable state commit protocol | Mnesis / `mnesis-bombay` | Mnesis `SnapshotStore` |
| `Cache<K, V>` | Behavior plus explicit eviction policy | Actors | Optional `lru` |
| `Replicated<S, D>` | Behavior plus a typed replicated delta | Actors | Optional `crdts` |

Bombay must reuse [Mnesis](https://github.com/devrandom-labs/mnesis) rather than
introducing a second persistence abstraction. Mnesis already owns event-sourced
aggregates, optimistic append, repositories, event streams, subscriptions,
snapshots, projections, saga state and projected intents, import/export, wire
formats, and store conformance. Its shipped adapters cover in-memory, embedded
Fjall, and PostgreSQL storage. `mnesis-bombay` is the integration boundary for
hosting those durable facts in Bombay; a behavior template emits and consumes
typed integration protocols and never performs repository I/O itself.

[`crdts`](https://docs.rs/crdts/latest/crdts/) is a candidate algorithm crate
for replicated templates when used with default features disabled and concrete
delta types kept visible. Mnesis remains Bombay's owned durability,
event-sourcing, projection, and saga substrate.

## Workflow and stream compositions

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Process<K, S>` | Correlated state machine | Actors | None |
| `Saga<S>` | Process plus compensation protocol | Actors | Optional `petgraph` |
| `Workflow<S>` | Dependency graph plus activation and completion events | Actors | Standard-library finite-graph scan; `petgraph` rejected because immutable one-run validation does not justify a public or dependency boundary |
| `Orchestrator<S>` | Workflow plus participant commands | Actors | Optional `petgraph` |
| `Barrier<K>` | Participant set plus release condition | Actors | Optional `fixedbitset` |
| `Latch` | Remaining count plus release | Actors | None |
| `Rendezvous<K>` | Participant arrivals plus generation | Actors | None |
| `Batch<P>` | Buffer plus size or deadline release | Actors | None |
| `Source<P>` | Demand observations plus sends | Actors | None |
| `Processor<I, O>` | Typed transformation plus demand | Actors | None |
| `Sink<P>` | Demand plus terminal observation | Actors | None |
| `FanOut<P>` | Source plus router | Actors | None |
| `FanIn<P>` | Sources plus aggregation | Actors | None |

Use [`petgraph`](https://docs.rs/petgraph/latest/petgraph/) for real dependency
graphs, cycle detection, topological traversal, and placement analysis. Do not
introduce it merely to represent the ordinary actor parent-child tree.
`fixedbitset` is appropriate only when barrier membership is fixed, dense, and
the representation materially improves a demonstrated workload.

## Cluster templates

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `NodeGuardian` | Node lifecycle plus system children | Actors | None |
| `Membership<N>` | Typed membership observations | Actors / Bombay runtime | Optional `foca` adapter |
| `FailureDetector<N>` | Heartbeat observations plus suspicion policy | Actors / Bombay runtime | Optional `foca` adapter |
| `Downing<N>` | Membership evidence plus downing policy | Actors | None |
| `SplitBrain<N>` | Partition evidence plus resolution policy | Actors | None |
| `Singleton<P>` | Lease or leadership plus stable proxy | Actors / Bombay runtime | Consensus adapter |
| `Shards<K, P>` | Directory plus placement plus activation | Actors | None |
| `ShardRegion<K, P>` | Local entities plus routing | Actors | None |
| `ShardProxy<K, P>` | Directory plus remote routing effects | Actors / Bombay runtime | Transport adapter |
| `Placement<K, N>` | Membership plus placement strategy | Actors | Optional `petgraph` |
| `Rebalance<K, N>` | Placement delta plus migration protocol | Actors | None |
| `Activation<K, P>` | Directory plus staged fresh creation | Actors | None |
| `EntityPassivation<K>` | Inactivity plus directory update | Actors | None |
| `Replicator<D>` | Delta protocol plus peer set | Actors | Optional `crdts` |

[`foca`](https://docs.rs/foca/latest/foca/) is the strongest surveyed algorithm
candidate for a future Bombay-owned membership subsystem because it provides a
transport-agnostic SWIM implementation. Its outputs must be translated into
concrete `Membership` and `FailureDetector` protocol events; it does not own
Bombay cluster semantics.

Distributed leadership and durable shard coordination are not implicit parts
of `Singleton` or `Shards`. If required, an explicitly optional Bombay runtime
subsystem may evaluate [`openraft`](https://docs.rs/openraft/latest/openraft/).
The stable release line, feature surface, and Rust-version requirement must be
reviewed at adoption time rather than accepting a pre-release automatically.

## Operational boundary templates

These actors translate typed application or runtime observations. The external
systems that produce or consume those observations stay in the Bombay runtime.

| Canonical type | Composition | Owner | External dependency |
|---|---|---|---|
| `Health` | Typed component observations plus status policy | Actors | None |
| `Readiness` | Dependency observations plus readiness policy | Actors | None |
| `Metrics` | Typed metric events plus export effects | Actors / Bombay runtime | `metrics` or `prometheus-client` adapter |
| `Trace` | Typed trace events plus export effects | Actors / Bombay runtime | `tracing`, optional `opentelemetry` |
| `Audit` | Typed security/domain events plus durable export | Actors / Bombay runtime | Storage adapter |
| `Diagnostics` | Typed diagnostic requests plus reports | Actors | None |
| `Configuration<C>` | Versioned configuration protocol | Actors | None |
| `Features<F>` | Closed feature-state protocol | Actors | None |
| `Resources<R>` | Resource ownership states plus interpreter results | Actors / Bombay runtime | Capability-specific |
| `Authorizer<P>` | Concrete authorization protocol and decisions | Application | Security adapter |
| `Gateway<In, Out>` | External boundary translation plus an application public-protocol recipient | Bombay runtime | `tower`, `tonic`, or HTTP stack |
| `Ingress<P>` | Decoded external input plus typed delivery | Bombay runtime | Codec and transport adapter |
| `Egress<P>` | Typed communication plus external encoding | Bombay runtime | Codec and transport adapter |

## Bombay runtime capability matrix

The following facilities are deliberately not behavior templates. Templates
declare their required capabilities as typed events and effects; Bombay and its
driver realize them. Prefer the existing Bombay primitive shown here before
evaluating another implementation crate:

| Interpreter capability | Existing Bombay realization / candidate adapter | Constraint |
|---|---|---|
| Behavior driving and action interpretation | `bombay-engine::Driver` | One event invokes one fold; effects remain explicit and are interpreted at the boundary. |
| Mailboxes, lane priority, fairness, and physical backpressure | `bombay-communication` | Never expose the channel as a public actor protocol or reproduce mailbox mechanics in a policy template. |
| Endpoint registration and resolution | `bombay-address` | Registration authority and generation safety remain runtime facts. |
| Completion and lifecycle observation | `bombay-observe` | Translate publications into concrete typed observation events. |
| Monotonic keyed timers | `bombay-timers` | Runtime owns clocks and sleeping; behavior owns timeout policy. |
| Local entity activation and passivation | `bombay-entity` | Specialized local subsystem, not distributed sharding or discovery. |
| Ordered machine execution | `bombay-machine-executor` | Do not reproduce scheduling or concurrency inside a fold. |
| Async task execution | `tokio` in Bombay | Runtime only; behavior receives typed observations. |
| Cluster membership | `foca` | Translate SWIM output into Bombay protocols. |
| Event sourcing and persistence | Mnesis (`mnesis`, `mnesis-store`, its adapters, and conformance kit) | Mnesis owns durable truth, append positions, subscriptions, snapshots, projections, sagas, codecs, and store contracts. Do not create a competing Bombay persistence layer. |
| Mnesis command execution | `mnesis-bombay-core` and `mnesis-bombay-execution` | Preserve factual durable outcomes, conflict policy, command phase, and ambiguity without importing a runtime into the core contract. |
| Bombay-hosted durable entities | `mnesis-bombay` over `bombay-entity` and Bombay | The typed routing seam exists; the complete aggregate host, relay, projection, saga, and assembly topology remains staged integration work. |
| CESR framing and KERI identity protocols | Owned CESR/KERI crates: `cesr-rs`, `cesr-stream`, `keri-events`, `keri-codec`, and `keri-rs` | Use where the concrete boundary protocol is CESR/KERI; never as a universal internal envelope. |
| Other application serialization | Mnesis codecs or application-selected `serde`, `postcard`, `prost`, or `rkyv` adapters | Boundary only; never an internal untyped envelope. |
| Transport | Future Bombay transport using `quinn`, `rustls`, and `bytes` where appropriate | Preserve concrete protocol, KERI/authentication evidence where selected, and capability checks at the adapter. |
| External gRPC APIs | `tonic` | Gateway/admin boundary, not the actor wire core. |
| Service discovery | `hickory-resolver`, `mdns-sd`, `kube`, optional `libp2p` | Produce typed discovery observations. |
| Tracing | `tracing`, optional `opentelemetry` | No ambient semantic side channel in behavior. |
| Metrics | `metrics` or `prometheus-client` | Export interpreted observations. |
| HTTP/gRPC middleware | `tower` | External gateways only. |
| OS capabilities | `cap-std` | Preserve explicit authority. |
| Secret material | `secrecy` | Do not turn secret exposure into an actor capability lookup. |

`slotmap`, `generational-arena`, UUIDs, and ULIDs may be useful runtime
implementation details or correlation identifiers. They are not actor
identities and never prove fresh allocation. A nonce remains a creator-local
routing and correlation key; only successful interpreter installation commits
a fresh birth.

## Existing Bombay implementation inventory

This inventory records the local Bombay repositories audited on 2026-08-15.
It distinguishes a catalogue template from a specialized subsystem and from an
interpreter primitive. A primitive can make a template inexpensive to
implement, but it is not itself the template's typed protocol and pure behavior
fold.

| Repository / crate | What already exists | Catalogue coverage | Classification and reuse decision |
|---|---|---|---|
| [`bombay-behavior-actors`](../crates/actors/) | Concrete composition, lifecycle, routing, discovery, timing, persistence-policy, workflow, and operational folds listed below | `Machine`, `Stash`, `Watch`, `Task`, `Deadline`, `ReceiveTimeout`, `OneShot`, `Periodic`, `Lease`, shutdown, `Proxy`, `Supervisor`, `Pool`, `KeyedPool`, `Router<RoundRobin>`, `Router<Broadcast>`, `Router<LeastLoaded<_>>`, keyed consistent/rendezvous routers, `WorkQueue`, `PriorityQueue`, `CircuitBreaker`, `RateLimiter`, `Correlator`, `Acknowledgements`, `Buffer`, `Sequencer`, `OrderGate`, `Deduplicator`, `Registry`, `Resolver`, `Topic`, `PubSub`, `Presence`, `Cache`, `Workflow`, `Latch`, `Barrier`, `Health`, `Readiness`, `Configuration`, and `Features` | **Behavior templates and named specializations.** Extend these concrete, statically dispatched folds instead of recreating them. |
| [`bombay-transition`](https://github.com/devrandom-labs/bombay-entity/tree/main/crates/transition) | Deterministic `Reducer`/`Machine` transition calculus and concrete composition topology | Supports state-machine-derived actors | **Pure algorithmic subsystem.** Reuse where its different machine contract is appropriate; it does not replace a Bombay `Behavior`. |
| [`bombay-machine-executor`](https://github.com/devrandom-labs/bombay-entity/tree/main/crates/driver) | Exclusive, serialized, and linearized ordered machine execution | Execution support for machine-based constructions | **Runtime subsystem.** Do not reproduce execution or ordering inside an actor fold. |
| [`bombay-entity`](https://github.com/devrandom-labs/bombay-entity/tree/main/crates/entity) | Stable typed entity routing, single-flight local activation, generation-safe commit and retirement, bounded admission, passivation, and draining | Specialized `Directory`, `Activation`, and `EntityPassivation`; local support related to `Shards` | **Specialized subsystem.** Reuse it for local entities. It is not a general recipient directory, distributed sharding, persistence, or discovery actor. |
| [`bombay-address`](https://github.com/devrandom-labs/bombay-address) | Generation-safe exclusive endpoint registration and resolution with affine release authority | Primitives for `Registry`, `Resolver`, `Directory`, stable proxies, and routing | **Interpreter primitive.** Build typed actor protocols over it when subscriptions, policy, or actor-owned state are required. Its `Lease` is registration authority, not the time-derived catalogue `Lease<K>`. |
| [`bombay-observe`](https://github.com/devrandom-labs/bombay-observe) | Generation-safe, single-terminal publication with many observers, including pre-, during-, and post-completion observation and cancellation | Primitives for `Watch`, `Task`, `LifecyclePublisher`, and `TerminationMonitor` | **Interpreter primitive.** Reuse for completion and lifecycle realization; keep routing and policy in typed behaviors. |
| [`bombay-timers`](https://github.com/devrandom-labs/bombay-timers) | Keyed, generation-safe monotonic timer queue with replacement, cancellation, due extraction, and next-deadline calculation | Runtime support for `Deadline`, `ReceiveTimeout`, `OneShot`, `Periodic`, `RetrySchedule`, `Heartbeat`, time `Lease`, and `IdlePassivation` | **Interpreter primitive.** `Deadline`, `ReceiveTimeout`, `OneShot`, and `Periodic` wrappers exist; the remaining policies reuse this queue. Do not create another timer queue. |
| [`bombay-communication`](https://github.com/devrandom-labs/bombay-communication) (local checkout `fastpass`) | Bounded user lane, unbounded control lane, FIFO within each lane, control-first selection with an aging cap, physical producer backpressure, exact delivery ownership, and draining | Mailbox support related to `PriorityQueue`, `Buffer`, and `WorkQueue` | **Interpreter primitive.** Physical backpressure, mailbox priority, and fairness already exist. They are not policy actors or a general priority-queue behavior. |
| [`bombay-engine`](https://github.com/devrandom-labs/bombay/tree/main/crates/bombay-engine) | Actor-independent driver, environment boundary, explicit runtime effects, and run outcomes | Interpreter for pure transitions | **Runtime subsystem.** Actor folds must not duplicate its scheduling or effect execution. |
| [`bombay-rs`](https://github.com/devrandom-labs/bombay/tree/main/crates/bombay) | `System`, actor references and handles, address routing, mailbox adapters, fresh-child realization, timers, observation, shutdown, child retirement, stable-proxy supervision, and task completion | Realizes spawning, delivery, lifecycle publication, `Watch`, timer wrappers, shutdown, supervision, pools, and tasks | **Bombay interpreter.** Its address router and endpoint registry are runtime ports, not the configurable `Router` or discovery `Registry` behavior templates. |
| [`bombay-framework`](https://github.com/devrandom-labs/bombay/tree/main/crates/bombay-framework) | Application-facing facade, prelude, and examples | Demonstrates composition only | **Facade.** It contributes no additional semantic actor. |
| [Mnesis](https://github.com/devrandom-labs/mnesis) (local checkout `nexus`) | Pure aggregates and sagas; repositories and optimistic command execution; event streams and subscriptions; snapshots, projections, projected intents, import/export, codecs, wake sources, store conformance, and in-memory, Fjall, and PostgreSQL adapters | Durable substrate for `EventSourced`, `Recovery`, `Journal`, `Snapshots`, `Projection`, `Checkpoint`, saga/process-manager, inbox/outbox, and committed-event relay constructions | **Durability subsystem.** Reuse it as the durable authority. Its pure decisions and storage primitives are not actors, and Mnesis deliberately supplies no projection event-loop runner. |
| [`mnesis-bombay-core`](https://github.com/devrandom-labs/mnesis-bombay/tree/main/crates/core) | Runtime-neutral typed command identity, context, phase, interruption, addressing, and factual outcome vocabulary | Protocol basis for durable aggregate templates | **Integration protocol.** It is `no_std` and must remain independent of Bombay, transports, and concrete stores. |
| [`mnesis-bombay-execution`](https://github.com/devrandom-labs/mnesis-bombay/tree/main/crates/execution) | Runtime-independent Mnesis command execution, bounded conflict replay, commit-uncertainty classification, and observable command phase | Execution capability for durable aggregate templates | **Integration subsystem.** Reuse the same durable command path for direct and Bombay hosts; do not copy it into a behavior fold. |
| [`mnesis-bombay`](https://github.com/devrandom-labs/mnesis-bombay/tree/main/crates/bombay) | Typed aggregate-ID to `EntityId` routing and Bombay-owned reply envelope | Initial seam for actor-hosted aggregate execution | **Partial adapter.** The routing seam is implemented; aggregate hydration hosting, committed-log relay actors, projection actors, saga entity actors, effect delivery, and the application composition root remain roadmap work. |
| [CESR/KERI](https://github.com/devrandom-labs/cesr) (local checkout `cesr`) | CESR primitives and stream framing, canonical KERI event representation and codec, and a sans-I/O KERI key-state/verification fold | Concrete boundary support for `Ingress`, `Egress`, transport framing, verifiable identity evidence, and security-sensitive gateways | **Protocol subsystem.** Reuse it where the selected domain or wire protocol is CESR/KERI. Do not turn it into a universal actor envelope or move its pure protocol fold into the Driver. |

### Coverage by catalogue family

| Family | Implemented templates | Partly supplied by a sibling subsystem or primitive | Still absent as reusable templates |
|---|---|---|---|
| Fundamental and lifecycle | Machine composition, stash, `Guardian`, watch/`Link`, one-result `Task`, `TerminationMonitor` and terminal-propagation specializations, phased/tree shutdown, proxy, fixed and dynamic supervision, backoff supervision, worker pools, deadlines, receive timeouts | Task terminal publication, lifecycle publication, child shutdown, and independent structural observation through Observe and Bombay; restart budgeting within `Supervisor`; local entity activation and passivation | None within the implemented ledger; prospective lifecycle catalogue roles remain subject to their own capability audits |
| Routing and delivery | Round-robin, broadcast, versioned-load, stable-ring, and rendezvous router policies; bounded FIFO `WorkQueue`; stable bounded `PriorityQueue`; single-flight `CircuitBreaker`; `Correlator`; terminal-retaining `Acknowledgements`; bounded `Buffer`; `Sequencer`; `OrderGate`; `Deduplicator`; stash and pool dispatch | Address routing, generation-safe endpoint resolution, mailbox priority/fairness/backpressure, and the Bombay job-queue example | Scatter-gather, broker, aggregation, retrying delivery, and dead-letter compositions |
| Discovery and messaging fabric | Typed `Registry` with conflict and stale-unbind outcomes; immutable capability-separated `Resolver`; ordered typed `Topic`; keyed retained-membership `PubSub`; versioned generation-safe `Presence` | Local endpoint registration/resolution and local entity directory | Registry subscriptions, receptionist, groups, and sessions |
| Time derived | Deadline, receive timeout, one-shot, periodic, exclusive expiring `Lease`; typed-refill `RateLimiter` and reset-timed `CircuitBreaker` policies | The shared Bombay timer queue; its cancellation mechanism lacks a current Bombay effect lane | Retry schedule, heartbeat, idle passivation, and debounce/throttle compositions |
| Persistence | Bounded deterministic recency `Cache` policy; no general durable host yet | Mnesis supplies event sourcing, stores, optimistic append, subscriptions, snapshots, projection and saga primitives, import/export, and adapters; `mnesis-bombay` supplies the runtime-neutral execution path and initial Bombay routing seam | Bombay-hosted aggregate, recovery, committed-event relay, projection, checkpoint, inbox/outbox, and effect-delivery templates and adapters |
| Workflow and streams | Validated dependency-graph `Workflow`; one-generation typed `Latch`; fixed-membership cyclic `Barrier` with explicit generations | Mnesis supplies durable saga state/projected intents and projection folds; deterministic transition machinery, entity lifecycle machinery, and application examples also exist | Bombay saga/process-manager host, projection runner, rendezvous, election, queue/stream operators, batching, windowing, joins, and materialization |
| Cluster | None | Local entity activation/passivation and sharded map storage only | Membership, failure detector, distributed directory, placement, singleton, distributed sharding, replication, handoff, and transport actors |
| Operational boundary | Versioned typed `Health` aggregation with retained removal provenance; fixed-dependency versioned `Readiness`; atomic versioned `Configuration`; duplicate-free `Features` specialization | Bombay lifecycle reporting and terminal observation | Metrics, tracing, audit, diagnostics, resource, authorization, and gateway actors |

The most important negative findings are intentional boundaries:

- `AddressSpace` is not a registry actor, `ObservationSpace` is not a watch
  policy, `TimerQueue` is not a scheduler actor, and `AddressRouter` is not a
  routing-strategy actor.
- `LocalDirectory` implements local entity activation semantics; it must not be
  advertised as distributed discovery or sharding.
- Bombay Communication already owns physical mailbox backpressure, lane
  priority, fairness, and draining. A future `Buffer`, `PriorityQueue`, or
  `WorkQueue` actor must add typed policy rather than duplicate those mechanics.
- Mnesis provides the persistence subsystem. What remains absent is the full
  Bombay-hosted durable actor topology: aggregate hydration, committed-log
  relays, projection and saga hosts, effect delivery, durable inbox/outbox
  integration, and application assembly. Those are integration templates and
  adapters over Mnesis, not a new persistence engine.
- The repositories currently provide no distributed cluster implementation.
  Cluster catalogue entries remain design targets rather than shipped
  features.

The split also exposes one integration obligation: when Bombay upgrades to this
version of the template crate, its interpreter must consume the typed supervision
failure report lane. That is a runtime adapter update, not a missing actor.

### Consequences for implementation work

Do not implement the catalogue from top to bottom as a collection of services.
Select one template, complete its required contract, and prove its existing
runtime capabilities end to end through `bombay-engine::Driver` before adding
its public type.

The work naturally separates into these lanes:

1. **Freeze the implemented basis.** Verify and document `Machine`, `Stash`,
   `Watch`, `Deadline`, `ReceiveTimeout`, shutdown, `Proxy`, `Supervisor`,
   `Pool`, and `KeyedPool` against the template contract. This includes the
   Bombay adapter for the supervision-failure report lane.
2. **Build capability-backed templates.** Prefer templates whose mechanics
   already exist: `Task` and lifecycle publication over `bombay-observe`;
   the implemented `OneShot` and `Periodic` plus remaining `RetrySchedule`,
   `Heartbeat`, and timed passivation over `bombay-timers`; registry and
   resolver policy over `bombay-address`;
   local activation/passivation compositions over `bombay-entity`; and durable
   aggregate, relay, projection, and saga hosts over Mnesis through the
   `mnesis-bombay` integration boundary. Durable work must follow that
   repository's staged readiness gates rather than inventing a direct store
   adapter in this crate.
3. **Build pure derived policy templates.** Router strategies, correlation,
   aggregation, retry policy, buffering policy, and similar folds may compose
   the existing basis without adding executor, channel, clock, or registry
   machinery.
4. **Keep missing subsystems explicit.** Distributed membership, remote
   transport, consensus or fencing, and distributed sharding entries cannot be
   completed merely by adding a behavior wrapper. First define and implement
   their typed Bombay runtime capability and rejection contract.
5. **Leave domain protocols to applications.** `Worker`, `Saga`, workflow,
   authorization, gateway, and similar entries may supply reusable structure,
   but Bombay must not invent the user's domain messages or decisions.

An implementation is complete only when both halves exist: the passive fold is
independently testable as a behavior template, and a Bombay driver path can
realize every declared action and return every declared observation or typed
failure. A template with an imaginary interpreter path is not catalogue
coverage; a runtime primitive without a typed fold is not an implemented
template.

## Distilled crate adoption map

The crate question must be answered after separating the three parts of a
running actor:

```text
template dependency
    = pure data structure or algorithm used inside the fold

runtime dependency
    = mechanism used by the System environment to interpret a typed lane

domain dependency
    = application-owned model, protocol, codec, or external integration
```

Only the first category may normally enter `bombay-behavior-actors`. Runtime
crates belong in Bombay or a focused integration repository. Domain crates
belong in the application. The generic Driver gains no dependency when a new
template or capability is added.

### Dependencies for the template crate

| Need inside a pure template | First choice | Adoption decision |
|---|---|---|
| Typed composable errors | `thiserror` | **Already adopted.** Every public template error remains a concrete exhaustive sum, and `#[source]`/`#[from]` preserves nested failure provenance. |
| FIFO buffering or work queues | `std::collections::VecDeque` | **Use standard library.** Covers `Buffer`, `WorkQueue`, batching, throttling queues, and most stash-like state. |
| Ordered keys, sequence gaps, or release gates | `std::collections::BTreeMap` / `BTreeSet` | **Use standard library.** Covers `Sequencer`, `OrderGate`, ordered checkpoints, and deterministic key traversal when key order—not insertion order—is semantic. |
| Ordinary key lookup | `std::collections::HashMap` / `HashSet` | **Use standard library.** Hash iteration order must not become observable policy. |
| Insert-once priority | `std::collections::BinaryHeap` | **Use standard library.** Prefer it when priority never changes in place. |
| Deterministic insertion-order maps or sets | `indexmap` | **Eligible with first concrete use.** Likely users are `Registry`, `Topic`, deterministic recipient membership, and correlation tables whose insertion order is documented behavior. |
| Bounded recency retention | `lru` | **Eligible with first concrete use.** Use for `Deduplicator` or `Cache` only when capacity, refresh behavior, and eviction are explicit protocol policy. |
| Mutable in-place priority | `priority-queue` | **Eligible only if required.** Use for `PriorityQueue` or scheduling policy only when reprioritization is a real operation; otherwise use `BinaryHeap`. |
| Dependency graph algorithms | `petgraph` | **Eligible with first graph template.** Use for workflow DAGs, topological activation, cycle detection, or placement analysis—not ordinary actor parent/child topology. |
| Dense fixed participant sets | `fixedbitset` | **Conditional candidate.** Use for `Barrier` only after a fixed, dense membership representation is demonstrated to matter. |
| Replicated data algorithms | `crdts` with default features disabled | **Future and feature-gated.** Consider only with the first concrete replicated template after distributed runtime capabilities exist. Keep concrete delta types visible. |
| Consistent or rendezvous hashing | Reviewed local implementation using `std` | **Do not add a crate yet.** The surveyed crates did not justify importing their maintenance and policy surface; test a small local algorithm against an independent model. |
| Random routing | Explicit entropy observation | **No RNG dependency in Actors.** Entropy is a runtime input; the fold remains deterministic. |
| Backoff arithmetic | Local exhaustive policy enum | **No retry crate.** Constant, linear, and exponential policy are small, observable semantics and require checked arithmetic. |

This yields a deliberately small near-term manifest:

```text
bombay-behavior-actors
    -> bombay-behavior
    -> bombay-behavior-macros
    -> thiserror
```

`indexmap`, `lru`, `priority-queue`, `petgraph`, `fixedbitset`, and `crdts` are
not baseline dependencies. Each is admitted with the first template whose
public law needs it and after the exact release passes repository policy.

### Existing capability crates to compose, not import

| Templates enabled | Existing crate or subsystem | Where the dependency belongs | What remains to implement |
|---|---|---|---|
| `Task`, `LifecyclePublisher`, `TerminationMonitor`, `Reaper` | `bombay-observe` | Bombay System environment | Typed template event/send products and observation interpreters |
| `OneShot`, `Periodic`, `RetrySchedule`, `Heartbeat`, timed `Lease`, `IdlePassivation`, debounce/throttle/rate policies | `bombay-timers` | Bombay System environment | Pure policies plus schedule/cancel/result lanes; never another timer queue |
| `Registry`, `Resolver`, `Receptionist`, local `Directory` policy | `bombay-address` | Bombay System environment | Typed lookup, conflict, subscription, and notification protocols |
| `Buffer`, `WorkQueue`, policy `PriorityQueue`, `Bulkhead` | `bombay-communication` | Bombay mailbox environment | Pure queue/admission/overflow policy; never another mailbox or physical-backpressure implementation |
| `Activation`, `EntityPassivation`, stable local entity hosts | `bombay-entity` | Bombay or focused integration repository | Template policy and typed lifecycle mapping; never another local entity directory |
| Every template | `bombay-engine`, `bombay-transition`, `bombay-machine-executor` | Bombay engine/runtime | Reuse the universal Driver and exclusive turn machinery; never add a template-specific loop |
| `EventSourced`, `Recovery`, durable projection, checkpoint, saga/process manager, relay, inbox/outbox | Mnesis and `mnesis-bombay` | `mnesis-bombay`, not Actors | Bombay hosts, typed adapter protocols, receipts, and composition root; never another persistence abstraction |

“Composition” in this table means composition of public protocols and internal
event/effect algebras across crate boundaries. It does not mean adding these
runtime crates as dependencies of
`bombay-behavior-actors`. A template declares typed requirements; the System's
environment supplies the implementation and the compiler rejects an
insufficient environment.

### Candidates for capabilities Bombay does not yet have

These crates are adapter candidates, not template dependencies:

| Missing capability | Candidate already researched | Adoption boundary |
|---|---|---|
| Cluster membership and failure evidence | `foca` | Future Bombay cluster subsystem translates SWIM outputs into typed events. |
| Leadership or consensus where actually required | `openraft` | Optional distributed coordination subsystem after fencing and ownership laws are defined. |
| Remote transport | Owned CESR/KERI protocol crates plus `quinn`, `rustls`, and `bytes` where the selected transport requires them | Bombay transport adapter with concrete framing, identity/authentication evidence, ordering, and acknowledgement semantics. |
| Non-CESR application encoding | Mnesis codecs or application-selected `serde`, `postcard`, `prost`, or `rkyv` | Application or storage boundary only; never an internal erased message envelope. |
| DNS, local, Kubernetes, or peer discovery | `hickory-resolver`, `mdns-sd`, `kube`, optional `libp2p` | Discovery adapters produce typed observations for discovery templates. |
| HTTP/gRPC gateways | `tower`, `tonic` | Application/Bombay boundary, not the actor wire core. |
| Metrics and tracing export | `metrics` or `prometheus-client`; `tracing`, optional `opentelemetry` | Interpreter/operations adapters consume typed observations. |
| Explicit OS and secret authority | `cap-std`, `secrecy` | Application/runtime capability boundary; never a global lookup facility. |

No candidate in this table unblocks a behavior template by itself. The typed
capability, its successes and rejections, interpreter ordering, lifecycle, and
Driver path must be designed first.

### Crates deliberately rejected from the template crate

- Actor frameworks such as Actix, Ractor, Kameo, Heph, and Tokio Actors bring
  competing mailbox, identity, scheduling, lifecycle, or dispatch semantics.
- Async runtimes, channels, clocks, executors, and transport clients introduce
  effects inside what must remain a deterministic fold.
- Alternate CQRS, event-sourcing, and persistence frameworks would duplicate
  the architecture already owned by Mnesis.
- Serialization, `Any`, dynamic registries, and universal envelopes cannot
  substitute for concrete event and send products.
- UUID, ULID, arena, and slot-map crates may help runtime allocation or
  correlation, but they do not prove fresh actor creation and do not belong in
  template semantics merely to manufacture identity.

## Dependency policy

Keep `bombay-behavior` dependency-minimal. `bombay-behavior-actors` already
uses `thiserror` for composable typed error products. After that, the first
eligible external dependencies are:

1. `indexmap` for deterministic registries and recipient collections;
2. `lru` for bounded deduplication and caches;
3. `priority-queue` when mutable priority is actually required;
4. `petgraph` when the first graph-based workflow or placement template lands;
5. `fixedbitset` when a fixed dense barrier demonstrates the need; and
6. `crdts`, feature-gated with default features disabled, when the first
   replicated template lands.

This is an eligibility list, not permission to add speculative dependencies.
Before adoption, inspect the exact release for license, pinned Rust-version
compatibility, default features, transitive dependencies, `unsafe`, dynamic
dispatch, serialization assumptions, and maintenance. Add the dependency only
with the template whose concrete use demonstrates the need. The owned Bombay
ecosystem, not a surveyed framework, determines architectural semantics and
repository ownership.

## Admission checklist

Before implementing a catalogue entry:

1. State the semantic law and classify it as actor-model law, derived Bombay
   construction, or deliberate Bombay policy.
2. Classify the entry as a behavior, derived, capability-backed, or application
   template, or as a runtime-subsystem gap. Do not implement a runtime
   capability as though it were a template.
3. Complete the template contract: domain slot, state sum, event sum, action
   product, required capabilities, current realization, policy, and composition
   law.
4. Define the complete sums of states, inputs, successes, rejections, stale
   observations, retries, and terminal outcomes.
5. Define named effect products and show that every wrapper order preserves all
   lanes without positional access.
6. Trace one event through `bombay-engine::Driver` and identify the existing
   Bombay adapter that realizes every effect and returns every observation or
   rejection end to end.
7. Decide whether the standard library suffices. If not, document the exact
   external algorithm being reused and why it does not own actor semantics.
8. Test the pure fold with examples, composition cases, an independent model,
   exhaustive small-state checks, properties, fuzzing where applicable, and
   compile-fail cases for forbidden capabilities.
9. Add the dependency only with the first concrete template that needs it, behind
   a feature when the capability is optional or operationally heavy.
