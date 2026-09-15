# Solution design for the atomic actor catalogue (engineering record)

This document follows and cross-references
[`atomic-actor-features.md`](atomic-actor-features.md). The feature catalogue is
the requirement source; this document may not silently weaken it.

The minimal existing-core decision is in
[`atomic-actor-retained-core.md`](atomic-actor-retained-core.md), the minimized
state/type equations are in
[`atomic-actor-type-inventory.md`](atomic-actor-type-inventory.md), and the
independent DevX research cross-check is in
[`atomic-actor-research-audit.md`](atomic-actor-research-audit.md). The
disposition of every other current actor template is recorded separately in
[`atomic-actor-other-templates.md`](atomic-actor-other-templates.md); those
later repairs are not part of the five-actor implementation stage.

## Status language

- **Design-covered** means this document names the state, transition, effect,
  error, or test mechanism that satisfies the referenced requirement.
- **Open** means the requirement is recorded but its complete public type,
  transition, effect, and interpreter path have not all been selected.
- **Implemented** means production code and focused regressions exist.
- **Verified** means independent models, exhaustive/property/fuzz coverage,
  interpreter witnesses, and repository gates pass.

No new supervisor or pool production implementation exists. Several rows are
still open after the research/DevX audit; the coverage matrix must not present
them as design-covered. Nothing in this document may be reported as
implemented or verified yet.

Coverage is dependency-closed: a row is `Open` whenever its observable
transition depends on an open shared law, even if its local state transition is
otherwise specified. A row may be `Design-covered` only when its own law and
every foundational law it invokes are design-covered.

## Open design blockers

Production work is blocked until these are resolved with public types and
focused regressions:

1. an interpreter witness for pure initialization, authoritative
   installed-but-not-ready host commit, partial initialization-effect
   settlement, and post-commit drain in that exact order;
2. compile/interpreter realization of the selected activation
   permit/request/fact capability plus owner authorization and bounded
   occupied-ticket states, without moving hydration or I/O into `Behavior`;
3. compile/interpreter realization of action settlement and
   delivery-rejection ownership for every lifecycle, management, customer,
   parent-report, and diagnostic route, including turn-local settlement priority
   and creation-scoped dependency bundles; the prototype must preserve the
   explicitly unbounded transitive-chain limitation rather than claiming
   termination or fairness;
4. interpreter realization of the selected `WaitForActorGraph` versus
   `RetireActorGraphAfter { deadline }` actor-drain policy, exact forced-retirement fact, and
   live root residual state for uncancellable work;
5. a compile-proven total terminal lift across heterogeneous children,
   duplicate roles, and wrapper orders, with a semantic root sum and bounded
   diagnostics;
6. a compile-proven ordinary syntax that does not expose parent paths, reply
   aliases, repeated turbofish, or 64 public support names; and
7. end-to-end interpreter witnesses for readiness and rejected deliveries.

The opaque pool completion token, customer-route ownership, separate stale
diagnostic lane, canonical builder order, common-protocol heterogeneous fleet
limit, dynamic durable-owner split, and precise FIFO rotation are selected
local directions below. They are not implemented, and any matrix row that also
depends on open settlement, activation, shutdown, or completion lowering
remains `Open`.

## Contradiction-closure ledger

This table is the cross-document authority for the latest specification
review. `Selected/Open realization` means the semantic equation has one answer
but its retained-core association, compiler proof, or interpreter witness is
still missing; dependent coverage rows remain `Open`.

| Review issue | One selected answer | Status and authority |
|---|---|---|
| Initialization versus installation | Run the pure init fold first; establish and commit an installed-but-not-ready host before interpreting its initialization `Actions`; then settle those effects before activation. Post-commit rejection drains but never rewinds installation or prior effects. | Selected/Open realization; **Causal creation and activation policy**, `SH-READY`, proxy/pool state equations |
| Rejected delivery after source stop | Ordered per-item action settlement is owned by a statically known host and transfers outward if hosts stop. The live root retains residual ownership and returns a final value only after it settles. No emitter mailbox is required. | Selected/Open realization; **Action settlement and rejection ownership**, `SH-DELIVERY` |
| Turn-local settlement priority | The Driver processes settlement before the source's next ordinary user communication, so chains do not accumulate across separate user turns. No memory, termination, fairness, or eventual-return bound is claimed. | Selected/Open realization; **Action settlement and rejection ownership**, `SH-DELIVERY` |
| Exact activation capability | Committed installation yields actor-retained exact incarnation plus one-shot installation permit; `BeginActivation` consumes that permit and the concrete plan. The owner separately records one bounded occupied authorization ticket, released by exact settlement or outcome. | Selected/Open realization; **Exact activation capability**, `SH-READY` |
| Activation concurrency and late readiness | Every owner counts unresolved activation authorizations. Supervisors reserve conservatively at opaque proxy install; pools reserve at `BeginActivation`. Owners store waiting definitions or permits, exact occupied tickets, and an ordered authorization queue. Cancellation is logical after ownership transfer; late ready drains without publication. | Selected/Open realization; `SH-READY`, actor state equations |
| Startup-limit meaning | The public law always bounds unresolved activation authorizations. Supervisor reservation is deliberately earlier and conservative because its proxy reports no progress; pool reservation occurs at direct activation emission. | Selected; `SH-READY`, activation journeys and inventory |
| Dynamic cancellation ownership | Transaction-local preparation, proxy-create-emitted, waiting-for-authorization, install-emitted, awaiting atomic proxy outcome, ready, cancelling, and shutdown-owned phases are distinct. Worker installation and activation progress remain proxy-private. Only definition-owning phases return it. | Selected/Open realization; **Dynamic supervisor solution**, `DS-CANCEL` |
| Diagnostic failure | `Diagnostics[Route] = DeliverTo(Route) | Terminate`; failed diagnostic delivery becomes terminal settlement and never sends recursively. Proxy uses its mandatory exact parent. | Selected/Open realization; **Diagnostic policy**, `SH-DIAGNOSTIC` |
| Readiness/query wording | Routability begins only at exact `Ready`; dynamic public phase exposes only supervisor-observable reservation, proxy creation, activation authorization, awaiting-proxy-outcome, cancellation, drain, and retirement phases. | Reconciled; `SH-READY`, `DS-QUERY` |
| Competing runtime state machines | This solution is the sole normative ownership equation. The type inventory now references it and contains no duplicate private state tables. | Reconciled; type inventory introduction and runtime references |
| Keyed submission model | `Submit { key, payload, reply_to }` plus one concrete `Fn(&Key) -> Role`; no `KeyedJob` or `SelectWorker`. | Selected; keyed solution, inventory, DevX |
| Key retirement | The transition committing permanent role unavailability automatically unbinds every retained binding for that role, releases capacity, and diagnoses exact generations. Temporary recovery retains bindings; irrecoverability is a cause, not a parallel terminal phase. | Selected; `KP-RETENTION` |
| Pool ownership language | Exactly one authoritative customer obligation/correlation, not one Rust value. Retry deliberately retains a canonical payload and dispatches a clone. | Reconciled; **Job ownership**, `FP-ASSIGN` |
| Pool customer outcomes | Admission is `Accepted | Rejected`; terminal outcomes are exactly `Completed | ReturnedQueued | ReturnedAssigned`. No generic `Returned`. | Selected; **Job ownership**, shared pool inventory |
| FIFO readiness ordering | No serviceable backlog may coexist with eligible idle capacity. Initial/replacement readiness, recovery, and completion commit directly to `Busy` with the oldest eligible queued job before `Idle` is possible. | Selected; **Pool member state**, `FP-TOPOLOGY`, `FP-ASSIGN` |
| FIFO retry capacity | Backlog capacity bounds new waiting admissions. Retry may add at most one formerly assigned job per role beyond that admission bound, preserving an absolute `capacity + roster size` queue bound and original admission order. | Selected; **Job ownership**, `FP-ADMIT`, `FP-INTERRUPT` |
| Pool completion authority | Each dispatch issues affine worker-held authority plus pool-retained non-authorizing evidence, both bound to private worker-birth evidence. Completion matches the locked report's creator-local nonce plus that opaque evidence; the interpreter does not supply an exact incarnation capability. | Selected/Open lowering; **Job ownership**, shared pool inventory |
| Rejected worker assignment | Exact mailbox rejection consumes returned affine authority through typed reunion, reinserts the proven-unaccepted job by immutable admission ordinal, and quarantines the worker. Delivery settlement, completion, and worker stop form one exhaustive order-independent join. | Selected/Open realization; **Admission and dispatch**, `FP-ASSIGN` |
| Pool initialization ownership | While worker initialization is unresolved, the lifecycle host owns its linear settlement and activation plan; pool state retains only exact initialization correlation until one result transfers the lawful next values. | Reconciled/Open realization; **Pool member state**, AA-01 initialization law |
| Pool action application | Every action operation and settlement is a closed exact sum. Non-transactional apply returns applied prefix, rejected operation, and owned unattempted suffix; parent completion rejection returns the complete report instead of being discarded. | Selected/Open runtime boundary; **FIFO pool effects**, action settlement |
| Keyed admission completeness | One total match maps every member state to `AssignableNow`, `BacklogAdmissible`, or `Unavailable`; a separate total management projection maps targets to `Bindable` or `Unavailable`. Pre-ready drain uses its stored recover-or-retire disposition in both. | Selected; **Keyed pool solution**, `KP-AFFINITY`, `KP-REBALANCE` |
| Dynamic lifecycle owner | One mandatory durable route is selected on the builder; request routes are temporary and cannot transfer lifecycle ownership. | Selected; dynamic construction, `DS-START` |
| Forced-retirement termination | Deadline retirement atomically transfers exact outstanding ownership to the surviving host and records `RetiredForced`; that member is actor-side drained while late facts settle with the host. | Selected/Open realization; **Shutdown representation for owners**, `SH-SHUTDOWN` |
| Drain terminology and scope | The sole public sum is `ActorDrainPolicy = WaitForActorGraph | RetireActorGraphAfter { deadline }`. It governs actor-graph retirement only; residual-root settlement and process exit are separate. | Selected; shared inventory, shutdown representation, DevX |
| Root residual lifetime | Forced actor-graph retirement moves the still-live root run future to `ActorGraphDrained { residual }`; no final error/result exists until late activation and exact drain ownership settle. | Selected/Open realization; **Action settlement and rejection ownership**, **Shutdown representation for owners** |
| Terminal settlement lowering | One concrete root terminal sum receives total provenance-preserving static lifts from every heterogeneous child/wrapper sum. Duplicate roles never identify origin; outward lifts compose; wrapper-depth diagnostics must remain bounded. | Selected/Open realization; **Static terminal-projection equation**, type inventory |
| Settlement dependency representation | A closed staged bundle covers every current same-action occurrence operation: creation-result observation, exact established-creation observation, child-termination observation, child delivery, child input, and child shutdown. It selects direct-after-creation or after-required-observation ordering; rejection returns downstream values `NotAttempted`. | Selected/Open realization; **Creation-scoped dependency equation**, retained-core prototype |
| Coverage matrix | Counts are mechanically derived and open dependencies propagate to every observable row. | Reconciled; **Coverage matrix** |
| Public DevX | All previous aspirational snippets are withdrawn. Five complete examples plus completion/wrapper proof are required before syntax is selected. | Open; `atomic-actor-devx.md` |
| Other templates and Entity/Mnesis | Other template dispositions are a separate staged audit. Entity hosting, family drain, Mnesis hydration, and durable completion remain outside this five-actor stage. | Explicit scope boundary; other-template and research-audit documents |

