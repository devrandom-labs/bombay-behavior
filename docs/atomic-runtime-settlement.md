# Atomic runtime interpretation and settlement

Status: H02 static representation proven; H07 has lowered total item,
heterogeneous creation, and complete `Actions` settlement into Behavior; H10
fixes each named settlement product independently of the runtime; H03 records
the required Bombay terminal-custody contract. Bombay will change to implement
this architecture; its present interfaces do not constrain the atomic design.
This document is the sole normative owner of
generic action settlement, creation
dependency ordering, initialization settlement, terminal custody, residual
retirement, and the Bombay Engine integration requirement.

## Ownership

`Actions` is Bombay's typed realization of actor transition effects:
communications, staged fresh actor creation, and the next behavior or
termination decision. It is not literally Agha's effect triple: typed products,
termination, initialization, creator-local routing, and interpretation order
include derived constructions and Bombay policy.

Behavior owns only pure static products. Bombay interprets concrete
capabilities and owns runtime custody. Neither aggregate transitions nor generic
Behavior machinery may contain tasks, clocks, channels, address spaces,
observation publishers, callbacks, or runtime handles.

## Total item law

For each declared item, interpretation produces exactly one outcome:

```text
Accepted { receipt-or-promised-success }
Rejected { complete-original-item, exact-reason }
Blocked { complete-untouched-item, exact-prerequisite }
```

`Blocked` means a declared dependency did not commit; it is not a generic
failure or a reason to suppress independent siblings. A corrupt interpreter
result retains the recorded accepted/rejected prefix, the exact faulting item,
and the complete unattempted heterogeneous remainder.

The product law is total and static:

- lanes and items are interpreted in their declared stable order;
- rejection does not short-circuit independent later items;
- every accepted item is consumed and leaves only its specified receipt;
- every rejected item returns its complete original value;
- dependency-blocked items remain untouched with the exact prerequisite;
- no positional traversal, runtime graph, erased envelope, callback, or
  per-template settlement adapter is permitted; and
- a stopping actor is not required to receive its own settlement.

Any generic Behavior representation must prove this law through two unrelated
catalogue templates and two wrapper orders without placeholders before
catalogue migration begins.

Each action item fixes its accepted receipt, rejection, and prerequisite
types. `SendSettlements` projects a concrete sends product to one complete
settlement product without naming a runtime, root event, or structural path.
`InterpretSends` may decide only the attempted outcomes within that fixed
product. A stopping aggregate can therefore retain one typed settlement value
without coupling its state to Bombay or adding a template-specific adapter.

The H02 non-production model satisfies that representation gate with a recursive
heterogeneous product and the existing `SendLayer` inner-to-outer order. It
preserves complete values after rejection/blocking, attempts independent later
items, retains exact prefix/fault/remainder at every product position, and
rejects a deliberate short-circuit trace. This proves stable-Rust realizability,
not production implementation or Bombay custody.

## Creation dependencies

Actor-model creation requires fresh allocation. A creating Behavior issues one
checked `CreationId`, non-reused within the exact statically declared child
occurrence, and emits the complete child in the existing creation leg. The
occurrence and ID together are creator-visible correlation, not an address,
runtime route, actor identity, or establishment capability. Independent pure
layers do not share an ID issuer; equal numeric IDs in distinct occurrences are
lawful. The interpreter commits a successful fresh creation before same-action
communications or observations that name that occurrence and ID. This ordering
is Bombay policy.

`Creations` owns one ordered creation batch. Bombay accepts that whole batch for
routing or returns it unchanged with `ChildNamespaceExhausted`; it may not
consume only a prefix. On acceptance Bombay privately pairs every request with
one route from the current creator namespace, then the generic creation
interpreter attempts each routed request independently in declared order. An
expected child-establishment rejection does not suppress later creations or
independent sends. Interpreter corruption retains the completed prefix and the
exact routed remainder.

Behavior exposes no `ChildRoute` and never constructs an `Address::Nonce`.
Bombay owns the private mapping from exact typed child occurrence plus
`CreationId` to its runtime route. Child communications, lifecycle requests,
reports, and creation prerequisites travel through that typed occurrence and
carry its ID; runtime code resolves the pair to a route. An ID from one
occurrence cannot satisfy another occurrence even when their numeric values are
equal. A rejected routed creation remains in runtime settlement custody with its
child, ID, kind, and selected route. Retirement transfers unresolved batches
and routed creations through the same parent-to-root custody path as every other
affine runtime value.

After a complete interpretation, one typed `CreationsSettled` input returns the
entire creation leg to a live creator. The batch remains the ownership and
ordering unit; templates do not rebuild a per-child settlement join. `NoBirths`
requires no custody port. If creator admission is closed, the unchanged batch
continues through parent-to-root custody.

There is no separate route request, result, aggregate waiting phase, or
`ReturnToEmitter` continuation. Route preparation is the first generic step of
interpreting the one creation leg through the unchanged Driver. It creates no
mailbox, task, registry, callback, or lifecycle service.

An establishment attempt that accepts ownership of a child definition returns
an authoritative `ChildCreationOutcome<C, Occurrence>`. `Established` owns the exact
capability. `InitializationRejected` returns the current child and exact initialization
error. `HostRejected` returns the current child, uninterpreted initialization
actions, and exact reason. These are semantic creation rejections, not
interpreter corruption, and none reconstructs the pre-initialization child.
Rejection or corruption before ownership transfer instead returns the complete
original `CreateChild` through `ItemSettlement`.

A behavior declares that request through exactly one of two domain operations:
`CreateChild::birth` or `CreateChild::replacement`. `CreationKind` remains
authoritative carried provenance for interpreters and typed results; it is not
a third, context-free application constructor. Closed child-product composition
may reconstruct the same request internally while preserving its owned kind.

