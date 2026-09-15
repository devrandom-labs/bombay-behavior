# Fixed-supervisor falsification model

## Status and authority

This document is the AA-10 independent semantic model for one fixed
supervisor. It is a non-production falsification oracle. It does not select
Rust types, define another behavior algebra, implement a supervisor, or mark a
row in `docs/engineering/atomic-actor-solution.md` implemented. That document remains the
production design authority.

AA-10 is blocked, not complete. The state and transition oracle requires typed
child terminal/drain settlement to return an emitted install or replacement
when the proxy terminates without its normal outcome, but the locked boundary
has no implementation witness for that transfer yet.

Fresh allocation is the actor-model law used here: every stable proxy and
every worker incarnation behind it is freshly established rather than written
over an existing address. Creator-local child reservations, commit before
dependent effects, exact installed capabilities, action settlement,
activation authorization, and actor-graph drain are Bombay derivations or
policy choices. Ordered semantic roles, stable proxies, recovery policy, and
fixed-topology failure reaction are template laws.

Runtime route selection is owned normatively by
`docs/atomic-runtime-settlement.md`. The supervisor issues and stores only a
`CreationId` for each proxy creation; it never chooses or stores a runtime route
before emitting `CreateChild`. Any older `reservation` label below denotes that
creator-visible correlation before interpretation and routed evidence only in
the returned runtime settlement. It does not authorize a separate route
request, waiting state, or public route.

The model must falsify designs that make any of these claims false:

1. The roster is non-empty, ordered, and contains each semantic role once.
2. Each live role owns at most one exact stable proxy.
3. The supervisor consumes one atomic proxy outcome; it never reconstructs
   worker creation, initialization, activation, or readiness.
4. A role is advertised ready only after its exact proxy reports a successful
   initial or replacement outcome.
5. The configured positive activation bound limits unresolved proxy
   install/replace operations across the complete supervisor.
6. A recovery admission is selected from one immutable roster snapshot. The
   membership partition, prepared replacements, fallible correlations, and
   budget charge commit in one fold; failure before that fold changes none of
   them. Later cross-actor realization resolves each admitted participant
   independently.
7. Recovery timing, readiness, and activation authorization are independent
   prerequisites joined without flags.
8. Stale, duplicate, foreign, wrong-kind, and contradictory facts cannot
   advance a member or charge recovery twice.
9. Query is a total projection and cannot mutate topology, budget, recovery,
   authorization, or lifecycle ownership.
10. Shutdown closes recovery and drains every owned or still-creating proxy
    exactly once while leaving unrelated application children untouched.
11. Every accepted input produces one next state and one complete named action
    product. No effect is ambient.

“Atomic” means one local `Behavior` fold commits one supervisor state and its
complete `Actions`. It does not claim atomic delivery, creation, or work across
actors.

## Model vocabulary

The following names describe the oracle. They are not proposed public Rust
names.

```text
Role = one semantic member label, unique inside this supervisor
RoleOrder = the declaration order retained for the supervisor lifetime

ProxyBirthAttempt = fresh non-reused CreationId issued by the supervisor
RoutedProxyBirth = interpreter-private { attempt: ProxyBirthAttempt, route }
ExactProxy = externally routable exact capability for one committed stable proxy
WorkerAttempt = opaque non-routable evidence for one started worker

OperationKind =
    Initial
  | Replacement { recovery: RecoveryTicket, replaces: WorkerAttempt }

OperationTicket = one affine ID paired with one private supervisor witness

ProxyControl =
    Start { definition, activation }
  | Replace { definition, activation }

OperationInput = {
    ticket: OperationTicket,
    kind: OperationKind,
    control: ProxyControl,
}

RecoveryTicket = fresh non-reused correlation for one complete recovery decision
RecoveryOrdinal = checked one-based lifetime recovery count of the triggering role
TimerCorrelation = { timer, generation }

RecoveryParticipant =
    Stopped {
        previous: WorkerAttempt,
        stop,
    }
  | Online { current: WorkerAttempt }
  | AwaitingInitial { phase: InitialPhase }

FailureReaction = RetireMember | StopSupervisor

ActorDrainPolicy =
    WaitForActorGraph
  | RetireActorGraphAfter { deadline }

DiagnosticDisposition =
    DeliverTo { route }
  | Terminate

LifecyclePublication =
    NotPublished
  | Published { route }
```

An attempt, ticket, timer generation, role, or child route is correlation, not
actor identity or evidence of fresh allocation. Only committed creation yields
an exact installed proxy. Sequence arithmetic, address reuse, timing, or
adjacency cannot manufacture provenance.

Every payload named below is complete and owned. A state may retain an exact
correlation after its value has moved into an action, but it cannot retain a
second ownership claim on the moved value.

`WorkerIncarnationEvidence` is correlation and lifecycle provenance, not a
recipient or transferable delivery capability. The proxy alone retains the
routable worker capability. The only service capability the supervisor may
publish or return is `ProxyIncarnation`, preserving the stable proxy as the one
public communication path for the role.

`OperationKind` is supervisor settlement metadata. Only the matching concrete
`ProxyControl` value is delivered to AA-01's private control protocol; the
fixed supervisor does not widen the proxy message with its recovery ticket.
The action settlement owns the metadata and control as one staged source-side
product so rejection can return both without asking the proxy to echo either.

## Construction domain

### Worker-preparation correction

The original draft stored one callable factory and instructed the recovery
transition to invoke it. That representation is rejected: application execution
is not part of a pure `Behavior` transition. A callable used to prepare the
initial roster is consumed before the supervisor exists and is not stored in the
successful behavior.

Automatic recovery instead emits one typed batch worker-preparation action. Its
settlement must return the complete worker-source authority, the ordered selected
immutable role names, every `WorkerSubmission`, any exact rejection, and every
untouched role name.
Bombay interprets the statically selected capability; FixedSupervisor owns
selection and the later state change. No callback, registry, erased response, or
runtime lookup exists in Behavior.

H38 selects the typed batch action rather than a mandatory factory actor. The
actor alternative adds a delivery settlement and later reply join; delivery
acceptance cannot mean workers were prepared. The action uses H35 source
settlement directly. Bombay will implement its exact static interpreter and
retirement transfer through the required generic runtime contract. The atomic
implementation targets that contract and does not wait for the present Bombay
API shape. The old callable instruction below is not an alternate accepted path.

The source contract is a method-free static declaration over the concrete
role, worker, and activation-plan types. One non-empty action owns the complete
selected immutable-name batch; it never owns the unique member-role authorities,
which remain in the pending recovery. Bombay observes each name only as
`&Role`. Its accepted result is exactly either every prepared submission paired
with its returned name or the complete prepared prefix, rejected name and
reason, and untouched suffix. Source rejection, corruption, and no-attempt
remain H35 generic settlement alternatives; FixedSupervisor must not repeat
them in another sum.

One complete definition contains:

```text
FixedDefinition = {
    initial_factory: ConstructionOnly,
    roles: OrderedNonEmptyUnique<Role>,
    activation,
    activation_limit: PositiveMaximum,
    recovery: {
        eligibility,
        worker_source: OnlyWhenAutomatic,
        strategy,
        budget,
        timing,
    },
    failure_reaction: FailureReaction,
    actor_drain: ActorDrainPolicy,
    diagnostics: DiagnosticDisposition,
    lifecycle: LifecyclePublication,
}
```

