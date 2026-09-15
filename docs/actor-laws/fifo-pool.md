# FIFO-pool falsification model

## Status and authority

This document is the AA-30 independent semantic model for one FIFO worker
pool. It is a non-production falsification oracle. It does not select Rust
types, define another behavior algebra, implement a pool, or mark a production
coverage row implemented. `docs/engineering/atomic-actor-solution.md` remains the
production design authority.

The actor-model laws used here are isolated processing of one communication,
communications to known recipients, fresh actor creation, and explicit next
behavior. The pool itself, direct worker ownership, FIFO admission, bounded
backlog, circular worker selection, activation authorization, recovery,
at-least-once retry, opaque completion authority, action settlement, and
actor-graph drain are derived constructions or deliberate Bombay policies.
Neither Agha's actor model nor message-arrival indeterminacy supplies FIFO pool
semantics.

The model is deliberately not a supervisor containing a work queue. It reuses
policy values where their meanings are identical, but it contains no stable
proxy, supervisor member state, proxy operation, public worker capability,
nested supervisor behavior, keyed affinity, or redispatched supervisor event.

Runtime route selection is owned normatively by
`docs/atomic-runtime-settlement.md`. The pool issues and stores only a
`CreationId` for each direct worker creation; it never chooses or stores a
runtime route before emitting `CreateChild`. Any older `reservation` label below
denotes that creator-visible correlation before interpretation and routed
evidence only in the returned runtime settlement. It does not authorize a
separate route request, waiting state, or public route.

AA-30 is exercised by the clean-room `FifoPool` aggregate and independent
customer, recovery, retirement, and queue models. H04b proves typed assignment
completion lowering and H02 implements total action settlement. H03 assigns
terminal lifecycle custody to Bombay through the documented upstream contract;
that Engine/root integration remains an upstream gate rather than a FIFO
aggregate gap. Legacy `WorkerPool` tests remain historical characterization
evidence, not an architecture or independent model.

The model must falsify designs that make any of these claims false:

1. The roster is non-empty, ordered, and contains each semantic worker role
   once.
2. Every installed or replacement worker is a freshly created actor. A role,
   route, attempt, or sequence value is not actor identity.
3. The stable pool actor is the only public service capability. Exact worker
   capabilities remain pool-private.
4. The pool directly owns worker creation, initialization, activation,
   readiness, recovery, and drain. No nested proxy or supervisor owns them.
5. Every accepted job has exactly one authoritative customer obligation and
   is either queued, assigned, or being terminally settled—never two of them.
6. Backlog capacity counts queued jobs only. Zero capacity permits immediate
   assignment and rejects waiting ownership.
7. Every worker has at most one assignment. Only an exact ready incarnation is
   eligible.
8. No queued job coexists with an eligible idle worker after a completed fold.
9. Jobs leave the queue in admission order. Worker choice starts at one
   circular declaration-order cursor and advances after each assignment.
10. A completion must match its opaque authority, assignment, creator-local
    child nonce, private worker-birth evidence, role, and active phase. It
    resolves one customer outcome.
11. A matching worker stop resolves its active assignment once before recovery.
    `Retry` is at-least-once execution and preserves FIFO admission order.
12. Assignment delivery, completion, and worker stop may arrive in any order.
    Their join cannot duplicate a retry, terminal customer outcome, or recovery
    decision.
13. Irrecoverable topology loss cannot strand accepted queued or assigned work.
14. Shutdown closes admission, returns every queued and assigned job once,
    disables dispatch/retry, and drains every owned or still-creating worker.
15. Stale, duplicate, foreign, wrong-worker-birth, and contradictory facts
    preserve current ownership and have an explicit diagnostic disposition.
16. Every accepted input produces one next state and one complete named action
    product. No effect is ambient.

“Atomic” means one local `Behavior` fold commits one pool state and its complete
`Actions`. It does not claim atomic delivery, creation, execution, recovery, or
customer observation across actors.

## Model vocabulary

The following names describe the oracle. They are not proposed public Rust
names.

```text
Role = one semantic worker label, unique inside this pool
RoleOrder = immutable declaration order retained for the pool lifetime

WorkerBirthAttempt = fresh non-reused CreationId issued by the pool
RoutedWorkerBirth = interpreter-private { attempt: WorkerBirthAttempt, route }
WorkerBirthEvidence = opaque non-routable evidence for one committed birth
WorkerRecipient = pool-private exact routable capability for one committed worker
WorkerIncarnation = { recipient: WorkerRecipient, evidence: WorkerBirthEvidence }

BirthKind = Initial | Replacement { recovery: RecoveryTicket, replaces: WorkerBirthEvidence }
WorkerSubmission = complete owned worker definition and activation work

RequestCorrelation = caller-authored opaque submission correlation, echoed only
JobId = fresh non-reused customer-visible correlation issued by the pool
AdmissionOrdinal = fresh non-reused private total order for one accepted job
AssignmentId = fresh non-reused private correlation for one dispatch attempt
CompletionAuthority = opaque affine authority bound to AssignmentId and WorkerBirthEvidence
CompletionCorrelation = pool-retained non-authorizing half of that exact authority
CompletionEvidence = consumed CompletionAuthority paired with one result
DispatchCorrelation = {
    assignment: AssignmentId,
    retained: CompletionCorrelation,
    authority: CompletionAuthority,
}

RecoveryTicket = fresh non-reused correlation for one role recovery
RecoveryOrdinal = one-based admitted recovery count for that role
TimerCorrelation = { timer, generation }
ActivationTicket = fresh non-reused correlation occupying one authorization slot
ShutdownCorrelation = exact { role, WorkerIncarnation }
```

A child route, role, attempt, job, admission ordinal, assignment, recovery
ticket, timer, or activation ticket is correlation, not actor identity. Only
committed fresh creation yields `WorkerIncarnation`. Its recipient is routable
only inside the pool's effects and lifecycle state; customers and worker
messages never receive it as a service capability.

`CompletionAuthority` has no public constructor or inspectable fields. It is
the affine authority half of one freshly issued pair; the pool retains the
non-authorizing `CompletionCorrelation` half. The worker can only consume its
half with the assignment to form
`assignment.complete(result)`. The resulting completion chooses no customer,
pool address, role, parent path, send lane, or inspectable worker evidence.
The current `ChildReport` boundary attaches only the creator-local child nonce,
not an exact `WorkerIncarnation`. Exact completion matching therefore combines
that nonce with the authority's opaque `WorkerBirthEvidence`; retained
comparison evidence cannot manufacture a completion. A concrete lowering of
that private evidence remains a blocker rather than an interpreter assumption.

