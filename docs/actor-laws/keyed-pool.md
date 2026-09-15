# Keyed-pool falsification model

## Status and authority

This document is the AA-40 independent semantic model for one keyed
worker pool. It is a non-production falsification oracle. It does not select
Rust types, define another behavior algebra, implement a pool, or mark a
production coverage row implemented. For the clean-room campaign this document
owns the keyed aggregate law; lower-authority solution and synthesis documents
must be corrected when they disagree. Rust representation remains separately
unselected.

The semantic state, input, action, admission, assignment, retirement, and drain
sums are complete in this revision. The independent bounded oracles in
`src/keyed_model.rs` and its `keyed/recovery.rs` child pass in debug and
optimized builds. They separately explore 22,621 binding/assignment prefixes
and 22,621 recovery/shutdown prefixes. The first rejects deliberate generation
reuse, stale mutation, accepted-work retargeting, cross-role queue consumption,
and duplicate terminal outcomes. The second rejects candidate loss and worker
restart after shutdown while checking one exact worker source, restart schedule,
timer, and worker-shutdown owner after every prefix. AA-30 now supplies the first real
assignment-authority consumer, and `docs/atomic-runtime-settlement.md` owns the
retained total interpretation and terminal-custody contracts. This oracle does
not duplicate either implementation.

The actor-model laws used here are isolated processing of one communication,
communications to known recipients, fresh actor creation, and explicit next
behavior. Direct worker ownership, key selection, affinity, bounded per-role
backlogs, bounded binding retention, generation-safe reuse, opaque completion
authority, recovery, action settlement, and actor-graph drain are derived
constructions or deliberate Bombay policies. The actor model does not supply
keyed-pool semantics.

The keyed pool is one independent aggregate transition. It is not a FIFO-pool wrapper,
supervisor, stable proxy, nested behavior, or specialization that stores or
forwards another template's state. It may compare AA-30's recorded value laws,
but it owns distinct member, partition, binding, management, and transition
sums. It may reuse only the separately proven direct-worker and assignment
laws; it never depends on FIFO aggregate state.

Runtime route selection is owned normatively by
`docs/atomic-runtime-settlement.md`. The pool issues and stores only a
`CreationId` for each direct worker creation; it never chooses or stores a
runtime route before emitting `CreateChild`. Any older `reservation` label below
denotes that creator-visible correlation before interpretation and routed
evidence only in the returned runtime settlement. It does not authorize a
separate route request, waiting state, or public route.

The clean-room `crates/actors` keyed pool is subordinate implementation evidence
against this model, never authority for it. It owns direct workers, bounded
generation-safe bindings, `Unbind`, typed management outcomes, and per-role
backlogs. Any implementation that instead nests a `WorkerPool`, routes through
stable-proxy nonces, retains unbounded generationless bindings, omits typed
management, or admits work to a stopping role is the obsolete architecture
falsified by this model; it is not a parallel keyed-pool design.

The model must falsify designs that make any of these claims false:

1. The worker roster is non-empty, ordered, and contains each semantic role
   once.
2. Every installed or replacement worker is freshly created. Role, key,
   binding generation, route, and correlation are not actor identity.
3. One submitted key is separate from its owned payload and is evaluated by
   one concrete selector only when no binding is retained.
4. A binding proposed by admission commits if and only if the same transition
   accepts the submitted customer obligation. An explicit management
   rebalance may instead bind an absent key without submitting work.
5. Binding capacity and per-role backlog capacity are distinct bounds. Zero
   backlog capacity lawfully permits immediate assignment only.
6. Every accepted job permanently retains its admitted role. Rebalance,
   unbind, retry, and worker replacement cannot retarget it.
7. Work for one role never consumes another role's backlog capacity or idle
   worker.
8. Every live member phase projects to exactly one of `AssignableNow`,
   `BacklogAdmissible`, or `Unavailable`, without a wildcard or inferred
   readiness.
9. No serviceable queued job for a role coexists with that role in eligible
   idle state after a completed transition.
10. Each role has at most one active assignment. Completion matches opaque
    authority, exact worker-birth evidence, retained role, and active phase.
11. Rebalance and unbind change future admission only. Every accepted queued
    or assigned job retains its original role and single customer obligation.
    Every management mutation is guarded by an exact absence or generation
    expectation.
12. Reusing an unbound key receives a fresh opaque binding generation. Late
    management inputs for an absent or different generation cannot mutate it.
13. The transition that makes a role permanently unusable returns that role's
    queued and assigned work once, removes every retained binding to the role
    atomically, releases binding capacity, and diagnoses each removed key and
    generation. Temporary recovery retains bindings.
14. Shutdown closes admission, returns every accepted job from every role once,
    removes all bindings, and drains every owned or still-creating worker.