## System boundary

Five independent concrete folds are built in this order:

1. `Proxy<Worker>`;
2. `FixedSupervisor<Role, Worker, ...>`;
3. `DynamicSupervisor<Key, Worker, ...>`;
4. `FifoPool<Role, Job, Result, Worker, ...>`; and
5. `KeyedPool<Role, Key, Job, Result, Worker, Selector, ...>`.

The names above describe intended semantic families, not final generic
spelling. No shared runtime ownership engine is introduced during these five
implementations. Configuration values may be shared because they are the same
policy values; mutable lifecycle state and transition code are not shared
until independent folds prove one smaller lawful extraction.

Every fold returns one named send product, one closed child-creation product,
and one exhaustive next verdict. There are no lifecycle flags, parallel
`Option` fields encoding a join, or boolean transition results. A predicate may
be calculated transiently, but stored and returned semantic state is always a
sum type.

## Causal creation and activation policy

The central design decision is causal two-stage activation.

```text
owner creates empty stable proxy
    -> runtime commits or rejects proxy creation
    -> only after commit, owner sends InstallInitial(worker)
    -> proxy stages fresh worker birth
    -> runtime runs the worker's pure init fold
    -> runtime establishes exact host ownership and commits the
       installed-but-not-ready incarnation
    -> runtime interprets and settles initialization Actions
    -> partial initialization failure drains; success issues activation permit
    -> installed worker enters its selected activation mode
    -> exact ready/rejected/stopped/shutdown fact resolves activation
    -> proxy returns one atomic readiness outcome
```

This is a deliberate Bombay policy, not an actor-model guarantee. It solves a
major source of correlated flags:

- worker creation cannot be reported before stable proxy creation commits;
- the owner retains the worker definition until a proxy capability exists;
- stable creation rejection cannot coexist with a legitimate worker report;
- `Started` always contains the exact stable proxy capability. The proxy keeps
  the routable worker recipient private; any owner-visible worker incarnation
  value is opaque non-routable correlation evidence and is not published in
  `Started` or `Restarted`.

Pools have no proxy. They create workers directly, then apply the same
installation-versus-readiness distinction in their own fold. They do not share
the proxy's runtime state or transition implementation.

The runtime may still deliver a worker stop before the proxy receives matching
installation or readiness resolution. Those joins remain inside the proxy,
which owns all exact correlations. The proxy eventually emits one atomic
parent outcome:

```text
Ready(incarnation)
ActivationStartRejected(incarnation, complete unaccepted request,
                        terminal-or-forced drain fact)
ActivationRejected(incarnation, complete rejection,
                   terminal-or-forced drain fact)
InitializationFoldRejected(attempt, complete error)
InstallationRejected(attempt, complete host rejection and prepared init)
InitializationEffectsRejected(incarnation, failure classification,
                              terminal-or-forced drain fact)
StoppedBeforeReady(incarnation, complete stop fact)
Contradiction(complete conflicting facts)
```

Owners therefore never reconstruct readiness from separate creation,
activation, and stop reports. The semantic activation request/fact capability
is selected below; its association with the retained core and interpreter is
still an open implementation design. Committed creation alone is not an
implementation of this law.

