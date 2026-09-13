# Stable proxy falsification model

## Status and authority

This document is the AA-01 independent semantic model for a stable worker
proxy. It is a non-production falsification oracle. It does not define Rust
types, a second behavior algebra, an interpreter contract, or evidence that a
solution-matrix row is implemented. It is authoritative for the StableProxy
behavior it covers. [`docs/stable-proxy.md`](../../../docs/stable-proxy.md) maps
that behavior to the selected implementation, and
[`docs/atomic-runtime-settlement.md`](../../../docs/atomic-runtime-settlement.md)
owns runtime interpretation and custody.

The actor-model law used here is fresh allocation: creating a replacement must
establish a fresh actor rather than overwrite an existing address. Staged
creator-local correlation, commit-before-dependent-effects ordering, exact
installation, activation, owner-only control, and drain policy are Bombay
derivations or policy choices. The stable public identity and replacement
protocol are template laws.

Runtime route selection is owned normatively by
`docs/atomic-runtime-settlement.md`. The proxy issues and stores only a
`CreationId`; it never chooses or stores a runtime route before emitting
`CreateChild`. Any older `reservation` label in the transition oracle denotes that
creator-visible correlation before interpretation and routed evidence only
inside the returned runtime settlement. It does not authorize a separate route
request, waiting phase, public route, or combined value shared by proxy and
Bombay.

Older shutdown labels that distinguish worker-return order describe semantic
cases, not required Rust variants. Once the worker has stopped, the retained
current product is the worker, exact stop, remaining initialization or
activation value, and an optional exact shutdown receipt. Absence is lawful only
when no shutdown request was emitted. H147 proves that separate
`AfterDeparture`, `AfterStop`, and terminal `*AfterStop` Rust alternatives add
arrival history without changing an accepted transition.

The model must falsify designs that make any of these claims false:

1. One proxy owns at most one installed worker incarnation.
2. A service command is sent only to the exact current incarnation in `Ready`.
3. Installation is not readiness.
4. Replacement drains the exact predecessor and creates a fresh successor.
5. Every accepted transition produces one next state and one complete named
   action product; no effect is ambient.
6. Shutdown closes admission and settles every definition, creation,
   initialization, activation, and incarnation the proxy still owns.
7. Stale, duplicate, foreign, and contradictory facts cannot advance state.

“Atomic” below means one local `Behavior` fold commits a state and its complete
`Actions`. It does not claim atomic delivery, creation, or cross-actor work.

## Semantic values

The following names are model vocabulary, not proposed public Rust names.

```text
BirthKind =
    Initial
  | Replacement { replaces: WorkerIncarnation }

BirthAttempt = the fresh, non-reused CreationId issued by the proxy
RoutedBirth = interpreter-private { attempt: BirthAttempt, route }
WorkerRecipient = proxy-private routable capability for one committed worker birth
WorkerIncarnationEvidence = opaque non-routable provenance for that birth
WorkerIncarnation = private { recipient: WorkerRecipient, evidence: WorkerIncarnationEvidence }
InitSettlement = complete initialization effects retained by the lifecycle host
InitAttempt = exact correlation for host-owned initialization work
ActivationPlan = the concrete work required before the worker may be ready
ActivationPermit = a one-shot capability issued only after successful initialization
ActivationAttempt = a fresh, non-reused correlation for one consumed permit and plan

ActivationPhase =
    AwaitingStartSettlement
  | InFlight

DrainCompletion =
    ReturnToEmpty
  | StopProxy

ActivationFailure =
    StartRejected { request, reason }
  | Rejected { rejection }

WorkerInitializationFailure = EffectsRejected | InterpreterCorrupt
InitFailure = { failure: WorkerInitializationFailure, activation }

ExitEvidence =
    FromInitialization {
        observation, stop
    }
  | FromObserver {
        observation, stop
    }

ExitSettlement =
    Observed { evidence: ExitEvidence }
  | Transferred {
        observation, worker: WorkerIncarnation, reason
    }

WorkerDrain =
    Exited { exit: ExitSettlement }
  | Transferred { residual }

PreReadyStop =
    DuringInit {
        activation,
        exit: ExitSettlement
    }
  | InitializedAfterExit {
        permit, activation,
        exit: ExitSettlement
    }
  | ReadyAfterExit {
        attempt, proof,
        exit: ExitSettlement
    }

BirthDrainResult =
    FoldRejected {
        creation, error, definition_and_plan
    }
  | HostRejected {
        creation, rejection, prepared_init, activation
    }
  | Contradiction { facts }

InitDrainWork =
    Initialized { permit, activation }
  | Rejected { rejection, activation }

InitDrainResult =
    WorkAndExit {
        work: InitDrainWork,
        exit: ExitSettlement
    }
  | FailureAndExit {
        failure: InitFailure,
        exit: ExitSettlement
    }
  | StoppedDuringInit {
        activation,
        exit: ExitSettlement
    }
  | Transferred {
        phase: InitDrainPhase,
        residual
    }

InitDrainPhase =
    AwaitingBoth { attempt: InitAttempt }
  | InitSettled {
        work: InitDrainWork
    }
  | ExitObserved {
        attempt: InitAttempt,
        exit: ExitSettlement
    }

ActivationDrainResult =
    StartRejected { request, reason }
  | ReadyAfterCancel { proof }
  | RejectedAfterCancel { rejection }

ActivationDrainPhase =
    AwaitingBoth { phase: ActivationPhase }
  | ActivationSettled { result: ActivationDrainResult }
  | ExitObserved {
        phase: ActivationPhase,
        exit: ExitSettlement
    }
```

