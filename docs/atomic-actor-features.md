# Exhaustive feature catalogue for atomic actor templates

This document records the observable requirements that the replacement actor
templates must satisfy. It intentionally makes no decision about Rust types,
builders, internal state layout, shared helpers, or implementation strategy.
Those decisions belong in a later design document.

The catalogue was assembled from the existing supervisor, proxy, pool,
interpreter, model, property, exhaustive, and fuzz contracts. Existing
implementation names are not requirements unless the behavior itself appears
below.

The atomic templates in scope are:

1. stable worker proxy;
2. fixed supervisor;
3. dynamic supervisor;
4. FIFO worker pool; and
5. keyed worker pool.

Every feature group below has a stable identifier in brackets. Individual
bullet requirements are referenced by one-based ordinal: the first bullet
under `[PX-REPLACE]` is `PX-REPLACE-01`, the second is `PX-REPLACE-02`, and so
on. Adding a requirement appends a new ordinal; existing IDs are not reused.

## Requirements shared by every template

### [SH-ACTOR] Actor boundary

- The template is one concrete, statically dispatched `Behavior` fold.
- One input is processed at a time.
- A successful transition returns all communications, fresh creations, and
  the next behavior or termination decision through `Actions`.
- Initialization is a pure fold whose actions are interpreted before mailbox
  inputs.
- No template performs delivery, allocation, observation, scheduling, clock
  access, or shutdown directly.
- No template depends on an executor, transport, global registry, runtime
  query, ambient context, or hidden side channel.
- No template wraps an arbitrary application behavior merely to gain
  lifecycle machinery.

### [SH-STATIC] Static protocol and capability safety

- Public commands, private parent-to-child inputs, child-to-parent reports,
  runtime facts, effect lanes, errors, phases, reply recipients, child
  behavior alternatives, and creation products are statically known.
- No trait object, `Any`, downcast, erased message, runtime protocol lookup,
  string key, serialization envelope, or unsafe type escape is permitted.
- A semantic role or dynamic management key is never treated as an actor
  address, established capability, creator-local nonce, or proof of
  freshness.
- A creator-local nonce is never treated as an actor identity or proof that a
  child was installed.
- Every send to an exact installed actor uses an established capability or an
  exact creator-local child binding, as appropriate. A stable proxy never
  exports its worker recipient; owner-visible worker incarnation data is
  opaque non-routable evidence.
- Heterogeneous workers are represented by one closed application-defined sum
  whose variants remain statically dispatched.

### [SH-CREATE] Fresh creation and provenance

- Every new actor incarnation is staged as a fresh creation.
- A nonce collision, allocation exhaustion, address collision,
  initialization rejection, installation failure, or commit failure never
  overwrites an existing binding.
- Initial birth and replacement incarnation remain distinct provenance.
- Replacement provenance is attached when fresh creation commits, but a
  successful `Restarted` outcome is reported only after that exact replacement
  incarnation becomes ready.
- A rejected replacement never emits a successful restart or birth outcome.
- Same-action dependent observations or sends rely on the documented
  creation-before-send interpretation order and retain typed rejection.
- Converted or generated nonce values cannot silently repeat a previously
  issued creator-local nonce.
- Exhausted nonce, attempt, operation-ticket, timer identity, timer generation,
  assignment, and job sequences have typed outcomes; production panics are not
  accepted.

### [SH-READY] Installation, activation, and readiness

- Committed actor creation proves installation, not application readiness.
- The runtime first runs the concrete behavior's pure `init` fold exactly once.
  If that fold rejects, no host is committed and none of its returned effects
  exists. Activation never calls `init` a second time.
- After a successful init fold, the runtime establishes provisional/exact host
  ownership and commits an installed-but-not-ready incarnation before
  interpreting any initialization `Actions`.
- Initialization actions may partially succeed. Their later rejection cannot
  reject or erase the already committed installation and cannot pretend an
  earlier delivery, creation, or state transition did not occur.
- Host/allocation rejection before commit, initialization-fold rejection,
  initialization-effect rejection after commit, and initialization stop after
  commit are distinct typed pre-readiness outcomes. Post-commit rejection or
  stop drains the exact installed incarnation and issues no activation permit.
- Every worker definition selects one concrete statically dispatched
  activation plan. An immediate plan performs no external work but still
  resolves readiness only after behavior initialization succeeds.
- Committed installation yields an actor-retained exact installed-but-closed
  incarnation, initialization-settlement ownership, and the same concrete
  activation plan moved into creation. Only complete successful initialization
  settlement yields that plan plus the distinct one-shot activation permit
  tied to the incarnation; no plan is cloned or rediscovered.
- The activation interpreter accepts a typed `BeginActivation` request owning
  an opaque attempt, the one-shot installation permit, and the concrete plan.
  The actor's activating state retains the exact incarnation and attempt
  correlation, not the moved permit or plan.
- Start rejection returns the complete unaccepted request through the
  surviving action settlement; accepted activation eventually produces one exact
  `ActivationResolved` fact: ready or activation rejected.
- Fixed and dynamic supervisors and both pools select a positive maximum
  number of unresolved activation authorizations. Authorization means the
  owner's decision to let one worker progress toward `Ready`; that is the one
  user-facing counting law. A supervisor deliberately reserves its ticket
  conservatively when it emits the opaque proxy install input, while a direct
  pool reserves when it emits `BeginActivation` after initialization settles.
  A proxy has a structural local limit of one. Reserved definitions beyond an
  owner's bound remain actor-owned and have not crossed that template's
  authorization boundary.
- Readiness is correlated to the exact installed incarnation and activation
  generation; a role, key, nonce, or timestamp alone is insufficient.
- A supervisor does not publish `Started` or `Restarted`, a proxy does not
  route a command, and a pool does not assign a job until the exact worker is
  ready.
- Initialization actions and their settlement complete before an
  immediate-ready outcome or external activation request can make the worker
  routable. Failure prevents readiness but never reverses installation.
- Ready, rejected, stopped, and shutdown facts that may race are joined once
  by the actor that owns their exact correlations.
- Duplicate, stale, foreign, or contradictory activation facts preserve all
  authoritative inputs and cannot make an incarnation ready.
- Asynchronous hydration, I/O, and wall-clock waiting remain outside the pure
  behavior fold; only their typed completion fact enters the fold.
- Activation rejection is distinct from creation rejection and ordinary
  terminal outcome throughout recovery, diagnostics, and shutdown.
- Cancellation returns a definition only before the exact install/create
  action that transfers it. The interval after that transfer but before
  `BeginActivation` is already logical cancellation and cannot claim to return
  the definition. After activation emission, cancellation additionally closes
  publication irrevocably but does not claim that external hydration was
  physically cancelled.
- A ready fact arriving after logical cancellation is a typed
  `ReadyAfterCancellation` outcome. The incarnation is never advertised and
  is drained exactly once.
- A rejection arriving after logical cancellation resolves the attempt
  without fabricating readiness and retains the exact rejection.
- Forced shutdown may retire the actor-side attempt while external activation
  remains in flight. The activation settlement owner survives the actor and
  owns any later ready/rejected fact; a stopped actor is never its destination.

### [SH-CORRELATE] Fact correlation and transition integrity

- Every creation, stop, shutdown rejection, timer, completion, and child
  report is correlated to its exact pending operation and incarnation.
- A stale, duplicate, foreign, malformed, wrong-provenance, or contradictory
  fact cannot advance, revive, overwrite, or partially mutate current state.
- When rejection must return owned input or authoritative facts, the complete
  values are returned without reconstruction or cloning used to escape
  ownership.
