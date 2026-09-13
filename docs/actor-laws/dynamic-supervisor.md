# Dynamic-supervisor falsification model

## Status and authority

This document is the AA-20 independent semantic model for one dynamic
supervisor. It is a non-production falsification oracle. It does not select
Rust types, define another behavior algebra, implement a supervisor, or mark a
production coverage row implemented. `docs/atomic-actor-solution.md` remains
the production design authority.

Fresh allocation is the actor-model law used here: every accepted entry owns a
fresh stable proxy, and every worker behind that proxy is a fresh actor.
Creator-local creation correlations, exact capability settlement, operation tokens,
generation-safe key reuse, activation authorization, and actor-graph drain are
Bombay derivations or policy choices. Bounded keyed membership, explicit
management operations, one durable lifecycle owner, unexpected-exit handling,
and phase-sensitive cancellation are dynamic-supervisor template laws.

Runtime route selection is owned normatively by
`docs/atomic-runtime-settlement.md`. An accepted start issues and stores one
`CreationId` for its proxy; it never chooses or stores a runtime route before
emitting `CreateChild`. The generic creation settlement returns that exact
correlation and every rejected value. There is no separate route request,
waiting state, public route, or dynamic-supervisor-specific creation result.

The model is deliberately not a fixed supervisor with an optional role list.
It contains no worker factory, fixed roster, restart eligibility, restart
strategy, restart budget, or backoff policy.

The model must falsify designs that make any of these claims false:

1. The configured positive entry limit bounds every retained entry, including
   entries still creating, cancelling, draining, or retiring.
2. A semantic key is management identity only. It is never converted into a
   proxy route, nonce, incarnation, or proof of freshness.
3. Every accepted start reserves one fresh entry generation, proxy creation ID,
   cancel authority, and initial-operation correlation before commitment.
4. Admission and realization are distinct. `StartAccepted` and
   `ReplaceAccepted` never claim readiness.
5. The stable proxy is the only externally routable service capability. Worker
   incarnation evidence is opaque and non-routable.
6. At most one management mutation owns an entry at a time. Every submitted
   worker definition is either locally owned, transferred once, returned once,
   or held by exact settlement ownership.
7. Request reply routes are temporary. One mandatory durable lifecycle route
   owns later realization, unavailability, cancellation completion, and
   retirement facts.
8. Cancellation is exact and phase-sensitive. It cannot claim that emitted
   creation, delivery, initialization, or activation work was physically
   cancelled.
9. Removing an entry releases capacity only after every exact outstanding fact
   and action settlement transfers or resolves. Reusing its key receives a
   fresh generation and fresh proxy creation ID.
10. Stale, duplicate, foreign, wrong-generation, wrong-operation, wrong-kind,
    and contradictory inputs cannot mutate a current entry.
11. Query is a total read-only semantic projection and never exposes private
    definitions, routes, tokens, or proxy-private phases.
12. Global shutdown closes mutation and drains every retained or still-creating
    entry exactly once while preserving request, lifecycle, diagnostic, and
    residual ownership.
13. Every accepted input produces one next state and one complete named action
    product. No effect is ambient.

“Atomic” means one local `Behavior` fold commits one supervisor state and its
complete `Actions`. It does not claim atomic delivery, creation, cancellation,
or work across actors.

## Model vocabulary

The following names describe the oracle. They are not proposed public Rust
names.

```text
Key = application-authored semantic management key
EntryGeneration = fresh non-reused supervisor correlation for one accepted key use
EntryIdentity = { key, generation: EntryGeneration }

ProxyBirthAttempt = fresh non-reused CreationId issued by the supervisor
RoutedCreation = the existing interpreter-private route selected for that CreationId
ProxyIncarnation = externally routable exact capability for one committed stable proxy

WorkerIncarnationEvidence = opaque non-routable provenance for one installed worker
WorkerSubmission = complete owned worker definition and activation work for one attempt

OperationKind = Start | Replace
OperationCorrelation = fresh non-reused checked operation identity and admission order
CancelAuthority = opaque caller-held capability for one OperationCorrelation
OperationEvidence = opaque non-cancelling projection used in durable events
ProxyOperationWitness = private affine authority retained before one emitted
    proxy input settles
ProxyOperationId = opaque affine correlation returned only after that exact
    proxy input is accepted

PreparedProxyInput =
    Initial { worker: WorkerSubmission }
  | Replacement { worker: WorkerSubmission }

DefinitionOwnership = Present { worker: WorkerSubmission } | Transferred

StopCorrelation = fresh non-reused correlation for one explicit entry stop
ShutdownCorrelation = structural exact { EntryIdentity, ProxyIncarnation }
DeadlineCorrelation = { timer, generation }
```

`Key`, `EntryGeneration`, operation correlations, proxy-operation IDs, attempts,
timer generations, and child routes are correlation—not actor identity. Only a
committed creation yields `ProxyIncarnation`. Sequence arithmetic, address
reuse, key equality, timing, or adjacency cannot manufacture provenance.

The proxy alone retains its routable worker recipient. The dynamic supervisor
retains only `WorkerIncarnationEvidence` for exact replacement and stop
correlation. Status, capabilities, request replies, and lifecycle events never
grant a route around the stable proxy.

Every checked sequence has an explicit exhaustion outcome and never wraps.
Collision against retained state is a controlled contradiction, not an
invented result of the opaque StableProxy operation pair. Failed correlation
issue returns every still-owned input and commits no entry, operation,
activation occupancy, or action.

## Construction domain

One complete construction contains:

```text
DynamicDefinition = {
    entry_limit: PositiveMaximum,
    activation: ActivationContract,
    activation_limit: PositiveMaximum,
    unexpected_exit: KeepEmpty | Retire,
    actor_drain: ActorDrainPolicy,
    lifecycle: DurableLifecycleRoute,
    diagnostics: DeliverTo { route } | Terminate,
}
```

The durable lifecycle route is mandatory and selected once. It preserves its
concrete logical, established, or mixed capability form and static hosting
obligations. No request route can replace it. Diagnostics are independently
mandatory through either a concrete route or terminal settlement.

`ActivationContract` is the selected static interpretation law. Each
`WorkerSubmission` is the complete owned per-attempt definition and activation
work after that law is applied. The fold never reconstructs, looks up, or
clones an affine activation plan after admission.

A zero entry limit, zero activation limit, invalid deadline, or missing policy
is a construction failure. There is no fixed factory, worker roster, recovery
policy, callback, no-op route, default unexpected-exit behavior, or public
structural path.

A closed heterogeneous worker sum is valid only when every variant exposes one
common concrete public service protocol. Different public protocols require
different dynamic supervisors; the model never erases them into one envelope.

AA-20 selects no builder, typestate spelling, public enum names, or macro.

## Supervisor state

```text
SupervisorState =
    Operating {
        definition,
        entries: BoundedEntryTable,
        allocators,
    }
  | Draining {
        definition,
        entries: BoundedDrainTable,
        deadline: DeadlinePhase,
        allocators,
    }
  | Stopped
```

`BoundedEntryTable` contains at most `entry_limit` entries. Every key occurs at
most once. Every entry stores its exact generation and proxy creation ID or
exact proxy; the table does not derive either from the key.

The table need not retain declaration order. Activation fairness is explicit:
each waiting start or replacement already stores its ordered
`OperationCorrelation`, and newly available capacity issues the lowest
outstanding operation first. There is no separately
mutable queue whose contents can disagree with entry phases.

Activation occupancy is derived from exact dispatched or awaiting proxy-input
states. A prepared but un-emitted input owns its complete worker submission but
does not occupy activation capacity. Reserving capacity, forming the infallible
proxy-operation pair, and emitting the prepared input are one transition.

## Transaction-local start preparation

Start preparation is not a stored entry phase:

```text
PreparedStart = {
    identity: EntryIdentity,
    creation: CreationId,
    operation: OperationCorrelation,
    cancel: CancelAuthority,
    input: PreparedProxyInput,
}

PreparedReplace = {
    identity: EntryIdentity,
    proxy: ProxyIncarnation,
    base: ReplaceBase,
    operation: OperationCorrelation,
    cancel: CancelAuthority,
    input: PreparedProxyInput,
}
```

For one `Start`, the supervisor checks shutdown, duplicate key, and table capacity;
then fallibly reserves every field above before committing anything. Failure
returns the complete worker and an exact reason. Success commits
`Entry::Starting(CreatingProxy)` and emits the acceptance reply, fresh empty
proxy creation, exact creation observation, and exact proxy-exit observation in
one action.

There is therefore no observable stored `Reserved` phase between acceptance
and creation emission. A future executable design that introduces such a phase
must identify the real intervening settlement or prerequisite. A label that is
only a transient local preparation value must not appear in public query.

## Operating entry sum

Every retained entry is exactly one of:

```text
Entry =
    Starting(StartEntry)
  | Ready(ReadyEntry)
  | Empty(EmptyEntry)
  | Replacing(ReplaceEntry)
  | Stopping(StopEntry)
  | Cancelling(CancelEntry)
  | Retiring(RetireEntry)
```

No entry has `pending`, `ready`, `cancelled`, `stopping`, or `retired` flags.

### Start entry

```text
StartEntry =
    CreatingProxy {
        identity,
        creation: CreationId,
        operation,
        input: PreparedProxyInput,
    }
  | WaitingForAuthorization {
        identity,
        proxy: ProxyIncarnation,
        operation,
        input: PreparedProxyInput,
    }
  | AwaitingInputReceipt {
        identity,
        proxy: ProxyIncarnation,
        operation,
        witness: ProxyOperationWitness,
    }
  | AwaitingOutcome {
        identity,
        proxy: ProxyIncarnation,
        operation,
        proxy_input: ProxyOperationId,
    }
```

`CreatingProxy` and `WaitingForAuthorization` own the worker inside the
prepared input. `AwaitingInputReceipt` retains only correlation; the input has moved
to source-side delivery settlement. `AwaitingOutcome` exists only after exact
input acceptance.

### Available entries

```text
ReadyEntry = {
    identity,
    proxy: ProxyIncarnation,
    worker: WorkerIncarnationEvidence,
    last_operation: TerminalOperation,
}

EmptyEntry = {
    identity,
    proxy: ProxyIncarnation,
    previous: WorkerIncarnationEvidence,
    cause,
    last_operation: TerminalOperation,
}

TerminalOperation =
    Committed { operation: OperationCorrelation, kind: OperationKind }
  | Cancelled { operation: OperationCorrelation, kind: OperationKind }
  | None
```

At most one terminal operation is retained per entry. It makes an immediate
matching repeated cancel truthful without an unbounded operation-history
table. Accepting the next start/replace mutation retires the prior terminal
record; older authorities then produce `CancellationReceipt::Stale`. This is a deliberate
bounded-retention policy, not an actor-model rule.

### Replacement entry