H41b later corrected the custody split: Bombay retains the complete concrete
initialization settlement in the worker environment and transfers it through
runtime retirement. The proxy outcome above carries only the closed failure
classification. The normative contract is
[`atomic-runtime-settlement.md`](../atomic-runtime-settlement.md#exact-bombay-changes-for-worker-initialization-and-activation).

The capability boundary is explicit:

```text
StableProxyCapability = externally routable service recipient
WorkerRecipient = proxy-private routable child capability
WorkerIncarnationEvidence = opaque non-routable lifecycle correlation
```

The proxy's internal installed-incarnation product owns both
`WorkerRecipient` and `WorkerIncarnationEvidence`. Atomic parent outcomes
project only the evidence. Fixed and dynamic supervisors retain that evidence
for exact `replaces` and stop correlation, but their status, capability,
`Started`, and `Restarted` values expose only `StableProxyCapability`.

### Exact activation capability

The semantic capability is selected even though its final Rust integration is
not compile-proven:

```text
CommittedInstallation<Plan> {
    incarnation: InstalledIncarnation,
    initialization: InitializationSettlement,
    activation_plan: Plan,
}

InitializationResolved<Plan> =
    Initialized {
        incarnation,
        activation_permit: ActivationPermit,
        activation_plan: Plan,
    }
  | EffectsRejected { incarnation, settlement }
  | Stopped { incarnation, stop_fact }

BeginActivation<Plan> {
    attempt: ActivationAttempt,
    permit: ActivationPermit,
    plan: Plan,
}

ActivationResolved<Rejection> =
    Ready { attempt }
  | Rejected { attempt, rejection }

CancelActivation { attempt }
```

The creation request owns the concrete activation plan alongside the worker
definition. On committed installation the interpreter returns that same plan
with the exact incarnation and initialization settlement; it never clones,
reconstructs, or looks it up. `InstalledIncarnation` is exact and closed to
public traffic.
`ActivationPermit` is the distinct one-shot capability tied to that committed
installation. The emitted request consumes the permit and plan. Concurrency
authorization is not a second capability: the owning supervisor or pool
represents it by moving the member from its waiting sum variant into an exact
occupied-ticket entry in the same commit that emits the install or activation
request. The actor stores the incarnation and attempt; no linear value is
owned by both state and `Actions`.

The exact order is normative:

```text
pure init fold
  -> establish provisional/exact host ownership
  -> commit InstalledIncarnation, still closed to mailbox/user traffic
  -> interpret initialization Actions
  -> discharge their complete settlement
  -> issue ActivationPermit, drain, or record stop
```

An init-fold error occurs before host commit and executes no initialization
effect. Host/allocation rejection returns the prepared post-init behavior and
uninterpreted initialization `Actions` to settlement; it also executes no
initialization effect. Once installation commits, earlier initialization
effects are authoritative. A later initialization-effect rejection produces
`EffectsRejected` and drains that exact installed incarnation without issuing
an activation permit; it never reclassifies installation as rejected. An
initialization `Step::Stop` similarly reports `StoppedBeforeReady` after all
required initialization effects settle. `BeginActivation` never initializes
the behavior again. An immediate plan may resolve `Ready`; a reported plan
runs its concrete asynchronous future outside the fold. Both remain statically
dispatched.

Each fixed supervisor, dynamic supervisor, FIFO pool, and keyed pool owns an
activation-authorization limit and never owns more than that positive number
of unresolved authorization tickets. Authorization always means the owner has
allowed a worker to progress toward `Ready`. A supervisor reserves
conservatively with the opaque proxy install input because it receives no
worker-progress facts; a direct pool reserves with `BeginActivation` after
initialization settlement. A proxy's local state has a structural limit of
one. Definitions waiting for authorization remain in owner state. An occupied
ticket is released only by its exact action settlement or terminal outcome.

`CancelActivation` is logical. If external work cannot be cancelled, the
activation interpreter retains it but closes publication irrevocably. Late
`Ready` becomes `ReadyAfterCancellation` and transfers the exact incarnation
to drain; late rejection becomes `RejectedAfterCancellation`. If the actor has
already terminated or been forcibly retired, its lifecycle host—not its
mailbox—owns the unresolved activation record, including the exact
incarnation needed to settle or drain that late result.

### Action settlement and rejection ownership

The source actor is never the universal owner of delivery rejection. Before
interpreting any action, the runtime creates one typed settlement record for
every item in the complete named effect product:

```text
ItemSettlement<AcceptedFact, Rejection> =
    Accepted { item, fact: AcceptedFact }
  | Rejected { item, rejection: Rejection }
  | NotAttempted { item, value, blocked_by: SettlementItem }

ActionSettlement<NamedLanes> {
    each semantic lane: ordered settlements for every emitted item
}
```

Each lane's `Rejection` is a closed sum projected from that lane's concrete
effect type. It owns the complete payload, route kind, and exact reason.
`NotAttempted` is required when a declared prerequisite—for example a
same-action child creation—rejects. It returns the untouched dependent value
and refers to the one authoritative prerequisite settlement by an opaque,
non-reused item correlation. It does not clone or duplicate the prerequisite's
owned rejection into every dependent item. Independent later lanes are still
interpreted. Thus every emitted item has one settlement, several failures in
one action are all preserved, and a short-circuit cannot silently discard a
later value. For creation dependencies, `SettlementItem` is exactly the
bundle-scoped correlation of the rejected prerequisite defined below: the
creation settlement, or the first rejected required observation in named
interpretation order. It is not a general item ID, lane index, structural
position, or lookup key.

The complete named settlement product is retained by the actor's lifecycle
host and outlives both the transition and actor. A continuing actor may be the
primary consumer of rejection facts in lane order, but settlement transfers
ownership of each fact to its mailbox only after admission succeeds. The host
retains all remaining items. If the actor is stopping, has stopped, rejects a
fact, or stops while processing an earlier fact, the lifecycle host remains
the owner of every unadmitted item.

Settlement has a mandatory turn-local priority order. Before the Driver admits
the source actor's next ordinary `User` communication, it processes every item
in the current action batch in lane order by doing exactly one of:

```text
admit the typed settlement fact on the source's system lane
or
transfer the item to the source's surviving host settlement
```

Actions produced while folding an admitted settlement fact create a new batch
processed by the same priority rule before ordinary source traffic resumes.
This is a per-source/host-chain priority rule, not a global scheduling barrier.
It proves only that unresolved settlement does not accumulate across separate
ordinary user turns: the next such turn is not admitted until the transitive
settlement chain quiesces or ownership transfers outward.

This is not a decreasing measure or a bound on the transitive chain. A
settlement handler may emit actions whose handlers emit more actions; that can
consume unbounded time or memory and can starve ordinary traffic. An iterative
Driver queue prevents call-stack growth only—it does not prove queue bounds,
termination, fairness, or eventual return to user messages. A stronger claim
requires a future typed transfer/budget policy and its own progress proof.

Installation establishes this ownership recursively and statically. A child
host's unresolved settlement transfers into its parent's typed host settlement
when the parent actor cannot consume it. If that parent also stops, the value
continues outward without reinterpretation. The root application runner is the
terminal live owner. It does not turn unresolved ownership into a finished
error value. Its internal run state is:

```text
Running
ActorGraphDrained {
    actor_summary,
    residual: NonEmptyResidualSettlement,
}
Settled {
    actor_summary,
    terminal_settlement,
}
```

`NonEmptyResidualSettlement` owns the concrete external activation tasks,
their exact correlations, any incarnation produced by a late ready fact, and
the capability needed to drain that incarnation. It is affine runtime state,
represented by a closed statically dispatched product/sum—not an erased boxed
future, cloneable report, registry, or actor mailbox. The root run future
remains alive in `ActorGraphDrained` and consumes late facts until it can
transition to `Settled`; only then may it return the final result to its caller.
An API may publish the actor-side forced summary separately, but that
observation is not the final application result and transfers no residual
ownership.

There is no global settlement registry, lookup, logging fallback, or emitter
mailbox that must remain alive forever. A host may apply only the lane-specific
recovery named below; otherwise outward transfer to the still-live root is the
terminal disposition. `RetireActorGraphAfter` therefore bounds actor-graph retirement,
not completion of external work that the interpreter cannot cancel. A future
bounded-process-exit policy would need an explicit typed emergency-abandonment
outcome; it is not silently implied by `RetireActorGraphAfter` or `Drop`.

The internal host product may be topology-derived, but that structural type is
not the ordinary root error. Each actor/template projects terminal settlement
into one named semantic `TerminalSettlement` sum at its hosting boundary;
lane variants own the exact rejected values and carry a semantic role as a
value when needed. There is no generated variant per topology position and no
exposed `ChildChoice`, `SendLayer`, `Inside`, or occurrence path. The root runner
returns only the root's named projection (normally inferred through the run
method). A compile-pass/fail and diagnostic-size prototype must prove that an
ordinary application neither names nor receives the recursive host product;
until then the public root error and every dependent DevX row remain Open.

#### Static terminal-projection equation

`TerminalSettlement` is not a universal erased envelope. For one concrete
root terminal sum `R`, hosting constructs a closed family of statically
dispatched lifts:

```text
lift_own:   OwnTerminal -> R
lift_child_i: (ExactOrigin_i, ChildTerminal_i) -> R
lift_wrapper_j: WrapperTerminal_j<InnerTerminal> -> R
```

The family must satisfy all of these laws:

1. **Total heterogeneous lifting.** Every variant of every concrete child or
   wrapper terminal sum maps to exactly one variant of `R`; no common erased
   payload, `Any`, trait object, serialization, or catch-all variant exists.
2. **Exact provenance preservation.** Each lift consumes the exact source
   occurrence/capability, incarnation or generation, semantic lane, and owned
   rejection. `R` may carry a semantic role as a value, but a duplicate role
   value never substitutes for exact source provenance.
3. **Compositional outward transfer.** Passing through hosts is function
   composition: `lift_A_to_C = lift_B_to_C ∘ lift_A_to_B`. A wrapper may lift
   its own terminal variants, but it cannot inspect, discard, duplicate,
   reorder, or reinterpret an inner terminal variant. A terminal-transparent
   wrapper changes only the inferred private lift composition. A wrapper with
   a genuinely new terminal law requires a semantic variant in `R`, never a
   structural-position variant or caller-authored path.
4. **Closed duplicate-role handling.** Two children with equal semantic role
   values retain different exact origins internally. Their lifts may target
   the same domain variant only when that variant stores the distinguishing
   exact provenance; equality of role values is never the injection key.
5. **Bounded diagnostics.** Compile fixtures at one, two, and eight repetitions
   of a terminal-transparent wrapper must expose the same root semantic type
   and primary error. Diagnostics must contain none of the private structural
   product/path names, and their recorded rendered-size budget may not grow
   with wrapper depth.

The Rust mechanism that supplies these lifts is deliberately unselected. A
user-written callback or alias per child would violate the no-plumbing law;
automatic structural projection could reproduce the original type explosion.
The prototype must prove one inferred concrete mechanism through two
heterogeneous children, duplicate role values, and two wrapper orders before
`TerminalSettlement` becomes an authorized public name.

#### Creation-scoped dependency equation

Settlement does not use a general runtime dependency graph. The selected
cross-lane dependency is any current occurrence-dependent operation authored
against a child created in the same action. The closed current set is
`ObserveCreation`, `ObserveEstablishedCreation`, `ObserveChild`,
`ChildDelivery`, `ChildInput`, and `ShutdownChild`. Lowering groups them
structurally at authorship time:

```text
CreationBundle<CreateRequest, AfterCreation> {
    creation: CreateRequest,
    after_creation: AfterCreation,
}

AfterCreation =
    Direct(OccurrenceEffects)
  | AfterRequiredObservation {
        required: RequiredObservations {
            creation_observations,
            established_creation_observations,
            child_termination_observations,
        },
        then: OccurrenceEffects,
    }

OccurrenceEffects {
    child_deliveries,
    child_inputs,
    child_shutdowns,
}

LoweredAction<Independent, Creations> {
    independent: Independent,
    creations: closed heterogeneous product of CreationBundle values,
}
```

Each dependent value is physically owned by the same bundle as its one
creation prerequisite; it does not name a lane index, wrapper path, runtime
role, or registry key. The interpreter handles one bundle as follows:

```text
creation accepted(exact binding), Direct(effects)
    -> interpret every occurrence-dependent effect against that binding
       in named lane order

creation accepted(exact binding), AfterRequiredObservation { required, then }
    -> interpret every required observation against that binding in this
       named order:
       1. creation_observations, in item order
       2. established_creation_observations, in item order
       3. child_termination_observations, in item order
    -> preserve the authoritative settlement of every accepted and rejected
       observation; one rejection never suppresses another observation
    -> if all required observations settle accepted,
       interpret every occurrence-dependent effect in `then`

one or more required observations rejected
    -> retain every authoritative observation settlement independently
    -> select the first rejected observation in the named order above as the
       deterministic blocking prerequisite
    -> return every dependent delivery, input, and shutdown untouched as
       NotAttempted {
           value,
           blocked_by: bundle-scoped correlation of that first rejection,
       }

creation rejected(rejection)
    -> retain the one authoritative creation settlement
    -> return every observation, delivery, input, and shutdown item as
       NotAttempted {
           value,
           blocked_by: bundle-scoped creation-settlement correlation,
       }
```