- Preparation and validation finish before the single state commit.
- A failed multi-member decision performs no partial budget charge, member
  reservation, replacement send, assignment, dequeue, or binding change.
- Effect products use semantic lane names and append every lane exactly once
  in lane order.
- Adding an unrelated outer behavior composition cannot drop, duplicate,
  reorder, reinterpret, or require callers to recount a structural path for
  the template's events or effects.

### [SH-DELIVERY] Delivery acceptance and rejection

- Emitting one delivery in `Actions` means one delivery attempt, not proof
  that the destination received or processed the value.
- Logical, established, child-local, and structural-parent routes remain
  distinct concrete route kinds; a template never upgrades or downgrades one
  implicitly.
- Every named effect product projects one closed rejection sum with a variant
  per delivery lane. Each variant owns the complete payload, route kind, and
  capability-level reason.
- Every emitted item has one ordered settlement: accepted, rejected, or not
  attempted because an exact declared prerequisite rejected. A settlement is
  a named per-lane product, so several failures in one action cannot collapse
  into one sum value or discard later payloads.
- A rejected prerequisite suppresses only its dependent effects and returns
  them untouched with an opaque reference to the one authoritative
  prerequisite settlement. The rejection payload is not cloned into every
  dependent item. Independent later lanes are still interpreted in their
  documented order.
- Interpreting an action establishes a typed settlement owner before the first
  delivery attempt. Settlement outlives the source turn and source actor.
- Every lane declares a primary rejection consumer and the surviving
  settlement owner used if that consumer is stopped or unreachable.
- A continuing actor may receive its typed rejection as a later runtime fact;
  the settlement owner retains fallback ownership until that fact is admitted.
- The Driver gives the current action's transitive settlement chain priority
  on the typed system lane. It admits each fact to the live source or transfers
  it to the surviving host; the next ordinary user communication is admitted
  only if that chain quiesces or transfers outward. Thus unresolved batches do
  not accumulate across separate user turns.
- Settlement discharge is local to that source/host chain and does not impose
  a global actor-system barrier. Actions emitted while processing a settlement
  fact are themselves settled before the next ordinary source communication.
- This priority rule only prevents accumulation across separate ordinary user
  turns. A transitive settlement chain may be unbounded or non-terminating; the
  iterative Driver queue prevents stack growth but proves neither memory
  bounds, fairness, nor eventual return to ordinary traffic.
- The application root remains the live owner of transferred settlement until
  every external activation, late lifecycle fact, and required exact drain has
  resolved. A final runner result is not produced while residual ownership
  exists.
- A delivery emitted by a stopping actor, or rejected after the actor stops,
  goes directly to settlement. Returning it to the emitter is forbidden.
- A delivery rejection cannot roll back the already committed source-actor
  transition or pretend that the corresponding state change never occurred.
- Automatic retry is forbidden unless the template exposes and applies an
  explicit retry policy; otherwise rejection is a typed terminal or
  diagnostic outcome.
- A closed exact endpoint, unavailable logical destination, rejected child
  input, rejected structural-parent report, and late reply are distinct where
  their recovery choices differ.
- Long-lived customer and lifecycle destinations are retained by the actor
  that selected them. A worker receives no forgeable same-protocol capability
  whose substitution could redirect a pool or supervisor outcome.
- Delivery-rejection tests exercise both the source fold and the interpreter
  return path; inspecting the emitted `Actions` value alone is insufficient.

### [SH-DIAGNOSTIC] Diagnostic disposition

- Every template that can produce an operational diagnostic has exactly one
  semantic disposition: `DeliverTo(Route) | Terminate`. This is a domain sum,
  not authorization for a Rust `Diagnostics<Route>::Terminate` value whose
  unused `Route` requires annotation. The builder must infer either a
  route-bearing concrete state or a route-free terminal state. An
  owner-created proxy is fixed to `DeliverTo` its exact structural parent and
  needs no second builder route.
- `DeliverTo` attempts one delivery to the concrete logical, exact, or mixed
  route. It requires no second callback or fallback route.
- Rejection of a diagnostic delivery produces one terminal
  `UndeliverableDiagnostic` settlement containing the diagnostic and route
  reason. It never emits another diagnostic delivery.
- `Terminate` performs no diagnostic delivery. The fold emits the first
  diagnostic through a named terminal-settlement request in `Actions` and
  selects `Step::Stop`; the interpreter settles that explicit value with the
  surviving lifecycle host.
- A root host retains terminal or residual settlement in the live application
  run future and returns a final value only after it is settled; a child host
  transfers through the child's typed lifecycle settlement. Neither path
  depends on the stopped actor's mailbox.
- If an intermediate host also stops, the exact settlement transfers outward
  through its statically known host chain without reinterpretation. It cannot
  remain forever in an unreachable host.
- `Terminate` stops in the diagnostic-producing turn after preserving its
  other actions. Rejected diagnostic delivery stops a still-live source when
  its settlement fact is admitted; if it cannot be admitted, the host/root
  terminal settlement is the final disposition.
- Diagnostics are never silently logged, dropped, retried, or converted to a
  coarse `Crash`.
- A template without an external diagnostic consumer uses `Terminate`; it
  never supplies a dummy recipient or no-op callback.

### [SH-SHUTDOWN] Shutdown

- Shutdown is a typed input accepted in every live phase.
- The first shutdown request begins one terminal drain; later requests do not
  duplicate effects.
- Every owned established child is asked to stop at most once.
- Pending child creation is resolved before the owner decides whether a child
  must be stopped or is already absent.
- Rejected pending creation counts as drained and creates no fabricated child.
- A child installed while shutdown is pending is asked to stop exactly once.
- An exact shutdown rejection retains its capability-level reason.
- Delayed work or recovery retained at shutdown is cancelled without being
  emitted later; its exact timer fact can be consumed without reviving it.
- Termination occurs only after every owned child is resolved and drained.
- The final action retains all required reports and shutdown effects before
  selecting normal termination.
- Every owner chooses one `ActorDrainPolicy`: `WaitForActorGraph` or
  `RetireActorGraphAfter { deadline }`. The former may wait indefinitely; the
  latter bounds actor-graph retirement only. Neither is a process-exit policy.
- Deadline-retirement facts use interpreter-authored monotonic time and exact
  timer identity/generation. Rejection of deadline scheduling immediately
  forces the same typed host transfer with scheduling rejection as its cause;
  it never degrades `RetireActorGraphAfter` to unbounded waiting.
- Deadline retirement preserves a typed forced-retirement fact containing the affected
  child, incarnation, phase, outstanding ownership, and cause.
- Deadline retirement atomically transfers that outstanding ownership to the surviving
  lifecycle host. The actor-side member then counts as drained; exact late
  facts and rejected effects settle with the host and cannot revive the actor.
- `RetireActorGraphAfter` bounds actor-graph drain, not an uncancellable external
  activation. The root run future remains pending in a typed residual-drain
  state until transferred work resolves; an actor-side forced summary is not
  the final application result.
- Forced retirement never fabricates an accepted shutdown, normal child stop,
  successful activation, completed job, or completed restart.

## 1. Stable worker proxy

### [PX-IDENTITY] Identity and ownership

- One proxy provides one stable public worker-protocol identity.
- At most one worker incarnation is current behind that proxy.
- Every worker incarnation is a fresh child creation.
- The proxy alone owns the mapping from its internal worker nonce to current
  incarnation state and the routable worker recipient. Parent outcomes expose
  only opaque non-routable incarnation evidence.