15. Stale, duplicate, foreign, wrong-role, wrong-generation, wrong-worker-
    birth, and contradictory inputs preserve current ownership and have an
    explicit diagnostic disposition.
16. Every accepted input produces one next state and one complete named action
    product. No effect is ambient.

“Atomic” means one local `Behavior` transition commits one keyed-pool state and its
complete `Actions`. It does not claim atomic delivery, creation, worker
execution, external activation, or customer observation across actors.

## Model vocabulary

The following names are oracle vocabulary, not proposed public Rust names.

```text
Role = one semantic worker label, unique inside this pool
RoleOrder = immutable declaration order retained for the pool lifetime

Key = caller-supplied semantic affinity key
BindingGeneration = fresh opaque non-reused correlation
Binding = { key: Key, generation: BindingGeneration, role: Role }
AdmittedBindingEvidence = opaque copyable { generation: BindingGeneration, role: Role }

WorkerBirthAttempt = fresh non-reused CreationId issued by the pool
RoutedWorkerBirth = interpreter-private { attempt: WorkerBirthAttempt, route }
WorkerBirthEvidence = opaque non-routable evidence for one committed birth
WorkerRecipient = pool-private routable capability for one committed worker
CurrentWorker = { recipient: WorkerRecipient, evidence: WorkerBirthEvidence }

RequestCorrelation = caller-authored opaque submission correlation, echoed only
JobId = fresh non-reused customer-visible correlation issued by the pool
AdmissionOrdinal = fresh non-reused private total order for one accepted job
AssignmentId = fresh non-reused private correlation for one dispatch attempt
CompletionAuthority = opaque affine authority bound to assignment and worker birth
CompletionCorrelation = pool-retained non-authorizing half of that authority

ManagementRequest = fresh caller-visible correlation
BindingExpectation = Absent | Exact(BindingGeneration)
RebalanceCommand = {
    request: ManagementRequest,
    key: Key,
    expected: BindingExpectation,
    target: Role,
    reply: ManagementRoute,
}
UnbindCommand = {
    request: ManagementRequest,
    key: Key,
    expected: BindingExpectation,
    reply: ManagementRoute,
}
ManagementCommand = Rebalance(RebalanceCommand) | Unbind(UnbindCommand)
RecoveryTicket = fresh non-reused correlation for one role recovery
ActivationTicket = fresh non-reused authorization occupancy correlation
```

`BindingGeneration`, like every other correlation above, is not an actor
identity. It is allocated freshly for a newly committed binding rather than
derived from a per-key counter. Removing a binding therefore leaves no
permanent key tombstone. A later use of the same key obtains a fresh generation
from the fallible allocator.

`Binding` is the sole pool owner of its retained `Key`. A job accepted through
that binding receives only `AdmittedBindingEvidence`; it does not clone, own,
or keep the key. Multiple jobs may copy the non-authorizing evidence without
creating a binding, changing affinity, or extending binding retention. A
rejected admission or management command returns its one complete owned key.

The selector is one concrete statically known function `Fn(&Key) -> Role`. It
cannot select an address, nonce, worker birth, customer route, assignment lane,
or behavior implementation. Its output is validated against the immutable
semantic roster. It is evaluated at most once for one unbound submission and
is not evaluated for an already-bound key.

Every correlation allocator is total:

```text
Reservation<T> = Reserved(T) | Rejected(Exhausted | Collision { candidate: T })
```

Rejection never wraps, overwrites, reuses, or guesses a correlation. A later
transition table must name the complete owner returned by every allocation
failure before this model can be complete.

## Construction domain

```text
KeyedPoolDefinition = {
    initial_factory,
    roles: NonEmptyOrderedUnique<Role>,
    selector: one concrete Fn(&Key) -> Role,
    activation: ActivationContract,
    activation_limit: PositiveMaximum,
    recovery: RecoveryPolicy,
    backlog_capacity_per_role: NonNegativeMaximum,
    binding_capacity: PositiveMaximum,
    interruption: Fail | Retry,
    actor_drain: ActorDrainPolicy,
    diagnostics: DeliverTo { route } | Terminate,
}

ActorDrainPolicy =
    WaitForActorGraph
  | RetireActorGraphAfter { deadline }
```

Construction fails for an empty or duplicate roster, zero activation limit,
zero binding capacity, invalid drain deadline, missing selector or policy, or
a worker protocol that cannot accept the exact assignment product. Backlog
capacity is non-negative because zero is immediate-assignment-only. There is
no global backlog capacity and no FIFO placeholder selector.

The worker sum may be heterogeneous only when every variant accepts one common
concrete assignment protocol and produces one common concrete result type.
Static closed sums are allowed; erasure, a runtime registry, and downcasting
are not.