Semantic creation rejection blocks only operations whose typed prerequisite is the
uncommitted child binding. After creation commits, rejection of one observation
blocks only operations that depend on that observation; lawful child delivery,
input, shutdown, termination observation, and unrelated observations remain
independently eligible. The dependency relation is expressed by the concrete
child-operation item type and its `InterpretItem::Prerequisite`:
`ObserveCreation`, `ObserveEstablishedCreation`, `ObserveChild`,
`ChildDelivery`, `ChildInput`, and `ShutdownChild` are the only current
same-action child families. The runtime transaction retains their typed
resolution by exact child protocol, occurrence, and `CreationId`.
`ObserveCreation<P, Occurrence>` returns `CreationResolved<P::Addr>`; it does not
erase `P` to its address. No general dependency graph, lane index, or wrapper
position is consulted.

A route collision can only falsify Bombay's accepted batch-preparation receipt;
it is interpreter corruption, never replacement. Namespace exhaustion rejects
the untouched batch before route transfer. Replacement provenance is explicit
typed semantic data and never inferred from address reuse, route reuse, or
sequence arithmetic.

## Initialization and activation order

Initialization is part of the Behavior contract. Its actions are interpreted
before ordinary mailbox ingress and compose with wrapper initialization in one
defined order:

```text
fresh installation commit
  -> pure initialization fold
  -> total initialization-action interpretation
  -> initialization settlement and residual custody
  -> activation authorization and attempt
  -> exact readiness or failure fact
  -> ordinary ingress
```

Initialization may stop with final actions. Accepted initialization effects are
settled before ordinary ingress, even when the resulting actor is stopping.
Activation capacity is actor-side admission before an action is emitted; it is
not relabelled as an interpreter rejection afterward.

### Exact Bombay changes for worker initialization and activation

Bombay must keep the installed child closed to ordinary traffic and retain its
complete initialization-action settlement in the existing child lifecycle
host. The settlement never moves into StableProxy state or a supervisor report.
After `ChildCreationOutcome::Established`, an atomic owner retains its non-cloneable worker
state and emits `InitializeWorker<W, P>`. That request contains only a
cloneable exact worker target, opaque worker and initialization correlations,
and the moved activation plan. It does not move the aggregate's worker, name the
worker's action product, or search by runtime type.

The request is an ordinary `ActionItem` with accepted unit and uninhabited
capability rejection or prerequisite. Bombay interprets it by locating the
already existing child host through the supplied established target. Missing
support is interpreter corruption, not an invented runtime rejection. The host
calls `InitializeWorker::resolve` with exactly one
`WorkerInitializationOutcome::ReadyForActivation | EffectsRejected | Stopped` result, which
reunites that result with the affine plan as `WorkerInitializationReport`. The request
and result run inside the unchanged Driver and retirement barrier.

Bombay's local environment must interpret that request through its exact
installed-child capability. Acceptance transfers the request to the already
existing child lifecycle host; it is therefore a non-rejecting local custody
operation. Missing typed support is interpreter corruption and retains the
complete request. It is not an expected mailbox, observer, or capacity
rejection. When initialization settles, the host returns exactly one Tier-1
`WorkerInitializationReport` input:

```text
ReadyForActivation { worker evidence, original plan, one activation permit }
EffectsRejected {
    worker evidence,
    original plan,
    failure: EffectsRejected | InterpreterCorrupt
}
Stopped { worker evidence, original plan, exact stop }
```

The host derives `failure` by inspecting the complete settlement's
`SettlementStatus` without consuming that settlement. `Accepted` selects the
`Initialized` alternative and cannot inhabit `WorkerInitializationFailure`.
Expected capability rejection or dependency blocking selects
`EffectsRejected`; interpreter corruption or an unattempted suffix selects
`InterpreterCorrupt`. The exact settlement remains attached to the concrete
worker environment. StableProxy drains the worker and reports only this
semantic classification, its activation plan, and the exact worker result.

A foreign worker or initialization correlation returns the complete request
and report unchanged. Failed creator admission follows `RecoverEvent` and the
existing parent-to-root custody path. No template-specific Bombay conversion,
second mailbox, or detached task is allowed.

If StableProxy admission closes, Bombay retains the complete
`WorkerInitializationReport` input, including the activation plan, beside the worker
environment's exact action settlement. Parent retirement transfer moves both
values outward as one statically typed residual. This is not a lookup token and
does not require StableProxy to reopen or consume its own settlement.

On `Initialized`, the aggregate consumes the permit and plan with a fresh
activation attempt and exact worker target into `BeginActivation<W, P>`.
Activation capacity is checked by the aggregate before this request exists.
It is never reported as an interpreter rejection.

The selected Communication control capability has one rejection:
`ControlClosed`, which returns the complete submitted value. Consequently the
activation-start action has exactly one expected rejection, `OwnerStopped`,
and that rejection retains the complete `BeginActivation`. Mailbox fullness,
observer capacity, generic host failure, and activation capacity are not
members of this sum.

`ActivationPlan::activate(self)` is a statically dispatched future. After
accepting the request, Bombay must enqueue the matching `Started` input to the
owner before polling the plan. It then retains the typed
`JoinHandle<WorkerActivation<W, P>>` in the existing local environment, polls
it with the existing input schedule, and returns the matching ready or rejected
input through the unchanged Driver. Only an accepted request's consumed plan
may produce readiness.

The H22 production contract realizes that shape without a task in Behavior.
`ActivationPermit` retains the exact established worker target already supplied
to the initialization host. `BeginActivation::new` therefore accepts only the
returned plan and permit; no caller can substitute another same-protocol
worker. A private activation correlation is reserved one-to-one from the
permit's non-reused worker attempt. `BeginActivation::started` produces the
`WorkerActivation` value holding the matching started alternative. Consuming
`BeginActivation::activate` moves the plan exactly once to a
`WorkerActivation` holding either readiness or rejection. The public runtime
values expose consuming projections, not constructors for their private
alternatives.