Every correlation allocator has one complete result:

```text
Reservation<T> = Reserved(T) | Rejected(Exhausted | Collision { candidate: T })
```

A rejection never wraps, overwrites, or reuses a correlation. The transition
rules below say whether it rejects an unaccepted submission, returns an
accepted job, retires a worker attempt, or begins actor-graph failure. Merely
declaring a value fresh is not a total allocation law.

## Construction domain

One complete definition contains:

```text
FifoPoolDefinition = {
    initial_factory,
    roles: NonEmptyOrderedUnique<Role>,
    activation: ActivationContract,
    activation_limit: PositiveMaximum,
    recovery: RecoveryPolicy,
    backlog_capacity: NonNegativeMaximum,
    interruption: Fail | Retry,
    distribution: FIFO,
    actor_drain: ActorDrainPolicy,
    diagnostics: DeliverTo { route } | Terminate,
}
```

`RecoveryPolicy` contains eligibility, the typed worker source required by
permanent or transient recovery, restart budget and inclusive window, immediate
or checked delayed timing, and `RetireRole | StopPool` topology failure.
Temporary recovery contains no worker source. Recovery is always one role at a
time; a FIFO pool has no one-for-all or rest-for-one strategy.

`ActorDrainPolicy` is exactly:

```text
WaitForActorGraph
RetireActorGraphAfter { deadline }
```

Construction fails for an empty roster, duplicate role, zero activation
limit, invalid deadline, missing policy, or worker protocol that cannot accept
the exact assignment product. FIFO construction has no selector or placeholder
selector.

Before the pool exists, the initial factory is invoked for every role in
declaration order and every worker reservation is fallibly prepared. If one
factory or reservation fails, the construction error owns every already
prepared definition and correlation; no partial fleet, budget charge,
activation occupancy, or `Actions` commits. This is a Bombay all-or-none
initial-preparation policy. Later creation and activation realization resolve
independently for each committed role.

The worker sum may be heterogeneous only when every variant accepts one common
concrete assignment protocol and produces one common concrete result type.
Static closed sums are allowed; erasure and runtime registries are not.

AA-30 selects no builder spelling, typestate, public identifier representation,
trait, wrapper, alias, or macro.

## Pool state

```text
FifoPoolState =
    Operating {
        definition,
        members: OrderedMemberMap,
        backlog: Fifo<QueuedJob>,
        cursor: RoleCursor,
        activation: ActivationCapacity,
        budget: RestartBudget,
        allocators,
    }
  | Draining {
        definition,
        members: OrderedDrainMap,
        deadline: DeadlinePhase,
        allocators,
    }
  | Stopped
```

`OrderedMemberMap` has exactly one member for each declared role. Roles are
never inserted, removed, or reordered. `Retired` remains as a terminal cell so
cursor order and late-fact classification do not depend on sequence arithmetic.

The backlog owns jobs in original admission order. `backlog_capacity` is the
new-admission waiting bound: a submission may queue only while the current
backlog length is below it. An interrupted already-accepted assignment under
`Retry` may re-enter even when that admission bound is full. Reinsertion is by
its immutable `AdmissionOrdinal`, before every later-admitted queued job and
after every earlier-admitted queued job; it is not unconditional front
insertion. The absolute queue bound is therefore
`backlog_capacity + roles.len()`, because each role can contribute at most its
one formerly active assignment. There is no unbounded overflow and no second
in-flight map: a live assignment is owned by its exact member.

`RoleCursor` denotes the first declaration-order position inspected for the
next assignment. Selection wraps once, skips non-idle and retired roles, and
chooses the first exact `Idle`. After assignment to role `r`, the cursor moves
to the declaration-order successor of `r`; retirement never compacts or
reorders the ring.

`ActivationCapacity` is derived from member states containing an occupied
`ActivationTicket`. A declaration-order queue is derived from members waiting
for authorization; correlated flags or a second occupancy counter are
forbidden.

## Customer ownership sums

```text
CustomerObligation = {
    job: JobId,
    admitted_at: AdmissionOrdinal,
    customer: CustomerRoute,
    retained_payload,
}

QueuedJob = {
    obligation: CustomerObligation,
    assigned_role: None | Some(Role),
}

AssignedJob = {
    obligation: CustomerObligation,
    assignment: AssignmentId,
    completion: CompletionCorrelation,
    worker: WorkerIncarnation,
    role: Role,
}

AssignmentCommand = {
    execution_payload,
    completion: CompletionAuthority,
}
```

The queue owns `QueuedJob`. One busy member owns `AssignedJob`. The worker owns
only the moved `AssignmentCommand`; it never owns the customer route or the
canonical customer obligation.

Both `Fail` and `Retry` require a retained canonical payload while a cloned
execution payload is sent to the worker. Clone preparation occurs before the
candidate transition commits. If application cloning unwinds, no next state or
`Actions` exists; the model does not promise to reconstruct a value already
moved into a panicking fold.

The customer protocol is one closed sum:

```text
AdmissionOutcome =
    Accepted { request: RequestCorrelation, job: JobId }
  | Rejected { request: RequestCorrelation, payload, reason: AdmissionRejection }

TerminalCustomerOutcome =
    Completed { job: JobId, role: Role, result }
  | ReturnedQueued { job: JobId, payload, reason: QueuedReturnReason }
  | ReturnedAssigned { job: JobId, role: Role, payload, reason: AssignedReturnReason }
```

`RequestCorrelation` is supplied by the caller and echoed unchanged in both
admission outcomes. The pool neither allocates it nor trusts it as internal
job identity; customers using one route can still distinguish concurrent
requests. Admission commit moves it into the emitted `Accepted`; it is not
retained in the accepted job. Rejection moves it into `Rejected`. If either
delivery rejects, the lifecycle host owns that complete outcome. `Accepted`
and `Rejected` are admission outcomes. The other three are the only terminal
outcomes. A rejected submission was never accepted. A terminal outcome removes
its obligation from actor state when the delivery attempt is emitted; rejected
outcome delivery transfers that exact complete value to the lifecycle host and
never reconstructs an active job.

## Complete member sum