- A replacement never replaces an actor at an existing address.

### [PX-INIT] Initialization and installation

- Initialization is explicit and cannot run twice.
- The matching committed-creation fact proves only installed-but-not-ready;
  initialization effects still require exact settlement, and the initial
  worker remains unroutable until its later readiness outcome.
- Initial creation is tagged as ordinary birth, never inferred replacement.
- Worker creation is observed exactly once.
- Worker termination is observed for the exact created incarnation.
- A rejected initial creation leaves the proxy live but empty unless shutdown
  is already pending.
- A worker stop that becomes observable before its creation-resolution fact is
  either joined correctly in either order or made impossible by the later
  architecture; it can never result in a dead worker becoming ready.
- A rejected creation and an authoritative stop for the supposedly rejected
  incarnation form a typed contradiction retaining both facts.

### [PX-ACTIVATE] Worker activation

- Successful worker creation moves the proxy to an
  installed-and-initializing phase. Only complete initialization-action
  settlement yields the exact activation permit and moves it to activating;
  even immediate activation cannot bypass that phase.
- A reported-ready fact makes only the exact installed incarnation routable.
- Activation rejection drains the exact installed incarnation, then leaves
  the stable proxy live and empty and returns the exact rejection plus terminal
  or forced drain fact to its owner.
- Rejection of the `BeginActivation` action preserves the complete unaccepted
  permit, plan, and request reason, drains the installed incarnation, and is a
  distinct proxy outcome from rejection by an accepted activation plan.
- Initialization-fold rejection and host rejection leave the stable proxy
  live and empty without interpreting initialization effects. Post-commit
  initialization-effect rejection first drains the exact installed
  incarnation, then reports its rejection plus terminal-or-forced drain fact
  and leaves the proxy empty. All are distinct from activation rejection and
  none starts activation.
- Worker stop before readiness reports one stopped-during-activation outcome;
  it is never reported as `Started` followed by an inferred stop.
- Replacement is reported as completed only after the replacement
  incarnation is ready, not merely installed.
- Shutdown during activation resolves the exact activation/stop join and does
  not publish a transient ready capability.
- A command received while installed or activating is returned complete as
  unavailable.
- “Atomic installation/activation outcome” means one proxy fold commits one
  state and one `Actions` value; it makes no cross-actor atomic-delivery claim.

### [PX-ROUTE] Command routing and unavailability

- In the ready phase, one admitted command produces one delivery to the exact
  current worker incarnation.
- No command is delivered to a prior, pending, rejected, stopped, stopping, or
  shutdown worker incarnation.
- In every non-ready live phase, the complete original sender and command are
  returned exactly once as expected unavailability.
- Unavailability distinguishes at least initial installation, replacement,
  empty, stopping-for-replacement, and shutdown phases sufficiently for an
  owner to choose retry or rejection without guessing.
- Creation rejection is expected unavailability, not an actor crash.

### [PX-REPLACE] Replacement

- A replacement request carries the complete new worker definition.
- Only one replacement attempt is owned at a time.
- An overlapping replacement is rejected with the submitted definition
  intact.
- If a worker is running, replacement first requests shutdown of that exact
  incarnation.
- If the proxy is empty after a known prior incarnation, replacement can stage
  a fresh successor immediately.
- A replacement request reserves all locally fallible correlation state before
  asking the old worker to stop.
- The fresh creation is explicitly tagged with the exact incarnation it
  replaces.
- Worker-stop reporting and replacement creation effects are both preserved
  when emitted by the same transition.
- A rejected replacement leaves the proxy empty and preserves the last
  successfully installed incarnation as provenance for a later attempt.
- A successful replacement becomes routable only after its exact activation
  resolves ready; committed creation alone remains non-routable.
- Stale creation and stop facts cannot affect a later attempt.

### [PX-STOP] Worker termination

- A spontaneous stop of the exact current worker is reported once and makes
  the proxy empty.
- A stale or duplicate stop is rejected without state change.
- A stop expected by replacement is reported once and cannot initiate a
  second replacement.
- Worker stop and creation resolution are order-independent where the runtime
  can deliver them in either order.

### [PX-SHUTDOWN] Proxy shutdown

- Shutdown while ready asks the exact worker to stop and waits for its stop
  fact.
- Shutdown while worker creation is pending waits for the creation result.
- If that creation commits, the exact worker is stopped once; if rejected, the
  proxy terminates without a shutdown request.
- Shutdown while replacement is stopping the old worker cancels the not-yet-
  created replacement and continues draining the old worker.
- Shutdown while replacement creation is pending resolves that creation and
  drains any committed incarnation.
- Shutdown after an already observed worker stop never sends to the dead
  incarnation.
- A shutdown rejection is accepted only for the exact pending worker shutdown
  and preserves its typed reason.
- The proxy terminates normally only after it owns no live or unresolved
  worker.
- Rejection of a parent report during drain preserves the complete report and
  route rejection through the configured diagnostic/error path; it does not
  reopen the proxy or repeat worker shutdown.

## 2. Fixed supervisor

### [FS-BUILD] Definition and construction

- Construction requires a worker factory, at least one semantic child role, a
  complete recovery policy, and a topology-failure reaction.
- An incomplete definition cannot implement `Behavior` or call `build`.
- Duplicate semantic roles are rejected before initialization.
- An empty fixed fleet cannot be built.
- Initial factory rejection is typed and occurs before any initialization
  action is emitted.
- One factory constructs both initial and replacement workers for each role.
- A homogeneous fleet uses one worker behavior type.
- A heterogeneous fleet may use one closed worker-behavior sum only when every
  variant implements the same public protocol. Different public protocols are
  separate typed supervisor instances unless a later law proves a
  capability-safe heterogeneous product; they are never collapsed to one
  weak common envelope.
- Application actors and application births remain outside supervisor
  ownership; the supervisor never adopts unrelated child occurrences.
- Lifecycle publication is an explicit route-selected capability, not a
  hard-coded logical recipient. Logical, established, and mixed routes retain
  their existing static hosting obligations.
- A supervisor whose children are autonomous can be built without a lifecycle
  consumer and without supplying a no-op route; query and diagnostic laws must
  still remain truthful.
- Construction selects an activation contract, activation-authorization limit,
  `ActorDrainPolicy`, and diagnostic disposition (`DeliverTo` or `Terminate`).
  These are semantic requirements, not optional callbacks; lifecycle-event
  publication remains the independent optional capability.
- When lifecycle publication is selected, `Started` and `Restarted` carry the
  exact stable proxy capability needed to communicate with the ready role.
  They never carry a worker recipient; worker provenance remains private
  supervisor correlation evidence.

### [FS-TOPOLOGY] Stable child topology

- Exactly one stable proxy is staged for every declared role, in declaration
  order.
- Every staged proxy has exact creation and termination observation.
- A role is associated with its proxy through supervisor-owned state, never by
  converting the role into a nonce.
- A role becomes available only after its proxy is committed, its
  install/replace input is accepted, and that exact proxy emits its single
  atomic `Ready` outcome.
- After proxy commit, the supervisor may retain the worker definition in a
  `WaitingForActivationAuthorization` state. It does not send the install
  input until one global activation slot is reserved for that role.
- The supervisor owns at most its configured positive number of unresolved
  proxy install/replace operations. Waiting roles are authorized in declaration
  order and retain their definitions without entering the proxy.
- The exact established proxy capability is retained and can be reported to
  the supervisor's owner.
- Stable-proxy creation rejection cannot fabricate a worker, replacement, or
  availability outcome.