Duplicate roles, an empty roster, invalid timing, a non-positive activation
limit, and a rejected initial factory call are construction failures. The
initial factory prepares every initial definition before the supervisor exists
or any initialization action is emitted. A failure returns all still-owned
inputs and identifies the exact role; it cannot leave a partially constructed
behavior. Success drops the callable before producing the behavior.

Replacement definitions arrive only through the typed batch action above. A
closed heterogeneous worker sum is valid only when every variant exposes one
common concrete public protocol. Different public protocols are different
supervisors, not one erased envelope.

The canonical recovery value is constructed as
`Recovery::permanent(source, strategy, limit, release)`,
`Recovery::transient(source, strategy, limit, release)`, or
`Recovery::temporary()`. The first two variants own the concrete source that
crosses the worker-preparation action. The temporary alternative has no source,
strategy, limit, or release field and requires no placeholder or annotation.

Lifecycle publication is genuinely optional. `NotPublished` constructs no
event and requires no dummy route. Diagnostics remain mandatory through the
independent `DeliverTo | Terminate` choice.

Every selected route retains its concrete logical, established, or mixed
capability form and its corresponding static hosting obligations. The model
does not normalize those routes into one runtime-selected envelope.

AA-10 deliberately selects no builder, typestate marker, callback, trait, or
finished behavior spelling.

## Normalized supervisor state

```text
SupervisorState =
    Operating {
        definition,
        fleet: FixedFleet,
        budget: BudgetState,
    }
  | Draining {
        definition,
        fleet: DrainingFleet,
        budget: BudgetState,
        deadline: DeadlinePhase,
    }
  | Stopped
```

`FixedFleet` is one order-preserving partition of the declared roles. Each role
has exactly one topology authority, held either by an independent `Member` or
by one participant in exactly one `RecoveryBatch`. The immutable role value is
allocated once. Lifecycle events, diagnostics, and an emitted worker-preparation
action may retain read-only role names, but those names cannot select, move,
recover, or retire a member. The application-facing values expose `&Role`, not
the private shared representation.
`RecoveryBatch` and its
participants are one dependent semantic value: there is no separately mutable
member map plus batch map whose flags may disagree. A later Rust experiment
must find a concrete representation that preserves this equation.

The configured activation maximum is immutable. Occupancy is not a second
counter: every occupied slot is owned by exactly one initial operation or one
replacement whose input or proxy outcome is still pending. An exact proxy
outcome releases the slot even when the predecessor stop is still outstanding.
Capacity is the count of those exact unresolved operations. Waiting roles are
discovered from `RoleOrder`, so a separate queue cannot disagree with member
state.

The budget stores only committed evidence inside its active inclusive window.
It is bounded by the configured maximum; a maximum of zero stores no admitted
charge and denies every otherwise eligible decision.

## Complete independent-member sum

```text
Member =
    CreatingProxy {
        role,
        reservation: ProxyReservation,
        initial: OperationInput,
    }

  | CreatingProxyAfterExit {
        role,
        reservation: ProxyReservation,
        initial: OperationInput,
        exit,
    }

  | Starting {
        role,
        proxy: ProxyIncarnation,
        phase: InitialPhase,
    }

  | Online {
        role,
        proxy: ProxyIncarnation,
        worker: WorkerIncarnationEvidence,
    }

  | Empty {
        role,
        proxy: ProxyIncarnation,
        previous: WorkerIncarnationEvidence,
        cause,
    }

  | Stopping {
        role,
        proxy: ProxyIncarnation,
        cause,
        phase: ProxyStopPhase,
    }

  | Retired {
        role,
        cause,
    }
```

```text
InitialPhase =
    WaitingForAuthorization { input: OperationInput }
  | Dispatched {
        ticket: OperationTicket,
        settlement: InputSettlementCorrelation,
    }
  | AwaitingOutcome { ticket: OperationTicket }

ProxyStopPhase =
    ShutdownDispatched { settlement: ShutdownSettlementCorrelation }
  | AwaitingExit
  | Rejected { rejection }
```

`Starting` owns no worker incarnation. Its unresolved operation kind is always
`Initial`. `Empty` is possible only after one installed worker has stopped or
a replacement attempt against such a predecessor has failed. Failure before
any worker installation is irrecoverable for the current proxy contract and
therefore enters `Stopping` or whole-supervisor drain; it cannot manufacture
an `Empty` predecessor.

`Stopping` preserves a rejected shutdown reason while still waiting for exact
proxy exit. Rejection is not reclassified as a successful stop. Under
`RetireActorGraphAfter`, the deadline may transfer the remaining ownership to
the lifecycle host; under `WaitForActorGraph`, the member can wait forever.

The accepted-stop carrier owns the unchanged other members and one complete
stopped member. Recovery eligibility borrows the stopped member's terminal
outcome when choosing policy; it does not store a second normal/abnormal label
beside the outcome that already determines it.

## Recovery partition

An admitted recovery batch owns all selected member-role authorities as one
value. Its preparation action may temporarily own read-only names for those
same roles, never another topology authority:

```text
RecoveryBatch = {
    ticket: RecoveryTicket,
    trigger: RosterPosition,
    release: RestartReleaseState,
    members: Vec<RecoveryMember>,
}

RestartReleaseState =
    Ready
  | Scheduling { correlation: TimerCorrelation, request_settlement }
  | Waiting { correlation: TimerCorrelation }

RecoveryMember =
    Waiting { participant, prepared_replacement }
  | Replacing { participant_identity, replacement: WorkerReplacement }

WorkerReplacement = {
    previous_worker,
    readiness,
    stopped: None | Some { complete_stop },
    response: ReplacementResponse,
}

ReplacementResponse =
    InputPending { witness }
  | OutcomePending { operation }
  | OutcomeReturned { complete_outcome }
  | InputRejected { complete_settlement }
```

`RosterPosition` is assigned once at roster construction and moves with the
unique member authority. Admission proves that `members` is non-empty, contains
each selected position once, and contains `trigger`. `OneForOne` selects one
position, `OneForAll` selects every eligible position, and `RestForOne` selects
the trigger position and its eligible suffix. Physical collection placement is
not declaration order.

The trigger position remains after that participant returns independently. A
return removes exactly one position from `members` and restores one independent
roster owner; the recovery retires only when `members` becomes empty. This is
the same one-collection ownership equation as H32/H32r. It needs neither a
parallel recovery table nor a split recovery when a middle participant returns.

Every participant owns exactly one prepared replacement until it is emitted.
After emission, `WorkerReplacement` stores two independent current values: the
optional complete predecessor stop and the proxy operation's current response.
An online participant has no stop; an already-stopped participant has one. The
response changes from input pending to outcome pending only after the exact
receipt, then to the complete returned outcome. Exact input rejection instead
stores the complete settlement without erasing an already-owned stop. Operating
recovery consumes a completed or rejected product immediately; shutdown may
retain the same product while proxy retirement remains independent.

An `AwaitingInitial` subject owns the complete `InitialPhase`; the enclosing
`Waiting` phase separately owns exactly one prepared replacement. It cannot
issue replacement until the initial operation atomically reports a ready
worker, providing the exact predecessor. Initial failure removes that
participant through the configured topology-failure reaction; it never treats
an initial-empty proxy as replaceable.

The stopped trigger begins with `Some(complete_stop)` and input pending; an
online coordinated peer begins with no stop and input pending. A matching peer
stop fills only the optional stop and cannot open another recovery decision. If
the replacement outcome arrives first, `OutcomeReturned` retains it until the
exact stop report or its typed delivery settlement closes the reunion. Thus
stop and replacement outcome are order-independent and each is accepted once.

