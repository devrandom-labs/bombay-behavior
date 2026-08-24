# Actor composition

`bombay-behavior-actors` exposes concrete actor folds and typed event/effect
transformations. Applications connect those actors with ordinary typed
composition. A constructor recipe is not a separate actor template when it
only selects policy values, forwards to another constructor, or hides a nested
type.

These compositions are derived Bombay constructions. They preserve the pure
behavior boundary: one typed input produces complete `Actions` and a next
behavior decision, while the interpreter alone realizes delivery, creation,
observation, scheduling, and shutdown effects.

## The composition map

There are two orthogonal ways to compose Behavior Actors. They may be used
together, but they do not mean the same thing.

| Question | Static construction | What it proves |
|---|---|---|
| Does another law transform this actor's mailbox fold? | `Behavior::layer` with an existing concrete transformation | the complete resulting `Behavior`, including event, sends, births, phase, error, initialization, and next decision |
| May this actor send to a transferable destination? | `DeliveryRoute` | one exact protocol and its logical, established, or mixed concrete send product |
| May this topology owner send to its direct child? | `DeliveryRouteFor<Owner>` | the same transferable routes, or one `ChildRoute` resolved from `Owner::Birth` |
| Which actors can this actor create? | `Behavior::Birth`, `InstallationRequirements` | the closed, occurrence-preserving fresh-child algebra |
| Which intentional logical destinations must a runtime host? | `LogicalHostRequirements` | the owner-authored, transitive, duplicate-preserving logical protocol product |

### Same-mailbox layers

A layer constructs a new concrete behavior around one existing behavior. Both
participate in one mailbox fold. Put the domain state machine at the center and
add only transformations that own a distinct event/effect law:

```text
StopOnShutdown                         root lifecycle transformation
└── ReceiveTimeout                     activity/timer transformation
    └── Stash                          bounded hold/replay transformation
        └── DomainBehavior             application transition law
```

Callers compose at the value level; Rust infers the full nested type:

```rust,ignore
let behavior = domain
    .layer(|inner| Stash::new(inner, admission))
    .layer(|inner| ReceiveTimeout::new(inner, timer, idle, on_idle))
    .layer(StopOnShutdown::new);
```

`BehaviorLayer` itself performs no actor effect. Each concrete transformation
still owns and documents its event routing, initialization order, sends
product, failure, and terminal decision. Reordering layers can therefore
change the program and must be chosen from those laws, not from type
convenience.

### Actor-to-actor topology

Independent actors keep independent mailboxes. They compose through typed
routes and explicit topology ownership, not by flattening their protocols into
one envelope. A representative supervised routing graph is:

```text
Application topology owner
├── Router<Recipient<Proxy<PriorityQueue>>>       unicast selection
│                │
│                └── Delivery<Proxy<PriorityQueue>>
└── Supervisor<PriorityQueue>                     stable-fleet ownership
    └── Proxy<PriorityQueue>                       incarnation lifecycle
        └── PriorityQueue                          admission and ordering
            └── Delivery<Target>                   domain destination
```

`Router` and `Supervisor` are peers in the application topology. The
supervisor owns creation, replacement, provenance, and shutdown of stable
proxies. The router owns only membership and single-recipient selection; its
members are capabilities for those stable proxy protocols. The proxy owns the
fresh worker incarnation and translates a successful command into one
owner-proven `ChildDelivery`. The priority queue alone owns capacity, priority,
and release order.

The complete command path remains visible in concrete values:

```text
RouterMessage::Route(ProxyCommand::Forward(queue_command))
  → Delivery<Proxy<PriorityQueue>>
  → ChildDelivery<PriorityQueue, ChildHead>
  → Delivery<Target> + Delivery<PriorityQueueOutcome>
```

This path is executable in
[`router_to_supervised_proxy_to_priority_queue_preserves_every_hierarchy_edge`](../crates/behavior-testkit/tests/universal_layers.rs).
The test starts from the supervisor's staged proxy, starts the proxy's staged
queue, commits the queue creation, then folds offer and release commands
through every hop while asserting every `Actions` lane.

## Choose the owner of the law

The catalogue is a vocabulary of state-transition laws, not a list of stacks
that must always be used together:

| Required law | Owning actor or transformation |
|---|---|
| domain state and protocol | the application behavior or one catalogue core |
| bounded admission or ordering | `Buffer`, `PriorityQueue`, `OrderGate`, `Sequencer`, `WorkQueue` |
| one-recipient selection | `Router` with `RoundRobin`, `LeastLoaded`, `ConsistentHash`, or `RendezvousHash` |
| fan-out to a membership snapshot | `Topic` or `PubSub` |
| stable address across fresh incarnations | `Proxy` |
| fixed or dynamic replacement ownership | `Supervisor`, `Supervise`, or `DynamicSupervisor` |
| delayed replacement | the corresponding backoff supervision transformation |
| same-mailbox timing, observation, stashing, or shutdown | the concrete layer owning that transformation |
| ordered child shutdown | `ShutdownCoordinator` or `HeterogeneousShutdownCoordinator` |