```text
Member =
    Creating {
        role,
        reservation: WorkerReservation,
        kind: BirthKind,
        prior_stop: NoStop | StoppedBeforeBirth { stop },
    }

  | Initializing {
        role,
        worker: WorkerIncarnation,
        kind: BirthKind,
        init_attempt: InitAttempt,
    }

  | WaitingForActivationAuthorization {
        role,
        worker: WorkerIncarnation,
        kind: BirthKind,
        activation_permit,
        activation_plan,
    }

  | ActivationDispatched {
        role,
        worker: WorkerIncarnation,
        kind: BirthKind,
        ticket: ActivationTicket,
        begin_attempt: ActivationAttempt,
    }

  | Activating {
        role,
        worker: WorkerIncarnation,
        kind: BirthKind,
        ticket: ActivationTicket,
        activation_attempt,
    }

  | DrainingPreReady {
        role,
        worker: WorkerIncarnation,
        outstanding: PreReadyOutstanding,
        cause: PreReadyDrainCause,
        after_drain: Recover { stop } | Retire { reason },
    }

  | Idle {
        role,
        worker: WorkerIncarnation,
    }

  | Busy {
        role,
        worker: WorkerIncarnation,
        job: AssignedJob,
        join: AssignmentJoin,
    }

  | Recovering {
        role,
        predecessor: WorkerIncarnation,
        stop,
        recovery: RecoveryPhase,
    }

  | Stopping {
        role,
        worker: WorkerIncarnation,
        outstanding: StopOutstanding,
        after_stop: Retire { reason } | StopPool { reason },
    }

  | Retired {
        role,
        reason,
    }
```

`AssignmentJoin` is the exhaustive delivery/completion/stop join:

```text
AssignmentJoin =
    AwaitingDeliveryAndTerminal { delivery: AssignmentDeliveryAttempt }
  | DeliveryAcceptedAwaitingTerminal
  | CompletionBeforeDelivery {
        delivery: AssignmentDeliveryAttempt,
        completion: CompletionEvidence,
        later_stop: NoStop | Stopped { stop },
    }
  | StopBeforeDelivery {
        delivery: AssignmentDeliveryAttempt,
        stop,
        later_completion: NoCompletion | Completed { completion: CompletionEvidence },
    }
```

The delivery effect and lifecycle host own the moved `AssignmentCommand` while
settlement is unresolved; actor state retains only `AssignmentDeliveryAttempt`
and the non-authorizing completion half. `CompletionBeforeDelivery` owns the
returned completion input, while `StopBeforeDelivery` owns the authoritative
stop. They are disjoint, so no queue entry duplicates the stop. A completion
after `StopBeforeDelivery` is retained for stale or contradictory settlement;
a stop after
`CompletionBeforeDelivery` is retained in `later_stop` for one recovery after
completion settles. Shutdown transfers any unresolved join intact.

The lifecycle host—not `Initializing`—owns the linear initialization
settlement and activation plan while initialization is unresolved. Actor state
retains only `InitAttempt`. An exact `InitResult` later transfers either the
activation permit and plan, the rejected settlement and plan, or the stopped
plan back once. `ActivationDispatched` likewise retains correlation while the
host owns the moved begin request. These are the same ownership laws as AA-01,
applied directly rather than by reusing proxy state.

`PreReadyOutstanding`, `StopOutstanding`, and every recovery variant own
complete exact values rather than booleans. A definition or activation plan is
either local, moved into an action settlement, or transferred to the lifecycle
host—never reconstructed.

## Recovery phase

```text
RecoveryPhase =
    Scheduling {
        recovery: RecoveryTicket,
        timer: TimerCorrelation,
        prepared: PreparedReplacement,
        schedule_attempt: RestartScheduleAttempt,
    }
  | WaitingForTimer {
        recovery: RecoveryTicket,
        timer: TimerCorrelation,
        prepared: PreparedReplacement,
    }
  | ReadyToCreate {
        recovery: RecoveryTicket,
        prepared: PreparedReplacement,
    }
```

`PreparedReplacement` owns a fresh worker definition, fresh
`WorkerReservation`, checked delay evidence, and one budget charge. Emitting
the creation moves the member to `Creating`; no recovery phase and creating
phase own the definition simultaneously.

Recovery eligibility is total:

| Policy | Normal stop | Abnormal stop |
|---|---:|---:|
| Permanent | eligible | eligible |
| Transient | retire role | eligible |
| Temporary | retire role | retire role |

One matching stop can admit at most one recovery. Preparation invokes the
factory, reserves the recovery and worker-birth correlations, prunes budget at
the interpreter-authored monotonic stop time using an inclusive window,
validates the charge, and computes checked timing before commit. Any failure
commits no recovery, timer, creation, or budget charge; it applies the selected
topology-failure reaction to the already stopped role.

Immediate timing enters `ReadyToCreate`. Delayed timing enters `Scheduling`.
Exact schedule acceptance enters `WaitingForTimer`; exact rejection applies
topology failure and returns the complete prepared replacement to lifecycle
settlement. An exact timer moves to `ReadyToCreate` and emits creation in that
same fold. Stale or duplicate schedule/timer facts cannot create another
worker.

## Complete input sum

```text
FifoPoolInput =
    Submit { request: RequestCorrelation, payload, customer: CustomerRoute }
  | Shutdown

  | WorkerBirthResolved(WorkerBirthResolution)
  | InitializationSettled(InitializationSettlement)
  | ActivationBeginSettled(ActivationBeginSettlement)
  | WorkerReady(WorkerReadyFact)
  | WorkerStopped(WorkerStopFact)

  | AssignmentSettled(AssignmentSettlement)
  | WorkerCompleted { child: ChildRouteNonce, completion: CompletionEvidence }

  | RestartScheduleSettled(RestartScheduleSettlement)
  | RestartTimerElapsed(RestartTimerFact)
  | WorkerShutdownSettled(WorkerShutdownSettlement)

  | DrainDeadlineSettled(DeadlineScheduleSettlement)
  | DrainDeadlineElapsed(DeadlineFact)
```

Every fact carries exact role, reservation or incarnation, operation kind, and
correlation required by its transition. A payload match without exact source
and phase is never sufficient.

`WorkerBirthResolution` is:

```text
Committed {
    role,
    reservation: WorkerReservation,
    worker: WorkerIncarnation,
    init_attempt: InitAttempt,
}
Rejected {
    role,
    reservation: WorkerReservation,
    returned_submission: WorkerSubmission,
    reason,
}
```

Successful installation is authoritative before worker initialization effects
are interpreted. Initialization settlement and later activation are separate
facts; commit cannot be rolled back because one later effect rejects.

`InitializationSettlement` is one host-authored result:

```text
ReadyForActivation {
    role, worker, init_attempt,
    permit: ActivationPermit,
    plan: ActivationPlan,
}
Rejected {
    role, worker, init_attempt,
    rejected_settlement,
    plan: ActivationPlan,
    reason,
}
Stopped {
    role, worker, init_attempt,
    stop,
    plan: ActivationPlan,
}
```

The actor never stores the unresolved settlement or plan. Only this exact
result moves the lawful next values into actor state or drain ownership.

`AssignmentSettlement` is attached to the creator-local child nonce:

```text
Accepted { child: ChildRouteNonce, assignment }
Rejected {
    child: ChildRouteNonce,
    assignment,
    returned: AssignmentCommand,
    reason,
}
```

Rejection proves only that this delivery was not accepted. It is not fabricated
worker termination. Before the pool resolves the job, `AuthorityReunion`
consumes the returned command's affine authority together with the exact
retained `CompletionCorrelation`:

```text
AuthorityReunion =
    Cancelled { assignment, worker_birth: WorkerBirthEvidence }
  | Mismatch { retained, returned_authority }
```

`Cancelled` proves that neither half remains live. `Mismatch` transfers both
complete values to terminal settlement and cannot requeue, complete, or reuse
the assignment. After successful reunion the pool safely requeues the proven-
unaccepted job, quarantines and drains the exact worker, and applies recovery
only after exact terminal or forced transfer.

## Required semantic action product

The pool fold's minimum model product is:

```text
FifoPoolActions = {
    worker_creations: [CreateWorker { reservation, submission }],
    worker_creation_observations: [ObserveWorkerCreation { reservation }],
    worker_stop_observations: [ObserveWorkerStop { child: ChildRouteNonce }],
    activation_begins: [BeginWorkerActivation { child, attempt, permit, plan }],
    activation_cancellations: [CancelWorkerActivation { child, attempt }],
    worker_assignments: [DeliverAssignment { child, assignment, command }],
    worker_shutdowns: [ShutdownWorker { child, correlation }],
    restart_schedules: [ScheduleRestart { recovery, timer, deadline }],
    deadline_schedules: [ScheduleDrainDeadline { timer, deadline }],
    admission_deliveries: [DeliverAdmission { route, outcome }],
    terminal_customer_deliveries: [DeliverCustomerTerminal { route, outcome }],
    diagnostic_deliveries: [DeliverDiagnostic { route, diagnostic }],
    terminal_diagnostic_transfers: [TransferTerminalDiagnostic { diagnostic }],
    rejected_fact_transfers: [TransferRejectedFact { fact, reason }],
    forced_drain_transfers: [TransferDrainResidual { residual, cause }],
}

WorkerCompletionActions = {
    parent_completion_reports: [ReportCompletionToParent {
        authority: CompletionEvidence,
    }],
}
```

The heterogeneous product has one closed owned-operation view for ordered
application and rejection ownership:

```text
OwnedOperation =
    CreateWorker { reservation, submission }
  | ObserveWorkerCreation { reservation }
  | ObserveWorkerStop { child: ChildRouteNonce }
  | BeginWorkerActivation { child, attempt, permit, plan }
  | CancelWorkerActivation { child, attempt }
  | DeliverAssignment { child, assignment, command }
  | ShutdownWorker { child, correlation }
  | ScheduleRestart { recovery, timer, deadline }
  | ScheduleDrainDeadline { timer, deadline }
  | DeliverAdmission { route, outcome }
  | DeliverCustomerTerminal { route, outcome }
  | DeliverDiagnostic { route, diagnostic }
  | TransferTerminalDiagnostic { diagnostic }
  | TransferRejectedFact { fact, reason }
  | TransferDrainResidual { residual, cause }
  | ReportCompletionToParent { authority: CompletionEvidence }

AppliedOperation =
    WorkerCreationSubmitted { reservation }
  | WorkerCreationObservationStarted { reservation }
  | WorkerStopObservationStarted { child }
  | WorkerActivationBeginDelivered { child, attempt }
  | WorkerActivationCancelDelivered { child, attempt }
  | AssignmentDelivered { child, assignment }
  | WorkerShutdownDelivered { child, correlation }
  | RestartScheduleSubmitted { recovery, timer }
  | DrainDeadlineScheduleSubmitted { timer }
  | AdmissionDelivered { request }
  | CustomerTerminalDelivered { job }
  | DiagnosticDelivered { diagnostic_correlation }
  | TerminalDiagnosticTransferred { diagnostic }
  | RejectedFactTransferred { fact, reason }
  | DrainResidualTransferred { residual, cause }
  | ParentCompletionReported { child, completion_correlation }
```

`AppliedOperation` records exactly which owned operation crossed the runtime
boundary without pretending that creation, observation, scheduling, worker
execution, or customer receipt has already completed. Their later domain facts
remain distinct inputs. Each applied variant carries the correlation needed to
match that later fact; where the draft vocabulary does not yet define a
diagnostic correlation, that missing allocator is part of the lowering
blocker, not permission to use a generic label.

Because the locked `Environment::apply` is ordered but non-transactional, one
action application reports a closed prefix result:

```text
ActionBatchSettlement =
    Complete { applied: [AppliedOperation] }
  | Partial {
        applied_prefix: [AppliedOperation],
        rejected: { operation: OwnedOperation, reason },
        not_attempted_suffix: [OwnedOperation],
    }
```

`OwnedOperation` is the exact flattened sum of `FifoPoolActions` and
`WorkerCompletionActions`; it is not an erased envelope. `Partial` owns the
rejected operation and returns every unattempted payload to the lifecycle host
in authored order. A rejected `ReportCompletionToParent` therefore returns the
complete `CompletionEvidence` through `rejected.operation`; no separate generic
parent-rejection label is needed. The current runtime supplies neither this
complete prefix result nor the rejected parent operation: local parent
reporting discards a closed-parent send rejection. Both are explicit AA-30
blockers.

These are named semantic operations. No positional `.inner` traversal, proxy lane,
supervisor report, runtime registry, callback, channel, untyped envelope, or
ambient effect occurs inside the fold. A production representation may reuse
existing concrete lower-order products only after proving that it preserves
every lane, source, ordering, and rejection owner.

Initialization creation is interpreted before dependent observations. For one
fold, admission outcomes precede assignments, terminal customer outcomes
precede successor assignments, and diagnostics precede a terminal verdict.
These interpretation orders are explicit Bombay policies, not general actor-
model ordering guarantees.