StableProxy and FixedSupervisor own different work here. StableProxy owns the
worker shutdown-resolution and successor-start sequence and emits the exact
predecessor `WorkerStopped` report before its later replacement outcome.
FixedSupervisor owns admission of those two owner reports, lifecycle
publication, recovery capacity, and the rule that replacement readiness is not
published until both reports have been admitted. The supervisor never repeats
StableProxy's worker-control sequence.

The triggering worker's stop classification is consumed when recovery
disposition is chosen; its immutable roster position remains as the recovery's
trigger identity.
`RecoveryOrdinal` is consumed when the release is calculated. The committed
budget charge remains solely in `BudgetState`, where the next admission needs
it. None is copied into `RecoveryBatch`. Immediate release and an accepted exact
timer both produce `RestartReleaseState::Ready`; their arrival history cannot
change a later decision. Schedule rejection transfers the exact rejected
request directly into the configured failure reaction and therefore is not a
stored release phase.

Readiness is derived from the participant's current worker ownership, timing
from the batch's current `RestartReleaseState`, and authorization from current
activation occupancy. There is no stored cross-product of these prerequisites.
The transition that sees all three immediately emits the replacement input and
stores one `WorkerReplacement` with exact stop absence or presence and
`InputPending`; a ready-to-issue phase is never committed between turns.

Recovery batches may overlap in time only when their selected role sets are
disjoint. Exact recovery tickets distinguish them. A role already in any
batch, stopping, retired, or awaiting an admitted replacement is not selectable
by another batch.

## Complete input sum

Inputs arrive through distinct concrete protocol lanes.

### Management input

```text
Management =
    QueryStatus { reply_to }
  | QueryCapability { role, reply_to }
  | Shutdown
```

Query routes are temporary reply capabilities. They never become durable
lifecycle or diagnostic owners.

### Stable-proxy lifecycle input

```text
ProxyBirthResult =
    FoldRejected { reservation, proxy_definition, error }
  | HostRejected { reservation, proxy, init_actions, reason }
  | Committed { reservation, proxy: ProxyIncarnation }

ProxyInputResult = ActionItemResult<
    ProxyOperation,
    ProxyInputReceipt { proxy: ProxyIncarnation, ticket: OperationTicket },
    ProxyInputRejection,
    Never,
>

ProxyReport =
    InitialInstallation { outcome }
  | Replacement { outcome }
  | WorkerStopped { stop, observed_at }
  | Unavailable { sender, phase, command }

ProxyExit =
    BeforeBirth { reservation: ProxyReservation, exit }
  | AfterBirth { proxy: ProxyIncarnation, exit }

ProxyShutdownSettlement =
    Accepted { proxy: ProxyIncarnation, correlation }
  | Rejected { proxy: ProxyIncarnation, correlation, request, reason }
```

The generic result owns whether interpretation was attempted. Acceptance owns
only the receipt promised by the request. Rejection and corruption return the
complete `ProxyOperation`; an unattempted result retains it unchanged. There is
no supervisor-specific result sum or conversion seam.

`ProxyReport` is always accompanied by its exact child-source capability. The
source capability, expected pending operation kind, and the replacement
outcome's nested exact `replaces: WorkerIncarnationEvidence` together provide
report correlation. That evidence must equal the participant's retained prior
evidence. An initial operation occurs at most once for one exact proxy, and each
later replacement names a fresh predecessor, so a delayed earlier report
cannot satisfy a later pending operation. The supervisor does not add its
operation or recovery ticket to AA-01's flat four-variant proxy report.
Install/replace settlement is discharged before a later report from that input,
so `Dispatched -> AwaitingOutcome -> report` is a causal Bombay interpreter
policy, not an inference from arrival timing.

### Recovery and drain input

```text
RestartScheduleSettlement =
    Accepted { recovery: RecoveryTicket, timer: TimerCorrelation }
  | Rejected {
        recovery: RecoveryTicket,
        timer: TimerCorrelation,
        request,
        reason,
    }

RestartElapsed = { recovery: RecoveryTicket, timer: TimerCorrelation, observed_at }

DrainDeadlineSettlement =
    Accepted { timer: TimerCorrelation }
  | Rejected { timer: TimerCorrelation, request, reason }

DrainDeadline = { timer: TimerCorrelation, observed_at }

ActionSettlement = one closed lane-specific accepted, rejected, or
not-attempted settlement from the current action product
```

Workers and callers cannot author `observed_at`. The interpreter's monotonic
clock supplies it with the authoritative stop or timer fact. A regressing
clock fact is a typed rejection and cannot prune future evidence.

### Unexpected input

```text
UnexpectedInput = { input: FixedSupervisorEvent }
```

A mismatched input moves complete into one diagnostic outcome. The exact
expected route, proxy, operation, recovery, or timer authority remains solely
in current supervisor state. Copying it into the diagnostic would duplicate
admission authority; moving it would contradict unchanged-state preservation.
The unexpected input is not
dropped, reinterpreted, or converted into `Behavior::Error` merely because it
was unexpected.

## Public projections

Status is a total read-only projection in declaration order:

```text
MemberStatus =
    CreatingProxy
  | WaitingForActivationAuthorization
  | AwaitingProxyOutcome
  | Ready { proxy: ProxyIncarnation }
  | Empty
  | Recovering
  | Stopping
  | Retired

CapabilityResult =
    Ready { role, proxy: ProxyIncarnation }
  | Unavailable { role, phase }
  | UnknownRole { submitted_role }
```

The projections are exact:

| Owned state | Status | Capability |
|---|---|---|
| `CreatingProxy*` | `CreatingProxy` | `Unavailable` |
| `Starting(WaitingForAuthorization)` | `WaitingForActivationAuthorization` | `Unavailable` |
| `Starting(Dispatched \| AwaitingOutcome)` | `AwaitingProxyOutcome` | `Unavailable` |
| `Online` | `Ready { proxy }` | `Ready { proxy }` |
| `Empty` | `Empty` | `Unavailable` |
| any `RecoveryMember` | `Recovering` | `Unavailable` |
| `Stopping` or any drain member | `Stopping` | `Unavailable` |
| `Retired` or drained member | `Retired` | `Unavailable` |

The supervisor never reports proxy-private worker-creation, initialization, or
activation phases. Query emits one management reply and leaves the complete
state byte-for-byte semantically unchanged. Reply rejection belongs to action
settlement and cannot mutate the snapshot that was already produced.

Production witness H121 realizes this table directly from the current roster
owners. It sorts status by immutable roster position and scans capability by
borrowed role equality. Logical and exact reply routes select two named lanes in
the ordinary action product; no copied roster, lookup table, query actor, or
template-specific interpreter exists.

Production witness H123 realizes the exact initial-failure
`StopSupervisor` row through the ordinary `FixedShutdown` transition. It emits
one failure diagnostic, stops only committed proxies, retains pending proxy
creations, and makes repeated shutdown idempotent. It adds no alternate drain
state or response product.

## Lifecycle and diagnostic values