An attempt is neither an address nor proof of freshness. A creation becomes an
installed incarnation only when the interpreter commits fresh host ownership.
All correlation values are exact and non-reused. Two proxies issuing their
first worker attempt must still produce unequal evidence; a local ordinal is
diagnostic data, not identity. Sequence arithmetic, address reuse, timing, or
adjacency cannot manufacture provenance.

The proxy never publishes `WorkerRecipient` or the private
`WorkerIncarnation` product. It alone routes service through
`worker.recipient`. Parent outcomes project only
`WorkerIncarnationEvidence`; the stable proxy remains the sole externally
routable service capability.

Every payload in this model is complete and owned. Field names therefore use
domain words such as `worker`, `request`, `rejection`, `stop`, and `exit`
without repeating `complete_` or structural implementation phrases.

## Complete state sum

Each variant owns exactly the values listed. `kind` is the closed `BirthKind`
sum, not an optional predecessor.

```text
Dormant
    no installation has ever been attempted

Creating {
    kind, reservation: ChildReservation,
    stopped: Option<ExitSettlement>
}
    one complete worker definition and activation plan have moved into the
    staged creation action and its unresolved settlement; `stopped` is exactly
    absence or one authoritative pre-birth stop

Initializing {
    kind, worker: WorkerIncarnation, attempt: InitAttempt,
    stopped: Option<ExitSettlement>
}
    installation committed; the lifecycle host owns unresolved initialization
    settlement and the activation plan under this exact correlation; the proxy
    retains at most one already-arrived exact stop

Activating {
    kind, worker: WorkerIncarnation,
    attempt: ActivationAttempt, phase: ActivationPhase,
    stopped: Option<ExitSettlement>
}
    one exact permit and plan were consumed by one `BeginActivation` action;
    stop absence/presence remains current input to the readiness decision

Ready { worker: WorkerIncarnation }
    the one routable exact current worker

EmptyInitial
    the initial operation failed before installation committed

EmptyAfter { previous: WorkerIncarnation }
    a previously installed incarnation is terminally settled

StoppingForReplacement {
    current: WorkerIncarnation,
    next: ChildReservation,
    replacement
}
    the predecessor is being drained; the successor definition has not moved
    into creation yet

DrainingCreation { kind, reservation: ChildReservation }
    shutdown owns an unresolved creation settlement

DrainingCreationAfterExit {
    kind, reservation: ChildReservation,
    exit: ExitSettlement
}
    shutdown owns creation settlement and already has the pre-birth exit

DrainingInitFailure {
    kind, worker: WorkerIncarnation, failure: InitFailure,
    completion: DrainCompletion
}

DrainingInit {
    kind, worker: WorkerIncarnation, phase: InitDrainPhase
}

DrainingActivationFailure {
    kind, worker: WorkerIncarnation, attempt: ActivationAttempt,
    failure: ActivationFailure, completion: DrainCompletion
}

DrainingActivation {
    kind, worker: WorkerIncarnation, attempt: ActivationAttempt,
    phase: ActivationDrainPhase
}

DrainingWorker { worker: WorkerIncarnation }
    one shutdown/worker-exit observation remains unresolved

Stopped
    terminal; no mailbox event is admitted
```

`EmptyInitial` and `EmptyAfter` are distinct because only the latter has
replacement provenance. The drain variants contain closed work/cause and join
sums; they do not coordinate optional fields, cross-product flags, or
already-settled live combinations.

At most one of `Initializing`, `Activating`, `Ready`,
`StoppingForReplacement`, `DrainingInit*`, `DrainingActivation*`,
or `DrainingWorker` can exist, proving the installed-incarnation bound
structurally. `Creating` and `DrainingCreation*` own no installed capability.

The only empty-state projections are explicit:

```text
PreBirthEmpty(Initial) = EmptyInitial
PreBirthEmpty(Replacement { replaces }) = EmptyAfter { previous: replaces }
PostBirthEmpty(worker) = EmptyAfter { previous: worker }
```

Therefore an initial attempt that committed an incarnation and then stopped or
failed cannot return to `EmptyInitial`.

## Complete input sum

There are three ingress lanes. They are distinct protocols, even where the
table presents them together.

### Public service input

```text
Service { sender, command }
```

### Private owner control input

```text
InstallInitial { definition, activation }
Replace { definition, activation }
Shutdown
```

Only the established structural owner can construct the control capability.
Clients can construct only the worker's concrete service protocol.

### Lifecycle facts