Bombay must own activation work in one statically typed task product within the
actor's existing environment. Completed results return through the actor's
ordinary typed system-input admission. Unfinished work and completed results
whose owner admission has closed transfer through the existing retirement
barrier to the root custodian. The task product retains its concrete output; it
must not reduce it to `JoinHandle<()>`, abort and discard it, or detach it. This
is Bombay runtime work, not a Behavior-side runtime. It introduces no second
Driver, mailbox, erased output, or lifecycle service. Cancellation never claims
physical cancellation unless the concrete plan provides it.

H14 supplies the generic `ClassifySettlement` projection over the complete H10
static settlement product. Bombay inspects `SettlementStatus::Accepted`,
`Rejected`, or `Corrupt` without consuming or reconstructing that product and
without template-specific traversal. `Rejected` includes both a capability
rejection and a declared dependency block. Rejected or blocked effects take
precedence over a simultaneous initialization stop; the unchanged next decision
remains present in the complete settlement and the installed worker then drains
normally. `Corrupt` includes an interpreter fault or any unattempted suffix and
therefore enters the existing residual-custody path with the entire settlement.

`ActionSettlements` projects a concrete `Actions` type to this complete static
settlement without naming an interpreter. Lifecycle-host requests use that
associated type when they must return a worker's initialization settlement;
they do not restate the creation vector or send-product structure in each actor
template. The projection performs no interpretation and changes no custody.

Named products containing two independent effect lanes use Behavior's single
`settle_in_order` operation. It completely settles the declared earlier lane
before the later lane and retains the untouched later value if the earlier lane
reports interpreter corruption. Routing, lifecycle, discovery, timing, and
atomic actors all delegate that identical sequencing law to Behavior. The former
Actors-local routing copy has been deleted; aggregate products still own their
domain lane names and map the two returned settlements into those named fields.

StableProxy's `ProxyEffects` declares worker observation, initialization,
activation, shutdown, service delivery, owner outcome, and diagnostic lanes in
that order. Runtime-local and structural-owner operations use the one
`InterpreterRequests` product; service forwarding uses exact established
delivery. A concrete compile witness proves the actual `StableProxy::Sends`
implements `InterpretSends`, so no vector-only lane or template adapter remains.
Creation is still interpreted first by `Actions`; therefore the observation's
exact `CreationCorrelation<WorkerProtocol, ChildHead>` refers to that same
action. Creation settlement and the later `ChildStopped` event are independent
admissions. The proxy retains either arrival order explicitly and returns both
values if creation rejection contradicts an already observed stop. Bombay must
not serialize those admissions merely to simplify the aggregate.

## Terminal custody

Every affine terminal settlement follows one ownership path:

```text
child lifecycle host
  -> admitting parent
  -> next structural owner when parent admission is closed
  -> non-rejecting root custodian
  -> existing Engine retirement barrier
```

Closure returns the exact value; it never logs or discards it. Outward transfer
does not reopen a stopped actor. The path uses the existing Driver and actor
graph—no detached task, second mailbox, second lifecycle framework, or
Behavior-side runtime is permitted.

Forced retirement transfers unresolved creations, initialization and activation
settlements, assignment joins, customer outcomes, diagnostics, timers,
shutdowns, and queued ingress as explicit residual ownership. Actor-graph
retirement and external work are distinct; the root barrier cannot claim
quiescence while externally owned terminal work remains unaccounted for.

An aggregate may retain its unresolved actor-owned values in a private terminal
alternative of the final concrete behavior. Runtime work already accepted from
that aggregate remains in the environment residual. Bombay transfers both
products without inspecting, flattening, or reconstructing either one. This
separation prevents a pool-specific residual lane while preserving the exact
owner of every affine value.

FIFO and KeyedPool name the identical local stop cause once as
`ForcedRetirementCause`: exhausted worker-shutdown identifiers, a deadline not
scheduled with its complete interpreter result, or the exact elapsed deadline. The cause does not own a
worker roster and does not perform transfer. Each aggregate retains its own
unresolved workers; Bombay later moves the complete concrete behavior and
environment residual together.

## Exact concrete rejection vocabulary

Concrete reasons come from the selected locked dependency contracts:

- asynchronous Communication send waits for bounded capacity and rejects only
  with the closed complete payload; `Full` belongs to `try_send`;
- control send is unbounded and rejects only `ControlClosed(complete_item)`;
- Address claim distinguishes `AddressInUse` and permanent registration-ID
  exhaustion;
- Observe distinguishes `SubjectExists`, `UnknownSubject`, and exact
  generation-checked retirement;
- Timers use exact branded schedule tokens and exact-current cancellation; and
- orderly shutdown distinguishes accepted, already stopping, and already
  stopped, with child absence separately reported as not established.

H23 places exact orderly shutdown on the same generic item settlement as every
other action. Accepted exact shutdown leaves `ShutdownId`; either selected
rejection returns the complete `ShutdownEstablished` request and its exact
actor capability. The concrete endpoint port returns only
`Result<(), ShutdownRejection>`. StableProxy can therefore use exact shutdown
for post-commit worker drain without a proxy-specific adapter or weaker child
route.

Generic settlement must not fabricate mailbox-full, observer-capacity,
activation-capacity, or typed exhaustion variants unsupported by those APIs.