```text
LifecycleEvent =
    Started { role, proxy: ProxyIncarnation }
  | Restarted {
        role,
        proxy: ProxyIncarnation,
        recovery: RecoveryTicket,
    }
  | WorkerStopped { role, stop, disposition }
  | Unavailable { role, sender, phase, command }
  | MemberRetired { role }

RecoveryDisposition =
    Ineligible
  | Admitted { recovery: RecoveryTicket }

RecoveryDenialReason =
    RecoveryTicketsExhausted
  | RestartTimersExhausted
  | RestartLimitReached { active, requested, maximum }
  | ClockRegressed { previous, observed }
  | RecoveryCountExhausted { admitted }
  | ReleaseCalculationFailed { reason }

OperationalDiagnostic =
    WorkerPreparationFailed {
        trigger,
        prepared,
        reason,
        remaining,
    }
  | RecoveryDenied { trigger, stop, reason: RecoveryDenialReason }
  | ProxyCreationRejected { role, rejection }
  | ProxyInputRejected { role, input, reason }
  | ProxyOutcomeFailed { role, outcome }
  | ProxyDied { role, exit }
  | ProxyShutdownRejected { role, proxy, request, reason }
  | UnexpectedInput { input: FixedSupervisorEvent }
  | ForcedRetirement { role, residual, cause }
```

`FactoryRejected` is construction custody, not an operational diagnostic: the
construction-only callable is consumed before a successful Behavior exists.
`WorkerPreparationFailed` is emitted only after an operating recovery action
returns its source. It retains the prepared prefix and untouched roles but not
the reusable source authority.

`Started` and `Restarted` exist only after the exact atomic proxy-ready
outcome. They expose the stable proxy capability and no worker recipient or
worker evidence. Committed proxy creation, accepted proxy input, and a worker
creation fact hidden inside the proxy cannot fabricate either event. The
supervisor retains `WorkerIncarnationEvidence` privately for correlation. A
replacement failure is never `Restarted`.

`MemberRetired` reports the durable topology change and carries only the role.
The exact operational diagnostic emitted before the configured failure reaction
is the sole owner of why retirement began. Repeating a summarized cause in the
lifecycle value would create a second authority; labeling shutdown or deadline
arrival as that cause would retain transition history rather than current
topology. Forced actor-graph retirement remains a distinct runtime-custody
diagnostic and does not fabricate `MemberRetired`.

`RecoveryDenied` is the sole observable owner of both the triggering stop and
the exact denial reason. Repeating the reason in lifecycle output would create
a second semantic cause. A rejected recovery therefore constructs no
`WorkerStopped` lifecycle event: lifecycle owns ineligible and admitted stops;
the operational diagnostic owns rejected recovery.

The Rust realization uses this one `RecoveryDenialReason` sum in restart
admission and stores that same value with the complete triggering worker stop
in `RecoveryDenied`. Public inspection borrows both values; a second private
six-way sum or variant-for-variant diagnostic conversion is not part of the
law. The post-denial topology retains a stop-less trigger and the unissued
prepared workers separately. A later restart-schedule rejection is different:
its diagnostic owns the rejected timer request, so topology continues to own
the worker stop. Both paths use the same topology reaction only after that
ownership distinction has been made.

If lifecycle publication is `NotPublished`, lifecycle events are not built and
no discard operation occurs. Diagnostics always follow the selected
disposition. `DeliverTo` emits one delivery; rejection becomes one terminal
`UndeliverableDiagnostic`. `Terminate` emits the diagnostic directly to
terminal settlement and selects `Stop` after preserving every other action in
the turn. Neither path recursively diagnoses diagnostic rejection.

## Required semantic action product

The governing solution names these fixed-supervisor lanes:

```text
proxy_creations
proxy_creation_observations
proxy_stop_observations
proxy_install_inputs
proxy_replacement_inputs
proxy_shutdowns
worker_preparations
restart_schedules
lifecycle_events
management_replies
terminal_diagnostics
delivery_rejections
```

This is the semantic lane inventory, not a second interpretation-order
specification. The generic ordering rule is owned by
[`atomic-runtime-settlement.md`](../atomic-runtime-settlement.md):
among the currently retained fixed-supervisor lanes, worker preparation
precedes proxy operations, which precede restart scheduling, lifecycle
publication, management replies, and diagnostics. Adding a missing lane must
preserve that relative order and the creation-dependency rules above.

The oracle requires those operations plus a truthful representation of a
route-delivered operational diagnostic. `terminal_diagnostics` can represent
`Terminate` but cannot by itself represent `DeliverTo(route)`, and
`lifecycle_events` cannot own diagnostics because lifecycle publication is
optional. Until an existing concrete product is proven to carry both choices
without reinterpretation, the minimum model product treats this as one closed
semantic lane:

```text
diagnostics =
    Deliver { route, diagnostic }
  | Terminal { diagnostic }
```

`delivery_rejections` denotes the closed interpreter settlement product, not a
catch-all message envelope. An input rejected by the supervisor becomes an
`OperationalDiagnostic::UnexpectedInput`; rejection of an emitted action
arrives through its lane-specific settlement. These are different laws.

This diagnostic-product mismatch is a falsification finding, not a production
API amendment. A later executable task must reconcile the solution, matrix,
retained-core decision, and research audit before adding a production lane.

The fold still returns the real `Actions` with its next behavior or terminal
decision. Creation commits before same-action observations or proxy input.
Within one recovery release, replacement inputs are emitted in declaration
order. Diagnostics are emitted before a terminal verdict in the same action.
The interpreter order and every accepted, rejected, or not-attempted settlement
remain focused proof obligations.

## Ownership table

| Value | Before acceptance | After action emission | Rejection or final settlement |
|---|---|---|---|
| Initial definition and activation | construction, then `CreatingProxy` or `Starting(WaitingForAuthorization)` | proxy creation settlement, then exact install-input settlement | returned complete on factory/creation/input rejection; never reconstructed |
| Proxy reservation | exact member state | staged creation and observation settlement share one scoped reservation | consumed by matching birth/exit result; collision never overwrites a route |
| Exact proxy | member after committed proxy birth | inputs and shutdowns borrow its delivery authority; member retains lifecycle ownership | moves through exact stop or forced-transfer drain; never inferred from role |
| Activation slot | free capacity derived from member states | one operation ticket in a dispatched/awaiting state | released only by matching atomic proxy outcome, rejected input, or forced drain transfer |
| Prepared replacement | recovery participant | exact replacement-input settlement | returned on input rejection or transferred through drain; never cloned for retry |
| Recovery decision | local prepared candidate before admission | committed budget plus one exact `RecoveryBatch` | pre-commit failure changes no peer; post-commit facts consume only matching participants |
| Timer request | recovery batch | restart-schedule settlement | rejected request remains diagnostic ownership; accepted timer is consumed once by exact input |
| Worker stop | exact proxy report | recovery owner, configured lifecycle event, diagnostic, or explicit publication omission | no second copy is retained to trigger another decision; shutdown moves only a stop still required by an unresolved failure path |
| Proxy report | exact child-source lifecycle input | lifecycle/diagnostic action or state transition | wrong source/kind moves complete into one unexpected-input diagnostic |
| Query route | one management request | one management-reply settlement | rejection is owned by the supervisor host; it never becomes durable ownership |
| Lifecycle event route | selected definition | lifecycle-event settlement | rejection cannot rewind readiness/recovery and transfers outward complete |
| Diagnostic | transition-local value | diagnostic delivery or terminal settlement | one terminal non-recursive outcome on rejection |
| Delayed prepared work at shutdown | recovery batch | no later recovery action; it transfers into drain settlement | late timer is consumed stale and cannot revive the batch |
| Outstanding child drain | draining fleet | proxy shutdown/observation settlement | exact stop closes once; deadline transfers residual ownership without fabricating stop |

