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

## Application routing and fixed supervision

`Supervisor<A, C>` owns one fixed stable-proxy fleet. Its public protocol is
the ownership protocol: worker lifecycle facts, replacement decisions, and
coordinated shutdown. It does not also pretend to be an application command
router.

When an application command selects a worker, that selection is application
state-transition policy. Keep it in the application behavior and emit a typed
delivery to the chosen stable proxy, or place a concrete routing actor in
front of the supervisor. The supervisor remains responsible for fresh worker
incarnations and stable slot ownership; the router remains responsible for
command admission and destination selection. This composition makes both
protocols and both rejection laws visible to Rust.

Use `BackoffSupervisor<A, C>` when the standalone fleet must delay accepted
replacement requests. Use `BackoffSupervise<B, C>` when an existing behavior
creates and adopts its own workers. These are separate transformations because
their input folds and typed effect products differ. Backoff remains explicit
policy supplied with `Backoff`; it is not another actor template.

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