AA-40 selects no builder spelling, typestate, public identifier
representation, trait, wrapper, alias, or macro.

## Top-level ownership state

```text
KeyedPoolState =
    Operating {
        definition,
        roles: OrderedRoleTable,
        bindings: BoundedBindingMap,
        activation: ActivationCapacity,
        budget: RestartBudget,
        allocators,
    }
  | Draining {
        definition,
        roles: OrderedDrainRoleTable,
        bindings: DrainingBindingMap,
        deadline: DeadlinePhase,
        allocators,
    }
  | Stopped
```

`OrderedRoleTable` has exactly one `RoleCell` for each declared role. One cell
owns both that role's member phase and its FIFO queue; there is no separately
mutable member map and partition map whose phases can disagree. Roles are never
inserted, removed, compacted, or reordered.
`Retired` membership remains a terminal role cell so late inputs and binding
removal never depend on index arithmetic. `Irrecoverable` is a transition
cause, not a second retained terminal phase.

`BoundedBindingMap` owns at most `binding_capacity` entries. It contains only
currently retained bindings. Absence is not stored. Unbind and the transition
committing permanent role unavailability delete entries and release capacity
immediately. Queued and assigned work does not keep a binding entry alive; it
owns admitted-binding evidence and its admission record independently.

Each role partition owns one FIFO queue with its own full
`backlog_capacity_per_role`. A job admitted to role `R` can inhabit only `R`'s
queue or active assignment state. It cannot consume the queue budget of role
`S`, even when `S` is idle. Retry reinserts within the same role by immutable
admission ordinal rather than unconditional front insertion.

## Accepted-work ownership

```text
CustomerObligation = {
    job: JobId,
    admitted_at: AdmissionOrdinal,
    admitted_binding: AdmittedBindingEvidence,
    customer: CustomerRoute,
    retained_payload,
}

QueuedJob = {
    obligation: CustomerObligation,
    origin: NeverAssigned | Retried { interruption },
}

AssignedJob = {
    obligation: CustomerObligation,
    assignment: AssignmentId,
    completion: CompletionCorrelation,
    worker: CurrentWorker,
}
```

One accepted job has one authoritative `CustomerObligation`. It is owned by
exactly one per-role queue, exact member assignment, or terminal settlement.
The admitted-binding evidence is historical correlation data; it neither owns
the key nor preserves or recreates a removed binding. The caller's
`RequestCorrelation` is moved into the emitted `Accepted` admission outcome
and is not retained in actor state. Retry may retain a canonical payload while
an execution value has escaped to a worker, but that represents one semantic
obligation and explicitly at-least-once execution, not exactly-once external
side effects.

## Total admission target eligibility

Admission never asks whether a role is merely “alive” or “eventually ready.”
It exhaustively projects the exact member phase:

```text
AdmissionTargetEligibility =
    AssignableNow { current_worker }
  | BacklogAdmissible
  | Unavailable { reason }

AssignableNow = ReadyIdle

BacklogAdmissible =
    Prepared
  | Creating
  | Initializing
  | WaitingForActivationAuthorization
  | ActivationDispatched
  | Activating
  | ReadyBusy
  | RecoveryAdmitted
  | DrainingPreReady { after_drain: ResumeRecovery }

Unavailable =
    Stopping
  | Retired
  | DrainingPreReady { after_drain: Retire }
  | ShutdownOwned
```

The projection is a total match over the eventual complete keyed-member sum.
`DrainingPreReady` is classified from its stored exhaustive disposition, not
from a cause, timer, or inferred future. A new or existing binding can accept
to `BacklogAdmissible` only when that role's queue has capacity.

For an unbound submission, selection, role validation, binding-generation
reservation, job/ordinal reservation, capacity checks, and either immediate
assignment preparation or queue insertion preparation all succeed before one
binding and one customer obligation commit together. Any rejection returns the
complete submission and retains neither proposed binding nor job. The detailed
failure and transition sums are enumerated below.

## Generation-safe management

Management target eligibility is a separate total projection because a
rebalance admits no job and therefore asks neither immediate assignability nor
queue capacity:

```text
ManagementTargetEligibility =
    Bindable
  | Unavailable { reason }

Bindable =
    Prepared
  | Creating
  | Initializing
  | WaitingForActivationAuthorization
  | ActivationDispatched
  | Activating
  | ReadyIdle
  | ReadyBusy
  | RecoveryAdmitted
  | DrainingPreReady { after_drain: ResumeRecovery }

Unavailable =
    Stopping
  | Retired
  | DrainingPreReady { after_drain: Retire }
  | ShutdownOwned
```