- Stable-proxy stop retires only the matching role unless the configured
  failure reaction stops the complete supervisor.
- A stopped proxy is never sent another worker or shutdown command.
- Facts for application children, foreign proxies, or stale proxy generations
  are returned or rejected complete without changing owned topology.

### [FS-FACTS] Worker lifecycle facts

- Worker readiness/stop reports are accepted only from the exact owned proxy
  and exact pending supervisor operation. Replacement outcomes additionally
  match their nested prior-incarnation evidence; no supervisor recovery ticket
  is injected into the proxy protocol. Worker creation, initialization, and
  activation progress remain proxy-private; the supervisor consumes only the
  proxy's atomic outcome.
- Initial worker creation rejection is reported once and leaves no routable
  worker.
- Replacement creation rejection is reported once and is not called a
  completed restart.
- Duplicate, stale, malformed, foreign, and wrong-provenance reports do not
  advance the role.
- Authoritative facts that may arrive in either order form an exactly-once
  join. Install-action settlement is discharged before any later proxy report,
  so that specific order is causal rather than guessed.
- A contradictory stable-creation, worker-creation, worker-stop, or proxy-stop
  combination returns all conflicting facts without consuming the valid side
  of the join.
- Expected command-unavailability reports from an owned proxy are relayed to
  the authored owner with role, sender, phase, and command intact.

### [FS-OPERATE] Status, capability, and diagnostics

- The public management protocol provides a typed status query without
  exposing private member-state variants or structural paths.
- A status snapshot identifies every declared role and its supervisor-visible
  semantic phase: creating proxy, waiting for activation authorization,
  awaiting proxy outcome, ready, empty, recovering, stopping, or retired. It
  does not invent worker-creation or activation progress the proxy never
  reports.
- A ready role's snapshot or capability query may return its exact stable
  proxy capability; non-ready roles return a typed availability result rather
  than a sentinel or absent capability paired with flags.
- Request replies and durable lifecycle/diagnostic publication are separate
  route capabilities. A temporary query caller never silently becomes the
  owner of later events.
- Omitting lifecycle-event publication never omits diagnostics. If diagnostic
  delivery is rejected, `[SH-DELIVERY]` requires the complete diagnostic to
  remain owned by the typed rejection path; it is never discarded or
  reclassified as a successful lifecycle event. The exact return-state
  equation remains a shared delivery-design blocker.
- Querying does not mutate recovery budget, role order, readiness, or child
  ownership.

### [FS-ELIGIBILITY] Restart eligibility

- Permanent workers are eligible after normal or abnormal termination.
- Transient workers are eligible only after abnormal termination.
- Temporary workers are never automatically restarted.
- Ineligibility retires or leaves empty according to the documented topology
  policy and is not reported as budget failure.
- A supervision-failure exit is classified as abnormal for transient policy.

### [FS-STRATEGY] Restart strategies

- One-for-one selects only the triggering role.
- One-for-all selects every currently restartable role.
- Rest-for-one selects the trigger and currently restartable roles declared
  after it.
- Selection uses declaration order and an immutable pre-transition snapshot.
- A role that is unresolved, retired, stopping, or already owned by an
  admitted replacement cannot be selected as though it were ready.
- An initially unresolved member required by a coordinated strategy is
  retained in the recovery decision until its exact proxy outcome resolves the
  readiness prerequisite; it is never silently excluded.
- Overlapping failures and duplicate worker stops cannot duplicate a selected
  replacement or charge a second decision accidentally.
- Every selected replacement preserves the concrete worker variant for its
  role.

### [FS-BUDGET] Restart budget

- The budget applies to complete admitted replacement decisions, not
  individual mutation steps.
- A configured maximum of zero denies every otherwise eligible decision.
- The time window boundary is inclusive.
- Evidence older than the window is pruned before admission.
- Future-dated evidence is not incorrectly discarded by an out-of-order fact;
  interpreter-authored monotonic timestamps that regress are typed rejection.
- Rejected decisions do not partially charge the budget.
- Atomic multi-role decisions are either admitted and charged once or rejected
  without charge.
- Stored budget evidence remains bounded by the active window.
- Aged-out attempts make capacity available again.
- Denial reports the attempts in the window, requested replacement count, and
  configured maximum.
- Every budget timestamp is supplied by the interpreter's monotonic clock at
  the authoritative lifecycle fact; workers and callers cannot choose it.

### [FS-TIMING] Restart timing

- Immediate timing satisfies only the timer prerequisite. A replacement input
  is emitted in the triggering transition only when readiness and activation
  authorization are also satisfied; otherwise the prepared definition waits.
- Constant, linear, and exponential delays are one-based, checked, bounded,
  and reject zero or inverted configuration.
- Delay arithmetic overflow is typed.
- Each role's admitted recovery count advances with checked arithmetic. The
  count has no implicit reset; exhaustion is a typed denial rather than wrap,
  saturation, panic, or a fabricated delay overflow.
- A delayed decision retains every prepared replacement definition until its
  exact timer fires or shutdown cancels it.
- Timer identity and generation are exact correlation data.
- A stale, duplicate, wrong-generation, or foreign timer cannot release a
  decision.
- Timer arrival, readiness resolution, and activation authorization are three
  independent prerequisites. Their closed eight-state sum joins every
  admissible order exactly once; no pair of booleans or incomplete four-state
  join represents it.
- An activation slot is reserved atomically with the replacement input and is
  released only by that proxy operation's atomic terminal outcome or by
  transferred forced-retirement ownership.
- A delayed multi-role batch emits each selected replacement once and in
  declaration order.
- Denied delayed replacement leaves the affected role retired or empty rather
  than stranded in an installing phase.

### [FS-ATOMICITY] Factory and decision atomicity

- Every selected worker definition is prepared before the first replacement
  command or member-state commit.
- If any factory call rejects, no selected peer is partially reserved,
  stopped, replaced, or budget-charged.
- Factory rejection identifies the semantic role and retains the typed factory
  error.
- Replacement preparation never requires a no-op factory, callback, selector,
  or placeholder for an unaffected template.

### [FS-FAILURE] Failure reaction

- `RetireMember` retires the exact role whose topology can no longer be
  preserved and leaves unrelated live roles operating.
- `StopSupervisor` begins an orderly drain of every owned stable proxy and
  ultimately selects the advertised terminal result.
- Budget denial, backoff exhaustion, factory rejection, stable proxy death,
  and worker creation rejection each enter the configured reaction through a
  typed diagnostic.
- The diagnostic is emitted before any terminal verdict in the same action.

### [FS-SHUTDOWN] Fixed-supervisor shutdown

- Shutdown drains only supervisor-owned stable proxies.
- Unrelated application children and their creation facts remain untouched.
- Both established and still-installing proxies are included in the drain.
- A pending proxy installation is resolved without duplicate shutdown.
- Delayed replacement batches are cancelled and never emitted after shutdown.
- Final pending creation rejection completes shutdown even when ordinary
  failure policy would retire only one child.
- Shutdown rejection preserves the exact proxy and rejection reason.
- The final proxy stop selects normal supervisor termination after preserving
  all reports in that transition.

## 3. Dynamic supervisor

### [DS-BUILD] Definition and protocol

- Construction requires an explicit policy for unexpected worker exit and a
  maximum retained-entry capacity.
- No fixed-child list, fixed factory, restart strategy, restart budget, or
  placeholder callback is required.
- The public management protocol has typed start, stop, replace, and query
  commands.
- Every command names a semantic management key and carries its concrete
  logical, established, or mixed reply route.