`ObserveChild<P, Occurrence>` names the concrete child protocol because its
same-action dependency is the one existing
`CreationCorrelation<P, Occurrence>`. The selected child-observation path
clones the hosted exact termination observation after the typed child binding
commits. It therefore has accepted unit, no capability rejection, and only the
creation prerequisite; `SubjectExists` applies to subject registration and is
not fabricated as a child-observation rejection. Equal address types do not
make two child protocols substitutable.

H05 makes this boundary executable without adding runtime dependencies to
Behavior. A research-local witness checks 13 exact facts against the selected
Address 0.2.0, Communication 0.1.2, Timers 0.1.0, frozen Bombay Observe, and
locked Behavior shutdown sources. Under Bombay's pinned Rust 1.96, the exact
Address suite passes 14/14, the Timers suite passes 2/2, and Communication's
library target builds (it has no library unit tests). These releases cannot be
linked into the Rust-1.95 atomic experiment; that MSRV boundary is intentional
evidence that concrete capability interpretation remains a Bombay concern.

## Required Bombay target

The atomic architecture targets the following Bombay contract. Bombay's
present method signatures are historical evidence only; they are not reasons
to weaken, defer, or reshape the accepted actor laws.

Bombay must:

1. return or re-home the complete rejected parent event;
2. move the final concrete Behavior value into retirement ownership instead of
   dropping it after `Step::Stop`, `Exhausted`, or a Driver error;
3. return a typed residual from environment retirement;
4. carry the Behavior and environment residual through the existing Driver
   retirement barrier; and
5. deposit that one typed transfer at a non-rejecting root custodian before the
   barrier releases.

### Typed application assembly

Bombay must also reverse its current root-first application construction order
for actor templates whose required destinations are application-owned actors.
The target order is:

```text
declare application actors by semantic role
    -> install them through Bombay's existing runtime ownership
    -> obtain their typed logical or exact capabilities
    -> construct the pure root from those capabilities
    -> initialize and activate the root
    -> return the typed root and application lifecycle capabilities
```

The one-time root constructor belongs to Bombay application assembly. It is not
a `Behavior`, is never retained by an atomic actor, performs no I/O, and cannot
be invoked from a transition. Bombay remains the sole owner of address
allocation, mailboxes, installation, activation, and startup rollback. If peer
installation or root activation fails, Bombay retires every already installed
application actor through the same retirement path and returns the exact startup
failure.

Application code names semantic roles, concrete actor behaviors, and typed
capabilities only. It does not name `MailAddr`, child-product positions,
occurrence paths, generated type aliases, or final composed behavior types.
Role selection must be inferred and statically checked; the public source shape
must not expose the existing `TopologyAt` cursor or `ApplicationRoute` product.
No registry, dynamic capability map, erased protocol, second mailbox, second
Driver, or atomic-template-specific host is permitted.

H139 shows why returning only `ApplicationHandle<Root::Protocol>` after root
activation is too late. A full `Start` message happens to infer every dynamic
type from its payload. The lawful `Stop { key }` message does not name worker or
activation types, so current stable Rust reports E0283 at `Application::new`.
Adding a turbofish, alias, explicit root type, fabricated `Recipient`, or dummy
worker field is rejected. The lifecycle actor's concrete
`DynamicLifecycle<Key, Worker, Plan>` protocol must supply those types during
role-first assembly instead.

This application-assembly change and the retirement-custody change are separate
Bombay responsibilities. Either may be implemented first, but both must use the
existing Driver and both must pass the final atomic integration probe.

H148 historically derived the target source equation independently of Bombay's
current root-first API: typed lifecycle and diagnostic capabilities enter one
pure root constructor, the six-policy `dynamic(entries, activation,
unexpected_exit, actor_drain, lifecycle, diagnostics)` call infers its key,
worker, and activation types. The real production construction test and five
command paths now own that contract; H589 removes the superseded facsimile while
retaining H148's compiler evidence in Git. Removing lifecycle provenance or
delivering `Stop` without the typed supervisor capability produced E0277/E0308
or E0282. Bombay must change its assembly order to supply this context; Behavior
Actors must not add an annotation, raw address, callback, or alternate
constructor to compensate for the present runtime API.

This is the required Bombay architecture. A stopped aggregate can retain
complete affine terminal values in its private final state, so returning only
environment state would still lose ownership. Bombay changes to carry both the
final concrete Behavior and the environment residual. Atomic implementation may
target this contract before the upstream patch lands; it must not add a local
workaround. Final end-to-end verification records the upstream revision that
supplies these five links. H03 authorizes neither a Behavior-side custodian nor
a second Driver, mailbox, task, or lifecycle service.

Bombay's application facade re-exports only the non-hidden semantic names listed
by [`atomic-actor-devx.md`](engineering/atomic-actor-devx.md). Interpreter code may import
the doc-hidden associated request, event, effect, and settlement types through
`behavior_actors::atomic`; none becomes a second application spelling.

## Required Bombay implementation

Bombay must realize the target through its sole Driver path. No Bombay source is
modified by this campaign, but upstream will make the following coherent
replacement. Applying only a subset leaves affine values unowned.

### `crates/bombay/src/interpret.rs`

1. Delete the `InterpretationError<C, S>` two-leg short-circuit sum. Expected
   capability rejection is data in `ItemSettlement`; it is no longer a commit
   error.
2. Implement the generic creation leg in two stages. First, one
   `InterpretItem<Creations<CreateChild<BehaviorAddr<C>, C>>, ...>` call either
   returns the complete batch untouched with `ChildNamespaceExhausted` or
   returns one ordered `Creations<RoutedCreation<BehaviorAddr<C>, C>>` value.
   Then `EstablishChild<Occurrence, C>` settles each routed child independently.
   Its output is fixed by the routed creation, not selected by Bombay.
   `Created` returns the exact established capability.
   `InitializationRejected` returns the current routed child and exact
   initialization error. `HostRejected` returns the current routed child,
   uninterpreted initialization `Actions`, and exact reason. Corruption retains
   the exact routed suffix through `InterpreterFault`.