If two laws belong to different actors, connect them with `DeliveryRoute`. If
one law transforms the same mailbox fold, construct it with `Behavior::layer`.
If neither is true, the application is defining a new topology or a genuinely
new transition law; hiding that fact in a generic wrapper would be incorrect.

## Application routing and fixed supervision

`Supervisor<A, C>` owns one fixed stable-proxy fleet. Its public protocol is
the ownership protocol: worker lifecycle facts, replacement decisions, and
coordinated shutdown. It does not also pretend to be an application command
router.

When an application command selects a worker, that selection is application
state-transition policy. Keep it in the application behavior and emit a typed
delivery to the chosen stable proxy, or place a concrete routing actor beside
the supervisor and give it those proxy capabilities. The supervisor remains
responsible for fresh worker incarnations and stable slot ownership; the
router remains responsible for command admission and destination selection.
This composition makes both protocols and both rejection laws visible to Rust.

`Router` is deliberately unicast and transfers ownership of a command to one
selected route; it does not require the command to be `Clone`. Use `Topic` or
`PubSub` when fan-out and its explicit cloning cost are the intended law.

Use `BackoffSupervisor<A, C>` when the standalone fleet must delay accepted
replacement requests. Use `BackoffSupervise<B, C>` when an existing behavior
creates and adopts its own workers. These are separate transformations because
their input folds and typed effect products differ. Backoff remains explicit
policy supplied with `Backoff`; it is not another actor template.

## Bombay application provisioning

Bombay must distinguish two creation owners:

- `Root::Birth` contains only children the root fold can itself create and
  address with its nominal `ChildRoute`s.
- `Application` contains peers provisioned by the application declaration.

An application child must therefore stop appearing in the root's `births =
{ ... }` declaration merely to make it runnable. That old arrangement forces
the declaration to name the child's fully composed type before Rust can infer
it. It is the source of aliases such as `ManagedTask =
StopOnShutdown<Task<...>>`.

The application declaration should instead store each semantic slot and child
value in its own heterogeneous product. Its private staging fold may defer
nonce selection until root initialization, declare the runtime route, and
lower the resulting values through the existing `Children` product. The
public spelling then contains values, not nested type declarations:

```rust,ignore
struct Tasks;
struct Events;

let application = Application::new(root)
    .child(Tasks, WorkQueue::new(worker).stop_on_shutdown())
    .child(
        Events,
        Topic::<MailAddr, Event, Recipient<EventSink>>::new()
            .stop_on_shutdown(),
    );
```

`Tasks` and `Events` are nominal application roles, not aliases for actor
types. The expression passed to each `child` call determines the concrete
child type. Each call returns the next inferred application-product type, so
neither the composed child types nor the final `Application<...>` type is
written by the caller. A zero-state generic actor such as an empty `Topic`
still needs enough protocol arguments to determine its law; that is real
protocol information, not a mechanical nesting alias.

Internally, Bombay needs one private declaration fold with an associated
`Product: ChildProduct<MailAddr>`. It should return
`Children<MailAddr, Product>` after allocating collision-free creator-local
nonces and declaring the corresponding role routes. The running application
then uses exactly these Behavior projections:

```rust,ignore
type RootNode<R> = <<R as Behavior>::Birth as BirthMode>::Child;
type AppNode<L> = <<L as StageApplicationChildren>::Product
    as ChildProduct<MailAddr>>::Choice;
type RunningNode<R, L> =
    <RootNode<R> as BirthNodeAppend<AppNode<L>>>::Output;