Each bundle-scoped correlation is created while producing the corresponding
creation or observation settlement and is copied only as non-authoritative
correlation. When several required observations reject, every rejection keeps
its own correlation and owned value; only the first correlation in named
interpretation order is copied into downstream `NotAttempted` settlements.
Resolving it requires no lookup because the enclosing staged settlement owns
the prerequisite and all dependents together. Independent effects remain
outside the bundle and are still interpreted. Multiple child creations are a
closed heterogeneous product of independent bundles, not a map. `Direct`
encodes creation → occurrence effects;
`AfterRequiredObservation` encodes creation → required observations →
occurrence effects. A future occurrence-dependent operation must be added to
this closed product and its settlement tests before it can lawfully appear in
the same action; there is no catch-all request and no general DAG API.

The completeness boundary is explicit. Ordinary logical delivery is
independent of a creator-local occurrence. `ObserveEstablishedCreation` is
inside the bundle because it is authored from the pre-commit `ChildRoute` and
returns the exact established capability only after creation commits.
`EstablishedDelivery`, `ObserveEstablished`, and `ShutdownEstablished` are
different: they require that resulting established capability as their input,
so they cannot lawfully be authored in the original creation action.
`ChildReport` originates at the child rather than the creating actor. Thus the
six families above exhaust the current operations that can consume the
pre-commit occurrence in the same creating action; adding another such family
reopens this equation.

Fresh allocation is the actor-model law. Using a creator-local route to stage
an exact-capability observation after commit is Bombay's derived construction.
Requiring that observation for stable proxies, interpreting the three required
observation lanes in the named order above, and selecting the first rejection
as the downstream blocking correlation are deliberate Bombay policy choices;
they are not guarantees supplied by the actor model.

The retained `Actions` surface does not yet expose this grouping. A focused
core prototype must prove either a truthful lowering from existing named
products or the smallest required interpreter-facing product change. Until it
does, `NotAttempted` and every same-action dependent-effect row remain Open;
adding handwritten paths or a runtime dependency registry is not an allowed
fallback.

The required lane ownership is:

| Lane | Primary recovery while source lives | Surviving owner |
|---|---|---|
| proxy → worker command | proxy, which may report unavailable without replaying implicitly | proxy lifecycle host |
| owner → proxy install/replace | fixed/dynamic supervisor operation state | supervisor lifecycle host |
| pool → worker assignment | pool active job state | pool lifecycle host with complete customer obligation |
| fresh proxy/worker creation and creation observation | owning proxy/supervisor/pool pending-creation state | owning actor lifecycle host |
| activation begin/cancel | owning actor activation state | owning actor lifecycle host with exact unresolved activation record |
| child shutdown/observation request | owning proxy/supervisor/pool drain state | owning actor lifecycle host |
| restart/deadline scheduling request | owning recovery/drain state | owning actor lifecycle host |
| parent report | none after source commit; no implicit replay | reporting child's lifecycle host terminal settlement |
| management reply | none after source commit; no implicit replay | dynamic-supervisor host terminal settlement |
| lifecycle event | none after source commit; no implicit replay | supervisor host terminal settlement |
| customer outcome | none after active correlation removal; no implicit replay | pool host terminal settlement with complete outcome |
| finalization/task/terminal/shutdown report | none; the source may stop in the same action | source lifecycle host directly |
| operational diagnostic | policy below | lifecycle host directly |

This settlement is a foundational interpreter contract, not an actor delivery,
callback, global registry, or retry engine. Its precise association with the
current `Behavior`/`Actions` types and child lifecycle facts must be proven by
the focused core prototype before any dependent actor row can be implemented.
It does not add a fourth behavior effect: `Actions` still contains the explicit
communications and creations, and settlement is their typed interpretation
result.

### Diagnostic policy

Every configurable diagnostic-producing actor selects one semantic sum:

```text
Diagnostics[Route] = DeliverTo(Route) | Terminate
```

That notation is the domain equation, not the selected Rust representation.
A generic enum would leave `Route` unconstrained for `Terminate` and violate
the no-placeholder law. The compile prototype must instead produce an inferred
route-bearing builder state for delivery and an inferred route-free state for
termination, with one exhaustive internal transition law. It may not solve
inference with a default generic, public marker argument, or dummy route.

`DeliverTo` makes one attempt. Rejection immediately produces terminal
`UndeliverableDiagnostic { diagnostic, reason }` in action settlement; it does
not send a diagnostic about the diagnostic. `Terminate` skips delivery and
emits the original diagnostic through a named terminal-settlement interpreter
request in `Actions` while selecting `Step::Stop` in that same turn. The
interpreter settles that explicit request directly with the lifecycle host.
This gives an autonomous actor a truthful construction without a dummy route
or ambient side channel while keeping the exact value recoverable.

An owner-created proxy has a mandatory exact structural parent, so its
diagnostic disposition is fixed to `DeliverTo(parent)` by construction rather
than exposed as another builder axis. Rejection follows the same terminal,
non-recursive settlement law.

`Terminate` stops the actor in the diagnostic-producing action after preserving
its other required effects. A later `UndeliverableDiagnostic` rejection causes
a still-live actor's admitted settlement fact to select `Step::Stop`; if that
fact cannot be admitted, the host/root terminal disposition applies directly.
A diagnostic policy therefore has no recursive send, hidden effect, or
indefinite retained error state.

## 1. Stable worker proxy solution

### Construction and protocol

`Proxy<Worker>` is created empty. Its public protocol is exactly the worker's
public protocol, preserving stable service identity. Its owner has a private
typed control protocol:

```text
InstallInitial(Worker)
Replace(Worker)
Shutdown
```

The proxy reports to its established parent through one concrete report sum:

```text
InitialInstallation(InstallationOutcome)
Replacement(ReplacementOutcome)
WorkerStopped(complete stop fact)
Unavailable(original sender, lifecycle phase, complete command)
```

### Runtime state

```text
Dormant

CreatingInitial {
    attempt,
}

CreatingInitialAfterStop {
    attempt,
    stop_fact,
}

InitializingInitial {
    incarnation,
    initialization_settlement,
    activation_plan,
}

InitializingInitialAfterStop {
    incarnation,
    initialization_settlement,
    activation_plan,
    stop_fact,
}

ActivatingInitial {
    incarnation,
    activation_attempt,
}

ActivatingInitialAfterStop {
    incarnation,
    activation_attempt,
    stop_fact,
}

Ready {
    incarnation,
}

EmptyInitial

EmptyAfter {
    last_incarnation,
}

StoppingForReplacement {
    current_incarnation,
    reserved_attempt,
    replacement_worker,
}

CreatingReplacement {
    attempt,
    replaces,
}

CreatingReplacementAfterStop {
    attempt,
    replaces,
    stop_fact,
}

InitializingReplacement {
    incarnation,
    replaces,
    initialization_settlement,
    activation_plan,
}

InitializingReplacementAfterStop {
    incarnation,
    replaces,
    initialization_settlement,
    activation_plan,
    stop_fact,
}

ActivatingReplacement {
    incarnation,
    replaces,
    activation_attempt,
}

ActivatingReplacementAfterStop {
    incarnation,
    replaces,
    activation_attempt,
    stop_fact,
}

DrainingCreation {
    attempt,
    creation_kind,
}

DrainingCreationAfterStop {
    attempt,
    creation_kind,
    stop_fact,
}

DrainingWorker {
    incarnation,
}

DrainingInitialization {
    incarnation,
    initialization_settlement,
    activation_plan,
}

DrainingActivation {
    incarnation,
    activation_attempt,
    cause: ActivationDrainCause,
}
```

`ActivationDrainCause` is a closed private sum containing the complete
unaccepted `BeginActivation` request, accepted-plan rejection, logical
cancellation, or shutdown provenance. It is not an optional rejection field;
the final proxy outcome consumes exactly the variant that caused the drain.

`EmptyInitial` and `EmptyAfter` are distinct because replacement provenance is
present only in the latter. No `Option<incarnation>` hides that distinction.

### Transition rules

- `InstallInitial` is accepted only in `Dormant`. Fresh nonce reservation and
  collision checking precede the creation action.
- `Command` in `Ready` emits one exact child delivery. Every other live state
  emits one `Unavailable` report containing the untouched sender and payload.
- Successful committed creation enters `Initializing*`, owning the exact
  installed incarnation and initialization settlement. Complete successful
  settlement yields the one-shot permit and enters `Activating*`; only an
  exact ready fact enters `Ready`. Immediate activation may collapse the
  activation request/resolution, but never the committed initialization phase.
- Initialization-fold or host rejection executes no initialization action and
  returns the proxy to the appropriate empty state. Initialization-effect
  rejection after commit enters `DrainingInitialization`; it reports the
  distinct rejection only after the exact installed incarnation is terminally
  settled, and cannot become empty earlier. Activation rejection follows the
  same drain-before-outcome law through `DrainingActivation`.
- `Replace` in `Ready` reserves the replacement nonce first, then emits one
  shutdown request and owns the replacement definition.
- `Replace` in `EmptyAfter` stages the replacement immediately.
- `Replace` elsewhere returns the submitted worker in a typed rejection.
- A matching stop in `Creating*` moves to the corresponding `AfterStop` state
  and emits no premature parent result.
- Matching installed creation from `Creating*AfterStop` enters the matching
  initialization/stop join and can emit only `StoppedBeforeReady`, never
  `Ready`, even if later initialization and activation facts succeed.
- Rejected creation from an `AfterStop` state produces `Contradiction` with
  both authoritative facts.