```text
BirthResult =
    FoldRejected { reservation, definition_and_plan, error }
  | HostRejected {
        reservation, behavior, init_actions, activation, reason
    }
  | Committed {
        reservation, worker: WorkerIncarnation, init: InitAttempt
    }

InitResult =
    Initialized {
        worker: WorkerIncarnation, init: InitAttempt,
        permit: ActivationPermit, activation: ActivationPlan
    }
  | EffectsRejected {
        worker: WorkerIncarnation, init: InitAttempt,
        failure: WorkerInitializationFailure, activation: ActivationPlan
    }
  | Stopped {
        worker: WorkerIncarnation, init: InitAttempt,
        activation: ActivationPlan,
        exit: ExitEvidence
    }

ActivationStartResult =
    Accepted { worker: WorkerIncarnation, attempt: ActivationAttempt }
  | Rejected {
        worker: WorkerIncarnation, attempt: ActivationAttempt,
        request, reason
    }

ActivationResult =
    Ready { worker: WorkerIncarnation, attempt: ActivationAttempt, proof }
  | Rejected { worker: WorkerIncarnation, attempt: ActivationAttempt, rejection }
  | ReadyAfterCancel { worker: WorkerIncarnation, attempt: ActivationAttempt, proof }
  | RejectedAfterCancel {
        worker: WorkerIncarnation, attempt: ActivationAttempt, rejection
    }

WorkerExit =
    BeforeBirth {
        reservation: ChildReservation,
        evidence: ExitEvidence
    }
  | AfterBirth {
        worker: WorkerIncarnation,
        evidence: ExitEvidence
    }

ProxyDiagnostic =
    UnexpectedWorkerReservation { phase, result: ReservationResult }
  | UnexpectedWorkerStart { phase, worker: WorkerStart }
  | UnexpectedWorkerStop { phase, stopped: WorkerStop }
  | UnexpectedWorkerInitialization { phase, initialization: WorkerInitialization }
  | UnexpectedWorkerActivation { phase, activation: WorkerActivation }
  | UnexpectedWorkerShutdown { phase, shutdown: WorkerShutdown }
```

Each alternative owns the complete unexpected input. `phase` is observable
status data, not correlation authority. The exact expected reservation,
worker, attempt, or shutdown token remains solely in the unchanged proxy state.
Requiring the diagnostic to own that token as well would either duplicate an
affine authority or make unchanged-state preservation impossible in safe Rust.

The split between activation start settlement and later activation result is
intentional. A typed `Ready` value injected by a test is only model input. It
is not implementation evidence.

Delivery rejection for service sends, parent reports, worker shutdowns, and
logical cancellation remains lifecycle-host settlement rather than proxy
mailbox input. Those settlements are still mandatory and must be witnessed in
AA-06; excluding them from this input sum does not discard them or make the
proxy their universal rejection owner.

The proxy's normal template report remains the solution's four-variant sum:

```text
ParentReport =
    InitialInstallation { outcome: InstallationOutcome }
  | Replacement { outcome: ReplacementOutcome }
  | WorkerStopped { stop }
  | Unavailable { sender, phase, command }

InstallationOutcome =
    Rejected { definition_and_plan, phase, reason }
  | Resolved { result: WorkerStartResult }

ReplacementOutcome =
    Rejected { definition_and_plan, phase, reason }
  | CancelledBeforeBirth {
        replaces: WorkerIncarnationEvidence,
        replacement,
        reason: Shutdown
    }
  | Resolved {
        replaces: WorkerIncarnationEvidence, result: WorkerStartResult
    }

WorkerStartResult =
    CreationRejected {
        rejection: WorkerCreationRejection,
        activation,
        stopped: None | ExactWorkerStop
    }
  | Ready { attempt: WorkerAttempt, readiness }
  | Unavailable { attempt: WorkerAttempt, drain: WorkerDrain }
```

`WorkerCreationRejection` is the sole owner of the exact pre-commit cause and
returned worker value. `WorkerDrain` is the sole owner of the exact committed
worker failure cause and every proxy-held affine value. `WorkerStartResult`
classifies only the three decisions its consumer makes; it does not repeat
either nested cause as another outer alternative. `stopped` is present on a
creation rejection only when the proxy had already accepted the exact stop.

Every `attempt` and `replaces` field in `ParentReport` is opaque evidence. A
`WorkerStopped` report likewise contains terminal provenance and no
`WorkerRecipient`. The owner can correlate lifecycle without acquiring a
delivery route around the stable proxy.

Control rejection and contradiction are nested outcomes, not additional
top-level parent-report variants. Shutdown completes through behavior
termination and lifecycle-host settlement; it is not a fifth parent report.
An owner never rebuilds one atomic outcome by joining several lower-level
reports.

`ProxyDiagnostic` is deliberately not smuggled into that sum. It travels on
the separate model-required `diagnostics` lane to the
established owner. This is part of the recorded product disagreement and must
be reconciled before production authority changes.