impl<R, L, Routes> Behavior for RunningApplication<R, L, Routes>
where
    R: Behavior<Protocol: Protocol<Addr = MailAddr>> + BehaviorBase,
    L: StageApplicationChildren<Routes>,
    RootNode<R>: BirthNodeAppend<AppNode<L>>,
{
    type Birth = Births<RunningNode<R, L>>;

    // init:
    // 1. initialize R;
    // 2. select app nonces disjoint from R's initialization creations;
    // 3. lower L through Children::into_creates;
    // 4. call BirthNodeAppend::append_creations(root, application);
    // 5. rebuild Actions with the original sends and become decision.

    // transition:
    // delegate to R, then call append_creations(root_creates, Vec::new()).
}
```

`BirthNodeAppend` keeps `RootNode<R>` as an exact structural prefix. Existing
root roles therefore retain the same child and position after application
peers are appended. It also preserves every child value, nonce,
`CreationKind`, and within-lane order, with root creations before application
creations. It does not allocate, install, or validate freshness. Bombay still
owns nonce selection and must surface initialization-twice or child-product
rejection as a typed private running-application error rather than panic.

This lets Bombay delete its root-shaped vacant-slot machinery:
`RuntimeApplicationTopology`, `EmptyApplicationTopology`, `VacantChild`,
`OccupiedChild`, `AvailableApplicationRole`, `FillApplicationRoleAt`, and
`InjectApplicationChildAt`. A smaller role-indexed application product remains
because it owns a genuine law absent from `Children`: semantic naming,
deferred nonce allocation, and construction of runtime child handles.

The runtime must derive installation storage from the running application's
combined birth algebra, not from `Root::Birth`. Consequently
`InstallationRequirements` automatically includes the root, genuine root
children, application peers, and every transitive birth. Intentional logical
destinations remain a separate owner contract: Bombay consumes
`LogicalHostRequirements::LogicalHosts`; it must not infer them from arbitrary
send products or add a protocol registry.

### Unicast and broadcast in Bombay

`Router` is only unicast. Use `RoundRobin`, `LeastLoaded`, `ConsistentHash`, or
`RendezvousHash`; each successful route transfers one owned message to at most
one member. Bombay must remove stale `Router<Broadcast>` examples and imports,
not recreate a broadcast strategy or compatibility wrapper.

Fan-out is the distinct `Topic`/`PubSub` state-transition law:

- `Topic<A, P, Route>` owns one insertion-ordered membership snapshot;
  `TopicMessage::Publish(P)` sends a clone to every member.
- `PubSub<A, K, P, Route>` additionally owns keyed topic introduction,
  known-empty retention, and per-topic membership;
  `PubSubMessage::Publish { topic, value }` fans out within one key.

Choose `Route` truthfully. `EstablishedRecipient<P>` broadcasts to exact
installed incarnations and adds no logical-host requirement.
`Recipient<P>` broadcasts to intentional logical identities and Bombay's
application owner must include every such protocol occurrence in
`LogicalHostRequirements`, including meaningful duplicates. For stable
replaceable workers, subscribe the logical `Recipient<Proxy<C>>`, not a stale
exact worker incarnation. Different destination protocols remain different
actors and are connected with typed `MessageAdapter`s; `Topic` never erases
them into a common envelope.

## Root shutdown

Use `StopOnShutdown<B>` when a shutdown request means that this actor stops
directly. Use `FinalizeOnShutdown<B>` when shutdown must run one typed finalizer
and preserve all of its sends, creations, and terminal decision. If shutdown
must first be delegated to a coordinator, place that coordinator at the root;
no guardian alias or builder is required.

These transformations compose like every other wrapper:

```rust,ignore
let direct = StopOnShutdown::new(application);
let finalizing = FinalizeOnShutdown::new(application, finalize);
let coordinated = ShutdownCoordinator::new(application, plan);
```

The choice is deliberate Bombay policy. It is not automatic discovery of a
nested shutdown handler.

## Plans derived from committed children

`ShutdownCoordinator` and `HeterogeneousShutdownCoordinator` own the distinct
homogeneous and heterogeneous ordered-shutdown folds. A topology-owning
application that cannot construct its plan until children commit records those
committed `EstablishedChild` capabilities in its own state. Once complete, it
emits `ReportShutdownPlan::new(plan)` in its ordinary `Actions` send product.
The interpreter returns the corresponding typed `InstallShutdownPlan<P>` event
to the coordinator.

The topology owner must preserve the following policy:

- only a successfully committed creation contributes a child target;
- rejection remains a typed application transition failure;
- every declared role contributes exactly once;
- an early shutdown request remains pending until installation; and
- a plan installs at most once.

This is ordinary actor communication between two concrete folds. A generic
child-plan wrapper cannot own these laws because it does not own the
application's topology or role state.

## Observation

`Watch<B>` is the recurring logical-name observation transformation. It
continues observing later incarnations of the same logical peer.
`TerminationMonitor` owns a correlated, exact-once observation lifecycle and
consumes its terminal relationship. Exact-incarnation monitoring uses that
same monitor law with an established target and an ordinary typed reaction.

Those recurrence laws are different, so they remain separate folds. Target
aliases and established-watch wrappers add no law and are unnecessary.

## Pools and shutdown products

`WorkerPool` and `KeyedWorkerPool` each implement their public transition law
directly. They may reuse private data helpers, but neither delegates its
`Behavior` fold to a hidden generic actor engine. FIFO assignment and
persistent key affinity have different state transitions.

Likewise, homogeneous and heterogeneous shutdown coordinators retain separate
folds. Their phase products select different concrete child effect lanes, so a
generic execution engine would hide the very distinction the public types are
meant to prove.

## Audit record

The complete catalogue classification and change ledger are in
[Actor-template composition audit](template-composition-audit.md). The broader
capability and adversarial-test record remains in
[Behavior Actors template-law audit](template-law-audit.md).