## Ownership table

| Value | Actor-side owner | Owner after emission | Terminal law |
|---|---|---|---|
| Unaccepted submission | incoming `Submit` | rejected-admission settlement | returned complete; never enters queue/member state |
| Queued customer obligation | FIFO backlog | terminal-customer settlement when removed | assigned once or returned once |
| Assigned customer obligation | exact `Busy` member | terminal-customer settlement on result/interruption/shutdown | removed before another terminal outcome can emit |
| Execution payload and completion authority | transition-local candidate | assignment-delivery settlement, then exact worker | rejection returns both; state retains only canonical obligation/correlation |
| Unsettled assignment join | exact `Busy` member with one customer obligation | exact assignment/completion/stop settlement, or lifecycle host on forced drain | one exhaustive variant selects one terminal job disposition |
| Initialization plan/settlement | lifecycle host while unresolved; actor retains `InitAttempt` only | exact `InitializationSettlement` transfers the lawful next values | never duplicated in `Initializing` |
| Worker definition | construction/recovery candidate | creation settlement | rejection returns exact definition; commit establishes fresh worker |
| Exact worker capability | one member | effects borrow delivery authority; lifecycle host owns forced residual | exact stop or forced transfer retires it |
| Activation slot | activation member phase | activation settlement after emission | ready, rejection, stop, or forced transfer releases once |
| Recovery candidate | local preparation then `Recovering` | timer/creation settlement | rejection returns complete prepared ownership |
| Customer outcome | transition-local | route delivery settlement | rejection transfers outward; never recreates job |
| Diagnostic | transition-local | diagnostic route or terminal settlement | rejection is terminal and non-recursive |
| Drain residual | exact drain member | surviving lifecycle host | never fabricated as successful stop/completion |

## Transition notation

- `AD`: submission admission and immediate assignment/queue/rejection.
- `WB`: worker birth resolution.
- `IN`: initialization settlement.
- `AZ`: activation authorization release.
- `AS`: activation-begin settlement.
- `WR`: exact readiness.
- `WS`: exact worker stop.
- `DS`: assignment delivery settlement.
- `CP`: exact completion.
- `RS` / `RT`: restart schedule settlement / timer fact.
- `S0`: first shutdown normalization.
- `S=`: repeated shutdown, no duplicate effects.
- `WD`: worker drain settlement or exit.
- `DDS` / `DF`: drain-deadline schedule settlement / exact deadline.
- `F-`: preserve semantic ownership and emit one complete rejected-fact
  diagnostic through the configured disposition.
- `NA`: `Stopped` accepts no actor input; late settlements belong to the
  surviving lifecycle host.

All cells below assume exact creator-local nonce, private worker-birth evidence,
role, kind, correlation, and phase. Any mismatch is `F-`.

## Total pool-mode matrix

| Mode | Submit | Shutdown | Worker lifecycle | Assignment settlement/completion | Recovery timer | Drain timer |
|---|---:|---:|---:|---:|---:|---:|
| `Operating` | AD | S0 | WB/IN/AS/WR/WS/WD | DS/CP | RS/RT | F- |
| `Draining` | reject unchanged | S= | WB/IN/AS/WR/WS/WD | DS/CP | RS/RT | DDS/DF |
| `Stopped` | NA | NA | NA | NA | NA | NA |

Submission during drain emits
`Rejected { request, payload, ShuttingDown }`; the
customer route and payload never enter pool ownership.

## Total operating-member matrix

| Member | Birth | Init | Activation settlement | Ready | Stop | Assignment settlement | Completion | Restart timer |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `Creating` | WB | F- | F- | F- | join | F- | F- | F- |
| `Initializing` | F- | IN | F- | F- | WS | F- | F- | F- |
| waiting authorization | F- | F- | F- | F- | WS | F- | F- | F- |
| `ActivationDispatched` | F- | F- | AS | F- | WS | F- | F- | F- |
| `Activating` | F- | F- | F- | WR | WS | F- | F- | F- |
| `DrainingPreReady` | F- | settle | settle | settle stale | WD | F- | F- | settle only |
| `Idle` | F- | F- | F- | F- | WS | F- | F- | F- |
| `Busy::AwaitingDeliveryAndTerminal` | F- | F- | F- | F- | join | join | join | F- |
| `Busy::CompletionBeforeDelivery` | F- | F- | F- | F- | join | join | F- | F- |
| `Busy::StopBeforeDelivery` | F- | F- | F- | F- | F- | join | stale CP | F- |
| `Busy::DeliveryAcceptedAwaitingTerminal` | F- | F- | F- | F- | WS | F- | CP | F- |
| `Recovering` | WB only after emission | F- | F- | F- | F- | F- | stale CP | RS/RT |
| `Stopping` | F- | settle | settle | settle stale | WD | settle | stale CP | settle only |
| `Retired` | F- | F- | F- | F- | F- | F- | stale CP | F- |

`Creating` may observe an exact stop before its birth observation because child
report and creation-observation paths have no global arrival order. It stores
that one stop. Later commit joins the exact incarnation and resolves it without
advertising readiness; later creation rejection plus prior authoritative stop
is a contradiction retaining both facts.

## Initialization and activation

Initialization emits every preflighted worker creation in declaration order.
Each member enters `Creating` before the action is exposed. Creation rejection
retires or stops the pool according to topology failure and never reports a
successful birth.

Creation commit enters `Initializing { init_attempt }`; the lifecycle host
retains the unresolved initialization settlement and activation plan. Exact
`ReadyForActivation` transfers its permit and plan into
`WaitingForActivationAuthorization`. `Rejected` or `Stopped` transfers its
complete plan into `DrainingPreReady`; neither rolls back the committed worker
or begins recovery while an installed actor remains owned.

After every transition that releases activation capacity, `AZ` scans waiting
roles in declaration order. Reserving an `ActivationTicket`, changing the
member to `ActivationDispatched`, and emitting `BeginActivation` are one fold.
Ticket exhaustion/collision enters `DrainingPreReady` with complete activation
ownership; it cannot leave a silent installed worker waiting forever.

The model accepts begin settlement in either order with worker stop and forced
drain; no synchronous interpreter guarantee is assumed. Acceptance enters
`Activating`. Rejection enters `DrainingPreReady` and returns the complete
activation request through exact settlement. Exact `Ready` releases its ticket and invokes
the same `fill_fifo` transition used after completion and retry. If backlog is
non-empty, the member becomes `Busy` directly; it never transiently commits
`Idle`.