An unknown role is `UnknownTarget`, not an eligibility member. An
irrecoverable input synchronously chooses and commits a permanently unavailable
member transition; it is never retained as a parallel member phase. Temporary
`RecoveryAdmitted` and `DrainingPreReady { ResumeRecovery }` remain bindable
and retain existing bindings. Entering terminal pre-ready drain, `Stopping`,
`Retired`, or global shutdown ownership atomically extracts every binding for
that role. Thus no permanently unusable role retains future-admission affinity.

The management protocol is exhaustive:

```text
ManagementOutcome =
    BoundAbsent {
        request,
        current: AdmittedBindingEvidence,
    }
  | Rebalanced {
        request,
        prior: AdmittedBindingEvidence,
        current: AdmittedBindingEvidence,
    }
  | RebalanceUnchanged {
        request,
        current: AdmittedBindingEvidence,
    }
  | Unbound {
        request,
        removed: Binding,
    }
  | AlreadyUnbound {
        command: UnbindCommand,
    }
  | Rejected {
        command: ManagementCommand,
        reason:
            StaleExpectation { actual: Absent | Exact(BindingGeneration) }
          | UnknownTarget
          | TargetUnavailable
          | BindingCapacityExhausted
          | GenerationReservationRejected
          | ShuttingDown,
    }
```

Successful management outcomes need not echo a key: absent rebalance moves
the command's key into `Binding`; successful existing rebalance retains the
table's existing key; successful unbind returns the removed `Binding` and its
key. Every rejected outcome returns the complete command unchanged.

`ManagementRoute` follows the same explicit capability-clone custody law as the
customer route. The transition prepares one target clone before committing a rejected
outcome so the original route can remain inside the returned command. A
successful outcome consumes the command route as its delivery target and does
not retain it. Rejected outcome delivery returns the target clone and the
complete command, including the original route. This duplicates routable
capability data only; request, key, expectation, and generation evidence remain
single semantic values.

For a key absent from `BoundedBindingMap`:

| Input | Result |
|---|---|
| `Rebalance { expected: Absent, target }` and target is `Bindable`, capacity remains, generation reservation succeeds | Move the command key into `Binding { fresh generation, target }`; emit `BoundAbsent`. |
| `Rebalance { expected: Absent, .. }` but target is unknown/unavailable, capacity is full, or generation reservation rejects | Preserve absence; return the complete command in the exact typed rejection. |
| `Rebalance { expected: Exact(_), .. }` | Preserve absence; return the complete command as `StaleExpectation { actual: Absent }`. |
| `Unbind { expected: Absent, .. }` | Preserve absence; return `AlreadyUnbound` with the complete command. |
| `Unbind { expected: Exact(_), .. }` | Preserve absence; return the complete command as `StaleExpectation { actual: Absent }`. |

For a key bound at exact generation `g` to role `r`:

| Input | Result |
|---|---|
| Either command with `expected: Absent` or `Exact` other than `g` | Preserve the binding; return the complete command as `StaleExpectation { actual: Exact(g) }`. |
| `Rebalance { expected: Exact(g), target: r }` and `r` is still `Bindable` | Explicit accepted no-op: preserve generation `g`, consume no capacity or allocator value, and emit `RebalanceUnchanged`. |
| `Rebalance { expected: Exact(g), target }` for another `Bindable` role and fresh generation `g2` is reserved | Move the stored key into `Binding { generation: g2, role: target }`; emit `Rebalanced { prior: {g, r}, current: {g2, target} }`. No second table entry exists. |
| Exact rebalance to an unknown or unavailable target, or fresh-generation reservation rejection | Preserve the complete old binding; return the complete command in the exact rejection. |
| `Unbind { expected: Exact(g) }` | Remove and return the complete `Binding`, release capacity immediately, and emit `Unbound`. |

Expectation comparison precedes target validation. No stale command can
observe a later binding and then mutate it. Role-changing rebalance prepares
the fresh generation before replacing the old binding, so allocation failure
cannot leave an absent entry. Same-role rebalance deliberately preserves the
generation: a management request that changes no affinity does not invalidate
otherwise exact later inputs.

## Complete role and worker sum

```text
RoleCell = {
    role,
    member: KeyedMember,
    backlog: Fifo<QueuedJob>,
}

KeyedMember =
    Prepared { submission, reservation, kind }
  | Creating { reservation, kind, prior_stop }
  | Initializing { worker, kind, init_attempt }
  | WaitingForActivationAuthorization { worker, kind, permit, plan }
  | ActivationDispatched { worker, kind, ticket, begin_attempt }
  | Activating { worker, kind, ticket, activation_attempt }
  | DrainingPreReady { worker, outstanding, cause, after_drain }
  | Idle { worker }
  | Busy { worker, assigned: AssignedJob, join: AssignmentJoin }
  | Recovering { predecessor, stop, phase: RecoveryPhase }
  | Stopping { worker, outstanding, after_stop }
  | Retired { reason }

BirthKind = Initial | Replacement { recovery, replaces: WorkerBirthEvidence }
AfterPreReadyDrain = ResumeRecovery { stop } | Retire { reason }
AfterWorkerStop = Retire { reason } | StopPool { reason }
```