3. Replace `CommitActions::commit`'s creation `for` loop and subsequent
   `InterpretSends::interpret(...).map_err(...)` with exactly one
   `actions.interpret::<_, B::Event, behavior::Here>(&mut self.capabilities)`
   call. That call already owns creation-before-sends order, lawful continuation,
   heterogeneous residuals, and the exact `become` verdict.
4. Return the resulting `ActionSettlement` to the existing host settlement
   admission. Do not discard it after checking `Complete`, and do not convert
   `Interpretation::Corrupt` into an error lacking the settlement value.
5. Keep `ActionInterpreter` as the one concrete capability product used by the
   existing Engine `Driver`; add no loop, mailbox, task, registry, or adapter.

### `crates/bombay/src/application_runtime.rs`

1. Delete the blanket `SendInterpreter` implementation and migrate every
   `InterpretRequest`, `InterpretDelivery`, `InterpretEstablishedDelivery`,
   `InterpretChildDelivery`, and `InterpretChildInput` implementation to the
   single static `InterpretItem<Item, RootEvent, Path>` ownership port.
   Runtime implementations do not select a settlement associated type;
   `ActionItem` and `SendSettlements` already fix the exact result vocabulary.
2. Each implementation must return its complete capability-specific sum:

   - async logical and exact delivery acceptance consumes the message and
     returns only its receipt; `Unknown` or `Closed` returns the original
     delivery and exact reason;
   - child delivery/input consults the exact occurrence binding. A rejected
     creation resolution produces `Blocked { item, prerequisite }`; genuine
     absence produces `Rejected { item, MissingChild }`; closed admission
     returns the original item;
   - timer scheduling uses only the selected Timers reason/token contract;
   - Observe uses only `SubjectExists`, `UnknownSubject`, and exact generation
     facts;
   - control ingress returns `ControlClosed(complete event)` instead of ignoring
     `ControlSender::send`;
   - child shutdown returns its exact accepted/already-stopping/not-established
     settlement and never reports success merely because a local fact was
     enqueued; and
   - `ReportToParent<Report>` returns the complete report when parent admission
     is closed.
3. Replace the legacy nonce-oriented `CreationResults` with a typed
   current-action prerequisite store keyed by the exact declared child
   occurrence plus `CreationId`. Beginning an action clears only the prior
   action transaction. Equal numeric runtime routes and distinct occurrences
   cannot satisfy each other's prerequisite.
4. Child hosting consumes `RoutedCreation` and returns `ChildCreationOutcome<C,
   Occurrence>` as its accepted receipt. Successful commit returns `Created`.
   A pure initialization error returns `InitializationRejected` with the routed
   child and exact error. Allocation or host rejection after initialization
   returns `HostRejected` with the routed child and still-uninterpreted
   initialization `Actions`. Post-commit initialization-action failure is not a
   creation rejection; it enters the created child's drain.
5. Remove every `let _ = control.send(...)` and equivalent ignored
   `ControlSender` result. Each becomes an owned item settlement or terminal
   residual.

### `crates/bombay/src/local.rs` and `crates/bombay/src/launch.rs`

`CommitActions` must expose the complete action settlement rather than
`Result<(), E>`. Initialization follows the same path as active turns. A child
is not published live until its initialization `ActionSettlement` has been
admitted or transferred; an initialization `Step::Stop` still settles its final
actions. A post-commit corrupt or rejected initialization effect drains the
installed incarnation and retains all residuals; it cannot be mapped back to
`CreationRejection` or roll back an accepted prefix.

For every creating behavior, host settlement owns one generic creation-entry
pass before ordinary ingress. In authored order it moves each entry into
`ChildCreationSettled<C, Occurrence>` and attempts the statically declared
creator system lane. This is one generic `Births<C>` capability, not an atomic
template adapter. A live creator receives the exact `ChildCreationOutcome`, including
the non-cloneable child and initialization value on rejection. If admission is
closed, the control send returns the complete root event;
`RecoverEvent<ChildCreationSettled<C, Occurrence>, Path>` recovers the exact
product and the lifecycle host retains it. A stopping creator may transfer the
entry directly without reopening its mailbox. `NoBirths` requires no
settlement lane or placeholder event.

The same mechanism must work for a non-atomic creating catalogue template and
for an atomic aggregate before Bombay accepts it. The Driver does not inspect a
variant, runtime type, or positional index: the concrete environment is
monomorphized over the creator event, child occurrence, and compile-time path.
The result of each attempted admission is an admitted receipt or the complete
still-host-owned creation product, never a discarded `ControlSender` result.

### Source action results

Creation uses the one generic action-settlement path, not the source-result
protocol used by emitted requests. The same Driver operation must also process
request items whose exact interpretation result returns to the emitting actor.
Stable-proxy owner operations and direct-pool assignments are the first two
unrelated request consumers.

Behavior supplies only static declarations and owned values:

- the concrete action owns its accepted receipt, rejection, prerequisite, and
  source identity;
- the generic source-action product fixes every returned input as the exact
  `SettledItem<Item, ItemSettlement<...>>` and preserves authored order;
- `ProxyOperation` uses that exact result. Its accepted value is
  `ProxyInputReceipt`, containing the creator-local child route, exact proxy,
  and operation correlation; rejection retains the complete operation;
- `AssignmentDelivery` uses the same exact result. Its accepted value contains
  exact worker and assignment correlation; rejection retains the complete
  assignment delivery;
- `PrepareWorkers` uses the same result for fixed supervision and direct-pool
  recovery. Its accepted value is one
  `WorkerPreparation` containing the exact worker-source authority, ordered
  selected roles, complete prepared submissions or partial worker rejection,
  and untouched suffix; and