```text
ReplaceBase =
    WasReady { worker: WorkerIncarnationEvidence }
  | WasEmpty { previous: WorkerIncarnationEvidence, cause }

ReplaceEntry =
    WaitingForAuthorization {
        identity,
        proxy,
        base: ReplaceBase,
        operation,
        input: PreparedProxyInput,
    }
  | AwaitingInputReceipt {
        identity,
        proxy,
        base: ReplaceBase,
        operation,
        witness,
    }
  | AwaitingOutcome {
        identity,
        proxy,
        base: ReplaceBase,
        operation,
        proxy_input,
    }
```

`StableProxy` is the sole owner of predecessor drain. It does not publish a
replacement outcome until its internal predecessor shutdown/exit reunion has
closed, and its owner never reconstructs that outcome from lower-level worker
events. DynamicSupervisor therefore owns no predecessor-stop phase. The
replacement outcome's nested `replaces` evidence must equal the retained base;
that validation is the supervisor's only predecessor responsibility after
input transfer.

### Explicit stop entry

```text
StopOrigin =
    Available { prior: ReadyEntry | EmptyEntry }
  | Starting { phase: StartOutstanding }

StopEntry =
    WaitingForProxyBirth {
        identity,
        creation: CreationId,
        stop: StopCorrelation,
        start_outstanding,
    }
  | ShutdownDispatched {
        identity,
        proxy,
        stop,
        origin: StopOrigin,
        shutdown: ShutdownCorrelation,
        settlement,
    }
  | AwaitingProxyExit {
        identity,
        proxy,
        stop,
        origin: StopOrigin,
        shutdown: ShutdownCorrelation,
    }
  | ExitBeforeShutdownSettlement {
        identity,
        proxy,
        stop,
        origin: StopOrigin,
        exit,
        shutdown: ShutdownCorrelation,
    }
```

Stop during a pending start is accepted and waits for exact proxy creation. It
is not operation-token cancellation: it closes the complete entry by key,
reports stop admission to its caller, and transfers the interrupted start
obligation to the durable lifecycle owner. Stop during replacement or
cancellation is rejected as unavailable and does not steal that operation.

### Cancellation entry

```text
CancelKind = Start | Replace { base: ReplaceBase }

CancelEntry =
    AwaitingProxyBirth {
        identity,
        creation: CreationId,
        operation,
        kind: Start,
        definition_disposition: ReturnedToCancelReply,
    }
  | DrainingProxy {
        identity,
        proxy,
        operation,
        kind: CancelKind,
        outstanding: CancelOutstanding,
        shutdown: ProxyStopPhase,
    }

CancelOutstanding =
    NoTransferredInput
  | InputSettlement { witness: ProxyOperationWitness, settlement }
  | ProxyOutcome { proxy_input: ProxyOperationId }
  | OutcomeBeforeExit { outcome }

ProxyStopPhase =
    NotEmitted
  | ShutdownDispatched { shutdown, settlement }
  | AwaitingExit { shutdown }
  | Rejected { shutdown, request, reason }
  | ExitObserved { exit }

StartOutstanding =
    DefinitionLocal { operation, input }
  | InputEmitted { operation, witness: ProxyOperationWitness, settlement_or_outcome }

RetireOutstanding =
    Idle
  | Start { outstanding: StartOutstanding }
  | Replace { operation, base, witness: ProxyOperationWitness, settlement_or_outcome }
  | Cancellation { operation, outstanding: CancelOutstanding }
  | Stop { stop, settlement_or_exit }
```

Cancellation before proxy-input emission returns the complete submitted worker
through `CancellationReceipt::Returned`. If proxy creation is already in flight, the
entry still waits for its exact result; a late committed proxy is drained.

Cancellation after input emission returns `CancellationReceipt::Pending`. It never
returns the worker, even if later delivery rejection transfers that input to
the lifecycle host. A late ready result is retained only for drain and never
emits `Started` or `Replaced`.

### Retirement entry

```text
RetireCause =
    UnexpectedWorkerExit
  | StableProxyExit
  | StartFailed
  | ReplacementFailedAfterTopologyLoss
  | Cancellation
  | Stop
  | DiagnosticTermination

RetireEntry = {
    identity,
    proxy: NoProxy | Pending(CreationId) | Exact(ProxyIncarnation),
    cause: RetireCause,
    outstanding: RetireOutstanding,
    shutdown: ProxyStopPhase,
}
```

An entry is removed only when `outstanding` is empty and its proxy is rejected
before birth or authoritatively exited. Removal is a state transition, not a
permanent tombstone.

## Public command sum

```text
Management =
    Start { key, worker, reply_to }
  | Replace { key, worker, reply_to }
  | Stop { key, reply_to }
  | Query { key, reply_to }
  | Cancel { authority: CancelAuthority, reply_to }
```

Here `worker` means the complete `WorkerSubmission`, not merely the inner
behavior after its activation work has been separated. A future Rust surface
may infer and hide that product, but its ownership cannot disappear.

AA-20 omits a separate `Retire` command. For a ready or empty entry, explicit
`Stop` already drains the stable proxy and releases the key after exact exit.
Retirement after unexpected exit is policy-driven and automatic. Adding
`Retire` without another state-transition law would create two public ways to
perform the same operation.

Every request route is used once for its immediate reply and then moves to
action settlement. It is never stored as the durable lifecycle owner. A
`CancelAuthority` is returned in every cancel reply (or its rejected-delivery
settlement), so the supervisor never retains or clones that affine authority.

## Request receipts

