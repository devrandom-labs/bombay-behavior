# Fixed supervisor

Status: `active`. The current implementation covers construction, startup,
coordinated recovery, diagnostics, lifecycle publication, management queries,
shutdown, and forced retirement. Four stateful fuzz targets cover shutdown,
recovery, delayed release, and three-role recovery correlation. Remaining
owner-partitioned viable mutation review, the local closure audit, the final
repository gates, and Bombay root custody remain open.

The complete transition matrix is owned by
[`actor-laws/fixed-supervisor.md`](actor-laws/fixed-supervisor.md).
This document maps that law to the current Bombay Behavior design without
repeating the matrix or its research history.

## Responsibility

`FixedSupervisor` owns:

- one non-empty ordered roster of unique application roles;
- one `StableProxy` for each role;
- each role's current membership and worker status;
- activation capacity across unresolved proxy operations;
- `OneForOne`, `OneForAll`, and `RestForOne` recovery selection;
- restart eligibility, limits, release timing, and recovery correlation;
- topology-failure disposition;
- optional lifecycle publication;
- operational diagnostics;
- status and capability queries; and
- complete actor-graph retirement.

It does not create or replace workers directly. `StableProxy` owns worker
startup, replacement, service forwarding, and worker shutdown. The supervisor
coordinates several stable proxies without copying that law.

It is neither a dynamic membership supervisor nor a pool. Its roster never gains
a role after construction, contains no customer job, and is not selected by a
mode flag shared with another aggregate.

## Construction

`fixed(...)` is the sole construction path. Applications provide:

- a function that prepares the initial `WorkerSubmission` for each role;
- validated `OrderedRoles`;
- `ActivationPolicy`;
- `Recovery`;
- `FailureReaction`;
- `ActorDrainPolicy`; and
- `DiagnosticDisposition`.

Lifecycle publication is the builder's only optional transformation. Every
semantic policy is explicit; no no-op recipient or placeholder callback is
required.

The initial worker function runs only while the builder prepares the roster. It
is not stored in the resulting behavior and cannot run during a transition.
Automatic recovery instead emits one typed `PrepareWorkers` action. Its exact
settlement returns the worker source, prepared submissions, rejection, and
untouched role names without an application callback inside the actor.

Construction either returns one supervisor owning every prepared role and
submission, or returns `FixedConstructionRejected`. Its `workers` value is the
shared `InitialWorkerRejection`: the function, accepted prefix, rejected role
and reason, and unexamined suffix. The shared representation is owned by
[`atomic-actor-architecture.md`](atomic-actor-architecture.md); this document
owns only fixed-supervisor construction policy. A duplicate role is rejected by
`OrderedRoles` before the supervisor exists.

## Roster ownership

The roster preserves declaration order. Each role has one immutable roster
position used only when the law observes order. The position is not an address,
actor identity, or public path.

A role is owned by either an independent member or one admitted recovery. An
admitted recovery owns a non-empty unique set of participating members and the
stable position of its trigger. A member cannot occur in two recoveries. When a
member returns to service it can leave the recovery independently; the recovery
retires after its final participant leaves.

The current roster uses its existing ordered collections to answer selection,
authorization, query, and shutdown-order questions. It does not keep parallel
membership tables or before/after history.

## Startup and activation capacity

Initialization emits one fresh `StableProxy` creation and one observation
request for every declared role. Creation results are accepted only for the
expected creation ID and kind. A rejected creation leaves that role without a
proxy; a foreign or stale result is returned unchanged.

`ActivationPolicy` limits unresolved proxy operations, not actor count. The
supervisor authorizes waiting roles in declaration order while capacity exists.
An exact accepted proxy-operation settlement is required before the matching
proxy outcome is admissible. Capacity is released when the admitted outcome
resolves, even if another member value still awaits its matching stop.

A ready outcome makes the member available through its stable proxy. A failed
startup follows the configured diagnostic disposition and topology reaction.
Stopping the complete supervisor reuses ordinary supervisor shutdown; retiring
one member reuses that member's exact proxy retirement.

## Worker-stop classification and recovery selection

Recovery policy classifies an exact worker stop:

- `Temporary` never starts automatic recovery;
- `Transient` recovers only after an abnormal stop; and
- `Permanent` recovers after every exact stop.

An ineligible stop leaves the member empty. An eligible stop selects members by
strategy:

- `OneForOne` selects only the stopped role;
- `OneForAll` selects the complete eligible roster; and
- `RestForOne` selects the stopped role and eligible roles declared after it.

Normal or abnormal classification is derived from the exact owned worker stop
at this decision. The accepted stop does not retain a second classification
that could disagree with the terminal outcome.

Selection is all-or-nothing. If a required role cannot participate, the
unchanged roster is restored and the configured topology response is applied.
Disjoint recoveries may coexist, but their role sets may not overlap.

One accepted selection moves the sole `WorkerSource` into one `PrepareWorkers`
action. The supervisor retains the exact recovery correlation and cannot issue
replacement work until that action returns. Accepted preparation pairs each
selected role with exactly one `WorkerSubmission` in declaration order. Worker
rejection, source rejection, interpreter corruption, and no-attempt return
complete ownership through the same generic settlement vocabulary.

The worker source has one private custody sum: it is available to the
supervisor, owned by one emitted worker preparation, or retained as that
complete returned preparation while the supervisor retires. There is no
parallel optional return field. A shutdown return changes custody only; it
cannot reopen recovery or issue another preparation.

## Restart admission and release

Returned prepared workers are not yet authorized replacements. In the same
actor transition that accepts the exact preparation result, the supervisor
carries every prepared worker directly into one restart-admission transaction.
It does not store a separately observable prepared roster phase. The transaction
combines:

- its checked lifetime recovery count;
- the inclusive restart window and maximum;
- checked release calculation;
- one non-reused recovery ID; and
- for delayed release, one non-reused timer ID and generation.

`RestartLimit` owns the inclusive window and maximum. `RestartRelease` owns the
release calculation.

The transaction commits all of those values together or returns them unchanged.
It cannot partially spend a count, history entry, recovery ID, or timer key.
Its six denial alternatives are stored once in `RecoveryDenialReason`; recovery
admission moves that value and the complete triggering worker stop into
`RecoveryDenied`, which exposes both by reference without rebuilding a parallel
diagnostic sum. Denied prepared workers remain in the topology response selected
by `FailureReaction`; admitted workers become one durable recovery batch. Timer
schedule rejection does not use this stop transfer: its diagnostic owns the
rejected timer request while topology retains the worker stop.

The requested replacement count retains the native non-empty roster cardinality.
Only an accepted budget charge is narrowed to the configured `u32` limit, so an
oversized coordinated recovery is reported as an unchanged restart-limit denial
rather than internal corruption.

Release policies are:

- immediate;
- constant delay;
- linear delay, `initial × ordinal`; and
- exponential delay, `initial × 2^(ordinal - 1)`.

Arithmetic is checked before applying the configured maximum. Overflow is a
typed denial, never saturation or panic. Delayed release emits the generic
`ScheduleAfter` action. Only its exact accepted timer may later authorize the
recovery; rejected, stale, foreign, early, or duplicate timer inputs cannot.

## Replacement progress

Each participating member keeps its stable proxy and one current replacement
product. That product owns:

- the previous worker attempt and readiness;
- the exact predecessor stop, if it has arrived; and
- the current proxy response: input pending, outcome pending, returned outcome,
  or rejected input.

The exact input settlement, predecessor stop, and proxy outcome update only
their corresponding value. The accepted input settlement must precede the proxy
outcome. After that admission, predecessor stop and proxy outcome may arrive in
either order and produce the same restart. Duplicate, stale, early, or foreign
inputs return unchanged.

Successful readiness returns the member to service and publishes lifecycle
events in semantic order. Input rejection or proxy failure preserves the
complete diagnostic payload and enters member retirement or supervisor shutdown
according to policy. The next waiting member may use released activation
capacity even while an earlier predecessor stop remains outstanding.

## Diagnostics and lifecycle publication

`DiagnosticDisposition` has two exhaustive choices:

- `DeliverTo` attempts one typed diagnostic delivery; or
- `Terminate` transfers the diagnostic with the stopped behavior.