Each `RoleCell` owns at most one active assignment and one queue. Only `Idle`
is assignable. A worker becoming ready first inspects its own role queue and
commits directly to `Busy` when work exists; it commits `Idle` only when that
queue is empty.

The direct-worker creation, initialization, activation, and exact-stop law is
identical to AA-30: fresh creation commits before initialization settlement;
the lifecycle host owns affine initialization and activation work while the
actor retains exact correlations; readiness requires committed birth,
successful initialization, accepted activation, and exact readiness proof;
post-commit failure drains before recovery or retirement. This document uses
that shared value law but does not embed FIFO aggregate state, cursor, or global
queue behavior.

Activation occupancy is derived from `ActivationDispatched` and `Activating`.
Waiting roles are released in immutable roster order. Reserving a ticket,
moving permit and plan to the activation action, and storing the dispatched
phase are one transition. Ticket failure drains the installed worker with the
complete activation ownership; it never strands a silent worker.

## Recovery sum

```text
RecoveryPhase =
    Scheduling { recovery, timer, prepared, schedule_attempt }
  | WaitingForTimer { recovery, timer, prepared }
  | ReadyToCreate { recovery, prepared }

PreparedReplacement = {
    submission,
    reservation: WorkerReservation,
    checked_release,
    charge,
}
```

Recovery is one role at a time. It shares the complete `Permanent | Transient |
Temporary` eligibility, inclusive monotonic restart limit, checked immediate or
delayed release, and exact timer-correlation law with AA-30. It owns no fixed
strategy and selects `RetireRole | StopPool` on topology failure. Permanent and
transient recovery retain a typed worker source; temporary recovery retains no
source. Eligible recovery selects one role through the same non-empty
worker-source request as AA-30. Bombay executes the request outside `Behavior`;
the keyed pool alone interprets the returned submission or rejection.

Temporary recovery preserves every binding to the role. A terminal recovery
decision enters permanent role unavailability below. Factory, budget, clock,
release, timer, reservation, creation, initialization, or activation failure
cannot silently shrink ownership or fabricate a ready worker.

## Admission protocol and outcomes

```text
KeyedPoolInput =
    Submit { request, key, payload, customer }
  | Rebalance(RebalanceCommand)
  | Unbind(UnbindCommand)
  | Shutdown
  | WorkerBirthResolved(WorkerBirthResolution)
  | InitializationSettled(InitializationSettlement)
  | ActivationBeginSettled(ActivationBeginSettlement)
  | WorkerReady(WorkerReady)
  | WorkerStopped(WorkerStopped)
  | AssignmentSettled(AssignmentSettlement)
  | WorkerCompleted { child: ChildRouteNonce, completion: CompletionEvidence }
  | RestartScheduleSettled(RestartScheduleSettlement)
  | RestartTimerElapsed(RestartElapsed)
  | WorkerShutdownSettled(WorkerShutdownSettlement)
  | DrainDeadlineSettled(DeadlineScheduleSettlement)
  | DrainDeadlineElapsed(DrainDeadlineElapsed)

AdmissionOutcome =
    Accepted { request, job, binding: AdmittedBindingEvidence }
  | Rejected { request, key, payload, customer, reason }

AdmissionRejection =
    ShuttingDown
  | BindingCapacityReached
  | UnknownSelectedRole
  | RoleUnavailable
  | RoleBacklogFull
  | NoRecoverableWorkers
  | BindingGenerationUnavailable
  | JobCorrelationUnavailable
  | AssignmentCorrelationUnavailable
```

The customer route follows one explicit clone law. Before commit, the transition
clones it once: the clone targets the admission outcome, while the original
moves to the accepted `CustomerObligation` or the rejected outcome. Rejected
delivery therefore returns a complete delivery containing the targeting clone
and the original route. Payload, key, customer obligation, and completion
authority are never cloned by this rule.

Admission is one transaction:

1. reject global shutdown or absence of every serviceable or recoverable role;
2. borrow the submitted key to search the binding table;
3. for an existing binding, bypass the selector and use its exact role and
   generation;
4. for an absent binding, invoke the selector exactly once, validate the role,
   check binding capacity, and reserve a fresh generation;
5. project the exact role phase to `AssignableNow`, `BacklogAdmissible`, or
   `Unavailable` and check only that role's queue capacity;