- Start and replace carry complete owned worker definitions.
- The management key is separate from every creator-local proxy and worker
  nonce.
- Construction selects entry capacity, activation, activation-authorization
  limit, unexpected-exit, `ActorDrainPolicy`, one durable lifecycle route, and
  one diagnostic disposition. No temporary request recipient becomes the durable
  lifecycle owner, and no no-op callback or dummy route represents diagnostic
  termination.

### [DS-START] Start

- Start acceptance is distinct from committed installation.
- A new key reserves one dynamic membership entry and stages one fresh stable
  proxy.
- `StartAccepted` identifies the management key but does not claim the proxy
  or worker is installed.
- `Started` is emitted only after the exact stable proxy capability is
  committed and the initial worker reports ready.
- `Started` returns the exact established stable proxy recipient.
- A duplicate key returns the submitted worker unchanged with
  `AlreadyExists`.
- Start while global shutdown is in progress returns the worker unchanged with
  `ShuttingDown`.
- Rejected stable-proxy creation returns a typed start failure and retires the
  reserved entry.
- Rejected initial-worker creation returns a typed start failure and drains or
  retires the empty proxy without fabricating availability.
- The start reply route is used only for the start request. The one durable
  lifecycle route selected on the supervisor builder owns every later
  realization, unexpected-exit, and proxy-unavailability event. Individual
  start requests cannot replace or transfer that owner.

### [DS-QUERY] Query

- Query returns `Unknown` for an unknown key; absence is a semantic outcome,
  not `None` paired with another phase field.
- A known key returns one exhaustive public phase: reserved, creating proxy,
  waiting for activation authorization, awaiting proxy outcome, ready, empty,
  stopping, replacing, cancelling, draining, or retiring. The supervisor does
  not expose worker-creation or activation progress absent from `ProxyReport`.
- Query never changes membership or lifecycle state.
- Query during global shutdown reports the exact draining state rather than a
  guessed ready value.

### [DS-STOP] Stop

- Stop of an available or empty key is acknowledged as accepted and requests
  shutdown of that exact stable proxy.
- Stop completion is reported only after the matching proxy exit.
- Unknown, already retired, already stopping, or otherwise unavailable keys
  return typed rejection without state change.
- Exact runtime shutdown rejection is returned with its capability reason and
  does not masquerade as admission rejection.
- Stop waits for the exact pending proxy creation when invoked during start;
  it cannot send to an uncommitted child.

### [DS-REPLACE] Replace

- Replace is distinct from dynamic stop followed by a new key.
- Replace retains the stable proxy and requests one fresh worker incarnation.
- Acceptance is reported before realization.
- The submitted worker is returned intact when the key is unknown, stopping,
  replacing, retired, or globally shutting down.
- `Replaced` is emitted only after the replacement incarnation is ready.
- Replacement rejection preserves exact creation reason and does not report
  success.
- Replacement provenance names the exact prior worker incarnation.
- Stop and replace wait for their exact runtime facts and cannot consume one
  another's report.

### [DS-EXIT] Unexpected exit and unavailability

- A matching unexpected worker stop makes the key empty before policy is
  applied.
- `KeepEmpty` preserves the stable proxy for an explicit later replace.
- `Retire` drains the stable proxy and retires the key.
- A stale or foreign worker stop cannot vacate another key or incarnation.
- Every unavailable proxy command is returned to the durable lifecycle owner
  exactly once with key, sender, phase, and command intact.
- Unavailability is supported during installation, replacement, empty,
  stopping, and global shutdown phases.

### [DS-RETENTION] Capacity, retirement, and key reuse

- Reserving a new key at capacity returns the submitted worker unchanged with
  a typed capacity rejection.
- Retired entries are removed after all proxy, activation, delivery, and
  shutdown facts for their exact entry generation are resolved.
- A removed key may be started again with a fresh entry generation and fresh
  proxy; it is never address reuse or actor replacement.
- Every delayed fact and private report carries enough exact generation and
  capability evidence that an earlier use of the same key cannot affect the
  new entry.
- `KeepEmpty` entries continue to consume capacity; explicit `Stop` is required
  to drain the proxy and release the key.
- The table retains no permanent tombstone. A late fact for a removed
  generation is returned as a typed stale diagnostic with its complete
  payload.

### [DS-CANCEL] Accepted-operation cancellation

- Start and replace acceptance return an opaque operation token distinct from
  the management key and actor nonce. Its constructor and fields are private,
  but the accepting caller can move the value into `Cancel`.
- Operation phase is exhaustive: proxy creation emitted with actor-owned
  definition, proxy committed while waiting for activation
  authorization with actor-owned definition, install input emitted, awaiting
  the proxy's atomic outcome, ready, cancelling, or shutdown-owned.
- Cancellation in a phase that still owns the definition returns it. Emitting
  an install/replacement input transfers the definition; no later outcome may
  claim to return it.
- Cancellation after definition transfer is logical. It suppresses
  `Started`/`Replaced`, waits for delivery/creation/activation settlement, and
  drains any incarnation that commits or becomes ready.
- Cancellation names that exact token and returns one exhaustive
  `CancellationReceipt`: `Returned`, `Pending`, `Committed`, `Cancelled`,
  `Stale`, or `Draining`.
- Cancelling start drains any committed proxy after all transferred install and
  activation ownership settles. A late ready incarnation is never advertised.
- Cancelling replacement never revives the old worker or reports replacement
  success. A late replacement incarnation is drained exactly once.
- Stop is a lifecycle operation on the entry, not an alias for cancelling an
  unrelated start or replace token.

### [DS-SHUTDOWN] Dynamic-supervisor shutdown

- Global shutdown rejects new management mutation commands.
- Every known installing, available, empty, replacing, or stopping stable
  proxy is resolved and drained exactly once.
- Entries already removed from the table are absent and require no effect.
- Pending creation rejection counts as drained.
- The supervisor terminates normally only after every dynamic entry is
  retired.
- Outer shutdown composition can deliver the shutdown input and interpret all
  dynamic outcome and child lifecycle lanes without path-specific caller code.

## 4. FIFO worker pool

### [FP-BUILD] Definition and construction

- Construction requires a worker factory, at least one unique semantic worker
  role, a complete recovery policy, a backlog capacity, an interruption
  policy, and FIFO distribution.
- An incomplete definition cannot implement `Behavior` or call `build`.
- A zero-worker pool is rejected before it can accept owned work.
- Duplicate worker roles are rejected before initialization.
- Factory rejection before activation emits no partial worker fleet.
- The worker public message protocol must accept the pool's exact assignment
  product; a wrong worker protocol fails statically.
- The customer reply protocol retains the exact job, result, role, admission,
  completion, and terminal-return outcomes selected by the pool.
- Construction or application hosting selects a typed operational-diagnostic
  disposition. Expected diagnostics cannot disappear into an unnamed lane or
  require a no-op callback.
- Construction also selects worker activation and `ActorDrainPolicy`; installation
  alone is never the implicit activation policy.

### [FP-TOPOLOGY] Worker topology and recovery

- The pool directly owns one fresh worker-incarnation sequence per declared
  semantic role; no stable proxy is required because workers are private to
  the stable pool actor.
- Initial worker creation, worker stop, replacement, budget, timing, failure,
  and shutdown are interpreted by the pool's own fold.
- Pool lifecycle handling is not delegated to a nested supervisor behavior or
  proxy whose events are redispatched.
- Fresh worker replacement preserves the semantic pool role and future
  dispatch eligibility without preserving a worker address.
