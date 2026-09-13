# FIFO pool

Status: `feature-complete` for the local aggregate; final whole-catalogue audits
and Bombay root custody remain.
Ordered dispatch, assignment reunion, direct-worker start/activation, recovery
eligibility, role retirement, and dispatch shutdown custody have focused models
and production tests. Complete worker-source settlement, immediate and delayed
replacement release, restart-limit denial, topology disposition, and exact
restart schedule/timer correlation now have focused production tests. Exact
operating stops before readiness and their returned initialization/activation
values now have focused production tests without another worker state machine.
Standalone pre-ready failures now stop their installed worker and wait for the
exact shutdown/exit reunion before recovery. Late activation results during
drain advance only the exact existing progress and never restore eligibility.
Restart schedules outside the actor remain pending through drain; accepted
restart timers transfer their cancelled replacement immediately. Orderly
retirement releases the drained roster, while forced retirement retains the
unchanged unresolved roster and exact cause in the final concrete behavior.
Generated operating and FIFO-only fuzz evidence now exist. The canonical
worker and direct-construction compile contract, two actual timer-wrapper
orders, six focused compile denials, and result-type inversion are retained.
The warning-custody audit, complete viable mutation audit, and replacement
performance baseline are retained. The complete FIFO suite passes 90/90 in
debug and optimized modes. Its 305-candidate mutation run has 118 caught, 187
unviable, zero surviving, and zero timed-out candidates without exclusions.
The optimized canonical fixture sustains a median 2,732,844 complete assignment
cycles/s across five 100,000-cycle runs; this is pure aggregate throughput, not
mailbox or Engine throughput.
Bombay owns the documented terminal-custody change. The complete
transition law is owned only by
[`actor-laws/fifo-pool.md`](actor-laws/fifo-pool.md).

A test-only independent customer desk now compares the production aggregate's
public customer actions after every input in all 24 permutations of assignment
delivery acceptance, completion, exact worker exit, and shutdown. Its
oldest-observation queue independently predicts first-terminal precedence,
route and payload custody, shutdown return, and terminal uniqueness. It reads no
private aggregate state. Recovery and full-drain sequence parity are closed by
the direct independent comparisons below.
An independent recovery desk also matches all 20 combinations of operating or
shutdown ownership, both topology reactions, and every preparation outcome.
An independent retirement desk matches every ordering of one ready worker's
stop, shutdown return, deadline rejection, accepted scheduling, and exact
deadline firing. It proves normal role release and forced role retention without
reading private pool state. A generated queue desk additionally compares 192
shrinkable scripts of up to 128 repeated submissions, delivery acceptances, and
completions against the real aggregate after every applicable input. Its sole
`VecDeque` predicts bounded admission and oldest-first dispatch without reading
private pool state; a newest-first inversion fails on the exact next payload.
The FIFO-only fuzz target retains the real emitted assignment plus concrete
stale and foreign inputs. It discovered and now preserves both exact worker-stop
orders in which accepted or rejected delivery settlement reunites the final
assignment prerequisite. Three named seeds, 5,000 development runs, and 5,000
optimized runs preserve terminal uniqueness and every unrelated action lane
without importing a private pool phase.

## Responsibility

`FifoPool` owns direct worker incarnations, one bounded global backlog,
immutable admission order, direct assignment authority, completion correlation,
worker interruption, at-least-once retry or exact failure return, direct-worker
restart policy, exactly one terminal customer outcome per accepted job, and
complete shutdown extraction and drain.

It contains no stable proxy or hidden supervisor. It owns no key, binding,
per-role queue, affinity selector, or keyed management operation. It is not a
mode of `KeyedPool`.

## Collaboration and policy

Fresh worker allocation is the actor-model law. FIFO admission, circular idle
selection, immutable retry ordering, interruption disposition, restart policy,
and terminal customer custody are Bombay pool constructions. Assignment and
direct-worker lifecycle values may be shared with KeyedPool only after the same
law suite passes both consumers without modes, ignored outcomes, or weaker
errors.

The pool retains the canonical customer obligation and route. A worker receives
only a moved assignment containing execution payload and affine completion
authority. Delivery settlement, completion, and exact worker exit form the
four-way order-independent join owned by the aggregate; a generic settlement
lane cannot replace it.

A queued obligation stores only its immutable admission order and an optional
assigned role. The role remains necessary for a later assigned-job return. The
event that caused retry does not remain in queue state because worker exit and
rejected delivery have identical future behavior. Forced-retirement and emitted
diagnostic payloads are different: they remain complete affine custody until
Bombay or the selected diagnostic route accepts them.

Exact worker shutdown reuses the generic protocol described in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md): the lifecycle
host retains the complete `ActionItemResult<ShutdownEstablished<...>>`, while
the pool later receives only `EstablishedShutdownResolved<P>` and the exact
`ChildStopped` input. Direct-worker state must not duplicate the emitted request
or import StableProxy's private worker state.