```text
WorkerChangeReceipt = { key, cancel: CancelAuthority }
WorkerChangeRejection<Reason> = { key, worker, reason: Reason }

Start result =
    Result<WorkerChangeReceipt, WorkerChangeRejection<StartRejection>>

Replace result =
    Result<WorkerChangeReceipt, WorkerChangeRejection<ReplaceRejection>>

Stop result = Result<Key, StopRejection<Key>>

CancellationReceipt =
    Returned { authority, worker }
  | Pending { authority, phase }
  | Committed { authority, resulting_phase }
  | Cancelled { authority }
  | Stale { authority }
  | Draining { authority }

QueryReply = Unknown { key } | Known { key, phase }
```

Start rejection reasons include `AlreadyExists`, `AtCapacity`, `ShuttingDown`,
entry-generation exhaustion, management-operation exhaustion, and
proxy-creation exhaustion. Replace rejection includes unknown, unavailable,
shutting down, and management-operation exhaustion. Every rejection owns
the complete submitted worker. Stop rejection includes unknown, unavailable,
already stopping, shutting down, and stop-correlation exhaustion or collision;
no stop state or shutdown action is committed when that reservation fails.

## Public query projection

```text
PublicPhase =
    CreatingProxy
  | WaitingForActivationAuthorization
  | AwaitingProxyOutcome
  | Ready { proxy: ProxyIncarnation }
  | Empty
  | Stopping
  | Replacing
  | Cancelling
  | Draining
  | Retiring
```

`Query` is a byte-for-byte semantic no-op on entry state, allocator state,
capacity, and activation occupancy. It exposes no worker definition, child
route, entry generation, proxy-operation correlation, cancel authority, proxy-private
installation phase, or worker recipient. Delayed lifecycle events retain the
opaque entry generation because they cross key reuse; an immediate query does
not expose bookkeeping its caller cannot use. During global shutdown it
projects the drain phase. Absence is `Unknown`, not an `Option` combined with
flags.

## Durable lifecycle and diagnostic values

```text
DynamicLifecycle =
    Started { identity, operation: OperationEvidence, proxy: ProxyIncarnation }
  | StartFailed {
        identity, operation: OperationEvidence,
        definition: DefinitionOwnership, reason,
    }
  | Replaced { identity, operation: OperationEvidence, proxy: ProxyIncarnation }
  | ReplacementFailed {
        identity, operation: OperationEvidence,
        definition: DefinitionOwnership, reason,
    }
  | StopFinished {
        identity,
        proxy,
        result: Result<ProxyExit, StopFailure>,
    }
  | UnexpectedWorkerStopped {
        identity, observed_at, disposition: KeptEmpty | Retiring,
    }
  | OperationCancelled { identity, operation: OperationEvidence, kind, result }
  | WorkerChangeInterrupted {
        identity,
        operation: OperationEvidence,
        interruption:
            ExplicitStop { stop: OperationEvidence }
          | SupervisorShutdown { change: Start | Replacement },
        definition: DefinitionOwnership,
    }
  | CommandUnavailable { identity, sender, proxy_phase, command }
  | EntryRetired { identity, cause }

DynamicDiagnostic =
    CorrelationReservationRejected { key, operation_kind, reason }
  | ProxyCreationRejected { identity, rejection }
  | ProxyInputRejected { identity, operation, input, reason }
  | ProxyOutcomeFailed { identity, operation, outcome }
  | ProxyShutdownRejected { identity, proxy, request, reason }
  | StableProxyDied { identity, proxy, exit }
  | RejectedLifecycleFact { rejected }
  | ForcedRetirement { identity, residual, cause }
```

Lifecycle events never expose a routable worker recipient. Exact predecessor
evidence from a worker-stop report moves into the entry; the durable event gets
the disjoint observation time and semantic disposition. Service capability
publication is limited to the stable proxy. A failure or interruption carries
a still-local worker definition as `Present`; after proxy-input emission it
says `Transferred` and the exact input settlement remains the sole owner.
Every event delivery has exact settlement ownership; delivery rejection cannot
roll back admission or realization.

When one transition emits both lifecycle and diagnostic values, the exact
source payload moves to only one of them. The other receives an explicit
semantic classification or a disjoint field projection. The model never
clones an affine rejection, exit, worker definition, command, or incarnation
fact merely to satisfy two consumers.

Diagnostics follow `DeliverTo(route) | Terminate`. Diagnostic delivery
rejection is terminal and non-recursive. Lifecycle and diagnostics are
independent lanes: neither substitutes for the other. `Terminate` transfers
the diagnostic and every still-live entry obligation to the surviving
lifecycle host and enters the same global drain equation; it does not drop the
diagnostic or continue an ordinary entry-local transition.

## Stable-proxy input sum

```text
ProxyCreationResult = the generic CreationsSettled result for the exact
    CreationId and StableProxy creation

ProxyInputResult = ActionItemResult<
    ProxyOperation,
    ProxyInputReceipt { creation, proxy, operation: ProxyOperationId },
    ProxyInputRejection,
    Never,
>

ProxyReport =
    Initial { outcome }
  | Replacement { outcome }
  | WorkerStopped { stop, observed_at }
  | Unavailable { sender, phase, command }

ProxyExit = { identity, proxy, exit }

ProxyShutdownSettlement =
    Accepted { identity, proxy, shutdown }
  | Rejected { identity, proxy, shutdown, request, reason }
```

The generic result owns whether interpretation was attempted. Acceptance owns
only the receipt promised by the request. Rejection and corruption return the
complete `ProxyOperation`; an unattempted result retains it unchanged. There is
no supervisor-specific result sum or conversion seam.