- structural composition processes inner then owned lanes, matching the one
  declared interpretation order.

An action may not choose another result type or translate the generic
alternatives. Such a hook is a per-template settlement adapter: it can duplicate
the interpretation law and can silently discard the exact rejected or
unattempted action. Domain-specific receipts remain lawful as the accepted type;
later domain outcomes and application replies remain separate protocols.

Those declarations perform no delivery. Bombay must add one generic static
operation over the settlement products returned by `CommitActions`. For each
source result, that operation must:

1. retain ownership in the current actor host;
2. construct the statically selected current-actor system input;
3. submit it through the existing unbounded control capability;
4. remove it from host custody only after control admission succeeds;
5. recover the exact input from the returned closed-control event when
   admission fails; and
6. after the first closed admission, transfer the current value and every later
   value without trying to reopen the actor.

The Driver processes one admitted result turn and the complete transitive action
chain it produces before offering the next result from the earlier product.
Only after that chain is quiescent or transferred outward may it continue with
the next ordinary `User` communication. This must be an iterative queue owned by
the existing Driver; it is not a recursive call, detached task, second mailbox,
or second actor loop. The ordering does not claim termination or fairness: an
actor can continually produce more source results and starve ordinary traffic.

The static operation must compose through every generated named sends product
and `SendLayer` without Bombay matching product fields, variants, or structural
positions by hand. The generic source product supplies the exact input type once.
Neither FixedSupervisor, DynamicSupervisor, FIFO pool, nor keyed pool supplies a
runtime admission adapter. The same mechanism must compile with one unrelated
existing catalogue action before the Bombay migration is accepted.

### Exact Bombay and Timers changes for scheduling

Timer scheduling follows the same total action law; it is not a
FixedSupervisor exception. `ScheduleAfter` currently declares the later
`TimerElapsed` input but has no `ActionItem` settlement. The present Bombay
interpreter computes `Instant::now().checked_add(after)` and returns only unit
or `DeadlineOverflow`. The selected `bombay-timers 0.1.0` queue replaces an
existing equal key and panics when its internal generation or insertion
sequence exhausts. Those are current implementation constraints to replace,
not the architecture.

Behavior Actors must give both scheduling requests ordinary total contracts:

```text
ScheduleAfter:
  accepted = TimerScheduled { id, generation }
  rejected = DeadlineOverflow
           | QueueGenerationExhausted
           | QueueSequenceExhausted

ScheduleAt:
  accepted = TimerScheduled { id, generation }
  rejected = QueueGenerationExhausted
           | QueueSequenceExhausted
```

The two rejection sums remain distinct because an already absolute
`ScheduleAt` cannot incur relative-deadline overflow. Adding an impossible
alternative merely to share a type is prohibited. Generic `ItemSettlement`
already returns the complete original request for every rejection and preserves
independent later lanes. `TimerScheduled` is only the exact scheduling receipt;
the eventual `TimerElapsed` remains a later typed input and must never be
fabricated at acceptance.

Timers must change its existing queue operation to return a closed error before
changing counters, current-key state, or heap ownership. Bombay maps those two
queue errors without collapsing them, maps relative deadline overflow only for
`ScheduleAfter`, and returns the exact `TimerScheduled` receipt after insertion.
The current queue token remains Timers-owned scheduling authority; a Behavior
does not receive or recreate it. Poison recovery, memory exhaustion, and polling
are not new Behavior rejection alternatives.

FixedSupervisor privately selects a non-reused timer identity for each delayed
recovery and carries an explicit generation. Two overlapping delayed recoveries
therefore cannot use the same queue key, because the selected queue truthfully
treats equal keys as replacement. Immediate recovery emits no schedule and owns
no dummy timer. Recovery identity, timer identity, and release ordinal stay
distinct values; none is inferred from another or from product position.

Bombay implements these as the same generic static `InterpretItem` operations
used by unrelated timing templates. It must not inspect FixedSupervisor,
introduce a schedule adapter, create another timer queue, or return unit after
successful scheduling. Its Driver settles scheduling in the named action order,
admits the exact settlement through the normal source-result operation, and
retains it through the existing retirement product if actor admission closes.

### Exact Bombay changes for atomic diagnostics

Behavior Actors supplies one shared `DiagnosticAction<Route, Diagnostic>` for
the identical diagnostic law used by FixedSupervisor, DynamicSupervisor,
FifoPool, and KeyedPool. The action is exactly:

```text
Deliver { route, diagnostic }
| Terminal { diagnostic }
```

The route type selects the real rejection vocabulary statically:
`Recipient<P>` uses `LogicalDeliveryReason`, `EstablishedRecipient<P>` uses
`ExactDeliveryReason`, and the route-free `Infallible` alternative uses
`Never`. Bombay must add ordinary `InterpretItem` implementations for these
three concrete route families. It must not switch on a runtime protocol, erase
the diagnostic, or add an aggregate-specific interpreter.

For `Deliver`, Bombay consumes the route and diagnostic through the existing
logical or exact delivery capability. Accepted delivery returns
`DiagnosticAccepted::Delivered`. Rejection returns the complete original
`DiagnosticAction` and the exact selected delivery reason. Waiting for bounded
Communication capacity is backpressure and is not a rejection variant.

For `Terminal`, Bombay performs no delivery and returns
`DiagnosticAccepted::Terminal(diagnostic)`. The complete diagnostic therefore
remains inside the accepted action settlement owned by the current actor host.
When source admission is closed or the source stops in the same transition,
that settlement follows the normal child-to-parent-to-root custody path and
ends at the non-rejecting root custodian. Terminal transfer is not a log call,
detached task, second mailbox, or special Driver branch.