## Transition notation

The matrices use these cells:

- `Q`: emit the total status or capability reply and preserve state.
- `S0`: close admission, cancel un-emitted recovery work, and enter one drain.
- `S=`: shutdown is already active; preserve the drain and emit no duplicate
  shutdown or deadline request.
- `PB`, `PE`, `IS`, `PR`, `PS`, `RS`, `TF`, `DDS`, `DF`, `SET`: apply the matching
  proxy-birth, proxy-exit, input-settlement, proxy-report, proxy-shutdown,
  restart-schedule, timer, drain-deadline-schedule, deadline, or lane-settlement
  rule.
- `F-`: preserve the semantic state and emit exactly one complete unexpected-input
  diagnostic through the configured disposition.
- `NA`: `Stopped` admits no mailbox input; late settlement belongs to the
  surviving lifecycle host.

All runtime-input cells require exact expected source, kind, operation, and
correlation for admission.
Every mismatch is `F-`. `F-` continues only under successful diagnostic
delivery. `Terminate` or an undeliverable diagnostic preserves other actions
and selects terminal settlement as specified above.

## Total supervisor-mode matrix

| Mode | Status query | Capability query | Shutdown | Proxy birth/exit | Proxy input/report | Recovery timer | Drain timer | Delivery settlement |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `Operating` | Q | Q | S0 | PB/PE | IS/PR/PS | RS/TF | F- | SET |
| `Draining` | Q | Q | S= | PB/PE | IS/PR/PS | RS/TF | DDS/DF | SET |
| `Stopped` | NA | NA | NA | NA | NA | NA | NA | NA |

Queries remain available during orderly drain and show `Stopping` or
`Retired`. `DrainDeadline*` in `Operating` and a recovery timer with no exact
batch are `F-`. Late recovery facts during `Draining` settle or diagnose the
cancelled exact batch but cannot emit a replacement.

## Total operating-member matrix

Nested phase rules below exhaust each non-empty cell. The five `Needs*` row
labels are readable projections of `WorkerReplacement`'s stop/response product,
not five separately stored Rust alternatives.

| Owned member state | Proxy birth | Proxy exit | Input settlement | Initial outcome | Replacement outcome | Worker stopped | Unavailable |
|---|---:|---:|---:|---:|---:|---:|---:|
| `CreatingProxy` | PB | PE | F- | F- | F- | F- | F- |
| `CreatingProxyAfterExit` | PB | F- | F- | F- | F- | F- | F- |
| `Starting` | F- | PE | IS | PR | F- | F- | PR |
| `Online` | F- | PE | F- | F- | F- | PR | PR |
| `Empty` | F- | PE | F- | F- | F- | F- | PR |
| `RecoveryMember(AwaitingInitial)` | F- | PE | IS | PR | F- | F- | PR |
| `RecoveryMember(Waiting)` | F- | PE | F- | F- | F- | PR | PR |
| `RecoveryMember(NeedsReceiptAndStop)` | F- | PE | IS | F- | F- | PR | PR |
| `RecoveryMember(NeedsReceipt)` | F- | PE | IS | F- | F- | F- | PR |
| `RecoveryMember(NeedsStopAndOutcome)` | F- | PE | F- | F- | PR | PR | PR |
| `RecoveryMember(NeedsOutcome)` | F- | PE | F- | F- | PR | F- | PR |
| `RecoveryMember(NeedsStop)` | F- | PE | F- | F- | F- | PR | PR |
| `Stopping` | F- | PE | F- | F- | F- | F- | PR |
| `Retired` | F- | F- | F- | F- | F- | F- | F- |

The exact proxy source is checked before report kind. A report from another
owned role is foreign to this member and cannot be redirected by equal payload
shape. An unowned application child never enters this matrix.

## Initialization and proxy creation

Initialization reserves every `ProxyReservation` and pairs one affine initial
`OperationTicket` with one private witness per role in declaration order before
emitting the first creation. Each retained initial `OperationInput` owns that
ticket. Pair allocation is fresh by construction: another reservation cannot
collide with it, and process-wide allocation failure is not relabelled as a
supervisor rejection. If any proxy-route reservation rejects, every locally
paired operation is retired, the complete ordered initial set is returned, and
no proxy creation, observation, input, or partial fleet is emitted. Otherwise
one creation bundle per role contains:

1. the fresh stable-proxy creation;
2. exact creation observation;
3. exact established-capability observation; and
4. exact termination observation.

This is a Bombay staged-creation policy and a future real-boundary proof
obligation, not a general actor-model guarantee.

An emitted proxy operation retains its private witness only until one exact
settlement is admitted. Admission consumes the witness's correlation authority.
An accepted settlement retains the operation ID only when a later exact proxy
outcome requires it; a rejected, corrupt, or unattempted settlement already
owns the complete operation and ID. Retaining the consumed witness beside that
settlement would store arrival history rather than current authority. Foreign
settlement admission returns the witness and complete settlement unchanged.
Roster search attempts this affine admission in declaration order and restores
every declined member to the same position without allocation or a separate
owner query.

For an exact `ProxyBirthResult`:

- `Committed` enters `Starting(WaitingForAuthorization)` with the exact proxy
  and retained initial input. It does not consume activation capacity merely
  because the proxy exists.
- `FoldRejected` or `HostRejected` retains the complete initial input and
  applies the configured topology-failure reaction. `RetireMember` enters
  `Retired` because no proxy exists. `StopSupervisor` starts global drain.
- In `CreatingProxyAfterExit`, rejection plus authoritative exit is one
  contradiction retaining both facts. Commit proves an installed but already
  terminal proxy and retires that role or begins whole-supervisor drain; it
  never sends the initial input.

An exact `ProxyExit::BeforeBirth` changes `CreatingProxy` to
`CreatingProxyAfterExit`. An `AfterBirth` exit for the exact live proxy is
handled by the stable-proxy-death reaction. `RetireMember` preserves the exact
cause and removes only that role from live topology; `StopSupervisor` begins
global drain. No worker failure is inferred from proxy death.

If proxy exit races an emitted initial or replacement operation, the exit
atomically transfers that operation's input/report settlement to the surviving
lifecycle host and releases its activation slot by transfer, not by pretending
an atomic proxy outcome arrived. The affected participant leaves its recovery
batch. The complete exit moves into the `ProxyDied` diagnostic settlement;
the retired member retains only exact terminal correlation. Any later proxy
report or rejected-report settlement meets that same host-owned exit and is
returned as one contradiction retaining both facts. The actor does not clone
the exit into state or forget the pending operation merely to retire quickly.

This host-transfer equation is required for totality but remains an open
shared-settlement realization. An executable prototype must prove that the
locked interpreter can perform the transfer; otherwise the focused finding is
recorded rather than replaced by a mailbox flag.

After every transition that releases activation capacity, the supervisor scans
waiting roles in declaration order. It reserves at most the remaining positive
capacity and emits each eligible initial or replacement input in that same
order. Reservation and emission are one commit.

## Initial operation settlement and outcome

Authorization changes:

```text
Starting(WaitingForAuthorization { input: { ticket, kind, control } })
    -> Starting(Dispatched { ticket, settlement })
       + proxy_install_inputs(control)
         whose source settlement owns { ticket, kind, control }
```

The complete input leaves state. The fold consumes its already-reserved ticket;
authorization never performs a new fallible correlation allocation. Exact
settlement then follows:

- `Accepted` changes `Dispatched` to `AwaitingOutcome`; the operation ticket
  remains occupied.
