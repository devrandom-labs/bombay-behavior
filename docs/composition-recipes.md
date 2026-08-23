# Proven composition recipes

`bombay-behavior-actors` exposes two ordinary construction functions for
wrapper orders whose placement changes ownership of lifecycle or timer facts.
They return existing concrete stacks: recipes are not builders, macros,
runtime actors, or new behavior implementations.

These orders are derived Bombay API policy, not laws of the actor model. The
recipes only make two reusable, correctness-sensitive compositions easy to
select without hiding any policy input.

## Supervised restart backoff

Use `supervised_backoff` when a standalone fixed fleet needs delayed,
generation-safe replacement:

```rust,ignore
let behavior: BackoffSupervisor<MailAddr, Worker> = supervised_backoff(
    ChildTopology::new([1, 2], worker_factory),
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        3,
        Duration::from_secs(30),
    ),
    Backoff::exponential(Duration::from_millis(100), Duration::from_secs(5))?,
    restart_timer,
)?;
```

This is exactly equivalent to:

```rust,ignore
BackoffSupervisor::new(
    Supervisor::new(topology, restart)?,
    backoff,
    timer,
)
```

Its only construction error is `FleetError<A::Nonce>` from `Supervisor::new`.
`BackoffConfigError` belongs to prior `Backoff` validation. `BackoffError`
belongs to later transition-time delay calculation. The recipe does not merge
these distinct phases into a builder error.

Use `BackoffSupervise<B, C>` directly when a real inner application behavior
has an additional `Births<C>` lane. The standalone recipe must not fabricate
that capability.

## Coordinated terminal application

Use `coordinated_terminal_application` when an application must coordinate a
validated heterogeneous shutdown plan, trigger it once after a timeout, and
publish the exact terminal outcome of one selected child:

Named child roles lower into the existing target sum without positional
construction at the call site:

```rust,ignore
let routes = ApplicationChildrenRoutes::new(worker_nonce, query_nonce);
let workers = shutdown_target::<Application, _, ShutdownTargets>(
    ApplicationChild::Workers,
    routes.workers,
);
let queries = shutdown_target::<Application, _, ShutdownTargets>(
    ApplicationChild::Queries,
    routes.queries,
);
let validated_shutdown_plan = HeterogeneousShutdownPlan::new([
    vec![queries],
    vec![workers],
])?;
```

`shutdown_target` only selects the statically proven `ShutdownChoice` branch
and copies the route nonce. Plan validation still owns empty-phase and global
duplicate-nonce rejection; the coordinator still owns phase and terminal-fact
transitions.

When the plan is assembled only after the interpreter reports a committed
named creation, retain both available capabilities instead of weakening the
exact endpoint or reconstructing a route from its address:

```rust,ignore
let child = established_child::<Application, ApplicationChildrenWorkers>(fact)?;
let workers: ShutdownTargets =
    child.shutdown_target::<Application, ShutdownTargets>();

// The creator-local role enters the plan while the exact incarnation remains
// available for EstablishedDelivery, ObserveEstablished, or ShutdownEstablished.
let exact_worker = child.actor();
let validated_shutdown_plan =
    HeterogeneousShutdownPlan::new([vec![workers]])?;

// `Application` stores this capability before it is wrapped. If a Guardian is
// outside the coordinator, lift the coordinator's own lane through that one
// outer event layer.
let coordinator_ingress =
    ShutdownPlanIngress::<HeterogeneousShutdownPlan<ShutdownTargets>, Here>::new()
        .inside();

// The transition that receives the last committed creation fact reports the
// plan through its ordinary Actions sends product.
let actions = Actions::send(InterpreterRequests::one(
    coordinator_ingress.report(validated_shutdown_plan),
));
```

`EstablishedChild` is a named product of the existing `ChildRoute` and
`EstablishedActor` capabilities. It performs no creation or interpretation;
an `EstablishedCreation::Rejected` result returns its typed rejection and
constructs neither capability.

`ReportShutdownPlan` is an explicit interpreter request to the selected
ancestor ingress. Its interpreter constructs one root event with
`into_event`; the coordinator changes state only when that communication is
later folded. This keeps plan construction and installation inside the normal
`Actions` boundary instead of relying on driver mutation or a direct
`Active::on_path` call.

```rust,ignore
let behavior: CoordinatedTerminalApplication<
    Application,
    ShutdownTargets,
    ApplicationChildrenWorkers,
> =
    coordinated_terminal_application(
        application,
        validated_shutdown_plan,
        TimerId(7),
        Duration::from_secs(20),
        request_shutdown,
        ChildTermination::<_, ApplicationChildrenWorkers>::new(supervised_pool_nonce),
        propagate_abnormal,
    );
```

The returned concrete type is:

```text
PropagateTermination<
    OneShot<HeterogeneousShutdownCoordinator<B, S>>,
    ChildTermination<BehaviorAddr<B>, ObservedOccurrence>,
>
```

The outer terminal observer and shutdown coordinator may both observe child
lifecycle facts. They remain independent structural observation requests; one
consumer must not steal the other's fact. An unmatched child fact is inert at
the propagation layer. The terminal policy either discharges or publishes the
original outcome without reclassification.

The function is infallible because `HeterogeneousShutdownPlan::new` validates
non-empty phases and global child-nonce uniqueness before construction. Timer
identity, duration, the pure `OneShotReaction`, occurrence-indexed observed
child, and terminal policy are all explicit.

## Initialization and testing

Wrapper initialization is inside-out and accumulates effects without dropping
or reordering them. For the coordinated recipe, application initialization is
preserved, the one-shot schedule is added, and the outer exact-child
observation is added to its own named lane. These effects must be interpreted
before the first mailbox event.

Owner tests compare recipe-produced stacks with equivalent manual stacks over
initialization and transition traces. Type assertions verify exact return
types; error tests retain existing variants and nonces. Alternative wrapper
orders are different concrete constructions and must be named and tested
independently rather than presented as equivalent spellings.

`WorkerPool::new(topology, configuration, complete_to)` remains the canonical
pool construction boundary. It has no sensitive wrapper order, so a second
recipe would only hide already explicit policy.