- Every direct-worker member has exhaustive pre-ready phases for creation,
  installed initialization settlement, waiting for activation authorization,
  activation dispatched, and activating. Each phase owns only its exact
  definition, incarnation, settlement, permit, plan, or attempt.
- Each pool owns one positive activation-authorization limit, a
  declaration-order waiting-role queue, and exact occupied tickets. An
  installed worker may wait with its activation permit and plan until
  authorization; it is never silently counted as activating.
- Reserving a slot and emitting `BeginActivation` are one commit. Mandatory
  action settlement establishes accepted activation before the next ordinary
  pool input; the exact ready, rejected, stopped, or forced-transfer outcome
  releases the slot once.
- Installed, waiting-for-authorization, dispatched, or activating workers
  remain ineligible for dispatch until their exact readiness fact commits.
- Every transition that makes a worker ready first checks the applicable
  backlog. It commits `Busy` plus assignment of the oldest eligible queued job
  when one exists, and commits `Idle` only when none exists.
- Post-commit initialization rejection, activation rejection, cancellation,
  and shutdown own distinct pre-ready drain causes; recovery or retirement
  begins only after exact terminal or forced-transfer settlement.
- Worker-incarnation replacement never changes the role used for assignment.
- Restart denial or irrecoverable worker loss cannot strand queued or assigned
  jobs.

### [FP-ADMIT] Job admission

- Submission carries a complete owned job, concrete customer reply route, and
  caller-authored request correlation echoed by both admission outcomes. The
  request correlation is never trusted as pool job identity.
- The pool retains the customer route; it is never copied into the worker
  assignment or echoed back by worker code.
- After classifying shutdown, serviceability, and full backlog, the pool
  assigns a fresh job id and immutable admission ordinal without trusting a
  customer-supplied id as either internal correlation.
- If an eligible worker is idle, the job is accepted and assigned in the same
  transition.
- Assignment state is committed before the dispatch effect is exposed.
- If no worker is idle and backlog capacity remains, the job is accepted once
  and appended in FIFO order.
- Capacity is the new-admission waiting bound and counts only jobs currently in
  the backlog; assigned work is owned by a busy worker state. `Retry` is not a
  new admission and may temporarily place one formerly assigned job per worker
  beyond that bound, so the absolute queue bound is capacity plus roster size.
- Capacity zero is valid and permits only immediate assignment.
- A full backlog returns the customer, reply recipient, and payload unchanged.
- Full/unserviceable rejection occurs before job allocation. Job-correlation
  and immediate-dispatch preparation failures are distinct typed rejections;
  failure does not silently fall back to queue admission.
- Admission during shutdown returns the owned job unchanged.
- A payload clone needed for dispatch occurs before admission state commits.
  Unwind cannot leave a recorded assignment or dequeue because no transition
  was returned, but caller ownership recovery is not promised across panic.

### [FP-ASSIGN] Assignment

- Every dispatch carries a fresh opaque affine completion authority and owned
  job payload.
  The pool separately retains job id, customer route, exact worker role, and
  incarnation.
- Token fields and constructors are private. Worker code can only move the
  token through `assignment.complete(result)`; it cannot construct a token or
  choose a customer destination.
- A job has exactly one authoritative customer obligation and active
  correlation at a time. Under `Retry`, the pool retains the canonical
  obligation/payload while a worker owns a cloned execution payload; those are
  deliberately two Rust values but never two customer completions.
- No job is simultaneously queued and assigned.
- No worker owns more than one assignment unless the worker protocol
  explicitly defines concurrency; the current template is single-assignment.
- Idle-worker selection uses a circular declaration-order cursor. Each
  assignment selects the first ready idle worker at or after the cursor,
  skipping unavailable roles, then moves the cursor to the next surviving
  role. Becoming idle does not jump a worker ahead of the cursor.
- Removing or retiring a worker does not reorder surviving FIFO jobs.
- A dispatch batch preserves FIFO job order even when internal worker entries
  are removed.
- Retry reinserts by immutable admission ordinal, not unconditionally at the
  front. Multiple interrupted workers therefore preserve their original
  admission order regardless of stop arrival order.
- No serviceable queued job may coexist with an eligible idle worker. This
  invariant is restored in the same fold that processes initial readiness,
  replacement readiness, retry recovery, or completion; a later submission
  therefore cannot jump ahead of older backlog.
- Rejected assignment delivery returns the exact worker command. A typed
  reunion consumes its affine completion authority with pool-retained evidence
  once before ordered reinsertion, then quarantines the exact worker for
  drain/recovery. This is safe return, not an at-least-once interruption;
  delivery settlement, completion, and worker stop may arrive in any order and
  join without a second job disposition.

### [FP-COMPLETE] Completion

- A matching completion authority resolves to exactly one active pool-owned
  record containing assignment id, job id, customer route, worker role, and
  private worker-birth evidence. The locked parent report supplies only the
  creator-local child nonce, so matching combines that nonce with opaque
  authority-carried birth evidence; it does not claim the interpreter attaches
  an exact incarnation capability.
- Matching completion returns the result exactly once, releases that worker,
  and immediately dispatches the oldest eligible backlog job when present.
- Duplicate completion is stale and cannot emit a second customer outcome.
- Cross-worker completion is stale and cannot release either worker.
- Completion from an old worker incarnation is stale and preserves the current
  assignment.
- A stale completion returns the complete result through the separate
  operational-diagnostic disposition without mutating current ownership or
  sending another customer outcome.
- Completion and successor dispatch effects coexist in one action without
  loss or reordering.
- Completion may precede assignment-delivery settlement. Completion,
  settlement, and worker stop form one exhaustive order-independent join; no
  synchronous settlement guarantee is assumed.

### [FP-INTERRUPT] Worker interruption

- A worker stop matching an idle worker makes only that exact worker
  unavailable and enters recovery.
- A worker stop matching a busy worker first resolves ownership of its exact
  assignment.
- `Fail` returns the interrupted job exactly once with a typed interruption
  outcome.
- `Retry` retains the complete job for at-least-once execution and reinserts it
  by its immutable admission ordinal. Queue metadata retains prior role and a
  semantic interruption classification, never a second copy of the
  authoritative stop fact.
- Retry is explicitly at-least-once processing: the worker may have performed
  application work before stopping, so the pool does not claim exactly-once
  execution or side effects.
- Clone preparation needed to retain a retry copy occurs before state commit.
  If cloning unwinds, no new pool state or `Actions` commit exists; the design
  makes no promise that Rust unwind restores caller ownership or prevents the
  actor from failing.
- Worker stop, assignment settlement, completion, and worker creation facts
  that can arrive in different orders join exactly once without losing,
  duplicating, or prematurely failing the assignment.
- Duplicate worker stop during replacement cannot cause a second retry or
  failure outcome.
- A returned assignment after restart exhaustion or shutdown is consumed as
  stale without a second customer outcome.

### [FP-FAILURE] Retirement and failure

- An irrecoverably retired worker causes no future dispatch to that role.
- Backlog work that can run on other FIFO workers remains eligible.
- If no live or recoverable worker can ever serve queued work, every stranded
  job is returned exactly once with a typed terminal reason.
- Pool failure diagnostics and customer job outcomes remain separate named
  effect lanes.
- A supervision failure cannot be reconstructed from a missing worker or
  sequence arithmetic.

### [FP-RETENTION] Correlation retention

- Every accepted queued or assigned job retains its customer route. Only an
  active assignment retains full completion correlation; queued jobs have no
  completion authority.
- Terminal customer outcome removes the active correlation after its delivery
  attempt is emitted; any rejected delivery follows `[SH-DELIVERY]` and is not
  reconstructed from a tombstone.