Every proxy report is accompanied by its exact child-source capability. The
entry generation, exact source, expected operation kind, and replacement
outcome's nested `replaces` evidence form complete correlation. The supervisor
does not widen AA-01's proxy protocol with its cancel authority.

The source carried by a proxy input result or proxy exit selects its entry once
before the phase matrix. A source that selects no entry is returned as rejected
input. After selection, the phase checks only its distinct operation or worker
evidence; it does not compare the selecting source with itself again or invent a
second phase-local wrong-source outcome.

## Required semantic action product

The minimum model product is:

```text
proxy_creations
proxy_creation_observations
proxy_stop_observations
proxy_initial_inputs
proxy_replacement_inputs
proxy_shutdowns
management_replies
lifecycle_events
diagnostics = Deliver { route, diagnostic } | Terminal { diagnostic }
deadline_schedules
rejected_inputs
delivery_settlements
```

All operations are named semantic lanes. No positional traversal, nested
`.inner`, runtime registry, dynamic envelope, callback, or direct effect occurs
inside the fold. The actual production product may use existing lower-order
lanes only if it proves this complete observable algebra without
reinterpretation.

## Ownership table

| Value | Actor-side owner before emission | Settlement/host owner after emission | Terminal law |
|---|---|---|---|
| Start worker | prepared start, then creating/waiting entry | proxy-input settlement after emission | returned only before transfer; otherwise lifecycle host settles it |
| Replace worker | prepared replace in waiting entry | replacement-input settlement after emission | same phase boundary as start |
| Entry generation | exact retained entry | never emitted as authority | retired only when entry removal closes all facts |
| Cancel authority | accepting caller | moved into one cancel request | every reply or rejected-delivery settlement returns it; the supervisor never retains it |
| Operation correlation | active entry | lifecycle/diagnostic settlement after terminal realization | at most one bounded terminal record remains |
| Stable proxy | entry after committed birth | actions borrow delivery authority; lifecycle host retains ownership | exact exit or forced transfer retires it |
| Worker evidence | entry or replacement join | never a routable action capability | replaced by fresh evidence or retired with exact stop |
| Activation slot | derived from dispatched/awaiting state | host owns unresolved proxy input/outcome | matching outcome, input rejection, or forced transfer releases it |
| Request reply | incoming command | request-delivery settlement | never becomes durable lifecycle ownership |
| Lifecycle event | transition-local value | lifecycle-delivery settlement | rejection transfers outward without state rewind |
| Diagnostic | transition-local value | diagnostic route or terminal settlement | rejection is terminal and non-recursive |

## Management-command matrix

| Entry phase | Start same key | Replace | Stop | Cancel matching active op | Query |
|---|---|---|---|---|---|
| absent | admit if capacity and preparation succeed | reject/return worker | reject unknown | unknown/stale | `Unknown` |
| `Starting::CreatingProxy` | reject/return worker | reject/return worker | accept and wait birth | return worker, wait birth | `CreatingProxy` |
| `Starting::WaitingForAuthorization` | reject/return worker | reject/return worker | accept and drain proxy | return worker, drain proxy | waiting authorization |
| `Starting::AwaitingInputReceipt` | reject/return worker | reject/return worker | accept and drain outstanding | pending cancellation | awaiting outcome |
| `Starting::AwaitingOutcome` | reject/return worker | reject/return worker | accept and drain outstanding | pending cancellation | awaiting outcome |
| `Ready` | reject/return worker | admit replacement | accept stop | matching last op committed | ready capability |
| `Empty` | reject/return worker | admit replacement | accept stop | matching last op terminal | empty |
| `Replacing::WaitingForAuthorization` | reject/return worker | reject/return worker | reject unavailable | return replacement, restore base | replacing |
| emitted/awaiting replacement | reject/return worker | reject/return worker | reject unavailable | pending cancellation | replacing |
| `Stopping` | reject/return worker | reject/return worker | reject already stopping | active start/replace token is shutdown-owned | stopping |
| `Cancelling` | reject/return worker | reject/return worker | reject unavailable | already cancelled | cancelling |
| `Retiring` | reject/return worker | reject/return worker | reject unavailable | shutdown-owned or stale | retiring |

During global drain, every mutation rejects with `ShuttingDown`; `Cancel`
returns `CancellationReceipt::Draining`; `Query` remains total.

## Lifecycle-fact matrix

Notation:

- `A` accepts the exact fact and performs the named phase transition;
- `J` accepts one side of an order-independent join;
- `D` settles only the matching drain obligation;
- `F` preserves state and emits one complete rejected-fact diagnostic.

Exact entry generation and proxy source are checked before this table. A fact
that fails either check is always `F`.

| Entry phase | Proxy birth | Proxy exit | Input settlement | Initial outcome | Replacement outcome | Worker stop | Unavailable | Shutdown settlement |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `Starting::CreatingProxy` | A | A | F | F | F | F | F | F |
| `Starting::WaitingForAuthorization` | F | A | F | F | F | F | A | F |
| `Starting::AwaitingInputReceipt` | F | A | A | F | F | F | A | F |
| `Starting::AwaitingOutcome` | F | A | F | A | F | F | A | F |
| `Ready` | F | A | F | F | F | A | A | F |
| `Empty` | F | A | F | F | F | F | A | F |
| `Replacing::WaitingForAuthorization` | F | A | F | F | F | A | A | F |
| `Replacing::AwaitingInputReceipt` | F | A | A | F | F | F | A | F |
| `Replacing::AwaitingOutcome` | F | A | F | F | A | F | A | F |
| `Stopping` | A or F by origin | J | D | D | D | D | A | J |
| `Cancelling` | D | D | D | D | D | D | A | D |
| `Retiring` | D | D | D | D | D | D | A | D |
| global `DrainEntry` | D | D | D | D | D | D | D | D |