Stop or forced transfer in any occupied activation phase releases exactly one
ticket. Late readiness after stop is stale and never restores eligibility.

## Admission

`Submit { request, payload, customer }` evaluates one transaction:

1. reject `ShuttingDown` or `NoRecoverableWorkers` before allocation;
2. inspect worker availability and queue occupancy; when no `Idle` member
   exists and the new-admission bound is full, reject `BacklogFull` before
   allocation;
3. reserve one fresh accepted-job pair
   `{ JobId, AdmissionOrdinal }`; pair exhaustion/collision rejects with
   `JobCorrelationUnavailable` and retires every candidate;
4. if an `Idle` member exists, reserve one fresh `DispatchCorrelation`;
   rejection emits `DispatchCorrelationUnavailable` and does not fall back to
   queue admission;
5. prepare the retained execution clone and exact assignment action; unwind
   produces no transition;
6. commit `Busy`, advance the cursor, emit
   `Accepted { request, job }`, then emit the assignment; or
7. when step 2 proved queue capacity, append the obligation with
   `NeverAssigned` in `AdmissionOrdinal` order and emit
   `Accepted { request, job }`.

These branches are disjoint. Full/unserviceable rejection performs no internal
allocation; job-pair rejection and immediate-dispatch rejection return the
original request, route, and payload; no preparation failure silently queues a
job or leaves it accepted without an owner. Every candidate from an
uncommitted attempt is retired and never reused.

The `Accepted` delivery attempt does not gate assignment or queue ownership.
Its rejection transfers the complete admission outcome to the lifecycle host;
it cannot roll the job back or become its terminal outcome.

## FIFO fill and correlation failure

`fill_fifo` is a pure finite transition applied after readiness, completion,
retry insertion, assignment return, or member retirement:

1. maintain the queue sorted by immutable `AdmissionOrdinal`; a newly retried
   job is inserted after all smaller ordinals and before all larger ordinals;
2. while both the queue and an exact `Idle` member exist, inspect the smallest
   ordinal and the first idle role at or after the cursor;
3. prepare its execution clone and reserve a fresh assignment/completion pair;
4. on success, remove that head, commit the member to `Busy`, advance the
   cursor, and append one assignment action;
5. on correlation exhaustion/collision, remove that head and emit exactly one
   terminal outcome selected by its origin—`ReturnedQueued` for
   `NeverAssigned`, or `ReturnedAssigned` for `Retried`—then continue; and
6. on no idle member or empty queue, stop.

Returning a job on correlation failure is necessary because leaving it at the
head beside an idle worker would violate the maintained invariant and could
wait forever after permanent exhaustion. A clone unwind produces no fold and
therefore does not run step 5.

After every committed operating transition:

```text
backlog.is_empty() OR no member is Idle
```

## Assignment settlement and completion

Delivery settlement, completion report, and worker stop have no assumed
cross-path order. `AssignmentJoin` consumes them as follows:

- `AwaitingDeliveryAndTerminal + Accepted` becomes
  `DeliveryAcceptedAwaitingTerminal`.
- `AwaitingDeliveryAndTerminal + completion` becomes
  `CompletionBeforeDelivery`; no customer outcome is emitted yet.
- `AwaitingDeliveryAndTerminal + stop` becomes `StopBeforeDelivery`; no
  interruption outcome or recovery is admitted until delivery settles.
- `CompletionBeforeDelivery + stop` retains that stop in `later_stop`.
- `StopBeforeDelivery + completion` retains the completion in
  `later_completion`; stop remains the winning terminal ordering.
- `CompletionBeforeDelivery + Accepted` resolves `Completed`, then evaluates
  its retained later stop once when present.
- `StopBeforeDelivery + Accepted` applies `Fail | Retry` once, diagnoses any
  retained later completion as stale, then evaluates recovery once.
- Any exact `Rejected` settlement first attempts affine `AuthorityReunion`.
  With no retained completion it safely reinserts the proven-unaccepted job by
  `AdmissionOrdinal` with
  its prior assigned role, quarantines the
  worker, and evaluates any retained stop once. If a completion is already retained, the claimed returned authority
  contradicts its prior consumption; the fold emits
  `ReturnedAssigned { ContradictoryAssignmentSettlement }`, transfers both
  facts terminally, and begins whole-pool drain.

No branch relies on synchronous settlement. The customer obligation remains in
the exact `Busy` join until one settlement branch selects its disposition.
Wrong assignment, child nonce, worker-birth evidence, or phase is `F-`.

A completion matches the current assignment only when its creator-local child
nonce, opaque `WorkerBirthEvidence`, assignment correlation, and retained
non-authorizing half all agree. It never relies on the interpreter attaching an
exact incarnation capability. A matching completion after accepted delivery:

1. removes the active correlation;
2. consumes the canonical obligation;
3. emits `Completed` once;
4. makes the exact member available;
5. runs `fill_fifo` in the same fold; and
6. commits `Idle` only if no queued job can be dispatched.

A duplicate, cross-worker, old-birth, unknown-authority, or wrong-phase
completion preserves all current state and emits the complete result as a
stale operational diagnostic. It never selects a customer route from a
tombstone.

## Interruption and recovery

An exact stop of `Idle` makes the role unavailable and evaluates recovery.
An exact stop after accepted delivery removes the active completion
correlation:

- `Fail` emits `ReturnedAssigned { WorkerStopped }` once;
- `Retry` retains the canonical obligation and reinserts it by immutable
  `AdmissionOrdinal` with
  its prior assigned role; the next dispatch
  reserves fresh authority only when an exact idle worker is available; and
- dispatch-correlation rejection for that retried head emits
  `ReturnedAssigned { RetryPreparationRejected }` once.

Ordered reinsertion handles multiple interrupted workers: if A was admitted
before B, stopping A then B still produces `[A, B]`, not `[B, A]`. Retry is not
a new admission: it may exceed the new-admission backlog limit, but the
complete queue remains bounded by `backlog_capacity + roles.len()`.
`fill_fifo` may assign the job to a different exact idle role in the same fold.
Because the prior worker may have executed before stopping, this is explicitly
at-least-once execution.

The queue retains only the immutable ordinal and whether the obligation was
previously assigned to one role. Worker exit and rejected delivery have
identical future ordering, dispatch, shutdown, and customer-outcome semantics,
so retaining which event caused reinsertion would store history rather than
current queue truth.
The authoritative stop remains solely in `AssignmentJoin` until recovery
consumes it, then in `Recovering`; it is never copied into the queued job.
Duplicate stop during recovery is `F-`.