- Completion tokens and assignment/job ids are never reused before typed
  exhaustion, so an old token cannot match a later job.
- The pool retains no unbounded `ReturnedAssignment` tombstone table.
- A completion for a non-active token is an unknown/stale operational
  diagnostic carrying the result; it cannot select a customer.

### [FP-SHUTDOWN] FIFO-pool shutdown

- Shutdown immediately removes and returns every queued job exactly once.
- Every assigned job is returned exactly once according to the documented
  shutdown outcome; later completion is stale.
- No retry or successor dispatch is emitted after shutdown begins.
- Pending worker creation and delayed recovery are resolved or cancelled
  without losing jobs.
- Every owned worker incarnation is drained exactly once.
- Worker stop and completion in either admissible order complete the drain
  without duplicate job outcomes.
- The pool terminates normally only after every job has been returned or
  completed and every owned worker is drained.

## 5. Keyed worker pool

Keyed pooling reuses only the FIFO laws listed here. No unlisted FIFO
requirement is inherited implicitly.

| Concern | Shared FIFO law | Keyed substitution or restriction |
|---|---|---|
| Direct workers | Fresh direct-worker creation, exact readiness, one active assignment per worker, and complete lifecycle/drain ownership apply. | Each worker has one immutable semantic role; no proxy or supervisor state is reused. |
| Customer ownership | One authoritative obligation, correlated admission outcome, opaque completion authority, typed assignment-delivery settlement, and one terminal customer outcome apply. | Accepted work additionally retains non-authorizing admitted-binding evidence `{ generation, role }`; it does not retain or clone the submitted key. |
| Queue order | Immutable admission ordinal and retry reinsertion by that ordinal apply. | There is one FIFO queue per role. A role's retry may add at most its one formerly active assignment beyond that role's admission bound, for an absolute per-role bound of `capacity + 1`. There is no global retry-overflow equation. |
| Dispatch eligibility | Only exact ready-idle workers are immediately assignable, and readiness cannot leave older serviceable work queued. | The retained role selects the only candidate worker. The no-serviceable-backlog invariant is scoped to that role; another idle role cannot serve it. |
| Worker selection | None. | There is no circular worker cursor or cross-worker scan. Binding/selection chooses a role before dispatch. |
| Capacity | Zero backlog means immediate assignment only. | Backlog capacity is per role, and binding-table capacity is a separate positive bound. There is no global backlog capacity. |
| Recovery and interruption | Recovery eligibility, exact lifecycle correlation, `Fail`, explicit at-least-once `Retry`, and complete stranded-work return apply as value laws. | Recovery and retry preserve the admitted role. Rebalance cannot retarget accepted work. |
| Shutdown and settlement | Admission closes, every accepted job settles once, workers drain exactly once, and action/rejection settlement remains explicit. | Every role partition is extracted, all bindings are removed, and late keyed management/completion facts are generation-classified. |

### [KP-BUILD] Definition and selector

- Construction requires one concrete statically dispatched selector.
- FIFO construction requires no selector, placeholder selector, or no-op
  policy.
- Every keyed submission carries its semantic key separately from its owned
  payload; the payload implements no pool-specific key trait.
- The selector maps the submitted key to a semantic worker role, never to an actor
  nonce or address.
- Captured selector state remains concrete and statically dispatched.
- Selection is evaluated once at admission and its chosen role is retained by
  accepted work.
- Construction selects a per-role backlog capacity and a maximum number of
  retained key bindings. No ambiguous global backlog capacity is inferred.

### [KP-AFFINITY] Affinity and admission

- A valid binding routes new work only to the selected role's worker
  incarnations.
- Fresh worker-incarnation replacement preserves affinity automatically.
- If the selected role is busy, only that role's backlog capacity determines
  targeted admission.
- Work bound to one role is never stolen by another idle role.
- A retired selected role refuses new work with the complete owned job.
- Committing permanent unavailability for one affinity role terminates that
  role's queued and assigned work without disturbing live roles or their jobs.
- Target eligibility is an exhaustive admission sum:
  `AssignableNow`, `BacklogAdmissible`, or `Unavailable`. The total projection
  covers every private member variant and every pre-ready drain disposition;
  no state falls through to a default branch.
- Ready-idle is `AssignableNow`. Prepared, creating, initializing,
  waiting-for-authorization, activation-dispatched, activating, ready-busy,
  and admitted recovery are `BacklogAdmissible`. Stopping, retired, and
  shutdown-owned are `Unavailable`. `DrainingPreReady` projects by its
  retained exhaustive after-drain sum: recover or retire. Irrecoverability is
  a transition cause that commits a permanently unavailable phase, not a
  second retained member state.
- A key with no retained binding is selected once by the concrete selector and
  bound atomically with accepted admission when binding capacity permits and
  the target is either immediately assignable or backlog-admissible with
  remaining per-role capacity. Rejection, including zero-capacity backlog for
  a non-idle target, retains neither the job nor a new binding.

### [KP-REBALANCE] Rebalance

- Rebalance is an explicit typed command.
- Every rebalance carries `Absent | Exact(binding_generation)` expectation.
  Rebalance of an absent key requires `Absent`; rebalance of an existing key
  requires its exact current generation.
- A valid rebalance changes future admission only.
- Already queued and assigned jobs retain the role chosen when they were
  accepted.
- Rebalance target eligibility is a separate exhaustive projection. Prepared,
  creating, initializing, waiting-for-authorization, activation-dispatched,
  activating, ready-idle, ready-busy, admitted recovery, and
  pre-ready-draining-with-`ResumeRecovery` are bindable. Stopping, retired,
  terminal pre-ready drain, and shutdown-owned are unavailable. An unknown or
  unavailable target returns the complete request and preserves the prior
  binding.
- An explicitly unbound key can be bound by a valid rebalance.
- A role-changing rebalance reserves a fresh generation before atomically
  replacing the old binding. A same-role rebalance is an accepted no-op that
  preserves the current generation.
- Stale absence or generation expectation returns the complete command
  unchanged and cannot validate or mutate a later binding.
- Repeated and short rebalance sequences match an independent binding model.
- Rebalance never creates, replaces, revives, or proves a worker actor.
- Explicit `Unbind` removes only the future-admission binding. Already queued
  and assigned work retains its selected role.
- Unbind of an existing key requires its exact current generation. A stale
  absence or generation expectation returns the complete command unchanged.
- Unbinding an absent key returns typed `AlreadyUnbound { key }` and leaves
  state unchanged. Idempotence is an explicit protocol outcome, never guessed
  from map mutation success.

### [KP-END] Keyed completion, interruption, and shutdown

- Completion correlation includes the retained affinity role in addition to
  assignment, job, and incarnation.
- Retry returns interrupted work to the same retained role unless a separate
  explicit policy says otherwise; rebalance does not retarget it.
- A permanently retired role's returned work cannot later be revived by a
  stale lifecycle or completion fact.
- Shutdown returns jobs from every affinity partition exactly once and drains
  every owned worker incarnation.
- Assignment, customer response, lifecycle, recovery timer, and shutdown lanes
  remain independently named and are all interpreted at the same actor path.

### [KP-RETENTION] Binding retention

- Binding capacity counts only currently retained key-to-role bindings, not
  queued jobs or active assignments.
- Rebalance does not allocate a second binding entry for an existing key.
- Unbind releases capacity immediately because queued/assigned jobs retain
  their role independently.
- A later submission for the same unbound key invokes the selector again and
  creates a new binding generation.