`WorkerStopped` while replacement still waits for authorization changes a
`WasReady` base into the exact empty predecessor; it does not open an
unexpected-exit policy decision or a second operation because the accepted
replacement already owns the service mutation. For `WasEmpty`, another stop is
`F`. After replacement input transfer, StableProxy consumes predecessor stop
privately and emits only the completed replacement outcome. A later
`WorkerStopped` report can describe only the successor selected by that completed
outcome and is processed after the replacement result.

`Stopping` accepts proxy birth only when its origin still owns the matching
pending proxy creation. Input and outcome values in stopping/cancelling/
retiring states never publish success; they settle exact outstanding ownership
through the drain equation.

## Start transitions

Successful preparation commits the entry before emitting `StartAccepted` and
the proxy creation bundle. Acceptance delivery rejection cannot unreserve the
entry or recover the worker; the durable lifecycle owner remains authoritative
for later realization.

The entry retains only `OperationCorrelation`. `CancelAuthority` moves to the
request reply action and then to its delivery settlement; the supervisor never
stores or clones the caller's cancellation capability. A later `Cancel`
returns that authority to the fold, which privately projects the matching
correlation.

Exact proxy rejection removes the entry after preserving `StartFailed` with
the still-local definition as `Present` and all creation settlement. Exact
proxy commit stores `ProxyIncarnation` and either:

- waits with the complete prepared input when activation capacity is full; or
- consumes the lowest waiting `OperationCorrelation`, forms the infallible
  `ProxyOperationWitness`/`ProxyOperationId` pair, emits the initial input, and
  enters `AwaitingInputReceipt` in the same transition.

Input settlement acceptance enters `AwaitingOutcome`. Rejection releases
activation occupancy, marks the durable `StartFailed` definition
`Transferred`, preserves the complete input in diagnostic settlement, drains
the empty stable proxy, and never retries automatically.

An exact initial `Ready` outcome releases occupancy, enters `Ready`, stores the
opaque worker evidence privately, and emits exactly one durable `Started` with
the stable proxy. Any non-ready outcome emits `StartFailed` and retires/drains
the proxy. A report before accepted input settlement is rejected; the
interpreter must discharge source settlement before later child report
admission.

After every occupancy release, waiting starts/replacements issue in increasing
`OperationCorrelation` up to the positive limit.

## Replace transitions

Replace is admissible only from `Ready` or `Empty`. Before acceptance, the supervisor
constructs one complete transaction-local `PreparedReplace`, reserving its
operation/cancel pair. A failure
returns the complete replacement and leaves the base entry unchanged. Success
moves the cancel authority into the acceptance action and every other field
into `Replacing::WaitingForAuthorization`; no partial replacement phase is
committed.

While waiting for authorization, cancellation returns the replacement and
restores the exact base with a bounded `Cancelled` terminal record.
Authorization forms the infallible proxy-operation pair and moves the
replacement input to settlement.

Input rejection restores the base, emits one durable `ReplacementFailed` with
definition `Transferred`, and preserves the complete input through diagnostic
settlement. Input acceptance enters `AwaitingOutcome`.

StableProxy joins the ready predecessor's exact stop with successor work before
publishing one replacement outcome; an empty proxy has no predecessor drain.
DynamicSupervisor consumes that one completed outcome. A ready result enters
`Ready` with fresh worker evidence and emits one `Replaced` after validating the
nested `replaces` evidence against the retained base. A pre-birth failure enters
`Empty` with the validated `replaces` evidence; a post-birth failure enters
`Empty` with the successor attempt returned by StableProxy. Both emit
`ReplacementFailed`. This is the same `EmptyAfter` projection owned by AA-01;
the supervisor does not infer which worker the proxy now names. It never reports
success or returns an already-transferred worker definition.

Cancellation after emission suppresses both outcomes, drains the exact proxy,
and eventually emits `OperationCancelled` to the durable lifecycle owner. It
cannot revive the predecessor or pretend the proxy stopped creating work.

## Stop, unexpected exit, and unavailability

Stop accepts a ready, empty, or still-starting entry. Its immediate reply
describes admission only. The durable lifecycle owner receives completion or
runtime rejection.

Before `StopAccepted`, the fold fallibly reserves `StopCorrelation`. For a
committed proxy, the exact shutdown correlation is then formed totally from
the retained entry identity and proxy capability, and one shutdown is emitted.
Settlement rejection preserves the request and reason. If the proxy is
otherwise live, a stop-originating ready/empty entry is restored only when no
authoritative exit has arrived; a failed `StopFinished` is durable realization,
not an admission rejection. If exit and settlement arrive in the other order, the
closed join preserves both and never resurrects the entry.

For a still-creating proxy, stop retains the creation result obligation. Exact
creation rejection removes the entry. Exact commit emits one shutdown. Any
locally owned initial definition transfers to the durable
`WorkerChangeInterrupted { interruption: ExplicitStop }` outcome. An
already-emitted input resolves first: rejection moves the complete returned
input to diagnostic custody, while acceptance requires the exact late proxy
result in the same lifecycle value. No `Transferred` label substitutes for an
owned worker value.