For non-shutdown operations, the atomic parent outcome carries every
proxy-owned leftover. A creation rejection carries the complete creation
rejection, activation plan, and any already accepted exact stop. An unavailable
committed worker carries one `WorkerDrain`, whose exhaustive alternative owns
the exact initialization, activation, early-stop, shutdown, and stop values.
The lifecycle host separately retains the complete concrete action settlement
in the worker environment. That settlement follows the one runtime retirement
path; it is not a second Behavior outcome channel.

During shutdown there is deliberately no parent report. The lifecycle host
receives `BirthDrainResult` for a pre-birth behavior/host rejection
or contradiction. After commit it receives exactly one
`InitDrainResult`: completed
initialization work plus exact exit settlement, a stop produced
by initialization with the unstarted plan, or a forced transfer containing the
complete unresolved join and residual ownership. These are the only
initialization-leftover alternatives; none may be dropped or reconstructed.
This enriches the solution's abbreviated nested outcomes only inside the
non-production oracle and remains an AA-06 runtime proof obligation.

Initialization and activation share only `WorkerStopping`, whose
shutdown-result/stop law is identical. Their outer shutdown state is not one
generic product: initialization has no pending domain value, whereas activation
owns a real waiting-for-start or running value. A unit, marker, ignored field,
or fabricated initialization token may not be introduced to make those shapes
appear substitutable. Each concrete state must store only the exact work and
worker-return values that remain current.

For initialization, the current work value is exactly
`Option<WorkerInitializationRetirement>`: absence means the result has not
returned; presence owns the complete result while worker departure remains
unfinished. A second result cannot replace the first and is returned complete.
No separate pending/returned initialization sum is lawful. Its stopped outcome
owns `Option<initialization stop>` rather than an arrival-order alternative:
presence retains the initializer's independently returned stop when the
enclosing worker already owns the observed stop; absence means the initializer
stop occupies that enclosing worker slot.

`WorkerExit::BeforeBirth` is admissible only in a state that owns the same
`ChildReservation`. `AfterBirth` is admissible only in a state that owns its worker
incarnation. The attempt and creator-local route are
reserved, stored, emitted, compared, and retired as one private product. No
field is inferred from the other; no optional field, address inference, or
timing decides which provenance is present.

The lifecycle host, not `Initializing`, owns the linear `InitSettlement` and
`ActivationPlan` while initialization work is unresolved. The proxy owns only
`InitAttempt`. A matching `InitResult` transfers the exact permit/plan,
failure-classification/plan, or stopped/plan product back once while the host
retains the complete rejected or corrupt settlement for retirement. This is an intentional
falsification-model clarification of the solution's shorthand
`Initializing { initialization_settlement, activation_plan }`; AA-02 and
AA-06 must prove a concrete locked-boundary realization before that shorthand
can become Rust state.

## Authorized readiness chain

Readiness is valid only if all of this chain is realized through the locked
`Behavior -> Actions` boundary:

1. Fresh creation commits one exact installed incarnation, closed to service.
2. Initialization actions settle successfully for that incarnation.
3. Settlement issues the one-shot `ActivationPermit` tied to that incarnation.
4. The proxy reserves a fresh `ActivationAttempt` and, in the same fold,
   consumes the permit and concrete plan into an explicit `BeginActivation`
   action.
5. The activation interpreter accepts or rejects that complete action. If
   accepted, it—not a test harness or arbitrary client—owns the in-flight work
   and the unique authority able to produce the result.
6. Only `Ready { worker, attempt, proof }` from that
   accepted work can enter `Ready`.
7. Rejection, stop, cancellation, a foreign incarnation, a wrong attempt, a
   duplicate, or a late result cannot publish readiness.

Who produces `Ready`: the concrete immediate or asynchronous activation
interpreter designated by the consumed permit and plan. Who authorizes it: the
exact one-shot permit obtained from successful initialization settlement. What
starts asynchronous work: the real `BeginActivation` item in `Actions`. What
correlates it: both exact incarnation and fresh activation attempt. Who owns it
while in flight: the lifecycle host's exact activation settlement record,
while the proxy owns the corresponding semantic waiting state.

The concrete capability representation and interpreter integration remain
open for AA-03 and AA-06. If they cannot be expressed without changing locked
foundations, that is a falsification result. This section must not be replaced
by a helper that mints `Ready` directly.

## Required falsification-model effect product

The governing solution currently names these six proxy lanes:

```text
worker_deliveries
worker_creations
worker_creation_observations
worker_stop_observations
worker_shutdowns
parent_reports
```

That product is insufficient for this model because the proxy itself emits two
activation operations and must continue after rejecting lifecycle facts. The
minimum model product therefore adds three concrete typed send lanes:

```text
activation_begin_requests: BeginActivation
activation_cancel_requests: CancelActivation
diagnostics: ProxyDiagnostic
```

The resulting nine-lane product is a non-production falsification requirement,
not an amendment to design authority. It records a focused disagreement with
the solution's six-lane product. AA-03 may not implement activation until the
solution, matrix, retained-core decision, and research audit are reconciled or
an existing named lane is proven to own the same exact operation without
reinterpretation.