- `Rejected` releases the ticket and returns the exact input. It emits one
  `ProxyInputRejected` diagnostic and applies the configured failure reaction.
  There is no automatic retry.
- A proxy outcome before accepted settlement is `F-`; the interpreter must
  discharge install settlement before making the later child report
  admissible.

In `AwaitingOutcome`, an exact `InitialInstallation` result releases the
operation ticket:

- `Ready { worker }` stores that opaque evidence in `Online` and, when
  selected, emits `Started { role, proxy }`.
- Any complete non-ready result is retained inside
  `ProxyOutcomeFailed`, then applies `RetireMember` by draining the still-live
  proxy or begins whole-supervisor drain. It never emits `Started`.

An `Unavailable` report from the exact owned proxy is expected in every live
non-retired member phase. It is relayed as one lifecycle event when publication
is selected, preserving role, sender, proxy phase, and command. Without
lifecycle publication, the same complete value is an operational diagnostic;
it is never discarded. This is a deliberate model policy required to preserve
the command obligation when no lifecycle consumer exists.

## Recovery admission

Only an exact `WorkerStopped` from `Online` can trigger automatic recovery. It
first produces the semantic `Empty` subject, then evaluates one transaction
from the pre-transition fleet snapshot:

1. classify the complete stop as normal or abnormal;
2. apply permanent, transient, or temporary eligibility;
3. select one-for-one, one-for-all, or rest-for-one roles in declaration order;
4. reject every role already recovering, stopping, retired, or owned by an
   admitted replacement;
5. include an initially unresolved role required by a coordinated strategy as
   `AwaitingInitial` rather than pretending it is ready;
6. reserve one exact batch-preparation correlation, emit the typed ordered
   worker-preparation action, and enter the pending preparation phase;
7. join its exact source settlement, retaining the source authority, every
   prepared submission, every selected role, and any rejection;
8. reserve the recovery ticket, one operation ticket per selected replacement,
   and every fallible timer correlation;
9. prune budget evidence at the interpreter-authored stop time using an
   inclusive window;
10. reject clock regression explicitly;
11. validate the complete multi-role budget charge;
12. compute the checked one-based delay from the triggering role's next
    admitted `RecoveryOrdinal`; and
13. commit the membership partition, prepared ownership, exact correlations,
    budget charge, and resulting actions in one transition.

Steps 1–5 are pure local preparation. Step 6 commits only the pending
preparation phase and its action. Steps 8–13 occur only after the exact batch
settlement succeeds. Failure changes no peer, emits no replacement, and charges
no budget. The trigger's authoritative stop has still occurred;
the configured failure reaction applies to that exact now-empty role.

Every selected replacement receives one new affine operation pair before the
recovery commits. A pair cannot equal another live or retired pair. The
complete ordered candidate set therefore has no operation-allocation rejection
case; worker preparation, recovery/timer correlation, budget, and checked
release remain the fallible pre-commit decisions.

The delay ordinal is deliberately the triggering role's admitted recovery
ordinal. Selected peers do not combine their unrelated history into one delay.
This fills a policy choice left implicit by the governing documents and must
be compared before production work.

No source selects a reset rule. Bombay therefore preserves the triggering
role's lifetime admitted count and advances it with checked arithmetic.
Exhaustion is `RecoveryCountExhausted { trigger, admitted }`; it does not wrap,
saturate, panic, reset implicitly, or masquerade as delay arithmetic overflow.
This is a deliberate Bombay representation policy, not an actor-model
guarantee.

Eligibility is total:

| Policy | Normal stop | Abnormal stop |
|---|---:|---:|
| Permanent | eligible | eligible |
| Transient | ineligible | eligible |
| Temporary | ineligible | ineligible |

Ineligible termination is not topology failure. The exact role remains
`Empty` behind its live stable proxy and no recovery budget is charged. This
is the AA-10 policy selected for the feature catalogue's previously open
“retires or leaves empty” choice. It allows status to distinguish an intact
but unavailable role from a lost proxy. Only explicit proxy/topology failure
uses `FailureReaction`.

When lifecycle publication is configured, the exact ineligible stop moves into
the role's `WorkerStopped { disposition: Ineligible }` event. The durable
`Empty` role then owns no second stop. When publication is omitted, no lifecycle
event is constructed and the explicit omission policy discharges the already
classified stop. `Empty` retains no historical report merely for a later
shutdown. Ineligibility is not an operational failure and therefore does not
manufacture a diagnostic merely to dispose of the stop.

Selection is total:

- one-for-one selects the triggering role;
- one-for-all selects every restartable role in the immutable snapshot;
- rest-for-one selects the trigger and every restartable role declared after
  it;
- an online peer contributes its exact current worker;
- the trigger contributes its exact stopped predecessor;
- an initial operation that was already admitted may contribute one
  `AwaitingInitial` participant;
- every other unresolved or terminal role is not restartable.

If a coordinated strategy requires an inadmissible role rather than one of the
explicitly supported awaiting-initial phases, admission rejects the complete
decision. It never silently shrinks the selected set.

Factory rejection identifies its role and returns every prepared peer
definition. Budget denial reports active attempts, requested replacement
count, and maximum. A maximum of zero denies every non-empty decision. Each
admission appends one charge whose `replacements` equals participant count to
the supervisor's `BudgetState`; the sum of active charge counts never exceeds
the maximum. `RecoveryBatch` does not duplicate that evidence.

## Recovery prerequisite transitions

Immediate timing starts in `Ready`. Delayed timing emits one exact schedule
request for the batch. Schedule acceptance changes `Scheduling` to `Waiting`;
rejection applies the topology-failure reaction to the trigger and returns every
still-unemitted replacement. An exact timer input changes `Waiting` to `Ready`
once. Stale, duplicate, wrong-generation, foreign, early, or regressing timer
inputs are `F-`.

Replacement issue requires these three current truths:

- its exact predecessor/readiness prerequisite is satisfied;
- its batch release is `Ready`; and
- activation capacity is currently available.

Readiness and timing remain satisfied in their owning participant and release state.
Capacity is derived afresh and is consumed only in the transition that issues
the input, so no stored phase claims a slot before issuance. That transition
consumes the prepared input's already-reserved `OperationTicket`, emits the
complete replacement input, and stores `NeedsReceipt` for a stopped participant
or `NeedsReceiptAndStop` for an online participant. No fallible correlation
allocation remains after batch admission. A batch may issue several newly
eligible participants in one turn only up to available capacity, in declaration
order.

An `AwaitingInitial` participant continues to process its exact `InitialPhase`:

- accepted input settlement advances normally;
- a ready initial outcome emits `Started`, supplies the exact predecessor,
  satisfies readiness, and may issue replacement in the same fold;
- rejected input or non-ready atomic outcome cannot satisfy readiness. The
  participant leaves the batch and applies topology failure for that role;
- the remaining admitted participants continue. Already emitted replacements
  are not rolled back, because atomicity governed admission, not cross-actor
  completion.

The recovery batch retires only after every participant has left through a
ready replacement, typed failure reaction, shutdown transfer, or forced
retirement. Its ticket is never reused.

## Replacement settlement and outcome

For an exact `NeedsReceiptAndStop` or `NeedsReceipt` participant:

- accepted settlement changes only the remaining requirements and retains its
  occupied operation ticket;
- rejected settlement releases the ticket, retains the complete input in one
  diagnostic, and applies topology failure to that role without retry.