An exact spontaneous worker stop from `Ready` first makes the entry unavailable
and emits `UnexpectedWorkerStopped`. `KeepEmpty` enters `Empty` and continues to
consume capacity. `Retire` enters `Retiring`, shuts down the stable proxy, and
releases capacity only after exact drain. Neither policy restarts a worker.

Stable-proxy death in any phase retires that exact entry generation. It settles
the active operation through typed host transfer, emits the corresponding
lifecycle failure and diagnostic, and cannot affect another entry or later use
of the same key.

Every exact `Unavailable` report is delivered once to the mandatory durable
lifecycle route with key, generation, sender, phase, and command intact. It is
valid during starting, ready, empty, replacing, stopping, cancelling, and
draining. Foreign or stale source capability is a diagnostic, not a fold error
that loses the command.

## Cancellation and bounded token retention

Cancel first selects the semantic key and matches the checked, supervisor-global
operation carried by its opaque authority. Operations never wrap or reuse, so
entry generation is not a second cancellation coordinate; it remains the
durable identity of the retained entry and its lifecycle messages.

- A matching locally owned definition returns once.
- A matching transferred definition returns `CancellationReceipt::Pending` and moves
  the entry into exact drain.
- A matching active cancellation returns `CancellationReceipt::Cancelled`.
- A matching most-recent committed record returns `CancellationReceipt::Committed`.
- A matching most-recent cancelled record returns `CancellationReceipt::Cancelled`.
- An older, removed-generation, foreign, or never-issued authority returns
  `CancellationReceipt::Stale` without mutation.
- Global drain returns `CancellationReceipt::Draining`.

The model retains no unbounded operation history. When a new operation is
accepted, the prior terminal record retires. This bounded policy must be
documented in any public cancellation contract; indefinite `Committed`
answers would require a growing tombstone set or an impermissible provenance
inference from sequence arithmetic.

## Entry retirement and key reuse

Capacity counts every retained entry, including cancelling and retiring ones.
An entry is removed only after:

1. pending proxy creation resolves;
2. every emitted proxy input settles or transfers;
3. every proxy-private activation outcome resolves or transfers;
4. exact proxy shutdown/exit resolves or transfers;
5. lifecycle, diagnostic, and rejected-delivery ownership has a host; and
6. forced-retirement residual ownership, if any, moves to the surviving root.

Removal retains no key tombstone. A later `Start` for the same key reserves a
fresh supervisor-global `EntryGeneration`, proxy creation ID, and operation.
Because child routes and generations are never reused, a late fact from an old
entry cannot match the new entry. It is returned complete as a stale diagnostic.

Allocator exhaustion rejects future starts with their workers intact. It does
not justify reuse of an old generation or child route.

## Global shutdown normalization

The first global shutdown atomically:

1. closes start, replace, and stop admission;
2. makes cancel report `CancellationReceipt::Draining` while query remains available;
3. transfers every locally owned start/replacement definition to one durable
   `WorkerChangeInterrupted { interruption: SupervisorShutdown { change } }`
   outcome and retains already-emitted definitions through their exact input
   settlement or proxy outcome;
4. preserves every emitted creation, proxy-input settlement, and atomic proxy
   outcome obligation;
5. transforms every retained entry into one exhaustive drain entry;
6. emits at most one shutdown for every committed live proxy;
7. retains every pending proxy creation until exact commit/rejection; and
8. reserves and emits one exact deadline request when selected.

```text
DrainEntry =
    ResolvingProxyCreation { identity, creation: CreationId, local_values }
  | DrainingProxy {
        identity,
        proxy,
        outstanding: EntryOutstanding,
        shutdown: ProxyStopPhase,
    }
  | Drained { identity, result: Graceful | Absent | Forced { residual } }

EntryOutstanding =
    Idle
  | StartWaiting { operation, input }
  | StartEmitted { operation, witness: ProxyOperationWitness, settlement_or_outcome }
  | ReplaceWaiting { operation, base, input }
  | ReplaceEmitted { operation, base, witness: ProxyOperationWitness, settlement_or_outcome }
  | ExplicitStop { stop, settlement_or_exit }
  | Cancellation { operation, outstanding }
  | Retirement { cause, outstanding }
```

Repeated shutdown is idempotent. Late creation, proxy report, input settlement,
shutdown settlement, and proxy exit can settle only their exact drain entry.
They cannot restore mutation or emit `Started`/`Replaced`.

Exact proxy exit consumes every outstanding operation through the same typed
child terminal/drain settlement required by AA-01 and AA-10. If the runtime
cannot return emitted input ownership when normal proxy reports are suppressed
during shutdown, that is a shared real-boundary blocker; the model does not
replace it with flags, abort, logging, or dropped values.

## Deadline drain

```text
DeadlinePhase =
    WaitingWithoutDeadline
  | Scheduling { correlation, settlement }
  | Waiting { correlation }
  | Fired { correlation, observed_at }
```

`WaitForActorGraph` may wait indefinitely and owns no timer.
`RetireActorGraphAfter` reserves one exact correlation. Reservation failure or
schedule rejection immediately forces every unresolved entry into
`Drained(Forced { residual })` with that exact rejection as cause; it never
degrades into unbounded waiting.

When the exact deadline fires, every unresolved entry transfers its complete
phase, proxy or creation ID, operation, local values, and action settlements to
the surviving lifecycle host. No accepted stop, ready worker, successful
replacement, or normal exit is fabricated. The root remains live until residual
external work settles.

The supervisor selects normal termination only when every entry is drained and
every final action has a settlement owner.

## Stale, overlap, and contradiction laws