- Matching stop from `Ready` emits `WorkerStopped` and becomes `EmptyAfter`.
- Matching stop from `StoppingForReplacement` emits the stop report, stages
  the already-reserved replacement, and moves to `CreatingReplacement`.
- Any pre-readiness failure after installation commits first stores its
  complete cause and drains that incarnation. The proxy emits its one atomic
  parent outcome and becomes empty only when the exact terminal or forced
  transfer fact closes that drain. The owning supervisor's occupied
  authorization ticket therefore cannot be released on a merely intermediate
  failure fact.
- Stale, duplicate, wrong-kind, and wrong-worker inputs preserve proxy state and
  emit one complete worker-specific diagnostic carrying the current proxy
  phase. The expected correlation remains solely in proxy state.
- Shutdown maps every live state to its corresponding drain state. It never
  sends to an already observed-dead worker and never realizes a cancelled
  replacement definition.
- A parent-report delivery rejection never rewinds these states. The complete
  rejected report goes directly to the proxy host's terminal settlement; the
  proxy does not recursively diagnose failure of its mandatory structural
  report.

### Proxy effects

```text
worker_deliveries
worker_creations
worker_creation_observations
worker_stop_observations
worker_shutdowns
parent_reports
```

The interpreter order is creations first by `Actions`, then observation
requests, worker deliveries/shutdowns, and finally parent reports. Focused
tests assert the complete product for every transition, including empty lanes.

## 2. Fixed supervisor solution

### Construction

The earlier five-axis product was incomplete. The candidate construction proof
must account for:

```text
Factory        = Missing | Selected(factory)
Roles          = Empty | NonEmpty(ordered unique roles)
Activation     = Missing | Immediate | Reported(concrete activation contract)
ActivationAuthorizationLimit = Missing | MaximumUnresolved(positive bound)
Recovery       = Missing | Selected(policy)
Failure        = Missing | Selected(reaction)
ActorDrain     = Missing | Selected(ActorDrainPolicy)
Diagnostics    = Missing | DeliverTo(concrete route) | Terminate
Events         = NotPublished | Published(concrete delivery route)
```

`Events` is not a mandatory proof axis: an autonomous fleet builds in
`NotPublished` without a dummy route. When selected, the concrete route kind is
preserved and receives ready stable-proxy capabilities and lifecycle events,
never worker recipients. A typed status query remains available independently.
The exact minimal typestate and
diagnostic-disposition types are open; this list is a semantic checklist, not
permission to publish one marker type per line.

`build` checks duplicate roles and prepares every initial factory result before
constructing the behavior. Infallible and fallible factories need distinct
truthful construction forms. A heterogeneous worker sum is accepted only when
all variants share one public protocol.

### Member state

```text
Declared { role, initial_worker }

CreatingProxy { role, initial_worker, proxy_attempt }

ProxyStoppedBeforeCreation {
    role,
    initial_worker,
    proxy_attempt,
    stop_fact,
}

WaitingForActivationAuthorization {
    role,
    proxy_route,
    proxy_capability,
    prepared_worker,
    operation_ticket,
}

InstallDispatched {
    role,
    proxy_route,
    proxy_capability,
    operation,
}

AwaitingProxyOutcome {
    role,
    proxy_route,
    proxy_capability,
    operation,
}

Online {
    role,
    proxy_route,
    proxy_capability,
    worker_evidence,
}

Empty {
    role,
    proxy_route,
    proxy_capability,
    last_worker_evidence,
}

AwaitingInitialThenReplace {
    role,
    exact pending initial proxy operation,
    recovery_ticket,
    prepared_replacement,
}

WaitingToRestart {
    role,
    proxy capability,
    prior_worker_evidence,
    recovery_ticket,
    prepared_replacement_and_operation_ticket,
}

ReplacementDispatched {
    role,
    proxy capability,
    prior_worker_evidence,
    recovery_ticket,
    operation_ticket,
}

AwaitingReplacementProxyOutcome {
    role,
    proxy capability,
    prior_worker_evidence,
    recovery_ticket,
    operation_ticket,
}

Stopping { role, exact proxy ownership }

Retired { role, terminal reason }
```

The proxy-creation join has only `CreatingProxy` and
`ProxyStoppedBeforeCreation`. Installed plus prior stop retires the dead proxy;
rejected plus prior stop returns both facts as a contradiction. Worker
creation/readiness is not reconstructed in this actor because the proxy returns
one atomic readiness outcome. The proxy's installed-but-activating states
remain hidden here.

Proxy commit moves the retained initial definition to
`WaitingForActivationAuthorization`. The supervisor owns one global bounded
activation-admission state with an ordered waiting-role queue and an exact set
of occupied operation IDs. Each initial ID is paired once with a private
witness during proxy-reservation preparation before any initialization creation
is emitted. Opaque pair identity makes collision unrepresentable; process-wide
allocation failure is not a supervisor rejection. Reserving an activation slot and emitting the
already-ticketed proxy install input are one commit. Mandatory settlement discharge resolves
`InstallDispatched` before another ordinary input or proxy report: acceptance
enters `AwaitingProxyOutcome`, while rejection returns the complete install
input to recovery. The slot is released only by the matching proxy atomic
outcome or forced ownership transfer during drain.

An initial outcome matches the exact proxy source and expected initial
operation. A replacement outcome additionally matches its nested `replaces`
`WorkerIncarnationEvidence` against the participant's exact retained prior
evidence. The supervisor does not inject or require its recovery ticket in the
proxy protocol; source, operation kind, and predecessor evidence are already a
complete correlation.

For replacement, one fresh affine operation pair per selected participant is
created during recovery preparation and stored with that participant's prepared
replacement. The issuing transition consumes the stored ID; it never allocates
one.
Worker preparation, recovery/timer correlation, budget, and checked release
remain the recoverable pre-commit failures. No operation ID can overwrite or
reuse another pair.

### Recovery decision