After assignment resolution, recovery eligibility and preparation run from
the exact stop. Eligible recovery emits the same typed non-empty worker-source
request used by fixed supervision, selecting this one role. Bombay interprets
that request outside `Behavior` and returns its complete generic settlement;
the pool owns what the returned submission or rejection means. Worker-source
failure, budget denial, clock regression, checked delay overflow,
recovery/birth correlation rejection, creation rejection, or schedule rejection
applies `RetireRole | StopPool`. `RetireRole` runs
`fill_fifo` across surviving members. If no live or recoverable member remains,
every queued job receives `ReturnedQueued { NoRecoverableWorkers }` in FIFO
order. `StopPool` enters the ordinary shutdown normalization rather than
dropping jobs through a terminal error.

## Shutdown normalization

The first `Shutdown` atomically:

1. closes admission;
2. removes every queued obligation in FIFO order and emits one
   `ReturnedQueued { PoolShutdown }` for each;
3. removes every assigned correlation and emits one
   `ReturnedAssigned { PoolShutdown }` for each;
4. disables retry, recovery admission, readiness publication, and successor
   dispatch;
5. cancels un-emitted prepared replacement and restart-timer ownership;
6. transforms every member into one exhaustive drain member;
7. emits at most one exact shutdown request per committed worker;
8. retains every pending worker creation until commit or rejection; and
9. for deadline policy, reserves and emits one exact drain timer.

The drain sum is:

```text
DrainMember =
    ResolvingCreation {
        role,
        worker,
        activation,
        creation_kind,
        stop: None | Some,
    }
  | Worker {
        role,
        actor,
        work: Created { activation }
            | Initializing
            | WaitingForActivation
            | ActivationDispatched
            | Activating
            | Idle,
        shutdown: NotRequested | Waiting { request, stop },
    }
  | AwaitingWorkerPreparation { role, request }
  | AwaitingRestartSchedule { role, request }
  | Drained { role }
```

The worker work sum owns actor-side correlations for unresolved initialization,
activation, assignment, and shutdown operations; the lifecycle host owns their
moved requests and linear values.
Customer ownership has already moved to terminal customer outcomes; a late
completion is stale diagnostic data and cannot be retained as another job.

Creation rejection enters `Drained`. An exact creation accepted during drain
starts no initialization or activation work. Without a prior stop it emits one
shutdown and enters `Worker { work: Created { activation }, ... }`; the
activation value stays local until exact stop transfers it to diagnostic
custody. With a prior exact stop, the worker is already retired and the
activation transfers immediately. A foreign or reversed result changes no
member. Shutdown rejection remains in the existing request/stop reunion while
exact stop observation is still possible; rejection alone never completes the
drain.

An exact worker stop consumes every matching outstanding child-side
settlement, preserves complete unresolved values with the lifecycle host, and
enters `Drained(Graceful)`. Repeated shutdown is `S=`. Late restart, readiness,
assignment, and completion facts settle or diagnose their exact cancelled
ownership and can never reopen admission or dispatch.

## Deadline drain

```text
DeadlinePhase =
    WaitingWithoutDeadline
  | Scheduling { timer: TimerCorrelation, attempt: DeadlineScheduleAttempt }
  | Waiting { timer: TimerCorrelation }
  | Fired { timer: TimerCorrelation, observed_at }
```

`WaitForActorGraph` may wait indefinitely. `RetireActorGraphAfter` uses exact
interpreter-authored monotonic time. Deadline correlation reservation failure
cannot silently weaken it to unbounded waiting: the rejecting shutdown fold
immediately transfers every unresolved member as
forced-retirement custody.

Exact schedule acceptance enters `Waiting`. Exact schedule rejection also
forces every unresolved member in that same fold. The residual preserves the
complete timer request, rejection reason, worker or birth ownership, and every
outstanding action settlement. Exact deadline firing performs the same forced
transfer with the deadline fact as cause.

Forced transfer does not fabricate worker stop, assignment acceptance,
completion, successful creation, recovery, or customer delivery. The stable
root remains alive in lifecycle-host settlement until every transferred
external obligation is resolved.

The pool selects normal actor termination only when every member is `Drained`
and every action in the final turn has an exact settlement owner. Unrelated
application children never enter this drain.

## Stale, overlap, and contradiction laws

- A role alone never identifies a worker fact.
- Birth resolution advances only its exact role, reservation, attempt, and
  expected birth kind.
- Initialization, activation, and readiness advance only their exact installed
  incarnation and expected phase.
- Assignment settlement advances only its exact child nonce, private birth
  evidence, assignment, and `AssignmentJoin` variant.
- Completion advances only the exact active accepted assignment and consumed
  opaque authority.
- A completion carrying old worker-birth evidence cannot release a new
  worker birth's job.
- Stop is consumed once by the exact member or drain state. It cannot admit a
  second recovery or interruption.
- A recovery schedule or timer advances only its exact recovery and timer
  generation.
- A delivery rejection cannot roll back already committed job, budget, cursor,
  or worker state; its complete payload moves to settlement.
- Customer-outcome rejection cannot recreate a queue entry or assignment.
- Contradictory authoritative facts preserve both facts for diagnostic or
  residual settlement.
- Diagnostic rejection is terminal and non-recursive.

Every rejected fact preserves current semantic ownership and emits one
complete `RejectedPoolFact` through the selected diagnostic disposition.

## Feature-catalogue trace

| Requirement family | Model evidence |
|---|---|
| `FP-BUILD` | complete construction product, non-empty unique roles, static assignment/result protocol, explicit diagnostics/activation/drain |
| `FP-TOPOLOGY` | direct fresh workers, exhaustive pre-ready and recovery sums, activation capacity, no proxy/supervisor state |
| `FP-ADMIT` | complete customer submission, bounded queue, zero-capacity rule, typed reservation failure, accepted/rejected split |
| `FP-ASSIGN` | private affine authority, one busy assignment, exact circular cursor, maintained no-idle-with-backlog invariant |
| `FP-COMPLETE` | nonce plus private birth evidence, affine reunion, order-independent settlement/completion/stop join, one terminal result |
| `FP-INTERRUPT` | fail/retry sum, admission-ordinal reinsertion, at-least-once law, no duplicated stop fact |
| `FP-FAILURE` | one-role retirement, surviving dispatch, complete stranded-work return, separate diagnostics |
| `FP-RETENTION` | accepted-job customer obligations plus assignment-only completion correlation and bounded action-settlement joins, no permanent tombstones |
| `FP-SHUTDOWN` | admission closure, exact queued/assigned extraction, pending creation, direct worker drain, forced residual transfer |