The fold returns the real `Actions`, including its next behavior or
termination decision. That decision is not an extra effect lane.
Initialization settlement remains an interpreter/lifecycle-host responsibility
and is not an ambient behavior effect.

An emitted item transfers its complete value to action settlement. State
retains only correlation and ownership that legitimately coexist with the
emitted item. Creations still precede dependent observations and sends. The
relative order of the independent named send lanes remains an interpreter
obligation; AA-06 must prove the selected order rather than infer it from
product position.

Creation must commit before same-action sends or observations that depend on
the new child. Initialization settlement precedes `BeginActivation`.
Replacement shutdown precedes successor creation because the successor
definition remains in `StoppingForReplacement` until the predecessor's exact
worker exit. No row emits both service delivery and unavailability.

## Ownership table

| Value | Owned before acceptance | Owned after action emission | On rejection or final settlement |
|---|---|---|---|
| Initial/replacement definition and plan | incoming owner-control value, or `StoppingForReplacement` while predecessor drains | complete staged creation item/settlement | returned complete by typed control or creation rejection; never reconstructed |
| Child reservation | proxy state and matching birth/exit observation contracts | the attempt and creator-local route travel as one private product | consumed once by matching result; a mismatched attempt or route is rejected complete and neither is inferred |
| Worker incarnation | interpreter host after committed birth; proxy stores the worker capability | delivery/stop actions borrow no second ownership claim; host retains lifecycle ownership | moves through a drain until exact exit settlement; never inferred from a nonce |
| Initialization actions | committed initialization settlement | concrete worker environment | every item is accepted, rejected, or not attempted; the exact settlement transfers through runtime retirement and never enters proxy state |
| Activation plan before initialization settles | staged birth settlement, then lifecycle-host initialization work | proxy stores only `InitAttempt`; the matching result returns the same plan with a permit, failure classification, or stop | host rejection returns it with prepared initialization; effects rejection retains it through exit drain; it is never reconstructed or silently dropped |
| Activation permit and initialized plan | exact `Initialized` result | consumed together by `BeginActivation`, or by a typed not-activated shutdown/early-stop settlement | start rejection returns the complete unaccepted request; accepted work remains host-owned |
| Activation attempt and result authority | reserved by proxy; authority minted only by accepted activation interpretation | host owns in-flight authority, proxy owns matching waiting correlation | exact result consumes once; cancellation closes publication but does not fabricate physical cancellation |
| Service sender and command | incoming `Service` value | exact worker delivery only in `Ready`, otherwise complete `Unavailable` parent report | delivery rejection belongs to the lifecycle host; proxy does not replay implicitly |
| Replacement definition | incoming `Replace`, then `StoppingForReplacement` | moves to creation only after exact predecessor stop | overlap returns it complete; shutdown settles it without creating a successor |
| Worker-exit observation | lifecycle host under the child reservation or worker incarnation | remains host-owned while the proxy stores correlation; one authoritative init stop or observed exit yields `ExitSettlement` | satisfied exactly once, or transferred to residual lifecycle ownership; a later result is diagnostic |
| Worker exit | lifecycle host until delivered in `WorkerExit` | consumed into an early-stop join, drain, empty transition, or one parent report | duplicates, wrong-provenance, and foreign results move to `diagnostics` |
| Unexpected lifecycle input | incoming lifecycle result | complete input and current phase move once to `diagnostics`; proxy state is unchanged | diagnostic delivery rejection is settled by the lifecycle host without recursion |
| Parent report | proxy action item until interpretation | parent-delivery settlement record | rejection never rewinds proxy state and is settled by the proxy host |

## Transition notation

The total matrix uses these cells:

- `I0`: accept initial install; reserve one fresh attempt/route product, enter
  `Creating(Initial)`, and emit one fresh creation plus its observation.
- `R0`: accept replacement from `Ready`; reserve the complete successor
  attempt/route product before emitting one exact predecessor shutdown, then
  enter `StoppingForReplacement` retaining the complete successor.
- `R1`: accept replacement from `EmptyAfter`; reserve the complete successor
  product and enter
  `Creating(Replacement { replaces })` and emit creation.
- `C-`: reject control with the complete submitted definition/plan and current
  phase. Repeated shutdown is handled by `S=` instead.
- `D+`: deliver the untouched command once to the exact `Ready` incarnation.
- `U`: emit one `Unavailable` report with untouched sender, command, and phase.
- `S0`: close immediately, settle any control outcome, and enter `Stopped`.
- `S1`: close admission and enter the corresponding drain variant without
  duplicating an already-emitted action.
- `S=`: shutdown is already in progress; remain in the same drain state and
  emit no duplicate child shutdown or cancellation.
- `BR`, `IR`, `AS`, `AR`, `WE`: apply the matching-result rules below.
- `F-`: keep the same semantic state and emit exactly one
  `ProxyDiagnostic` on `diagnostics`; every other action lane is empty
  and the next decision is `Continue`.
- `NA`: the event is not admitted because `Stopped` is terminal. Late
  settlements are owned by the lifecycle host, not a stopped mailbox.