This proposal originally copied trigger classification, budget charge, release
provenance, and an eight-way prerequisite product into the admitted recovery.
H76 rejected that Rust representation without changing the semantic law. The
normative current representation is owned by
[`fixed-supervisor.md`](../actor-laws/fixed-supervisor.md#recovery-partition):
the participant order is `before_trigger / trigger / after_trigger`; restart
release stores only `Ready`, an unsettled schedule, or one awaited timer;
participant readiness remains in its current subject; activation availability
is derived at issue time. Values already consumed by admission remain only in
their actual owner, never as recovery breadcrumbs.

Before admission:

1. classify eligibility from the complete terminal outcome;
2. select the complete ordered candidate set;
3. reject any overlap with an existing recovery ticket;
4. call every selected factory and retain every result locally;
5. reserve one exact operation ticket for every prepared replacement;
6. prune budget evidence using the event time;
7. reject out-of-order time explicitly rather than discarding future evidence;
8. validate the atomic budget charge;
9. compute checked delay and reserve exact timer correlation; and
10. commit admission membership, correlations, prepared ownership, and budget
    charge once.

Any failure before step 10 leaves every non-trigger member and the budget
unchanged. Later participant delivery, readiness, and replacement realization
may fail independently and are never rolled back as a batch. The trigger's
authoritative stop is represented as `Empty`; the
configured failure reaction then maps the topology to `RetireMember` or
`StopSupervisor` drain.

### Fixed supervisor effects

```text
proxy_creations
proxy_creation_observations
proxy_stop_observations
proxy_install_inputs
proxy_replacement_inputs
proxy_shutdowns
restart_schedules
lifecycle_events
management_replies
terminal_diagnostics
delivery_rejections
```

The AA-10 oracle remains blocked on the interpreter-facing child-terminal
settlement that must return an emitted install or replacement when a proxy
terminates without its normal outcome. These effect names do not implement
that transfer, and no fixed-supervisor row depending on it may be promoted
until a locked-boundary witness exists.

## 3. Dynamic supervisor solution

### Construction and public protocol

The builder has at least these mandatory choices:

```text
UnexpectedExit = Missing | KeepEmpty | Retire
EntryLimit     = Missing | MaximumEntries(non-zero bound)
Activation     = Missing | Immediate | Reported(concrete activation contract)
ActivationAuthorizationLimit = Missing | MaximumUnresolved(positive bound)
ActorDrain     = Missing | Selected(ActorDrainPolicy)
Diagnostics    = Missing | DeliverTo(concrete route) | Terminate
Lifecycle      = Missing | DeliverTo(concrete durable route)
```

Its public messages are:

```text
Start { key, worker, reply_to }
Stop { key, reply_to }
Replace { key, worker, reply_to }
Query { key, reply_to }
Cancel { operation, reply_to }
```

Each request contains only its request reply route. The builder's one durable
lifecycle route owns every later event; start cannot select, transfer, or
replace it.

### Entry state

```text
CreatingProxy { entry_generation, operation, worker, proxy_attempt }
WaitingForActivationAuthorization {
    entry_generation, operation, worker, proxy
}
InstallDispatched { entry_generation, operation, proxy, install_attempt }
AwaitingProxyOutcome { entry_generation, operation, proxy, install_attempt }
Ready { entry_generation, proxy, incarnation }
Empty { entry_generation, proxy, last_incarnation }
Stopping { entry_generation, proxy, stop_operation, shutdown_observation }
Replacing { entry_generation, proxy, prior_incarnation,
            phase: ReplacementPhase }
Cancelling { entry_generation, proxy_or_pending_proxy, operation,
             phase: CancellationPhase }
Draining { entry_generation, phase: DrainEntryPhase }
Retiring { entry_generation, exact_unresolved_ownership }
```

Start preparation is transaction-local: it reserves every correlation before
committing `CreatingProxy` and the creation action together. There is no stored
or public `Reserved` phase.

`ReplacementPhase` and `CancellationPhase` are closed private sums mirroring
the ownership boundary above: definition local before input emission, proxy
creation emitted, waiting for activation authorization with committed proxy
and definition, install input emitted, awaiting atomic proxy outcome, ready,
or shutdown-owned. Only the first three variants own a returnable worker
definition. `DrainEntryPhase` owns any pending proxy creation, transferred
install settlement, unresolved proxy outcome, exact proxy shutdown, and
forced-retirement deadline.

The supervisor owns a bounded activation-admission set and an operation-order
waiting queue. Reserving a slot and emitting install/replace are one commit.
Mandatory settlement discharge moves `InstallDispatched` to
`AwaitingProxyOutcome` before another ordinary command or proxy report. The
proxy alone owns worker creation, initialization, activation, and their races;
it publishes no progress facts and returns one atomic terminal outcome. That
outcome releases the slot.

The table is keyed by a semantic `Key`; proxy nonces are generated and retained
separately. Duplicate start and capacity rejection return the submitted worker.
`Started` is emitted only from `AwaitingProxyOutcome` after the proxy's atomic
`Ready` outcome and contains the retained exact proxy capability.

Retirement removes the entry after all exact facts resolve. Reusing the same
key creates a fresh `entry_generation`, so old facts cannot affect the new
entry. `KeepEmpty` consumes capacity. Accepted start/replacement operations
carry separate cancellation tokens; cancellation is a total match over each
operation phase, not a boolean on the entry.

Request reply routes are not retained in long-lived entry state after their
reply action is emitted; rejection ownership moves to action settlement. The
durable lifecycle route and diagnostic policy are supervisor-global builder
state rather than duplicated per entry.

The public query projection is exhaustive but does not expose owned values:
`CreatingProxy | WaitingForActivationAuthorization |
AwaitingProxyOutcome | Ready | Empty | Stopping | Replacing | Cancelling |
Draining | Retiring`; an absent key returns `Unknown`. It does not claim to
distinguish worker installation from activation because the proxy protocol
publishes no such progress facts. No public `Option` plus phase flags encodes
this sum.

`Stop` is the single public command that drains and removes a ready or empty
entry. There is no parallel retirement command; automatic retirement remains
the distinct `UnexpectedExit::Retire` policy.

Stop, replace, query, unexpected exit, unavailability, and global shutdown are
total matches over the entry sum. Commands rejected by the current state
return every owned input. A shutdown state owns a drain sum per entry:

The `Draining` variant above is the one shutdown equation; there is no second
informal `AwaitingProxyCreation | StoppingProxy | Retired` state list.

No management mutation is accepted after the global actor enters drain.

### Dynamic supervisor effects

```text
proxy_creations
proxy_creation_observations
proxy_stop_observations
proxy_install_inputs
proxy_replacement_inputs
proxy_shutdowns
management_replies
lifecycle_events
terminal_diagnostics
delivery_rejections
```

## 4. FIFO pool solution

### Construction

The builder product is:

```text
Factory       = Missing | Selected(factory)
Workers       = Empty | NonEmpty(ordered unique roles)
Activation    = Missing | Immediate | Reported(concrete activation contract)
ActivationAuthorizationLimit = Missing | MaximumUnresolved(positive bound)
Recovery      = Missing | Selected(policy)
Backlog       = Missing | Selected(capacity)
Interruption  = Missing | Fail | Retry
Distribution  = Missing | FIFO
ActorDrain    = Missing | Selected(ActorDrainPolicy)
Diagnostics   = Missing | DeliverTo(concrete route) | Terminate
```

Only the complete product builds. `Retry` and `Fail` both retain a job copy
while it is assigned because the worker owns the dispatched message. Payload
clone occurs before admission commit; the customer route never leaves the
pool. The public behavior therefore states its required clone bounds honestly;
if cloning unwinds, no pool state or `Actions` value has committed. This law
does not promise to recover a caller value already moved into the fold during
unwind.

### Job ownership

```text
QueuedJob {
    job_id,
    admission_ordinal,
    customer_route,
    retained_payload,
    origin: NeverAssigned
          | Retried { prior_role, interruption_classification },
}

AssignedJob {
    assignment_id,
    completion_correlation,
    worker_birth_evidence,
    job_id,
    customer_route,
    retained_payload,
    worker_role,
    worker_incarnation,
}
```

The FIFO backlog owns only `QueuedJob`. A busy member owns its single
`AssignedJob`; there is no separate in-flight collection. A dispatch gives the
worker an affine completion authority paired with the assigned record's
non-authorizing `completion_correlation`. The worker cannot inspect or
construct either value, and retained correlation cannot manufacture a result.

The invariant is one authoritative customer obligation and active completion
correlation, not one physical Rust value. Under `Retry`, the pool retains the
canonical retry payload while the worker owns a cloned execution payload;
neither value independently authorizes a customer outcome.

The canonical pool protocol is:

```text
Accepted { request, job }
Rejected { request, payload, reason }
Completed { job, role, result }
ReturnedQueued { job, payload, reason }
ReturnedAssigned { job, role, payload, reason }
```

Both admission variants echo the caller's opaque request correlation;
`Accepted` also returns the distinct pool-issued job identity. The pool never
trusts request correlation as internal identity or retains it in the accepted
job: admission commit moves it into `Accepted`, while rejection moves it into
`Rejected`. `Accepted` and `Rejected` are admission outcomes. The remaining
three are the only terminal customer outcomes. No generic `Returned` variant
exists: a role is known only after assignment. Rejection of any emitted outcome
transfers that exact value to action settlement and cannot recreate an active
job.

### Pool member state

The pool creates workers directly. The stable pool actor retains the mapping
from semantic role to each fresh worker incarnation; no stable proxy exists in
this topology. Pool lifecycle rules use the same policy values as fixed
supervision but are implemented directly in the pool fold. Its complete
pre-ready ownership progression is:

```text
Prepared { role, worker_definition }
Creating { role, creation_attempt }
Initializing {
    role, incarnation, initialization_attempt
}
WaitingForActivationAuthorization {
    role, incarnation, activation_permit, activation_plan
}
ActivationDispatched { role, incarnation, activation_attempt }
Activating { role, incarnation, activation_attempt }
DrainingPreReady {
    role, incarnation,
    cause: initialization rejection | activation-start rejection
           | accepted-plan rejection | cancellation | shutdown
    after_drain: ResumeRecovery(recovery decision) | Retire(terminal reason)
}

Recovering { role, exact decision, prepared worker or exact timer }
Stopping { role, exact outstanding worker ownership }
Retired { role, terminal reason }
```

The pool also owns one declaration-order activation waiting queue and an exact
bounded set of occupied activation tickets. While initialization is unresolved,
the lifecycle host owns the linear settlement and activation plan; actor state
retains only `initialization_attempt`. Its exact result transfers the permit
and plan into the waiting state. Reserving a ticket and emitting
`BeginActivation` are one commit. Activation settlement, stop, and forced drain
may arrive in either order; no synchronous interpreter guarantee is assumed.
Rejected dispatch returns the
complete request to recovery; exact ready, activation rejection, stop, or
forced transfer releases the ticket once. None of these pre-ready states is
eligible for job dispatch. A post-commit initialization or activation failure
enters `DrainingPreReady` and cannot enter recovery or accept work until exact
terminal or forced-transfer settlement. Ready worker states are split into:

```text
Idle { worker route, private worker-birth evidence }
Busy {
    worker route,
    private worker-birth evidence,
    assigned_job,
    assignment_delivery_completion_stop_join,
}
```

Recovery states that originate from `Busy` own one interruption resolution:

```text
ReturnFailure(assigned_job)
RequeueRetry(assigned_job)
```

The exhaustive assignment join orders delivery settlement, completion, and
worker stop before it emits or requeues the selected alternative. No permanent returned-assignment
tombstone is stored. A later result with that token becomes an operational
stale diagnostic and cannot select a customer.

The pool maintains this invariant after every fold:

```text
no serviceable queued job coexists with an eligible Idle member
```

Initial readiness, replacement readiness, recovery completion, and job
completion all run the same oldest-eligible dequeue transition. If backlog is
serviceable, the transition commits the worker directly to `Busy` and emits
the assignment; it never commits an intermediate `Idle`. Only an empty or
currently ineligible backlog permits `Idle`. FIFO assignment to a newly ready
role advances the circular cursor exactly as an ordinary assignment to that
role; readiness does not create a separate fairness rule.

### Admission and dispatch

1. Preserve the caller request correlation and classify shutdown,
   serviceability, idle availability, and full backlog before allocation.
2. Reject full or unserviceable submission unchanged; otherwise reserve one
   fresh `{ job identity, admission ordinal }` pair.
3. Apply the maintained invariant: an eligible idle role implies there is no
   older serviceable backlog job; this is a state proof, not a runtime assert.
4. Select the next ready idle role from the circular declaration-order cursor.
5. If idle, reserve a fresh assignment/correlation-authority pair and clone the
   execution payload before commit; retain the canonical payload, customer
   obligation, and non-authorizing correlation, then commit `Busy` and emit one
   assignment carrying the affine authority.
6. A job-pair or immediate-dispatch preparation failure rejects through its
   own typed branch and never falls back to queue admission.
7. Otherwise move the original payload into the queue ordered by immutable
   admission ordinal.

Capacity is the new-admission waiting bound. A zero-capacity pool takes branch
4 or 6. Retry is not new admission: it may insert one formerly active job per
role beyond that bound, so the complete queue remains bounded by capacity plus
roster size.
Queue-to-worker dispatch reserves its assignment/correlation-authority pair at
dispatch time. Exhaustion or collision returns that accepted queue head through
its exact `ReturnedQueued` or `ReturnedAssigned` provenance and continues the
finite FIFO fill; it cannot strand a serviceable head beside an idle worker.
Completion resolves the private authority, then validates the report's
creator-local child nonce plus private worker-birth evidence before mutation.
A match uses the pool-retained customer
route, emits one customer result, advances the cursor, and either commits the
member directly to `Busy` with the oldest eligible queued job or to `Idle` when
none exists. A mismatch emits the complete result only on the separate
diagnostic disposition and preserves active ownership.

### Recovery and shutdown

Worker lifecycle uses the same policy values as fixed supervision but remains
pool-owned transition code over direct worker children. A busy-worker stop
first resolves its exact
`AssignedJob` according to `Fail` or `Retry`, then begins recovery. Irrecoverable
retirement redistributes general FIFO backlog to surviving workers; if no live
or recoverable worker exists, all stranded jobs receive one terminal outcome.
`Retry` reinserts the interrupted assignment by its immutable admission
ordinal, preserving order even when several workers stop in a different order
from their jobs' admission. The queue stores only prior role and semantic
interruption classification; recovery alone owns the authoritative stop. Exact
assignment-delivery rejection uses typed affine reunion before the same ordered
reinsertion and is a safe pre-execution return. Delivery settlement,
completion, and worker stop join in any order without a second job disposition.

Shutdown atomically extracts all queued and assigned jobs into customer
outcomes, transfers their active completion joins, cancels delayed recovery,
and drains every direct worker child. Later completions are stale diagnostics
and emit no second customer outcome. `ActorDrainPolicy` explicitly chooses
`WaitForActorGraph` or exact `RetireActorGraphAfter` retirement with forced
facts.

### FIFO pool effects

```text
CreateWorker { reservation, submission }
ObserveWorkerCreation { reservation }
ObserveWorkerStop { child_nonce }
BeginWorkerActivation { child_nonce, attempt, permit, plan }
CancelWorkerActivation { child_nonce, attempt }
DeliverAssignment { child_nonce, assignment, command }
ShutdownWorker { child_nonce, correlation }
ScheduleRestart { recovery, timer, deadline }
ScheduleDrainDeadline { timer, deadline }
DeliverAdmission { route, outcome }
DeliverCustomerTerminal { route, outcome }
DeliverDiagnostic { route, diagnostic }
TransferTerminalDiagnostic { diagnostic }
TransferUnexpectedInput { input }
TransferDrainResidual { residual, cause }
ReportCompletionToParent { completion_evidence }
```

Each operation has its own accepted/rejected settlement carrying the complete
request or outcome. Ordered non-transactional application returns an applied
prefix, one exact rejected operation, and the closed owned unattempted suffix.
The current runtime does not yet provide that result, and local parent report
delivery discards closed-parent rejection; both remain explicit blockers.

## 5. Keyed pool solution

The keyed pool is a separate direct fold. It reuses configuration value types
and assignment/outcome value types, not a nested FIFO pool behavior.

Its builder replaces `Distribution = FIFO` and global backlog with:

```text
Distribution = Keyed(concrete selector)
Backlog      = PerRole(capacity)
Bindings     = Maximum(capacity)
```

Runtime binding state contains only retained entries:

```text
Bound { key, generation, worker_role }
```

Absence is represented by no table entry. There is no persistent `Unbound`
state, per-key counter, or tombstone. Each `Bound` entry is the sole pool owner
of its retained key. Each accepted job stores only copyable, non-authorizing
admitted-binding evidence `{ generation, worker_role }`; it does not clone or
own the key. Each role has its own bounded FIFO queue, so a busy selected role
cannot consume another role's capacity or idle worker.

Rebalance validates an exact absence/generation expectation and the target's
separate management eligibility, then changes only the binding table. It never
walks or edits queued or assigned jobs. `Unbind` removes only
future-admission affinity and releases binding capacity; a later admission or
explicit unbound rebalance allocates a fresh opaque generation without
retaining history for the absent key.

For admission, the current target member projects to exactly one of:

```text
AssignableNow = Idle

BacklogAdmissible = Prepared
                  | Creating
                  | Initializing
                  | WaitingForActivationAuthorization
                  | ActivationDispatched
                  | Activating
                  | Busy
                  | Recovering
                  | DrainingPreReady { after_drain: ResumeRecovery }

Unavailable = Stopping
            | Retired
            | DrainingPreReady { after_drain: Retire }
            | any member after global shutdown owns it
```

This is a total match over the complete member sum. `DrainingPreReady` is not
classified from its cause by guesswork: the fold has already retained one
exhaustive `after_drain` disposition. There is no wildcard/default arm.

An admission-created binding is committed only with accepted work.
`AssignableNow` dispatches immediately; `BacklogAdmissible` requires remaining
capacity in that role's queue; `Unavailable` rejects. If the per-role capacity
is zero and the target is not ready-idle, the job and proposed binding are both
returned unchanged. This law distinguishes eventual service eligibility from
immediate assignability without inferring readiness from a role or binding.

Management-created bindings obey a different total projection because they
admit no job and consume no backlog capacity:

```text
Bindable = Prepared
         | Creating
         | Initializing
         | WaitingForActivationAuthorization
         | ActivationDispatched
         | Activating
         | Idle
         | Busy
         | Recovering
         | DrainingPreReady { after_drain: ResumeRecovery }

Unavailable = Stopping
            | Retired
            | DrainingPreReady { after_drain: Retire }
            | any member after global shutdown owns it
```

Unknown role is a distinct typed rejection. `Irrecoverable` is not retained as
another member variant: the irrecoverable decision commits a permanently
unavailable drain or retired phase. Temporary `Recovering` and
`DrainingPreReady { ResumeRecovery }` remain bindable and retain bindings.

Every management command carries one expectation:

```text
BindingExpectation = Absent | Exact(BindingGeneration)
```

For an absent key, rebalance requires `Absent`, a bindable target, free binding
capacity, and a freshly reserved generation; success moves the command key
into the new binding without a job. `Exact(_)` is stale. Unbind with `Absent`
returns the typed `AlreadyUnbound` outcome; unbind with `Exact(_)` is stale.

For a binding at exact generation `g`, both rebalance and unbind require
`Exact(g)`. `Absent` or another generation returns the complete command
unchanged with `actual = Exact(g)`. A rebalance to the same bindable role is an
accepted no-op preserving `g`. A role-changing rebalance reserves fresh `g2`
before atomically moving the stored key into `{ g2, target }`; failure
preserves the old binding and returns the complete command. Exact unbind
removes and returns the complete binding. Expectation comparison precedes
target validation, so stale commands cannot inspect then mutate a later
generation.

The per-role keyed queue obeys the same ready-time dequeue invariant as FIFO:
a readiness or recovery transition for role `R` commits directly to `Busy`
with `R`'s oldest queued job when one exists. A new job for `R` cannot observe
an idle member while an older serviceable job remains in that role's queue.

The transition committing permanent unavailability extracts and terminates
that role's queued and assigned work once and atomically removes every retained
binding to that role, releasing capacity and emitting exact key/generation
diagnostics. Entering terminal pre-ready drain, terminal stopping, or retained
`Retired` state cannot leave a binding behind. Retry returns work to its
retained role while that role remains recoverable, and temporary recovery
retains bindings. A later rebalance cannot revive returned work. Shutdown
extracts every partition, deletes each returned active correlation after
committing its single customer outcome, removes every binding, and drains the
concrete direct-worker lifecycle owned by this fold. Late completions have no
customer destination and follow the operational diagnostic disposition; no
permanent tombstone is retained.

## Shutdown representation for owners

Fixed supervisor transforms proxy ownership, while both pools transform direct
worker ownership, into the same semantic drain alternatives:

```text
AwaitingChildCreation {
    exact attempt,
    prior stop fact if it arrived first,
}

AwaitingChildActivation {
    exact incarnation and activation generation,
    prior stop or rejection fact if it arrived first,
}

StoppingChild {
    exact route and capability,
}

Retired {
    terminal fact or creation rejection,
}

RetiredForced {
    exact forced-retirement fact,
    host_transfer_receipt,
}
```

Pending recovery decisions move to:

```text
CancelledTimer { exact timer identity and generation }
```

The retained replacement definitions are dropped as cancelled owned values;
they are never created. Exact later timer arrival consumes `CancelledTimer`.
Deadline retirement atomically transfers the complete outstanding ownership to the
surviving lifecycle host and records its typed receipt in `RetiredForced`.
That alternative is actor-side drained: exact late lifecycle, activation,
creation, delivery-settlement, and timer facts belong to the host and cannot
revive the actor. The actor selects `Stop` only when every drain member is
`Retired` or `RetiredForced` and every accepted pool job is completed, returned,
or included in an exact forced transfer.

If the surviving host is the application root, that transfer moves the root
run future to `ActorGraphDrained { residual }`; it does not complete the
future. A late ready fact is admitted to that residual state, the exact late
incarnation is drained, and only complete settlement permits `Settled` and the
final runner return.

`WaitForActorGraph` owns no deadline timer.
`RetireActorGraphAfter` owns one exact deadline timer and can enter
`RetiredForced` only from a still-outstanding alternative.
If the interpreter rejects that deadline schedule, the rejecting settlement
is itself the authoritative inability to enforce the configured bound. In the
same settlement turn, every still-outstanding member transfers to the
surviving host and enters `RetiredForced` with
`DeadlineScheduleRejected { request, reason }` as its cause. This immediate
forced transfer emits no accepted timer or child-stop fact and never falls back
to `WaitForActorGraph`.
Delivery rejection of a shutdown or final report never counts as a child
terminal fact.

## Error and outcome boundary

Expected domain rejection is a successful transition with a typed outcome and
unchanged state. Examples: duplicate dynamic key, full backlog, stale
completion, unavailable proxy, budget denial, invalid rebalance, and overlapping
replacement.

Controlled behavior error is reserved for contradictory authoritative runtime
facts or violated interpreter contracts that cannot be represented as an
ordinary request rejection. Every such error owns the complete conflicting
facts. Sequence exhaustion that occurs before an externally authoritative
transition returns a typed domain rejection with the submitted owned value.

No production branch uses `panic!`, `expect`, `unreachable!`, a sentinel, or an
empty collection as a semantic failure.

## Coverage matrix

The range in each row covers every bullet currently present under that feature
group in the feature catalogue. Counts are generated from the catalogue's
top-level requirement bullets; a changed count invalidates this table. Open
shared activation, settlement, completion-lowering, or drain realization is
inherited by every row whose observable transition uses it.

| Requirement range | Design mechanism | Status |
|---|---|---|
| SH-ACTOR-01..07 | Five direct folds and explicit `Actions` are selected; authoritative initialization/interpreter ordering remains open | Open |
| SH-STATIC-01..06 | Closed event/report sums, concrete generics, role/nonce/capability separation | Design-covered |
| SH-CREATE-01..08 | Fresh staged creates, provenance, and typed exhaustion selected; initialization and dependent-effect settlement remain open | Open |
| SH-READY-01..21 | Authoritative installation before initialization effects, exact activation request/fact, concurrency, logical cancellation, and late settlement; retained-core/interpreter realization absent | Open |
| SH-CORRELATE-01..07 | Exact correlation newtypes, prepare-before-commit, owned rejection values, named lanes | Design-covered |
| SH-DELIVERY-01..18 | Route preservation, turn-local settlement priority, live-root residual ownership, and semantic terminal projection selected; progress bounds and realization absent | Open |
| SH-DIAGNOSTIC-01..09 | `DeliverTo | Terminate`, outward transfer, and non-recursive failure selected; terminal-settlement realization absent | Open |
| SH-SHUTDOWN-01..16 | Dedicated drain sums, host transfer, live residual root, and selected `ActorDrainPolicy`; deadline/interpreter witness absent | Open |
| PX-IDENTITY-01..05 | One stable proxy with one exhaustive incarnation state | Design-covered |
| PX-INIT-01..08 | Empty proxy, private install, and local creation/initialization/stop join; initialization settlement remains open | Open |
| PX-ACTIVATE-01..10 | Installed/activating states and distinct initialization/start/activation outcomes; activation input remains open | Open |
| PX-ROUTE-01..05 | `Ready`-only delivery and complete unavailable report; activation and settlement inherited | Open |
| PX-REPLACE-01..11 | Reserved nonce and provenance specified; activation and report settlement inherited | Open |
| PX-STOP-01..04 | Exact stop matching and `AfterStop` joins; parent-report settlement inherited | Open |
| PX-SHUTDOWN-01..09 | Creation/activation/worker drain; parent-report rejection remains open | Open |
| FS-BUILD-01..13 | Canonical semantic axes known; minimal activation/diagnostic typestate and syntax remain open | Open |
| FS-TOPOLOGY-01..11 | Ordered role/member sum, waiting authorization, and exact proxy route/capability ownership; activation realization absent | Open |
| FS-FACTS-01..07 | Atomic proxy outcomes specified; readiness and lifecycle settlement inherited | Open |
| FS-OPERATE-01..06 | Status/capability snapshot and diagnostic disposition known; public protocol and settlement open | Open |
| FS-ELIGIBILITY-01..05 | Exhaustive `Recovery` policy match on complete terminal outcome | Design-covered |
| FS-STRATEGY-01..08 | Immutable snapshot selection and ticketed unresolved participants | Design-covered |
| FS-BUDGET-01..11 | Atomic charge, inclusive monotonic window, explicit out-of-order rejection | Design-covered |
| FS-TIMING-01..10 | Checked delay values and exact eight-state readiness/timer/authorization join; activation realization absent | Open |
| FS-ATOMICITY-01..04 | Prepare all factories and policy calculations before one commit | Design-covered |
| FS-FAILURE-01..04 | Failure reaction sum selected; diagnostic settlement and drain realization inherited | Open |
| FS-SHUTDOWN-01..08 | Owner-only drain is covered; deadline interpretation and rejected delivery inherit open shared laws | Open |
| DS-BUILD-01..07 | Exit/capacity/activation/durable-route requirements known; minimal builder remains open | Open |
| DS-START-01..10 | Keyed reservation and configured durable owner selected; readiness and reply/lifecycle settlement remain open | Open |
| DS-QUERY-01..04 | Read-only exhaustive projection selected; management-reply settlement and public protocol remain open | Open |
| DS-STOP-01..05 | Capability-owning stop states specified; reply/shutdown settlement inherited | Open |
| DS-REPLACE-01..08 | Stable proxy replacement and ready-only realization; activation path remains open | Open |
| DS-EXIT-01..06 | Empty/retain/retire sum; durable unavailability delivery remains open | Open |
| DS-RETENTION-01..06 | Bounded entries and generations selected; late-diagnostic settlement inherited | Open |
| DS-CANCEL-01..08 | Phase-exact ownership and outcomes selected; transferred install/activation/settlement realization remains open | Open |
| DS-SHUTDOWN-01..06 | Global mutation closure; deadline retirement and route rejection inherit open shared laws | Open |
| FP-BUILD-01..09 | Semantic axes known; minimal activation/diagnostic builder remains open | Open |
| FP-TOPOLOGY-01..12 | Direct pool-owned lifecycle, bounded authorization tickets, ready-time dequeue, and role preservation | Open |
| FP-ADMIT-01..11 | Authoritative obligation and atomic admission specified; reply/assignment settlement and completion lowering open | Open |
| FP-ASSIGN-01..09 | Opaque authority, exact rotation, and no-idle-with-backlog invariant specified; lowering and settlement open | Open |
| FP-COMPLETE-01..07 | Token/source match and stale lane selected; completion lowering and outcome/diagnostic settlement open | Open |
| FP-INTERRUPT-01..09 | At-least-once retry ownership specified; returned-outcome settlement inherited | Open |
| FP-FAILURE-01..05 | Retired-member extraction specified; diagnostic/customer settlement inherited | Open |
| FP-RETENTION-01..05 | Active-only correlation selected; safe late-result diagnostic settlement inherited | Open |
| FP-SHUTDOWN-01..07 | Atomic extraction/direct drain; rejected outcomes and deadline retirement remain open | Open |
| KP-BUILD-01..07 | Explicit submitted key plus one concrete selector selected; minimal builder and activation/diagnostic syntax open | Open |
| KP-AFFINITY-01..09 | Total member-state admission projection, per-role ready dequeue, and atomic binding admission specified; settlement inherited | Open |
| KP-REBALANCE-01..09 | Future-only rebalance/unbind specified; management reply settlement inherited | Open |
| KP-END-01..05 | Role-retained completion/retry specified; completion lowering and customer settlement inherited | Open |
| KP-RETENTION-01..06 | Automatic retirement unbind selected; exact diagnostic settlement inherited | Open |
| API-01..15 | Canonical fluent goals recorded; compile/diagnostic evidence absent | Open |
| EFFECT-01..22 | Named products and complete current creation-scoped dependency equation selected; lowering and rejected-delivery interpreter paths absent | Open |
| VERIFY-01..17 | Required state, residual-root, terminal-lift, dependency, and repository gates enumerated; no replacement implementation exists | Open |
| NONFEATURE-01..10 | Explicit deletion list after replacement proof | Design-covered |

## Implementation acceptance rule

The next lawful stage is foundational prototype work, not implementation of
the five actors. Its order is:

1. initialization/authoritative-host ordering witness;
2. complete creation-bundle lowering for every occurrence-dependent operation;
3. per-item settlement, static terminal lifting, and residual-root ownership;
4. exact activation permit/request/fact plus authorization tickets; and
5. compiler-pass/fail builder and bounded-diagnostic prototypes.

Each prototype begins with its focused law regression and the smallest
end-to-end interpreter witness. It may change production algebra only after the
design-provenance ledger and change-containment threshold are recorded for that
prototype. It may not simultaneously migrate actor templates.

Implementation of proxy, fixed supervisor, dynamic supervisor, FIFO pool, or
keyed pool remains blocked until all five foundational prototypes compose. A
row changes from design-covered to implemented only when its production symbols
point back to those regressions. Legacy deletion begins only after all five
templates are verified and migrated without compatibility wrappers.