An exact `WorkerStopped` for the owned predecessor is accepted in
`NeedsReceiptAndStop`, `NeedsStopAndOutcome`, or `NeedsStop`. It removes only
the stop requirement, may emit the optional lifecycle event, and never opens or
charges another recovery decision. A duplicate or stop for another worker is
`F-`.

For an exact `NeedsStopAndOutcome` or `NeedsOutcome` participant:

- every exact replacement outcome releases the activation ticket immediately;
- if worker stop is already owned, `Replacement::Ready { worker }`
  leaves the recovery batch, enters `Online`, and optionally emits one
  `Restarted` carrying the exact proxy and recovery ticket;
- if worker stop is still required, the complete result enters `NeedsStop`;
  readiness is not externally published until
  the matching stop report or its typed delivery settlement closes the join;
- after the join closes, every rejection, activation failure, pre-ready stop,
  or contradiction applies topology failure and never emits `Restarted`;
- an initial-installation report is wrong-kind even if its nested payload has
  the same worker type.

An exact `WorkerStopped` in `NeedsStop` closes the reunion and
applies the retained outcome once. Rejection of the stop report delivery closes
the same semantic join through lane settlement while preserving the rejected
report with the lifecycle host; it cannot fabricate a successful stop event.

Operating recovery consumes a terminal `WorkerReplacement` immediately through
one exhaustive replacement disposition. A ready outcome plus the exact stop is
`Restarted`; a non-ready outcome plus the exact stop is `ProxyFailed`; an input
rejection is `InputRejected`; every other current combination remains
`Waiting`. This disposition is a transition result, never another stored member
phase. It retains complete stop, outcome, or rejected-input ownership and chooses
no diagnostic disposition, topology reaction, lifecycle route, or action.
Shutdown retains the unchanged `WorkerReplacement` instead: retirement requires
both proxy retirement and return of the emitted replacement input or outcome.

The transient terminal values preserve ownership explicitly. `Restarted` and
`ProxyFailed` carry `StoppedWorker { readiness, stop }`; `InputRejected` carries
`RejectedReplacement { previous, readiness, stopped, operation, returned_worker
}`. The aggregate then consumes those values once. Success moves the successor's
attempt/readiness into `Online`; predecessor readiness is discharged only after
the exact stop has joined. Failure moves predecessor attempt/readiness into the
selected member-retirement or supervisor-drain reaction. When lifecycle
publication is configured, the exact stop moves into `WorkerStopped`; otherwise
a failed replacement retains it with retirement ownership. There is no separate
stop-presence flag or duplicated stop claim.

For successful replacement with lifecycle publication, `WorkerStopped` precedes
`Restarted` in the declared lifecycle lane. Omitted publication constructs no
lifecycle value. Input rejection and non-ready proxy outcome construct exactly
one complete operational diagnostic and apply the configured topology reaction;
neither can fabricate `Restarted`. Removing a terminal participant releases its
operation occupancy and allows the existing declaration-order authorization
transition to consider the next waiting participant in the same turn.

An exact spontaneous `WorkerStopped` opens a new recovery only in `Online`.
While `RecoveryMember::Waiting` still owns an online participant, its first
exact stop changes that participant to stopped and does not open another batch.
A second stop while the member is empty or recovering is stale unless it is the
one explicit replacement reunion above. A stop of an unrelated
peer that remains independent and online may trigger its own disjoint batch.

## Failure reaction

Every topology failure first constructs one exact diagnostic. Then:

- `RetireMember` asks the exact live proxy to stop and enters `Stopping`; if no
  proxy was committed or it already exited, the member enters `Retired`
  directly. Shutdown settlement and exact proxy exit form one order-independent
  join. Accepted settlement retires its receipt; non-accepted settlement remains
  complete in the retired member after exact exit. Unrelated roles and disjoint
  recovery batches remain live.
- `StopSupervisor` begins the same global drain as management shutdown after
  preserving the diagnostic in that action.

Factory rejection, budget denial, checked-delay failure, schedule rejection,
proxy creation rejection, proxy input rejection, failed atomic proxy outcome,
and stable-proxy death enter this equation. Ineligibility does not.

If diagnostic disposition is `Terminate`, it dominates `RetireMember`: the
diagnostic action selects terminal settlement and the lifecycle host owns the
remaining fleet drain. This is not silently converted into the configured
topology reaction. A production realization must reconcile the actor's
immediate stop verdict with exact child ownership before claiming this path is
implemented.

## Shutdown normalization

The first `Shutdown` atomically:

1. closes management mutation and recovery admission;
2. logically cancels every un-emitted prepared initial/replacement input;
3. preserves emitted input settlement and atomic-outcome ownership;
4. cancels delayed recovery batches so no later timer can issue work;
5. transforms every member into one exhaustive drain member;
6. emits at most one shutdown request for every exact installed proxy;
7. retains every pending proxy creation until it resolves; and
8. for deadline policy, reserves and emits one exact drain timer.

The drain partition is:

```text
DrainMember =
    ResolvingProxyBirth {
        role,
        reservation: ProxyReservation,
        retained_initial,
        prior_exit: NoExit | Exited { exit },
    }

  | DrainingProxy {
        role,
        proxy: ProxyIncarnation,
        outstanding: ProxyOutstanding,
        phase: ProxyStopPhase,
    }

  | Drained {
        role,
        result: Graceful | Absent | Forced { residual },
    }

ProxyOutstanding =
    Idle
  | InitialWaiting { input: OperationInput }
  | InitialEmitted { ticket, settlement_or_outcome }
  | RecoveryWaiting { recovery, prepared }
  | ReplacementEmitted { recovery, ticket, settlement_or_outcome }
```

These variants own the complete values that were local at shutdown. There is
no `pending` flag and no attempt to reconstruct an emitted input. Recovery
batches dissolve into their per-proxy outstanding ownership plus cancelled
timer settlement.

For `ResolvingProxyBirth`, exact rejection enters `Drained(Absent)` and creates
no proxy. Exact commit emits one shutdown for the installed proxy and enters
`DrainingProxy`. Commit after an authoritative pre-birth exit records a
contradiction and counts the already terminal proxy drained; it cannot send to
the dead proxy.

For `DrainingProxy`, shutdown settlement rejection stores the complete request
and reason in `ProxyStopPhase::Rejected` while the exact exit observation
remains live. It does not count as drained. An exact proxy exit consumes every
outstanding operation through the child's typed drain settlement, preserves
its complete result with the lifecycle host, and enters `Drained(Graceful)`.
The normal proxy report lane is not required to fabricate an initial or
replacement outcome during child shutdown.

This exact child-drain settlement is a model requirement exposed by AA-01 and
remains an open real-boundary proof obligation. If the interpreter cannot
return emitted install/replacement ownership when the child suppresses normal
parent reports during shutdown, later prototype work must record the focused
kernel gap rather than drop the operation ticket.

Repeated shutdown is `S=`. It emits no duplicate proxy shutdown or deadline.
Late restart timers and schedule settlements can settle cancelled requests but
cannot restore a `RecoveryBatch` or emit replacement.

## Deadline drain

```text
DeadlinePhase =
    WaitingWithoutDeadline
  | Scheduling { timer, settlement }
  | Waiting { timer }
  | Fired { timer, observed_at }
```

`WaitForActorGraph` uses `WaitingWithoutDeadline` and may wait indefinitely.
`RetireActorGraphAfter` uses exact interpreter-authored monotonic time. Wrong,
duplicate, stale, early, or regressing deadline facts are `F-`.