All `BR`, `IR`, `AS`, `AR`, and `WE` cells mean “matching exact correlation
only”; every mismatch is `F-`.

`F-` is a continuing diagnostic law, not `Behavior::Error`. The concrete
diagnostic alternative names the unexpected input kind, retains the complete
original input, and carries the current observable proxy phase. It does not
copy the proxy's expected correlation authority out of unchanged state.
Creating and initializing each have one public phase; their privately retained
optional stop is ownership needed by the later result, not another public work
phase.
If diagnostic delivery is rejected, the lifecycle host settles that complete
action directly and does not recursively send another diagnostic.

Reservation of both attempt and route is a pure, fallible precondition of
`I0`, `R0`, and `R1`. A collision or exhaustion of either component returns
the complete input through the appropriate
nested `InstallationOutcome::Rejected` or `ReplacementOutcome::Rejected` and
leaves the state unchanged. It never
emits creation and never replaces an existing binding.

## Total state/event matrix

| State | Install | Replace | Service | Shutdown | Birth result | Init result | Activation start | Activation result | Worker exit |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `Dormant` | I0 | C- | U | S0 | F- | F- | F- | F- | F- |
| `Creating` | C- | C- | U | S1 | BR | F- | F- | F- | WE |
| `Initializing` | C- | C- | U | S1 | F- | IR | F- | F- | WE |
| `Activating` | C- | C- | U | S1 | F- | F- | AS | AR | WE |
| `Ready` | C- | R0 | D+ | S1 | F- | F- | F- | F- | WE |
| `EmptyInitial` | C- | C- | U | S0 | F- | F- | F- | F- | F- |
| `EmptyAfter` | C- | R1 | U | S0 | F- | F- | F- | F- | F- |
| `StoppingForReplacement` | C- | C- | U | S1 | F- | F- | F- | F- | WE |
| `DrainingCreation` | C- | C- | U | S= | BR | F- | F- | F- | WE |
| `DrainingCreationAfterExit` | C- | C- | U | S= | BR | F- | F- | F- | F- |
| `DrainingInitFailure` | C- | C- | U | S1/S= | F- | F- | F- | F- | WE |
| `DrainingInit` | C- | C- | U | S= | F- | IR | F- | F- | WE |
| `DrainingActivationFailure` | C- | C- | U | S1/S= | F- | F- | F- | F- | WE |
| `DrainingActivation` | C- | C- | U | S= | F- | F- | AS | AR | WE |
| `DrainingWorker` | C- | C- | U | S= | F- | F- | F- | F- | WE |
| `Stopped` | NA | NA | NA | NA | NA | NA | NA | NA | NA |

The matrix is exhaustive over the complete input categories. Nested outcome
rules below make each matching cell exhaustive over its result sum.

### Shutdown expansion (`S0`, `S1`, and `S=`)

- `Dormant`, `EmptyInitial`, and `EmptyAfter` own no unresolved child work;
  they return the terminal behavior decision and transfer shutdown settlement
  to the lifecycle host (`S0`); no parent report is fabricated.
- `Creating` with no stop becomes `DrainingCreation`; with a stop it becomes
  `DrainingCreationAfterExit`. The already-transferred creation is not
  reconstructed or physically cancelled.
- `Initializing` with no stop becomes `DrainingInit(AwaitingBoth)` and initiates
  exact worker drain. With a stop it uses `ExitObserved` and emits no duplicate
  worker shutdown.
- `Activating` becomes `DrainingActivation`, preserving its `ActivationPhase`
  in `AwaitingBoth` when no stop is present, logically closes readiness
  publication, and emits the exact cancellation/drain actions. With a stop it
  preserves the exit in `ExitObserved` and emits no duplicate worker shutdown.
- `Ready` becomes `DrainingWorker` and emits one exact worker shutdown plus
  its worker-exit observation.
- `StoppingForReplacement` settles the still-local replacement definition as
  a nested `ReplacementOutcome::CancelledBeforeBirth`, becomes
  `DrainingWorker` for the predecessor, and does not create the successor.
- Shutdown in `DrainingInitFailure` or `DrainingActivationFailure` changes
  `DrainCompletion` from
  `ReturnToEmpty` to `StopProxy`. It does not lose the complete failure and
  emits no duplicate child drain (`S1`). If the completion is already
  `StopProxy`, shutdown is `S=`.
- All shutdown-owned drain states use `S=`. They retain their exact state and
  emit no duplicate cancellation, shutdown, or observation.

## Matching-result rules

### Birth result (`BR`)

- In `Creating` without a stop, `FoldRejected` or `HostRejected` emits the
  corresponding complete initial/replacement outcome and enters `EmptyInitial`
  for `Initial` or `EmptyAfter { previous: replaces }` for `Replacement`.
  `Committed` enters `Initializing` with the exact initialization correlation
  while the lifecycle host retains the linear settlement and plan.
- In `Creating` with a stop, rejection produces one `Contradiction` containing
  both complete authoritative inputs, then enters the pre-commit empty state.
  Commit enters `Initializing` carrying that stop; it cannot publish readiness.