- Late rebalance/unbind outcomes are correlated to the binding generation so
  they cannot modify a later reuse of the same key.
- The transition committing permanent unavailability atomically unbinds every
  retained binding to that role, releases their binding capacity, and emits
  one typed diagnostic per removed key/generation. `Irrecoverable` is one
  cause for that transition, not a retained terminal phase beside `Retired`.
  Temporary recovery and pre-ready drain with `ResumeRecovery` retain
  bindings. Already queued or assigned jobs retain admitted-binding evidence
  needed for their single terminal outcome; no permanently unusable binding
  remains.

## [API] Builder and public API requirements

- Builders encode missing versus selected semantic choices in their types.
- Builder values do not implement `Behavior`.
- Only complete built definitions implement `Behavior`.
- Value-dependent invalidity, such as duplicate roles or invalid delay bounds,
  returns a typed build error.
- Builders never request child nonces, timer ids, assignment ids, job ids, or
  actor addresses from the caller.
- Fixed, dynamic, FIFO, and keyed construction expose only choices meaningful
  to that template.
- No valid construction requires a no-op callback, dummy selector, empty
  factory, marker route, explicit output alias, wrapper path, or generic type
  annotation whose only purpose is satisfying internal machinery.
- Resulting behavior types are inferable from ordinary factory, role, selector,
  worker, and reply values.
- Public names describe semantic state, policy, command, result, or error—not
  structural positions such as inner depth or wrapper ownership.
- Each template has one documented fluent order. Typestate exposes only the
  next meaningful choices; arbitrary equivalent call permutations are not a
  second advertised API.
- Ordinary construction requires neither explicit reply-protocol aliases nor
  turbofish that repeats types already present in factory, worker, selector,
  route, or command values.
- Infallible and fallible worker construction are both truthful: an
  infallible factory does not manufacture an `Option`/error branch, and a
  fallible factory preserves its exact typed error.
- Worker code completes a pool assignment with
  `assignment.complete(result)`. It never names `ReportToParent`, a parent
  path, customer recipient, assignment id, or send-product lane.
- Every template has one complete copy-paste example covering imports, worker
  definition, construction, hosting, activation/readiness, command or job
  submission, replies/diagnostics, and shutdown.
- Compiler diagnostics are measured with focused compile-pass/fail fixtures;
  the typestate syntax remains a hypothesis until those fixtures show the
  error at the missing semantic choice rather than in nested associated types.

## [EFFECT] Effect and interpreter requirements

Each template exposes one named effect product containing only lanes it can
actually emit. Across the catalogue these include:

- worker-command deliveries;
- private parent-to-proxy installation and replacement inputs;
- proxy and worker creation observation;
- proxy and worker termination observation;
- exact child shutdown requests;
- restart scheduling requests;
- parent lifecycle and unavailability reports;
- dynamic management replies;
- pool worker assignments;
- pool customer outcomes; and
- supervision or pool terminal diagnostics.

Requirements:

- Every lane has a concrete static interpreter requirement.
- Interpretation order is documented and tested.
- Same-action creation is committed before any bundled creation observation,
  child-termination observation, child delivery, child input, or child
  shutdown.
- Every current same-action occurrence-dependent operation is owned by one
  staged creation-scoped bundle: `ObserveCreation`,
  `ObserveEstablishedCreation`, `ObserveChild`, `ChildDelivery`, `ChildInput`,
  and `ShutdownChild`. The bundle explicitly selects direct creation →
  occurrence effects or creation → required observations → occurrence
  effects. Independent effects remain outside; no dependency is inferred from
  lane position, wrapper path, role, or lookup.
- Creation rejection produces one authoritative prerequisite settlement and
  returns every bundled observation/delivery/input/shutdown untouched as
  `NotAttempted`. After accepted creation, every required observation is
  attempted in named lane/item order even when another rejects, and every
  rejection is preserved independently. All later occurrence effects become
  `NotAttempted` with the first rejected observation in that order as their
  deterministic `blocked_by` prerequisite. A future occurrence-dependent
  operation must extend the closed bundle and tests before same-action use.
  Independent effects still run.
- `ObserveEstablishedCreation` belongs to the bundle because its input is the
  pre-commit child route and its result supplies the exact committed
  capability. Established delivery, observation, and shutdown cannot enter
  because they require that capability as input; logical delivery is
  occurrence-independent and child reports originate from the child. These
  inclusions and exclusions are part of the completeness proof.
- One lane's interpreter failure does not allow a later lane to pretend an
  earlier required effect succeeded. A dependent item settles as
  `NotAttempted`; an independent item is still interpreted.
- Append and reducer operations preserve every lane, creation, and terminal
  verdict exactly once.
- Fixed and heterogeneous birth products dispatch to concrete installers
  without erased runtime selection.
- A minimal interpreter witness consumes every lane of every atomic template.
- Every delivery lane also has an interpreter-rejection witness satisfying
  `[SH-DELIVERY]`; a success-only interpreter does not implement the template.

## [VERIFY] Required verification per template

Each atomic template must have:

- focused example tests for every transition above;
- compile-pass tests for complete builder syntax;
- compile-fail tests for incomplete builders and protocol mismatch;
- complete-action assertions, not effect counts alone;
- both event orders for every legitimately unordered fact join;
- independent model tests using vocabulary different from production state;
- exhaustive small-state tests for lifecycle, budget, backlog, binding, and
  shutdown boundaries;
- property tests asserting invariants after every generated step;
- fuzz coverage for stale, duplicate, contradictory, overlap, and shutdown
  sequences;
- debug and optimized regressions for lifecycle facts that must not be
  accepted twice; and
- an interpreter-level witness for initialization ordering and every named
  lane.
- readiness/activation races, delivery rejection, bounded retention, key
  reuse, and deadline-retirement tests where those laws apply.
- exhaustive keyed-admission projection over every member and after-drain
  variant, with no wildcard model branch;
- readiness, recovery, and completion sequences proving that serviceable
  backlog never coexists with eligible idle capacity;
- a root-driver trace in which actor-side forced retirement occurs before late
  activation readiness, followed by exact incarnation drain and only then the
  final runner result;
- compile-pass/fail terminal lifts for heterogeneous children, duplicate role
  values, and one/two/eight wrapper layers with a recorded diagnostic-size
  bound; and
- interpreter traces for accepted/rejected creation and required observation,
  covering `ObserveCreation`, `ObserveEstablishedCreation`, `ObserveChild`,
  child delivery, child input, and child shutdown while proving dependent
  `NotAttempted` values and continued independent effects; the traces must
  include multiple required-observation rejections and prove that all are
  preserved while downstream items reference the first rejection in named
  interpretation order.

The original defect must be demonstrably reproduced by each regression it is
intended to prevent. A green build without that counterexample is not evidence
that the feature is implemented.

## [NONFEATURE] Existing surfaces that are not features

The replacement need not preserve these implementation artifacts:

- arbitrary `Behavior` supervision wrappers;
- fluent extension methods;
- `BehaviorLayer` construction of supervisors or pools;
- `FixedFleetOwnership`, slot products, ownership redispatch, or positional
  nesting paths;
- a pool implemented by nesting or forwarding another pool;
- duplicated flattened copies of another template's effect product;
- a public `utils` module;
- a mandatory policy whose only legitimate implementation is a no-op; or
- compatibility aliases that keep both old and new architectures alive.
- stable proxy children inside a pool, because the stable pool actor already
  owns private worker incarnation routing.

Their observable laws are included above where required. The artifacts
themselves are deleted after the replacement templates and migrations are
proven.