Exact deadline-schedule acceptance changes `Scheduling` to `Waiting`. Exact
rejection cannot silently weaken `RetireActorGraphAfter` into an unbounded
wait. In the rejecting fold, every unresolved member moves to
`Drained(Forced { residual })`; the residual preserves the complete rejected
request, timer correlation, capability reason, and every other outstanding
ownership claim. The rejection is the forced-retirement cause and transfers
directly to the surviving lifecycle host. Any operational diagnostic follows
the selected disposition independently; its delivery cannot gate or undo the
forced transfer.

When the exact deadline fires, the aggregate atomically selects forced
retirement for every unresolved `DrainMember`. Its terminal ownership must
contain the role, proxy or reservation, current semantic phase, every
outstanding operation/timer/action settlement, and cause. The implementation
may project this as `Drained(Forced { residual })` per member or retain the
complete stopped supervisor plus its exact deadline cause; it must not store
both. Ownership transfers to the surviving lifecycle host. No accepted
shutdown, proxy exit, worker readiness, successful restart, or normal lifecycle
event is fabricated.

The supervisor selects normal actor termination only when every member is
`Drained` and every action in the final transition has a settlement owner. A
forced actor-graph summary is not the application result: the live root remains
in residual settlement until late activation, proxy, and delivery ownership
resolves.

Unrelated application children and application births never enter
`DrainingFleet`, are never queried, and are never stopped by this actor.

## Stale, overlap, and contradiction laws

- A proxy birth or exit advances only the role owning its exact reservation or
  installed capability.
- An install/replacement settlement advances only its exact role, proxy,
  operation ticket, and expected dispatched phase.
- An atomic proxy report advances only the exact child source and expected
  operation kind. Nested payload similarity cannot substitute for kind.
- A duplicate report after its operation ticket released is rejected and
  cannot release another slot.
- A worker stop outside `Online` cannot start recovery.
- One role cannot join two recovery batches. Overlap rejects the new complete
  decision before factory results, budget, or peers commit.
- A timer fact advances only its exact recovery ticket and timer generation.
- Proxy creation rejection plus authoritative exit is a contradiction
  retaining both facts. Commit plus prior exit retires the exact dead proxy.
- A report claiming readiness from a proxy already authoritatively stopped is
  a contradiction, never a transient `Started`/`Restarted`.
- A delivery rejection cannot roll back an already committed member or budget
  transition. Its lane-specific payload moves to settlement.
- Diagnostic delivery rejection is terminal and non-recursive.
- No stale or contradictory input is consumed merely to make a later fact
  easier to accept.

## Feature-catalogue trace

| Requirement family | Model evidence |
|---|---|
| `FS-BUILD` | construction domain, non-empty unique role order, common protocol, optional lifecycle publication |
| `FS-TOPOLOGY` | normalized state, independent-member sum, exact proxy creation, activation occupancy derived from operation states |
| `FS-FACTS` | complete proxy lifecycle input, operating-member matrix, predecessor stop/outcome join, stale and contradiction laws |
| `FS-OPERATE` | total status/capability projection, distinct query/lifecycle/diagnostic routes |
| `FS-ELIGIBILITY` | complete permanent/transient/temporary table and selected intact-proxy `Empty` policy |
| `FS-STRATEGY` | immutable snapshot, ordered selection, awaiting-initial participant, disjoint overlap law |
| `FS-BUDGET` | pre-commit pruning, inclusive window, atomic counted charge, zero maximum, clock regression |
| `FS-TIMING` | exact timer settlement, trigger ordinal, checked delay, eight-state prerequisite sum |
| `FS-ATOMICITY` | twelve-step recovery preparation and one batch/budget commit |
| `FS-FAILURE` | exact operational diagnostic followed by `RetireMember` or whole-supervisor drain |
| `FS-SHUTDOWN` | normalized drain partition, pending creation, emitted-operation transfer, exact shutdown rejection, deadline residual |

Every family remains a model claim, not implementation or verification
status. Open shared settlement, diagnostic, activation, terminal-lift, and
deadline mechanisms keep their dependent production coverage rows open.

## Independent-model obligations

AA-10 defines the oracle structure for later tests. The executable model must
not copy the implementation branch structure. It should use independent
vocabulary such as roster cells, admitted recovery batches, occupied permits,
and drain obligations, then compare complete traces.

At minimum later tasks must cover:

- empty and duplicate-role construction rejection;
- initial proxy creation in declaration order and partial-preparation failure;
- proxy exit before/after birth resolution;
- positive activation bounds of one and two with declaration-order release;
- foreign, duplicate, and stale initial or replacement operation IDs, including
  proof that every new operation pair differs from every earlier pair;
- accepted and rejected install settlement before atomic proxy outcome;
- total query projections in every member phase;
- lifecycle and capability projections that expose only the stable proxy as a
  service capability;
- permanent/transient/temporary eligibility for normal and abnormal stops;
- one-for-one, one-for-all, and rest-for-one selection from immutable snapshots;
- unresolved-initial participants and overlapping failure rejection;
- zero budget, inclusive window boundary, pruning, multi-role atomic charge,
  and clock regression;
- immediate, constant, linear, and exponential timing, checked overflow, and
  all eight readiness/timer/authorization gates;
- factory rejection at every selected position with no peer mutation;
- stale/duplicate/foreign report, timer, exit, and settlement facts;
- lifecycle omitted versus delivered and both diagnostic dispositions;
- shutdown from every member phase, creation/exit ordering, rejected child
  shutdown, rejected deadline scheduling, late timers, and forced residual
  transfer; and
- complete named actions and accepted/rejected interpreter settlement.

## First-slice non-features and open findings

AA-10 intentionally does not provide:

- Rust supervisor, fleet, recovery, builder, protocol, event, or effect types;
- an executable fold or independent model implementation;
- a shared recovery framework or reuse of dynamic-supervisor/pool state;
- proxy-private worker creation, initialization, activation, or worker routing;
- an interpreter for activation capacity, delivery settlement, timers,
  diagnostics, deadlines, or residual root ownership;
- wrapper-composition, initialization-order, compiler/DevX, or migration proof;
- a capability-safe heterogeneous public-protocol supervisor; or
- edits to the locked kernel, legacy actors, macros, testkit, or runtime.

The model records four findings requiring later comparison:

1. the governing documents leave ineligible fixed members “retired or empty”
   without a selected construction policy; this oracle deliberately leaves an
   intact live proxy `Empty`;
2. delayed coordinated recovery does not state which member history determines
   its one-based delay; this oracle uses the triggering role's admitted
   recovery ordinal;
3. the named fixed-supervisor effect product has terminal diagnostics but no
   explicit route-delivered diagnostic operation even though lifecycle
   publication is optional; and
4. normal proxy death or shutdown with an emitted install/replace operation
   requires a typed child terminal/drain settlement that returns outstanding
   ownership even when no normal proxy outcome can reach the supervisor.

These are falsification results, not changes to production authority. Before
an executable task relies on any new mechanism, the solution, matrix,
retained-core decision, and research audit must be reconciled together or the
existing real boundary must be proven to express the same law unchanged.

This blocked specification may be compared with the independent dynamic
supervisor, FIFO pool, and keyed pool models, but no comparison may treat its
child-terminal settlement requirement as implemented or extract machinery
from that assumption. It does not make AA-11 eligible: executable fixed
supervision depends on AA-07's completed proxy boundary audit and eventual
AA-10 closure.