- In `DrainingCreation`, rejection settles shutdown and enters `Stopped`.
  `FoldRejected` and `HostRejected` transfer their complete values through the
  corresponding `BirthDrainResult` variant. Commit enters
  `DrainingInit(AwaitingBoth)`; exact
  initialization and terminal ownership must both settle.
- In `DrainingCreationAfterExit`, rejection produces the complete
  contradiction, transfers it as
  `BirthDrainResult::Contradiction`, and enters `Stopped`.
  Commit enters
  `DrainingInit` with the retained exit in `ExitObserved`.

No creation rejection can coexist with a successful birth report. A committed
creation is never reclassified as rejected because later initialization fails.

### Init result (`IR`)

- In `Initializing` without a stop, only a result matching both worker and
  `InitAttempt` is admissible. `Initialized` reserves a fresh
  activation attempt and, in the same fold, emits the exact `BeginActivation`
  action and enters
  `Activating` with `AwaitingStartSettlement`. `EffectsRejected` enters
  `DrainingInitFailure` with both proxy-owned values returned by the result—the
  closed failure classification and still-unstarted activation plan—then
  initiates exact drain. `Stopped` settles its returned plan as never
  activated and satisfies the outstanding stop observation with that same
  authoritative result. Its `exit` field must be
  `ExitEvidence::FromInitialization`. It emits `StoppedBeforeReady` carrying
  `PreReadyStop::DuringInit` plus the corresponding `ExitSettlement::Observed`,
  then enters `EmptyAfter { previous: worker }`.
- In `Initializing` with a stop, every outcome settles initialization without
  beginning activation. Only the exact initialization attempt is admissible.
  `Initialized` emits `PreReadyStop::InitializedAfterExit` carrying
  its returned permit/plan and the already-satisfied worker-exit observation.
  `Stopped` reconciles its exact stop with the stored exit and
  requires `exit = ExitEvidence::FromObserver`, then emits `DuringInit` with
  the corresponding `Observed` settlement;
  it does not settle the observation twice.
  `EffectsRejected` combines its failure classification and returned plan with
  the same exit settlement. The lifecycle host retains the complete action
  settlement. None can emit `Ready`; all enter
  `EmptyAfter { previous: worker }`.
- `DrainingInitFailure` already owns a settled rejection. It waits
  only for the exact worker exit. With `ReturnToEmpty`, it emits one
  `InitEffectsRejected` parent outcome carrying the failure classification,
  unstarted plan, and exit settlement. With `StopProxy`, it transfers
  exactly `InitDrainResult::FailureAndExit` instead.
  Shutdown changes only that completion and never discards either value.
- In `DrainingInit`, an exact init result changes `AwaitingBoth` to
  `InitSettled` and stores one exhaustive `InitDrainWork`, including the permit/plan or
  failure-classification/plan pair. From `ExitObserved`, the same result closes
  the join by transferring `WorkAndExit` to the lifecycle host and enters
  `Stopped`. `InitResult::Stopped` closes directly as `StoppedDuringInit`: from
  `AwaitingBoth` it satisfies and retires
  the exact outstanding observation with the same authoritative stop; from
  `ExitObserved` it reconciles the already-satisfied observation and does
  not settle it twice. A separate worker exit changes `AwaitingBoth` to
  `ExitObserved`, or closes `InitSettled` as `WorkAndExit`. A foreign
  correlation emits `ProxyDiagnostic`; two
  authoritative inputs with the same correlation but conflicting stop data emit
  the atomic `Contradiction` retaining both. No live state represents both
  obligations settled.

The exact host ordering used to interpret and settle initialization actions is
an open foundational realization. These rules state the ownership constraint;
they do not claim that AA-01 implements it.

### Activation start (`AS`) and result (`AR`)

- In `Activating(AwaitingStartSettlement)` without a stop, exact start acceptance changes
  only `phase` to `InFlight`; the host then owns work and result authority.
  Exact start rejection enters `DrainingActivationFailure` with the complete
  unaccepted request and initiates exact drain.
- An activation result is authorized only in `Activating(InFlight)`. Exact
  `Ready` enters `Ready` and emits the initial/replacement readiness outcome.
  Exact rejection enters `DrainingActivationFailure`, retains the complete
  rejection, and initiates exact drain. A result received before start
  acceptance is `F-`; the activation interpreter must causally publish
  acceptance before releasing its result authority. Cancellation-qualified
  results in a non-cancelling state are `F-` and cannot be reinterpreted.
- In `Activating` with a stop, exact start acceptance changes `phase` to
  `InFlight`; exact start rejection emits its complete atomic outcome with the
  retained stop. An authorized exact result in `InFlight` emits
  `PreReadyStop::ReadyAfterExit` for `Ready`, carrying the
  attempt, authority proof, and worker-exit settlement. A
  rejection emits the complete activation-rejection outcome with the same
  exit settlement. Every case enters
  `EmptyAfter { previous: worker }`; none publishes readiness.