Every row remains a model claim, not implementation status.

## Independent-model obligations

The later executable model must use independent vocabulary such as worker
cells, one queue, one accepted-job ledger, and one cursor. It must not copy
these production-oriented phase names or the eventual implementation branches.

At minimum it must cover:

- empty/duplicate construction, missing policy, zero activation limit, and
  factory/reservation failure at every role with no partial fleet;
- worker birth commit/rejection and stop-before-birth in both orders;
- initialization and activation acceptance/rejection, limits one and two, and
  declaration-order authorization;
- readiness with empty and non-empty backlog and no intermediate idle state;
- capacity zero, exact capacity, backlog full, shutdown admission, and no
  recoverable worker;
- job, assignment, completion, recovery, activation, birth, and timer
  exhaustion/collision transitions;
- cursor wrap, retired-role skipping, multiple idle workers, and FIFO batches;
- completion followed by successor dispatch in one complete action;
- duplicate, cross-worker, old-worker-birth, wrong-token, and unknown-token
  completion with result preservation;
- assignment settlement before stop, stop before settlement, and replay of
  each side;
- completion-before-stop and stop-before-completion under `Fail` and `Retry`;
- ordered reinsertion for two or more interrupted workers and immediate
  reassignment to another idle worker;
- permanent/transient/temporary recovery for normal and abnormal stop, budget
  boundaries, checked timing, schedule rejection, and restart exhaustion;
- role retirement with surviving capacity and complete stranded-job return;
- admission, customer outcome, assignment, diagnostic, creation, activation,
  timer, and shutdown delivery rejection;
- shutdown from every member phase, every queued/assigned cardinality, pending
  birth, pending assignment join, repeated shutdown, and late completion;
- deadline reservation failure, schedule rejection, exact firing, and forced
  lifecycle-host residual transfer; and
- complete named actions for every accepted and rejected transition.

Properties after every generated step must include:

```text
accepted_jobs = queued_jobs + assigned_jobs + terminally_settling_jobs
queued_jobs <= backlog_capacity + worker_roles
new submission queues only when queued_jobs < backlog_capacity
queued admission ordinals are strictly increasing
each worker has at most one assigned job
each job has at most one terminal customer outcome
backlog.is_empty() OR no worker is Idle
cursor is one position in the immutable declaration-order ring
active completion correlations are unique
retired correlations are never reissued
```

## Falsification findings and comparison obligations

AA-30 records these findings for governing-document reconciliation:

1. Unconditional retry-front insertion reverses two interrupted jobs when stop
   order differs from admission order. The corrected queue stores immutable
   `AdmissionOrdinal` and reinserts by that order.
2. The initial draft stored linear initialization settlement and activation
   plan in actor state. The corrected model follows AA-01: the lifecycle host
   owns them while actor state retains only `InitAttempt`.
3. The initial draft rejected completion before assignment settlement while
   accepting stop in that order, despite no locked causal guarantee. The
   corrected `AssignmentJoin` exhausts settlement, completion, and stop order.
4. `ChildReport` attaches only a creator-local nonce. The corrected completion
   law combines that nonce with opaque authority-carried
   `WorkerBirthEvidence`; it does not claim an interpreter-attached exact
   incarnation.
5. The initial retry queue copied the authoritative stop fact. Corrected queue
   provenance owns only admission ordinal, prior role, and semantic
   interruption class; the join/recovery path owns the stop once.
6. Rejected assignment delivery returns the affine authority. Corrected
   `AuthorityReunion` consumes returned authority and retained non-authorizing
   evidence exactly once before safe reinsertion.
7. The initial admission outcomes could not correlate concurrent requests on
   one customer route. Both corrected variants echo caller
   `RequestCorrelation`, distinct from pool-issued `JobId`.
8. The initial admission algorithm allocated before full-backlog rejection and
   allowed queue fallback to overlap preparation failure. Corrected branches
   classify first, allocate only for an admissible candidate, and give every
   preparation failure one typed rejection.
9. Generic settlement labels hid payload ownership and the locked runtime's
   non-transactional apply/closed-parent-report loss. The corrected model names
   every operation, exact settlement, applied prefix, rejected operation, and
   owned unattempted suffix. Concrete lowering remains blocked.
10. Recovery reuses fixed-supervisor policy values but not state; its selected
    topology failure is `RetireRole | StopPool`. Backlog capacity bounds new
    admission while retry preserves the finite absolute `capacity + roles`
    queue bound.
11. The current production pool contains stable proxies and a nested fleet
   ownership fold, exposes assignment/job identifiers to workers, and lacks
   the selected activation/opaque-authority/deadline laws. It is comparison
   evidence, not an implementation candidate. Its 550 green tests do not
   validate this oracle, and the current independent FIFO property model has
   one worker, so it cannot expose the two-worker retry-order counterexample.

These are falsification findings, not production API amendments. Before an
executable task relies on any selected mechanism, the feature catalogue,
solution, inventory, retained-core decision, research audit, and interpreter
boundary must be reconciled together or the existing concrete boundary must be
proved to express the same law unchanged.

## First-slice non-features and blocker

AA-30 intentionally provides no:

- Rust pool, worker, job, assignment, completion, recovery, builder, protocol,
  event, effect, or error type;
- executable fold, model test, property, fuzz target, or benchmark;
- proxy, supervisor, keyed-affinity, shared recovery engine, or nested pool;
- interpreter for creation, activation, assignment, customer delivery,
  diagnostics, deadlines, or residual root ownership;
- wrapper-composition, initialization-order, compiler/DevX, or migration proof;
  or
- change to the locked kernel, actors, macros, testkit, runtime, examples, or
  public API.

The semantic document is complete enough for bounded comparison, but AA-30 is
`[!]`, not complete as an executable aggregate prerequisite. H04b proves that
`assignment.complete(result)` lowers one unforgeable private parent completion
without exposing a path or customer capability. H02 owns exact report and
prefix/suffix action settlement. H03 records the required Bombay custody path;
that upstream runtime work is not implemented in this repository. A clean-room
FIFO model must still execute the independent-model obligations above before
production can cite AA-30 as executable evidence. AA-40 may compare this law,
but neither aggregate may extract shared production machinery before the same
suite passes both real consumers.
