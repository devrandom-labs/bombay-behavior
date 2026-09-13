# Stable proxy

Status: `feature-complete` for the local aggregate. Initial start, readiness,
exact pre-ready return, replacement, owner shutdown, complete local custody,
independent models, compile contracts, and two stateful fuzz targets are
implemented and verified. Parent-to-root terminal transfer remains assigned to
Bombay and does not belong in this aggregate.
Actor-graph deadline policy belongs to the owning supervisor, pool, or Bombay
root, not StableProxy. The complete transition
matrix is owned only by
[`actor-laws/proxy.md`](actor-laws/proxy.md).

## Responsibility

`StableProxy` owns one stable service identity and at most one current worker.
It owns initial worker start, replacement, unavailability, exact worker exit,
creation, initialization, activation, worker return, and structural-owner
outcomes and diagnostics.

It does not own restart strategy, restart budgets, fixed membership, dynamic
membership, pool jobs, customer outcomes, or runtime execution. Its public
service protocol is exactly the worker protocol. Start, replace, and shutdown
control remain private to the structural owner.

Fresh worker creation is the actor-model law. Stable service identity across
successive workers is a derived proxy construction. Creation-before-dependent-
operation ordering, owner-only control, activation admission, and lifecycle
publication are Bombay policies.

## One aggregate, separated concerns

`StableProxy<W, P>` is the only Behavior in this family. Its six files have
one domain owner each:

- `mod.rs` owns the aggregate, every state transition, construction, and the
  sole `Behavior` implementation;
- `state.rs` passively owns the complete current and terminal state sums;
- `protocol.rs` owns commands, events, outcomes, diagnostics, and returned
  terminal data;
- `effects.rs` owns the seven named action lanes and their declared order;
- `operation.rs` owns the parent request, receipt, rejection, and settlement
  contract;
- `worker/mod.rs` owns the proxy's pending and current worker values, creation
  correlation, and shutdown/stop correlation.

The shared `atomic/worker/initialization.rs` and
`atomic/worker/activation.rs` modules own the worker-host request and result
protocol used by every atomic aggregate. They are not StableProxy child files.
The pending proxy worker stores only creation and activation values; the
emitted `CreateChild` owns the concrete worker until settlement, so no worker
type marker exists in proxy state.

The worker module is not another actor aggregate. The worker's own `Behavior`
owns its domain decisions; this module stores only the parent proxy's exact
typed authority and observations. No transition verb or arrival path owns a
file, and `mod.rs` controls the public surface.
Fixed and dynamic supervisors consume
typed proxy outcomes and must not reproduce worker creation, forwarding,
readiness, replacement, or return transitions. Direct pools own a different
direct-worker lifecycle and never consume proxy state.

Mnesis core was inspected as a DDD structure reference. Its useful pattern here
is a flat, concept-owned kernel with one aggregate root and passive state. Its
`AggregateRoot`, command dispatch, event replay, versioning, and persistence do
not belong in StableProxy: the existing `Behavior` contract already owns the
pure actor transition, and adding Mnesis would create a second aggregate model.

## Current state and action law

The implemented closed phases are:

```text
Dormant
Starting {
    Creating { stopped: Option<exact stop> }
  | Initializing { stopped: Option<exact stop> }
  | Activating { progress: WaitingForStart | Running, stopped: Option<exact stop> }
  | ReturningWorker(WorkerStopping)
}
Ready
EmptyInitial | EmptyAfter
Replacing { ReturningPredecessor | SuccessorResultAwaitingShutdown }
ShuttingDownCreation | ShuttingDownInitialization | ShuttingDownActivation
ShuttingDownWorker | ShuttingDownReplacement
Stopped(complete private retirement values)
```

H77 corrects the initial literal state lowering. Creation, initialization, and
activation remain different phases, but each owns the same exact current truth:
the correlated stop is absent or has arrived once. One `Option<ChildStopped<_>>`
directly represents that invariant; separate `*AfterStop` phase and result sums
would store arrival history and duplicate transitions. Activation separately
owns `WaitingForStart | Running`. Its stop presence remains visible to the exact
readiness transition, so stop-before-ready still cannot publish availability.
No phase is represented by coordinated flags or several correlated optional
fields.

An accepted start issues one checked `CreationId`, enters `Creating`, and emits
the complete worker submission in `CreateChild` in that same transition. The proxy
does not wait for a route result and stores no runtime nonce. Bombay either
routes the complete creation batch or returns it unchanged with
`ChildNamespaceExhausted`; the proxy then returns the complete worker and
activation policy through generic creation settlement. A foreign or stale
creation result cannot replace the pending submission.

Every staged worker creation emits one exact `ObserveChild` request in the same
`Actions`. A committed worker emits one `InitializeWorker` request.
Successful initialization returns the original activation plan and one
non-cloneable `ActivationPermit`; only those values can construct
`BeginActivation`. Bombay admits `Started` before polling the plan. Only a
matching `Ready` received after matching `Started` opens service routing.