- `DrainingActivationFailure` already owns a settled activation failure. It
  waits only for the exact worker exit, then emits the complete failure
  outcome. Shutdown changes its drain completion to `Stopped` without
  discarding failure provenance.
- In `DrainingActivation`, matching start acceptance changes the activation
  phase inside `AwaitingBoth` or `ExitObserved` from
  `AwaitingStartSettlement` to `InFlight`. Matching start rejection changes
  `AwaitingBoth` to `ActivationSettled`, or closes `ExitObserved` and
  stops. An exact late result from `InFlight` follows the same two
  transitions. Neither ordinary `Ready` racing with cancellation nor
  `ReadyAfterCancel` is published. A result before start acceptance is
  `F-` under the causal interpreter policy; a result after
  `ActivationSettled` is a duplicate. Ordinary `Ready`/`Rejected` racing the
  logical close are normalized into the corresponding after-cancel
  settlement without publishing availability. An exact stop changes
  `AwaitingBoth` to `ExitObserved`, or closes `ActivationSettled` and stops. No live state
  represents both obligations settled.

The causal acceptance-before-result rule is an explicit Bombay interpreter
policy and an AA-06 proof obligation, not a general actor-model guarantee. If
the real boundary cannot enforce it, AA-03 must replace `ActivationPhase`
with a truthful closed out-of-order join before production code exists.

### Worker exit (`WE`)

- `Creating` with no stop accepts only `BeforeBirth` with its child reservation,
  stores that exact stop, and still waits for the creation result to decide
  whether a birth committed. `DrainingCreation` similarly becomes
  `DrainingCreationAfterExit`.
- Every `WorkerExit` must carry `ExitEvidence::FromObserver`; initialization
  exit evidence
  provenance on this ingress is wrong-kind and follows `F-`. An accepted exit
  is stored or emitted only after wrapping that delivered value in
  `ExitSettlement::Observed`.
- `Initializing` and `Activating` with no stop accept only `AfterBirth` with
  their worker incarnation and store that exact stop. A second stop is `F-`.
- `Ready` emits one complete `WorkerStopped` report and enters
  `EmptyAfter { previous: worker }`.
- `StoppingForReplacement` emits the predecessor stop report and, in the same
  fold, transfers the retained successor through the already-reserved complete
  attempt/route product into the fresh creation action, entering
  `Creating(Replacement { replaces })` with that same reservation.
- In any `DrainingInit*`, `DrainingActivation*`, or `DrainingWorker` state,
  only `AfterBirth` with the owned worker incarnation can advance. It
  either completes the failure drain, records `ExitObserved`, or closes a
  shutdown join whose work is already settled. A second stop is `F-`. Final
  outcome waits for any still-owned settlement.
- `DrainingWorker` consumes the exact stop, returns the behavior's terminal
  decision, and transfers final shutdown settlement to the lifecycle host. It
  emits no invented parent-report variant.

## Overlap, stale input, and outcome laws

- A second install or an install after `Dormant` is a typed rejection that
  returns its complete definition and plan.
- A replacement is accepted only in `Ready` or `EmptyAfter`. Every overlap
  returns the complete submitted successor.
- Every non-ready service command produces exactly one complete `Unavailable`
  report. No retry or implicit replay is promised.
- A lifecycle fact can advance only the state owning its exact worker and
  attempt and expecting that fact kind. Wrong-kind facts are not reinterpreted.
- Duplicate stop, creation, initialization, activation-start, activation
  result, and readiness inputs preserve proxy state and emit exactly one closed
  `ProxyDiagnostic` retaining the complete input.
- Parent-report delivery rejection never rewinds the transition. The lifecycle
  host settles the complete rejected report.
- A pre-readiness failure after committed installation is not reported as
  final until the exact incarnation and all transferred initialization or
  activation work are settled or transferred to terminal residual ownership.

## First-slice non-features and proof obligations

AA-01 intentionally does not provide:

- Rust proxy types, a fold, an executable local algebra, or test helpers;
- the real initialization fold/settlement implementation and its exact host
  ordering;
- a selected Rust representation for `ActivationPermit`, result authority,
  out-of-order activation joins, or asynchronous work;
- accepted and rejected interpreter traces;
- delivery settlement/rejection realization for service or parent reports;
- deadline expiry, forced retirement, root residual settlement, or leak-free
  late-work ownership;
- wrapper composition or initialization-order proof;
- catalogue parity, builder syntax, compiler/DevX acceptance, or migration;
- edits to the locked kernel, legacy actors, macros, testkit, or interpreters.

Consequently this document is ready only to guide AA-02’s focused failing
regressions and to be compared against production authority. It is not
implementation evidence. “Ready for comparison and policy enrichment” later
requires real `Behavior`/`Actions` integration, accepted and rejected
interpreter traces, applicable catalogue parity, wrapper-composition proof,
complete shutdown/residual ownership, and compiler and DevX acceptance.

If AA-02 through AA-06 cannot realize any table cell through the locked
boundary, the experiment records that focused counterexample. It must not
invent a parallel actor contract or silently weaken the table.