6. reserve fresh job and admission-ordinal correlations;
7. for immediate assignment, reserve the assignment/completion pair and
   prepare execution and customer-route clones; preparation failure does not
   fall back to queuing;
8. commit a proposed binding if and only if the same transition commits the customer
   obligation; and
9. emit acceptance followed by assignment, append to that role's queue, or
   return the complete unchanged submission through one rejection.

No `Key: Clone` law follows from this transition. An absent accepted key moves
into `Binding`; an existing binding already owns its key and the submitted
equal key is consumed by the accepted command. Rejection returns the submitted
key. A concrete map representation may impose only the comparison or borrowing
law actually needed; compiler convenience cannot strengthen it to cloning.

Zero per-role backlog capacity permits immediate assignment and rejects
waiting ownership. A retry may temporarily make one role's queue length
`capacity + 1`, because that role has at most one interrupted assignment. It
is inserted by immutable `AdmissionOrdinal`, never at the unconditional front.

After every operating transition, independently for each role:

```text
role.backlog.is_empty() OR role.member is not Idle
```

No fill operation can consume another role's queue or worker.

## Assignment and completion law

The complete `AssignmentJoin` and affine `AuthorityReunion` law is identical
to AA-30 and is applied inside one `RoleCell`:

```text
AssignmentJoin =
    AwaitingDeliveryAndTerminal { delivery }
  | DeliveryAcceptedAwaitingTerminal
  | CompletionBeforeDelivery { delivery, completion, later_stop }
  | StopBeforeDelivery { delivery, stop, later_completion }
```

Delivery acceptance, rejection, completion, and exact worker stop are accepted
in every runtime-permitted order. Completion matches only the child nonce,
opaque worker-birth evidence, assignment, retained non-authorizing half,
admitted role, exact worker birth, and active phase. Binding-table contents are
not part of completion matching and cannot retarget accepted work.

Matching completion emits one `Completed` outcome and fills only the admitted
role's oldest queue entry. Matching exit after accepted delivery applies `Fail
| Retry` once; retry retains the admitted role and reinserts by ordinal in that
same role. Assignment rejection must reunite the returned affine authority
with the retained half before the proven-unaccepted job can requeue. Mismatch
or a completion that already consumed the authority transfers both inputs
terminally and begins whole-pool drain. Duplicate, stale, foreign, old-birth,
or wrong-role completion preserves current ownership and transfers the complete
result through diagnostics.

Exactly one terminal customer outcome exists for each accepted job:

```text
TerminalCustomerOutcome =
    Completed { job, role, result }
  | ReturnedQueued { job, role, payload, reason }
  | ReturnedAssigned { job, role, payload, reason }

QueuedReturnReason =
    PoolShutdown
  | RolePermanentlyUnavailable
  | NoRecoverableWorkers
  | DispatchCorrelationUnavailable

AssignedReturnReason =
    PoolShutdown
  | RolePermanentlyUnavailable
  | WorkerStopped
  | AssignmentReturnedUnaccepted
  | RetryPreparationRejected
  | ContradictoryAssignmentSettlement
```

Removal from the queue or member precedes emission. Rejected customer delivery
transfers the complete outcome outward and never recreates work.

## Permanent role unavailability

Temporary recovery retains role bindings. The transition selecting permanent
unavailability atomically creates one `RoleRetirement`:

```text
RoleRetirement = {
    role,
    queued: [QueuedJob],
    assignment: NoAssignment | AssignmentDrain { assigned, join },
    bindings: [Binding],
    worker: WorkerDrain,
    cause,
}
```

In that transition:

1. admission and management projection for the role becomes unavailable;
2. every queued obligation is extracted in admission order and moved to one
   `ReturnedQueued` outcome;
3. an active assignment loses retry authority and moves with its complete join
   to terminal assignment settlement; it produces completion if completion had
   already lawfully won, otherwise exactly one `ReturnedAssigned`;
4. every binding whose role matches is removed from the table, releasing
   capacity immediately;
5. each complete removed `Binding` moves to one diagnostic containing its key,
   generation, role, and cause; and
6. the exact worker and all outstanding effects enter drain.

The table is the only key owner. Jobs retain only generation and role evidence,
so binding extraction never needs to clone a key and never invalidates customer
ownership. A rejected diagnostic transfers its complete removed binding to the
lifecycle host; it cannot restore the table entry.

No later rebalance, unbind, completion, recovery, or worker-ready input can
revive the retired role. Another role remains independently serviceable.

## Required semantic action product

```text
KeyedPoolActions = {
    worker_creations,
    worker_creation_observations,
    worker_stop_observations,
    activation_begins,
    activation_cancellations,
    role_assignments,
    worker_shutdowns,
    restart_schedules,
    deadline_schedules,
    admission_deliveries,
    management_deliveries,
    terminal_customer_deliveries,
    diagnostic_deliveries,
    terminal_diagnostic_transfers,
    rejected_input_transfers,
    forced_drain_transfers,
}