The first production consumer is FixedSupervisor's exact non-ready initial
proxy transition. It emits
`DiagnosticAction::Terminal(FixedDiagnostic::ProxyOutcomeFailed(...))` together
with `Step::Stop`. Bombay must settle the diagnostic before completing actor
retirement, retain the complete role/outcome payload, and then transfer the
remaining proxy child graph through the same retirement barrier. The aggregate
does not serialize that payload, log it as a substitute for custody, or keep a
second runtime coordination object.

The next production consumer is FixedSupervisor's committed-roster shutdown.
Its final `Step::Stop` retains the complete private `FixedShutdown` state. That
state includes ready workers and readiness values, locally cancelled initial
inputs, exact initial-input settlements, completed initial outcomes, proxy-stop
results, rejected proxy births, and any non-accepted shutdown operation reunited
with an exact proxy exit. Bombay must move that concrete stopped supervisor
value through the same retirement product. It must not inspect those private
alternatives, convert retained values to logs, or require FixedSupervisor to
publish a runtime-specific residual. A pending proxy creation remains an
ordinary previously emitted creation action: Bombay returns its exact generic
settlement. FixedSupervisor alone decides whether that means one newly committed
proxy must be shut down, an exact creation rejection means absence, or wrong
provenance must be returned unchanged. Recovery and deadline shutdown states
target this same generic retirement contract; they do not justify a
supervisor-specific Driver branch.

H119 realizes FixedSupervisor's deadline half. The first
`RetireActorGraphAfter` shutdown emits one ordinary relative scheduling action.
Its exact non-accepted result or its exact elapsed timer selects `Step::Stop`;
the private deadline state retains that exact cause while `FixedShutdown`
retains every unresolved member value. Bombay must therefore transfer the final
concrete supervisor and the environment's residual actions together. It must
not inspect the deadline state, synthesize per-member shutdown reports, or
discard the supervisor after extracting `Exit` or unit. The same generic
retirement product also covers an external worker-source operation that is
still resolving when the deadline forces retirement.

Bombay's required change is deliberately ignorant of FixedSupervisor's private
decomposition. `FixedShutdown` decides declaration-order roster completion,
`FixedShutdownMember` owns one member's remaining startup values, and
`ProxyStoppingMember` decides the exact shutdown-settlement/exit join. Bombay
only executes typed actions, returns their complete settlements, observes exact
child exit, and transfers the stopped concrete behavior to parent then root
custody. No in-memory supervisor coordinator belongs in Bombay.

`CommitActions` must preserve these receipts with every other item settlement.
`ActiveEnvironment::retire`, the Driver retirement barrier, and the root runner
must return or retain the terminal diagnostic exactly as specified by the
general custody changes in this document. A delivered diagnostic rejection is
already terminal settlement and must never be reintroduced into the aggregate
to emit a recursive diagnostic.

### Exact Bombay change for keyed customer rejection

AA-40 requires more custody than an ordinary reply delivery for one case only:
a synchronously rejected submission must preserve the original customer route
inside the rejected action while a clone targets the `KeyedOutcome::Rejected`
message. Behavior Actors represents that invariant with the doc-hidden
`CustomerDelivery<P>` action item. The public `KeyedOutcome` remains free of an
address generic, and neither the key nor payload is cloned.

Bombay must add one generic static `InterpretItem<CustomerDelivery<P>, ..>`
implementation alongside its existing logical and established delivery
implementations. It exhaustively handles the four concrete alternatives:

```text
Logical { delivery }
Established { delivery }
RejectedLogical { delivery, customer }
RejectedEstablished { delivery, customer }
```

The ordinary alternatives delegate to the corresponding existing logical or
established delivery capability. The rejected alternatives do the same while
holding `customer` untouched. Acceptance consumes the complete action and
returns unit, matching ordinary delivery. Logical rejection reconstructs the
same `CustomerDelivery` with the returned `Delivery`, original `customer`, and
`ReplyDelivery::Logical(reason)`; exact rejection does the corresponding
operation with `EstablishedDelivery` and
`ReplyDelivery::Established(reason)`. Interpreter corruption likewise returns
the complete reconstructed action and exact fault. The uninhabited delivery
prerequisite remains uninhabited.

This is one generic customer-action interpreter, not a KeyedPool branch or
settlement adapter. It does not inspect keys, roles, outcomes, bindings, pool
state, or aggregate type. Bounded Communication send capacity remains
backpressure. When actor admission is closed, the complete returned
`CustomerDelivery` travels in the existing environment residual and
parent-to-root custody path; the Driver must not reopen the stopped pool or
discard the original route. The research-local Bombay probe must cover logical
acceptance, logical rejection, exact acceptance, exact rejection, and source
retirement after rejection using the unchanged Engine Driver.

The operation returns a residual product. A residual owns every result not
admitted to the source, including the current result at closure and the exact
untouched suffix. That product joins the environment retirement value described
below. Bombay may not map it to a log, unit, a generic error, or a recreated
request. The root runner is the final non-rejecting owner.

This is the required upstream change. The atomic worktree does not modify
Bombay production source and does not add an in-memory coordination object to
StableProxy or either supervisor. H34 demonstrates the stable-Rust ownership
shape with distinct proxy and assignment results in both wrapper orders; the
real Driver and retirement proof remains a Bombay integration gate.