Readiness arriving before `Started` is returned unchanged as a diagnostic. A
foreign worker, initialization, activation, shutdown result, or worker stop is
also returned unchanged and cannot advance state. `WorkerAttempt` carries
opaque issued evidence; its ordinal is diagnostic data and cannot prove
identity. Two proxies' first attempts therefore remain distinct. Address reuse,
timing, and adjacency prove nothing.

Service input in `Ready` becomes one `EstablishedDelivery` to the exact current
worker. Service input in every other phase becomes one `Unavailable` owner
outcome containing the original sender and command.

An initialization-effects rejection, activation-start rejection, or accepted
activation rejection begins exact orderly shutdown of the committed worker.
For initialization, Bombay retains the complete concrete action settlement in
the worker environment and returns only the closed
`WorkerInitializationFailure` classification plus the affine activation plan.
The proxy reports the terminal owner result only after both shutdown resolution
and exact worker stop arrive. Either arrival order retains all proxy-owned
values. If the worker stopped earlier, later initialization or activation input
consumes the stored exact stop without reopening service. If readiness arrives
first, the worker becomes ready and a later stop follows the ordinary
ready-worker law; H77 deliberately preserves this ordering distinction.

`WorkerStartResult` has three decisions: creation rejected, worker ready, or
committed worker unavailable. `WorkerCreationRejection` owns the exact
pre-commit cause. `ProxyDrain<W, P>` owns the exact committed-worker cause and
retained values. The outer result never repeats those nested causes as another
alternative. Its ready and unavailable alternatives carry the opaque worker
attempt, not a worker actor or route.

`ProxyDrain<W, P>` is doc-hidden. Its six alternatives name current worker
outcomes: initialization rejected, stopped, or completed after an observed
stop; and activation start rejected, activation rejected, or activation
completed after an observed stop. A rejection that may follow an emitted
worker-shutdown request owns `Option` of its exact shutdown receipt. `None`
means no request was emitted because the worker had already stopped; it does
not encode which transition function ran. Initialization-returned and
observer-returned stops remain separately owned because they are independent
authorities. There is no duplicate cause, forwarding constructor, projection
operation, string case label, runtime lookup, callback, or generic policy.
It never owns the worker's complete initialization action settlement. That
runtime value follows Bombay's typed environment-retirement path. This keeps
proxy and supervisor types independent of the worker's concrete send and birth
products without erasing or discarding them.
The unexpected-initialization diagnostic owns the complete
`WorkerInitializationReport<W, P>` directly. That typed input exposes no runtime action
product in ordinary application syntax and needs no second custody wrapper.

H124 normalizes all unexpected worker inputs to the same ownership equation.
Each `ProxyDiagnostic` alternative owns the complete returned input and the
copyable current `ProxyPhase`. The exact expected creation, worker attempt,
initialization, activation, or shutdown authority remains solely in proxy
state. It is neither cloned into the diagnostic nor moved out while correct
admission is still possible.

`ProxyPhase` reports the current proxy work, not private stored-stop presence.
Creating remains `Creating` and initializing remains `Initializing` after an
exact early worker stop. The stop still participates in the later transition;
the public projection cannot inspect or authorize it.

Replacement first issues the successor's checked creation ID while retaining
its complete `CreateChild` value. The exact predecessor remains owned, stable service
admission closes, and the proxy requests exact predecessor shutdown. If the
creation sequence is exhausted, the complete successor returns and the ready
worker remains unchanged. The retained creation is emitted only after the exact
predecessor stop arrives.

Predecessor stop and shutdown settlement form an order-independent join. If the
stop arrives first, one successor creation is staged immediately and the exact
pending shutdown correlation remains with the replacement. If shutdown settles
first, the complete prepared successor remains local until stop. A successor
that becomes ready before the late predecessor settlement is retained and is
not published early. Replacement from `EmptyAfter` needs no predecessor return
and still uses a fresh creation ID and the same `WorkerStart` transitions. Overlap
returns the complete submitted successor.

`WorkerStart` and `ProxyReplacement` are separate private closed state values.
The former retains only creation-through-readiness or pre-ready return; the
latter retains only proxy replacement values and the predecessor/successor
join. Neither advances itself: `StableProxy` admits each event, selects the next
aggregate state, and returns the complete `Actions`. Bombay
continues to own namespace binding, actor creation, task execution, delivery of
typed results, and terminal custody. No OCC store, lock, allocator, runtime
handle, or second lifecycle service exists in the aggregate.

Both components consume the same private `WorkerStopping` transition. It owns
only the exact shutdown-result/stop reunion and returns every unrelated input
unchanged. It owns no start, replacement, supervisor, pool, or runtime policy.