WorkerCompletionActions = {
    parent_completion_reports,
}
```

Each field is a concrete named typed lane, not a generic pool envelope or a
flattened FIFO aggregate product. Generic item interpretation and settlement
are owned only by [`atomic-runtime-settlement.md`](../atomic-runtime-settlement.md):
accepted items leave their receipt, rejected items return the complete request,
blocked items retain the exact prerequisite, independent later items continue,
and corruption retains the committed prefix plus exact remainder. Creation is
the prerequisite only for operations using its uncommitted child binding.

Interpretation order is creation before its exact dependents; management and
admission outcomes before assignments they announce; terminal customer
outcomes before successor assignments; per-role assignments in roster order
when one transition releases several roles; and diagnostics before a terminal
verdict. These are Bombay policies, not actor-model guarantees.

## Total mode and member matrices

| Mode | Submit | Management | Shutdown | Worker lifecycle | Assignment/completion | Recovery timer | Drain timer |
|---|---:|---:|---:|---:|---:|---:|---:|
| `Operating` | admit/reject | exact management table | normalize drain | settle exact role | settle exact role join | settle exact recovery | reject input |
| `Draining` | reject unchanged | reject unchanged | idempotent | settle drain only | settle drain only | settle cancelled work | settle exact deadline |
| `Stopped` | not admitted | not admitted | not admitted | host-owned late input | host-owned late input | host-owned late input | host-owned late input |

| Member phase | Birth | Init | Activation settlement | Ready | Stop | Assignment settlement | Completion | Restart timer |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `Prepared`/`Creating` | exact birth join | reject | reject | reject | exact pre-birth join | reject | reject | reject |
| `Initializing` | reject | accept exact | reject | reject | exact drain | reject | reject | reject |
| waiting authorization | reject | reject | reject | reject | exact drain | reject | reject | reject |
| `ActivationDispatched` | reject | reject | accept exact | reject | exact join | reject | reject | reject |
| `Activating` | reject | reject | reject | accept exact | exact join | reject | reject | reject |
| `DrainingPreReady` | settle only | settle only | settle only | stale | drain | reject | reject | settle only |
| `Idle` | reject | reject | reject | reject | interrupt/recover | reject | stale | reject |
| busy delivery pending | reject | reject | reject | reject | join | join | join | reject |
| busy completion-first | reject | reject | reject | reject | join | join | duplicate | reject |
| busy stop-first | reject | reject | reject | reject | duplicate | join | retain stale/contradictory | reject |
| busy delivery accepted | reject | reject | reject | reject | interrupt/recover | reject | complete | reject |
| `Recovering` | only after exact creation emission | reject | reject | reject | reject | reject | stale | settle exact |
| `Stopping`/retirement drain | settle only | settle only | settle only | stale | drain | settle only | stale | settle only |
| `Retired` | reject | reject | reject | reject | reject | reject | reject | reject |

Every cell means exact role, reservation or worker birth, generation where
applicable, operation kind, assignment, worker-birth evidence, and phase.
Mismatch preserves state and moves the complete input to one non-recursive
diagnostic. Management expectations are evaluated by their separate exhaustive
tables above and never by this lifecycle matrix.

## Shutdown normalization and drain

The first `Shutdown` atomically:

1. closes submission and management admission;
2. extracts and removes every binding, moving each complete key, generation,
   and role value to terminal custody or a selected binding-removal diagnostic;
3. extracts every per-role queue in role order and FIFO order and emits one
   `ReturnedQueued { PoolShutdown }` per obligation;
4. removes the customer right from every active assignment and emits exactly
   one `ReturnedAssigned { PoolShutdown }`; any retained completion becomes
   stale diagnostic ownership and the remaining affine delivery join moves to
   drain;
5. disables dispatch, retry, recovery admission, and readiness publication;
6. retains pending creation, initialization, activation, assignment, timer,
   and shutdown settlement ownership in one exact drain role;
7. emits at most one shutdown per committed direct worker; and
8. reserves and emits one exact deadline request for deadline policy.

```text
DrainRole =
    ResolvingBirth { role, reservation, local_values, prior_stop }
  | DrainingWorker { role, worker, outstanding, stop_phase }
  | Drained { role, result: Absent | Graceful | Forced { residual } }

DeadlinePhase =
    WaitingWithoutDeadline
  | Scheduling { timer, attempt }
  | Waiting { timer }
  | Fired { timer, observed_at }