The initial factory is consumed only while constructing the all-or-none roster.
Its rejection is the shared `InitialWorkerRejection` described in
[`atomic-actor-architecture.md`](atomic-actor-architecture.md), embedded in
`FifoConstructionRejected` beside FIFO policy. Automatic recovery never calls
that closure inside `Behavior`; permanent and
transient recovery emit the generic typed worker-source request for one role,
which Bombay executes and settles outside the actor. Temporary recovery has no
source. One shared worker classification defines normal and abnormal stops for
both fixed supervision and FIFO. FIFO then owns the closed decision to prepare,
wait while its single affine source is in use, retire the role, or stop the
pool. A second simultaneous eligible stop cannot duplicate the source and waits
in declaration order. FIFO still owns budget, timing, and topology disposition.
Every successful, worker-rejected, source-rejected, corrupt, or unattempted
preparation returns the affine source exactly once. Under `RetireRole`, that
source immediately serves the first waiting role before ordinary dispatch;
under `StopPool`, ordinary shutdown drains surviving workers. Delayed restart
release admits only its exact schedule result and timer once.
When an exact temporary worker stop becomes actionable, `RetireRole` removes
that role from future dispatch, fills any surviving ready worker from the same
ordered backlog, and returns the remaining backlog only when no serviceable or
recoverable role remains. `StopPool` enters the ordinary pool shutdown path and
requests shutdown only from workers that have not already stopped. A temporary
worker never occupies recovery state.

An exact initialization or activation failure without a prior stop does not
retire an installed worker by assertion. The pool transfers the complete
returned failure through diagnostics, requests exact worker shutdown, and reuses
the same `Stopping` shutdown/exit reunion as rejected assignment delivery.
Activation failure releases its occupied capacity and admits the next waiting
role in declaration order. Recovery begins only after both shutdown settlement
and exact worker exit arrive.

Once pool shutdown begins, an activation already dispatched remains correlated
until its exact result returns. Exact `Started` advances that worker from
dispatched to activating. Duplicate or foreign starts are diagnostic-only. A
later readiness or rejection retires the activation progress into diagnostics;
it cannot make the worker idle, assign queued work, or start recovery. The same
worker remains in the ordinary exact shutdown/exit reunion.

FIFO and KeyedPool implement that retirement equation once in the direct-worker
model. Retirement stores `Option<WorkerStartupCustody>`: an inhabited value is
the exact activation plan, initialization attempt, activation permit, activation
start, or activation still owned locally; absence means no startup value remains.
It does not use service readiness to stand for absence. The shared transition
admits only exact initialization and activation inputs and returns complete
foreign, duplicate, rejected, or late values for aggregate diagnostics. Queue,
customer, binding, recovery, deadline, and final actor policy remain outside it.

A worker creation emitted before shutdown remains admissible during drain. If
the exact actor is established after admission closes, FIFO starts no worker
initialization or activation work: it emits one exact shutdown and holds the
activation value in the drain member until exact stop. If that stop was already
observed, FIFO retires the member immediately and transfers the activation to
diagnostic custody. Rejected creation transfers the returned worker,
activation, and any prior stop together. Foreign or reversed creation results
leave every pending member unchanged.

An emitted worker-preparation request remains pending after shutdown even when
all direct workers have retired. Only its private exact ticket can release that
drain member. Its complete accepted, worker-rejected, source-rejected, corrupt,
or unattempted return restores the affine source when valid and transfers every
cancelled submission, rejection, and stopped-worker value to diagnostics. It
cannot create a replacement or emit a restart schedule. A foreign return leaves
the member unchanged, and normal pool retirement waits for the exact return.

An emitted restart schedule likewise remains pending until its exact accepted,
rejected, corrupt, or unattempted return. Shutdown then transfers the stopped
worker, uncommitted replacement, and schedule return to diagnostics without
creating another worker. A foreign schedule cannot release that custody. Once
Timers accepts the schedule, Timers alone owns whether an elapsed event will be
published; pool shutdown immediately transfers the cancelled replacement and
does not wait for that event. Any later elapsed event is stale input. Worker
restart and actor-drain schedules use distinct identities and cannot settle one
another.

Normal retirement is the payload-free `Stopped` alternative; it retains no
role or shutdown history. Shutdown-identifier exhaustion, an exact returned
deadline schedule, or exact deadline firing instead enters the private
`ForcedRetirement` alternative. That alternative owns the unchanged ordered
drain roster and the exact cause. It does not reinterpret unresolved worker
creation, activation, assignment, recovery, or shutdown values as completed.
Bombay moves the whole final concrete behavior together with its environment
residual through the existing Driver retirement barrier. FIFO adds no residual
action lane, runtime adapter, actor loop, or task. Work still outside the actor,
including an accepted activation or worker-source request, remains in Bombay's
environment residual rather than being duplicated inside the pool.

## Realization gate

Canonical construction, submission, outcomes, and the required
`assignment.complete(result)` worker expression are owned by
[`atomic-actor-devx.md`](atomic-actor-devx.md). The implementation must prove
capacity zero/full boundaries, FIFO fill, multiple interrupted reinsertion,
every assignment join order, authority reunion, duplicate/stale/foreign
completion, recovery, complete shutdown, and exactly one terminal outcome.

No supervisor, proxy, generic pool engine, selector placeholder, structural
parent route, named interpreter send lane, helper type alias, or alternate pool
spelling may survive.
Repository-wide verification status and remaining gates are owned by
[`atomic-actor-verification.md`](atomic-actor-verification.md).