Owner shutdown is total in every implemented live phase. It closes stable
service admission immediately, emits no ordinary owner result, and never emits
the same worker shutdown twice. An uncreated successor is returned through the
existing replacement-cancellation outcome and is never created. Pending
creation still settles because its worker has already moved into the emitted
action. Initialization and activation results remain owned but cannot publish
readiness.

Initialization and activation each retain only their own exact unfinished work
and worker-return values during owner shutdown. They share `WorkerStopping`
for the shutdown-result/stop reunion, but no generic outer return product: an
initializing worker has no pending domain value corresponding to activation's
real waiting/running value. Introducing a unit or marker merely to share that
outer product is forbidden. A ready successor waiting for its predecessor
result is returned before the proxy stops. Both the predecessor result and
successor departure
remain owned regardless of arrival order. A rejected or already stopped
successor waits only for the predecessor result.

Initialization represents its returned work directly as
`Option<WorkerInitializationRetirement>` while the worker departs. There is no
parallel pending/returned enum: absence is the pending condition and presence
owns the one complete returned value. A duplicate remains outside that option
and is returned complete through diagnostics.

A stopped initialization is one outcome. Its optional `initialization_stop`
owns a second stop returned by the initializer only when the enclosing stopped
worker already owns the independently observed stop. Absence means the
initializer's stop is already the enclosing worker stop. There is no
`StoppedAfterObservation` alternative because that would preserve arrival order
instead of current ownership.

Once the worker has stopped, each shutdown join stores one current product:
the worker, the exact stop, the initialization or activation value still being
awaited or returned, and `Option<EstablishedShutdownResolved<_>>`. A receipt is
present only when the proxy emitted and settled a shutdown request; it is absent
when the worker had already stopped and no request was emitted. There are no
separate `AfterDeparture`, `AfterStop`, or terminal `*AfterStop` alternatives.
Those labels described arrival history and did not change any later decision.

`ProxyRetirement` is not an application API or runtime service. It is the final
private state of the concrete stopped `StableProxy`. Bombay must move that
whole behavior together with its environment residual through the existing
Driver retirement barrier, as specified in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md#engine-retirement-and-root-custody).

## Total interpretation

`Actions::creates` owns worker creation. `ProxyEffects` declares these send
lanes in stable order:

```text
worker observations
worker initializations
worker activations
worker shutdowns
worker deliveries
owner outcomes
diagnostics
```

Every runtime-local or structural-owner lane uses the one
`InterpreterRequests` product. Worker service uses `EstablishedDelivery`.
`Actions` still interprets child creation before these sends. A compile witness
requires the concrete `StableProxy::Sends`, not a homogeneous stand-in, to
implement the generic total interpretation contract.

The operation module also owns the crate-private receipt correlation evidence.
Its tests prove that receipt consumption returns one affine operation id while
the established proxy capability remains repeatable. FixedSupervisor has no
parallel receipt model or fixture module.

No proxy-specific interpreter adapter exists. Corruption retains the committed
prefix and exact untouched suffix under
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md).

## Bombay realization

Bombay owns concrete child hosts, initialization action settlements, activation
work, task retention, mailboxes,
observation, and retirement. The exact single-Driver changes required for
initialization, activation, parent/root custody, and retirement residuals are
normative in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md#exact-bombay-changes-for-worker-initialization-and-activation).
Bombay production remains read-only in this campaign; no Behavior-side runtime
substitute is permitted.

## Verification and external integration

The local aggregate is `feature-complete`. Current verification supplies
exhaustive small shutdown interleavings, independent models, generated longer
sequences, two stateful fuzz targets, optimized replay, compile denials, and
affine drop witnesses. The ready-worker target exercises exact and foreign
shutdown inputs; the replacement target exercises overlap, cancellation,
successor creation, and both predecessor-return orders. They share only the
identical four-state worker-return join.

Bombay must supply the documented parent-to-root custody contract. The atomic
implementation targets that contract without adding a Behavior-side runtime.
This external integration requirement prevents a whole-system completion claim;
it does not reopen StableProxy's locally owned transitions.
The repository-wide gates are owned by
[`atomic-actor-verification.md`](atomic-actor-verification.md).

There is one canonical construction path described in
[`atomic-actor-devx.md`](atomic-actor-devx.md). No legacy `With*` wrapper,
structural route, compatibility alias, public nonce, readiness flag, erased
callback, or application-authored final behavior type may return.

StableProxy deliberately has no `ActorDrainPolicy` and schedules no deadline.
The AA-01 law leaves deadline expiry and forced actor-graph retirement outside
its implementation evidence. FixedSupervisor, DynamicSupervisor, FifoPool, and
KeyedPool own their aggregate shutdown policies; Bombay root custody owns the
remaining runtime transfer. Adding a proxy timer lane would take on another
aggregate's responsibility and is prohibited.