```

Repeated shutdown emits nothing twice. Birth rejection is `Absent`; birth
commit is drained; commit plus prior authoritative stop is a contradiction,
not readiness. Shutdown rejection retains the request and reason while exact
stop observation remains live.

Deadline reservation or scheduling rejection cannot weaken the configured
bound into an infinite wait. It transfers every unresolved role, binding
removal, assignment join, child operation, timer, and external authority to
`Forced { residual }`. Exact deadline firing does the same. It fabricates no
stop, readiness, completion, acceptance, or recovery. The pool selects normal
termination only when every role is drained and every final action has an exact
settlement owner.

## Stale, overlap, and conservation laws

- Key equality alone never matches a management input after generation changes.
- A role alone never identifies a worker or assignment input.
- Binding expectation comparison precedes target validation and mutation.
- Birth, initialization, activation, stop, recovery, assignment, and completion
  inputs advance only exact retained correlations and phases.
- Rebalance, unbind, recovery, replacement, and retry never retarget accepted
  work.
- One role cannot consume another role's queue capacity or idle worker.
- A removed binding generation is never reissued or inferred from arithmetic.
- Customer, management, diagnostic, and rejected-input delivery rejection never
  rolls back committed state and returns the complete value.
- Contradictory authoritative inputs retain both values in terminal custody.
- Diagnostic rejection is terminal and never recursively diagnosed.

After every generated step:

```text
accepted obligations = queued + assigned + terminally settling
retained bindings <= binding capacity
each retained key has exactly one fresh generation and one role
each accepted job has one immutable admitted role
each role has at most one assignment
each job has at most one terminal customer outcome
each role queue is ordinal-ordered and <= backlog capacity + 1
role queue is empty OR its member is not Idle
permanently unavailable roles retain no binding
```

## Feature trace and executable evidence

| Requirement family | Model evidence |
|---|---|
| `KP-BUILD` | construction domain, concrete selector, direct-worker protocol, distinct positive binding and activation limits |
| `KP-AFFINITY` | atomic absent binding plus job, exact eligibility, per-role capacity, immutable admitted role |
| `KP-REBALANCE` | generation expectations, same-role unchanged, fresh role-changing generation, complete command recovery |
| `KP-END` | complete direct-worker lifecycle, assignment join, permanent role extraction, shutdown and drain |
| `KP-RETENTION` | sole key-owning binding table, no tombstone, fresh generation, stale denial |

The independent `keyed_model` uses bins, leases, work slips, and service cells
rather than these specification names. Its focused tests cover two roles and
keys, zero and one capacity limits, every member eligibility, absent and
exact expectations, same-role and cross-role rebalance, unbind and reuse, stale
generations, permanent retirement, assignment delivery/completion/stop
permutations, retry ordering, shutdown, and forced residuals. A depth-four
enumeration checks the complete invariant set after all 22,621 prefixes. Its
`keyed/recovery.rs` child uses a workshop, ordered bays, one recruiter, and a
candidate-ownership map. A separate depth-four enumeration checks 22,621
recovery/shutdown prefixes, including concurrent stops, declaration-order
source release, preparation returned during shutdown, all restart-schedule
dispositions, accepted-timer cancellation, and both worker receipt/exit orders.

The inversion tests corrupt each of the seven named laws independently. The
audit rejects generation reuse as `GenerationReused`, stale mutation as
`StaleMutation`, accepted-work movement as `QueueLocation`, cross-role
assignment as `AssignmentRole`, and double settlement as `DuplicateTerminal`.

AA-30's feature-complete production slice now supplies the first real opaque
completion and affine-authority consumer. Generic total interpretation and
terminal custody remain owned by `docs/atomic-runtime-settlement.md`. AA-40 is
the retained keyed semantic oracle; production lowering must reuse those
contracts rather than reopen or duplicate them.

The retained production startup checkpoint uses one passive direct-worker
transition owner for both AA-30 and AA-40 successful creation, initialization,
activation start, and readiness. AA-40 alone locates the role, applies activation
capacity, and drains that role's queue. A Primary job queued before readiness is
not consumed when Replica becomes ready; the deliberate cross-role inversion
fails at that exact assertion. Foreign worker identity, wrong creation kind,
foreign initialization, foreign activation, and duplicate activation start all
retain the selected role and move the complete input to diagnostics. Assignment
reunion, recovery, permanent role extraction, and shutdown now have clean-room
production witnesses. Catalogue-wide compatibility, Bombay runtime lowering,
and the final fixed-point audits remain open; this document does not claim those
wider gates.

The production recovery suite also crosses shutdown after the exact restart
schedule has been accepted and before its timer arrives. A drop-tracked worker
submission remains owned by the terminal cancellation diagnostic until that
action is released; shutdown emits no replacement creation and stops
immediately. Deliberately dropping the submission during the shutdown
transition fails at the ownership assertion.