H38 adds one exact interpreter obligation. Bombay's environment statically
implements `PrepareWorkers` for the application's concrete method-free
`WorkerSource<Role, Worker, Plan>` declaration. It invokes that source outside
Behavior. H45 requires that action to expose each selected application role only
as `&Role`: it owns immutable private role names while the pending supervisor
state retains every unique member-role authority. Bombay must neither request
`Role: Clone` nor reconstruct a role from roster position. It returns the same
source authority and role names with every prepared, rejected, corrupt, or
unattempted result. `WorkerPreparation` contains only the two accepted domain
outcomes: a complete non-empty sequence of returned names paired with prepared
submissions, or an exact prepared prefix, rejected name/reason, and untouched
suffix. Capability rejection, interpreter corruption, and no-attempt remain the
generic settlement alternatives and are not repeated in a FixedSupervisor enum.
The action is handled by the same `InterpretItem` and source-admission machinery
used for proxy operations, assignment delivery, and unrelated catalogue
actions. Bombay adds no dynamic registry, callback inside the actor, factory
actor mailbox, erased worker envelope, or `FixedSupervisor` Driver branch. The
result enters the same generic source-admission queue and retirement product
described above.

The concrete static interpreter advances the request through
`source_and_role()`, then consumes one attempt with `accept(submission)` or
`reject(reason)`. Acceptance returns
`ControlFlow::Continue(PendingWorkerPreparation)` while a selected name remains
and `ControlFlow::Break(WorkerPreparation)` only after the last one. The initial
request alone implements the action traits; the pending value cannot be
re-emitted as fresh work after it owns a prepared prefix. This progression is
the arity proof: Bombay never supplies a role to an accepted result and performs
no separate length validation. If the Driver retires during preparation, its
typed environment residual owns the current `PrepareWorkers` or
`PendingWorkerPreparation` value together with the source, prepared prefix,
current name, and untouched suffix.

Every request also carries one private non-reused preparation ticket. Bombay
must preserve it by moving the request or accepted result through the existing
generic settlement machinery; it must not inspect, construct, compare, log, or
reconstruct the ticket. Exact matching is FixedSupervisor policy.

If shutdown overlaps an emitted preparation, Bombay must not report actor
retirement merely because ordinary mailbox admission is closing. The current
request or progress cursor remains in the typed environment until interpretation
produces its complete generic result. Open source admission returns that result
to the draining FixedSupervisor; closed source admission moves it into the same
typed retirement residual. In neither case may Bombay cancel the request,
recreate its source, call a FixedSupervisor-specific adapter, or treat successful
preparation as permission to issue replacement work. The aggregate owns that
late-result decision.

The clean-room FixedSupervisor now realizes its half of this contract. Its
shutdown member retains the private preparation expectation beside the exact
StableProxy drain, and the stopped aggregate retains the complete generic result
after admission. Bombay must therefore preserve the final monomorphized
FixedSupervisor value when interpreting `Step::Stop`; extracting only a unit
exit status would lose the source, rejection, prepared submissions, or
interpreter fault held there. The required Bombay change remains the generic
Driver retirement product described below. It is not a FixedSupervisor branch
and does not require Bombay to know `PreparationExpectation`.

The FixedSupervisor's concrete internal event includes the concrete typed
preparation result but does not carry or constrain the worker source in its
unrelated proxy variants. `PrepareWorkers<Source, Role, Worker, Plan>` is the
static source selector because that action type uniquely identifies its
associated result. The named sends product includes one `worker_preparations`
source-action lane. The generic source-admission operation injects that result
through the ordinary `EventIngress` contract; the Driver must not pattern-match
FixedSupervisor, special-case the lane, or add a callback. The declared fixed-supervisor lane
order places worker preparation after proxy creation/observation work and before
proxy input, scheduling, lifecycle, reply, and diagnostic work. A later
production slice must compile this exact product before automatic recovery is
claimed.

FixedSupervisor proxy operations independently use `Here` as their static
same-actor source. Bombay therefore admits their exact generic
`ProxyOperation<Here, Worker, Plan>` result through ordinary `EventIngress`; the
operation does not use the aggregate event type as a recursive selector. The
worker-preparation result remains a distinct input type for that same current
actor, with no positional routing or runtime lookup. Bombay's static generic
admission implementation selects the actor through the action/result type pair;
it does not inspect the action value or infer a runtime destination.

Direct pools use this same interpreter operation for automatic recovery with a
non-empty sequence containing exactly one semantic role. The pool retains the
recovery decision and role authority while the action owns the source and role
name. Bombay gains no pool branch: the same static request progression returns
the source, name, and one `WorkerSubmission` or the exact rejection. Initial
pool construction remains separate and may invoke its factory before a
`Behavior` exists. A pool never stores or calls that factory from a transition.

### Engine retirement and root custody

`ActiveEnvironment::retire`, the local inbox drain, `Driver`, `run_with`, and
the application result must carry one typed retirement product through the
existing barrier. That product owns both the final monomorphized Behavior and
the environment residual; it does not inspect or erase the Behavior. Parent
rejection transfers the complete terminal value outward. The application root
owns a non-rejecting custodian and releases the barrier only after that product
is empty. This is the H03 contract as completed by H28's final-Behavior link,
not a new service: retirement remains part of the same `Driver` future.

### Required upstream verification

- compile every concrete `InterpretItem` implementation without a universal
  error type;
- replay H07 rejection, blocking, corruption, and wrapper-order suites against
  `ApplicationCapabilities`;
- prove creation semantic rejection continues to an independent timer or
  delivery while only its exact child operation is blocked;
- prove two heterogeneous creation alternatives return branch-preserving
  receipts and exact rejected requests;
- prove `ChildCreationSettled` returns a non-cloneable child through one live
  creator and through closed creator admission into host custody, using the
  same generic code for an unrelated creating template;
- prove stopping initialization settles final actions before publication or
  retirement;
- inject control closure at every parent/root transfer and recover the exact
  value at the root custodian; and
- run the unchanged single Engine `Driver` through the retirement barrier with
  no detached work and no ignored must-use result.