- A management key alone never matches a lifecycle fact.
- A proxy creation or exit advances only its exact entry generation and creation
  ID or exact capability.
- A proxy-input settlement advances only its exact generation, source,
  `ProxyOperationWitness`, operation kind, and dispatched phase.
- An initial outcome cannot satisfy replacement, and replacement cannot satisfy
  initial.
- Replacement outcome evidence must name the exact retained predecessor.
- A duplicate report cannot release another activation slot.
- A cancel authority cannot cancel another operation on the same key.
- An old-generation fact cannot advance a reused key.
- A stop correlation cannot consume a replacement or cancellation settlement.
- A delivery rejection cannot roll back an already committed state transition.
- Contradictory authoritative facts retain both sides and transfer to diagnostics.
- Diagnostic rejection is terminal and never recursively diagnoses itself.

Every rejected fact preserves the semantic entry state and emits one complete
`RejectedLifecycleFact` through the configured diagnostic disposition.

## Feature-catalogue trace

| Requirement family | Model evidence |
|---|---|
| `DS-BUILD` | construction domain, mandatory durable lifecycle owner, independent diagnostics, no fixed policy |
| `DS-START` | complete pre-commit preparation, fresh proxy, admission/realization split, stable capability publication |
| `DS-QUERY` | total public projection with no `Option`/flags or private phase leakage |
| `DS-STOP` | pending-birth stop, exact shutdown/exit join, durable completion |
| `DS-REPLACE` | base-preserving preparation, exact predecessor join, fresh ready outcome |
| `DS-EXIT` | complete unexpected-stop policy and durable unavailability |
| `DS-RETENTION` | bounded complete table, fresh global generation, no tombstones, exact stale diagnostics |
| `DS-CANCEL` | phase-exact definition ownership, logical post-transfer cancellation, bounded terminal token law |
| `DS-SHUTDOWN` | mutation closure, exhaustive entry drain, pending creation, deadline residual transfer |

Every row remains a model claim, not implementation status.

## Independent-model obligations

The later executable oracle must use independent vocabulary and compare
complete traces. At minimum it must cover:

- zero/invalid construction limits and missing mandatory policies;
- start at capacity, duplicate key, and every correlation reservation failure;
- proxy creation commit/rejection/exit orderings;
- activation limits one and two with operation-order issuance;
- accepted/rejected proxy-input settlement before atomic outcome;
- start cancellation in every phase, including late commit/readiness;
- replace from ready and empty, including StableProxy's internal predecessor
  stop/outcome orders and the supervisor's single completed result;
- replacement cancellation before and after definition transfer;
- stop during pending start, ready, empty, replacement, and cancellation;
- unexpected worker exit under both policies;
- total query projection in every operating and draining phase;
- operation-token replay, bounded terminal retention, and old-token staleness;
- remove/reuse of one key with a fresh generation and late old facts;
- foreign/duplicate/wrong-kind/wrong-source lifecycle facts;
- request, lifecycle, diagnostic, and shutdown delivery rejection;
- shutdown from every entry phase, pending creation, late ready, and repeated
  shutdown;
- deadline reservation failure, schedule rejection, exact firing, and residual
  root settlement; and
- complete named actions for every accepted/rejected transition.

## Falsification findings and comparison obligations

AA-20 records these findings for governing-document reconciliation:

1. The clean-room task text called durable lifecycle ownership optional, while
   the feature, solution, and DevX contracts make one durable route mandatory.
   This model follows the governing mandatory law.
2. The solution/type inventory list a public `Retire` command, but the feature
   law already makes `Stop` release ready/empty entries and automatically
   removes fully retired entries. No distinct `Retire` transition remains;
   retaining both would violate the one-operation/one-spelling DevX target.
3. The solution/query vocabulary stores and exposes `Reserved`, but successful
   preparation and proxy creation emission are one fold. Without a real
   intervening settlement, `Reserved` is transaction-local and not queryable.
4. The cancellation contract did not bound how long terminal operation tokens
   remain recognizable. This model retains only the active or most recent
   terminal operation per entry; older tokens return `CancellationReceipt::Stale`.
5. The stop requirements both restrict admission to available/empty and require
   stop during pending start to wait for creation. This model selects the latter
   explicit race law and accepts stop during `Starting`.
6. The governing dynamic action product omits explicit route-delivered
   diagnostic, deadline, fact-rejection, and child-terminal settlement
   operations required by its own laws. Activation remains proxy-private; the
   supervisor authorizes it by emitting the opaque proxy input, not through a
   second activation lane.
7. The current production template conflates key with child nonce, retains the
   start reply route as lifecycle owner, exposes `Option<phase>`, has no entry
   limit/generation/cancellation/deadline law, and reconstructs worker progress
   instead of consuming AA-01's atomic proxy outcome. It is comparison evidence,
   not an implementation candidate for this model.
8. Governing dynamic syntax supplies `worker` per operation while construction
   selects activation, but it does not state where a possibly affine concrete
   activation plan is materialized or returned. This model treats `worker` as
   the complete `WorkerSubmission` once the static activation contract is
   applied. An executable design must infer and hide that composition without
   cloning, reconstructing, or asking users to spell the product.

These findings do not authorize production surface. The solution, feature
catalogue, type inventory, coverage matrix, DevX contract, retained-core
decision, and research audit must be reconciled before an executable task
relies on a changed law.

This specification is ready to compare with the FIFO and keyed-pool models. It
does not make AA-21 eligible: executable dynamic supervision still depends on
AA-07's completed proxy and interpreter boundary audit.