`FixedDiagnostic` distinguishes startup failure, proxy-input rejection, worker
preparation failure, restart denial, restart scheduling failure, unavailable
service work, and unexpected typed input. Unexpected input retains the complete
input while the unchanged supervisor retains every expected correlation.
Terminal startup failure likewise leaves every unrelated roster member inside
the stopped supervisor; only Bombay's generic retirement transfer may move that
remaining ownership outward.

A preparation failure is valid for every recovery strategy and selected roster
size. Non-trigger participants return to their exact prior member states; the
trigger remains stopped. The diagnostic retains the prepared prefix, exact
rejection or interpreter disposition, and untouched suffix, while the supervisor
separately retains the reusable worker source and applies `FailureReaction`.

`FailureReaction` is independent of diagnostic delivery:

- `RetireMember` removes only the unavailable role; or
- `StopSupervisor` retires the complete supervisor.

Optional `FixedLifecycle` publication reports started and restarted workers,
ineligible and admitted worker stops, unavailable service commands, and member
retirement. When no lifecycle recipient is configured, no lifecycle value is
constructed. Lifecycle publication never replaces the operational diagnostic
that owns a failure cause.

## Management protocol

The public `FixedCommand` protocol contains:

- `Status`, returning every member status in declaration order;
- `Capability`, returning the stable service capability for one role or the
  exact unavailable/unknown-role result; and
- `Shutdown`.

Queries do not copy or change roster ownership. They remain admissible while the
supervisor is operating or draining. Only a ready member yields a service
capability; startup, empty, recovering, stopping, and retired members return the
corresponding `UnavailablePhase`.

Service commands use the stable proxy. If the proxy has no ready worker, the
supervisor accepts the exact returned command only while that proxy is still a
live member. The command is then moved to either the configured lifecycle
publication or `WorkerUnavailable` diagnostic. It is never stored as roster
state.

## Shutdown and retirement

The first shutdown closes recovery admission and projects every live member in
declaration order. It emits at most one shutdown operation for each live stable
proxy. Repeated shutdown emits none.

Each proxy retirement is a commutative join of its exact shutdown-operation
result and exact proxy exit. Either may arrive first; neither alone retires the
member. A rejected operation remains complete. A member already stopping is
adopted without issuing a duplicate operation.

Other work already outside the actor remains required during shutdown:

- proxy creation results;
- initial and replacement proxy-operation settlements;
- proxy outcomes and worker stops;
- worker-preparation settlements; and
- restart-schedule settlements and exact elapsed timers.

Late results may restore custody or finish retirement but may not reopen
recovery or issue replacement work.

`WaitForActorGraph` waits for every owned proxy to retire.
`RetireActorGraphAfter` also emits one exact deadline schedule. Its accepted
timer, or an exact schedule rejection/no-attempt/corruption, selects forced
retirement while preserving the complete unresolved supervisor and exact cause.
Foreign, stale, early, and duplicate deadline inputs return unchanged.

The stopped value retains every worker, submission, operation, result, stop,
timer request, and diagnostic still owned by the supervisor. Logging is not
custody, and Behavior does not spawn a task to dispose of those values.
Because retirement consumes or drops that complete value instead of inspecting
each private field in the producing crate, the corresponding fields carry
checked dead-code expectations. Replacing them with unit is forbidden by the
drop-count ownership regressions.

## Bombay collaboration

Behavior owns the pure transition and complete `Actions`. Bombay must:

1. interpret the named action lanes in declared order;
2. return every exact accepted, rejected, blocked, corrupt, or unattempted item;
3. admit initialization actions before ordinary ingress;
4. move the complete stopped supervisor into parent/root retirement custody;
5. keep accepting settlements through that retirement path without reopening
   the stopped actor; and
6. use the existing Engine driver and retirement barrier.

The required Bombay change is specified in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md). It is an upstream
composition requirement, not permission to add a FixedSupervisor-specific
driver, detached task, mailbox, settlement adapter, or callback.

Canonical construction and application syntax are owned by
[`atomic-actor-devx.md`](atomic-actor-devx.md). Verification ownership is mapped
in [`atomic-actor-verification.md`](atomic-actor-verification.md).
