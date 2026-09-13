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
| Must a transferable destination use this actor's address namespace? | `DeliveryRouteFor<Owner>` | the same logical, established, or mixed route, constrained to `BehaviorAddr<Owner>` |
| Which actors can this actor create? | `Behavior::Birth`, `BirthProtocols` | the closed, occurrence-preserving fresh-child algebra |

`LogicalHostRequirements` separately derives the ordered product of every
intentional logical `Delivery<P>` in the root and its transitive births. It
excludes established-incarnation delivery, creator-local child effects, and
interpreter requests while retaining repeated protocol occurrences. A runtime
may recursively require its own static `Hosts<P>` proof for that product; the
projection creates no host and performs no lookup.

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
routes and explicit ownership, not by flattening their protocols into one
envelope. Atomic actors have two intentionally different worker relationships:

```text
FixedSupervisor ─┐
                  ├─> StableProxy ─> worker
DynamicSupervisor┘

FifoPool  ────────> direct worker
KeyedPool ────────> direct worker
```

The supervisors reuse the one stable-service and worker-replacement law owned
by `StableProxy`. Pools own assignment, completion, recovery, and worker
retirement directly; they contain no supervisor or stable proxy. A `Router`,
queue, workflow, or domain actor remains a peer whose own transition law is
connected through a typed transferable capability.

The short responsibility map is
[`atomic-actor-architecture.md`](atomic-actor-architecture.md). The sole
application-facing construction and worker-authoring syntax is
[`atomic-actor-devx.md`](atomic-actor-devx.md); this composition guide does not
repeat either specification.

## Choose the owner of the law

The catalogue is a vocabulary of state-transition laws, not a list of stacks
that must always be used together:

| Required law | Owning actor or transformation |
|---|---|
| domain state and protocol | the application behavior or one catalogue core |
| bounded admission or ordering | `Buffer`, `PriorityQueue`, `OrderGate`, `Sequencer`, `WorkQueue` |
| one-recipient selection | `Router` with `RoundRobin`, `LeastLoaded`, `ConsistentHash`, or `RendezvousHash` |
| fan-out to a membership snapshot | `Topic` or `PubSub` |
| stable service identity across successive workers | `StableProxy` |
| declared-roster coordinated recovery | `FixedSupervisor` |
| bounded keyed service membership | `DynamicSupervisor` |
| global FIFO jobs or keyed-affinity jobs | `FifoPool` or `KeyedPool` |
| delayed replacement | the corresponding backoff supervision transformation |
| same-mailbox timing, observation, stashing, or shutdown | the concrete layer owning that transformation |
| ordered child shutdown | `ShutdownCoordinator` or `HeterogeneousShutdownCoordinator` |

If two laws belong to different actors, connect them with `DeliveryRoute`. If
one law transforms the same mailbox fold, construct it with `Behavior::layer`.
If neither is true, the application is defining a new topology or a genuinely
new transition law; hiding that fact in a generic wrapper would be incorrect.

## Application routing and atomic actors

`FixedSupervisor` owns declared-roster recovery and `DynamicSupervisor` owns
bounded keyed service membership. Neither also owns application command
selection. Both rely on `StableProxy` for the stable service protocol and
worker replacement, so application clients receive only the service
capability—not a worker route or supervisor control capability.

A pool is different: it accepts customer jobs and owns direct assignment to its
workers. It never exposes those worker routes. FIFO owns one global admission
order; keyed pooling owns per-role queues and future-admission affinity. Their
complete differences are owned by the two pool documents, not by a selector
mode in this guide.

When an application command selects a worker, that selection is application
state-transition policy. Keep it in the application behavior and emit a typed
delivery to the chosen stable service, or place a concrete routing actor beside
the supervisor and give it those capabilities. The supervisor remains
responsible for recovery and stable service ownership; the
router remains responsible for command admission and destination selection.
This composition makes both protocols and both rejection laws visible to Rust.

`Router` is deliberately unicast and transfers ownership of a command to one
selected route; it does not require the command to be `Clone`. Use `Topic` or
`PubSub` when fan-out and its explicit cloning cost are the intended law.

## Bombay application provisioning

Bombay must distinguish two creation owners:

- `Root::Birth` contains only children the root behavior can itself create and
  correlate with creator-issued `CreationId` values.
- `Application` contains peers provisioned by the application declaration.

An application child must therefore stop appearing in the root's `births =
{ ... }` declaration merely to make it runnable. That old arrangement forces
the declaration to name the child's fully composed type before Rust can infer
it. It is the source of aliases such as `ManagedTask =
StopOnShutdown<Task<...>>`.

The application declaration should instead store each semantic slot and child
value in its own heterogeneous product. Its private application-child owner
keeps one sequence for those declarations and lowers the resulting values
through the existing `Children` product. That sequence is independent of the
root's creation state; equal numeric IDs remain distinct because root and
application children occupy different static occurrences. Bombay selects
runtime routes only when it interprets that creation batch. The public spelling
contains values, not nested type declarations:

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

Internally, Bombay needs one private application-child product with an associated
`Product: ChildProduct<MailAddr>`. It should return
`Children<MailAddr, Product>` after issuing IDs from the application-child
sequence. The running application then uses exactly these Behavior projections:

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
    // 2. issue one ID per application-child occurrence;
    // 3. lower L through Children::into_creates;
    // 4. call BirthNodeAppend::append_creations(root, application);
    // 5. rebuild Actions with the original sends and become decision.

    // transition:
    // delegate to R, then call append_creations(root_creates, Vec::new()).
}
```

`BirthNodeAppend` keeps `RootNode<R>` as an exact structural prefix. Existing
root roles therefore retain the same child and position after application
peers are appended. It also preserves every child value, creation ID,
`CreationKind`, and within-lane order, with root creations before application
creations. It does not allocate, install, or validate address freshness. Bombay
still owns runtime route selection and must surface initialization-twice or
child-product rejection as a typed private running-application error rather
than panic.

This lets Bombay delete its root-shaped vacant-slot machinery:
`RuntimeApplicationTopology`, `EmptyApplicationTopology`, `VacantChild`,
`OccupiedChild`, `AvailableApplicationRole`, `FillApplicationRoleAt`, and
`InjectApplicationChildAt`. A smaller role-indexed application product remains
because it owns a genuine law absent from `Children`: semantic naming,
deferred creation-ID issuance, and construction of runtime child handles.

The runtime must derive installation storage from the running application's
combined birth algebra, not from `Root::Birth`. Consequently
`BirthProtocols` automatically includes the root, genuine root
children, application peers, and every transitive birth. Intentional logical
destinations remain concrete in the composed send products. Behavior does not
fabricate a completeness proof by asking the application to repeat those
destinations in metadata, and Bombay must not add a protocol registry to
compensate.

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
installed incarnations and adds no logical-host requirement. `Recipient<P>`
broadcasts to intentional logical identities, so the application topology must
install that concrete protocol. For stable replaceable workers, subscribe the
logical stable protocol, not a stale exact worker incarnation. Different
destination protocols remain different actors and are connected with typed
`MessageAdapter`s; `Topic` never erases them into a common envelope.

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

`FifoPool` and `KeyedPool` each implement their public transition law
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
