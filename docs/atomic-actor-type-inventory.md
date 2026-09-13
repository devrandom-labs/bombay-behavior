# Minimal type equations and actor relationships

This document derives types from the feature catalogue. It deliberately does
not turn every phase, error reason, effect lane, builder axis, or compiler
obligation into a public type.

The previous 64-name inventory is withdrawn. It confused four categories:

- semantic values a user deliberately chooses or observes;
- behavior definitions and public protocols;
- private state-machine proof types; and
- generated/interpreter plumbing.

Only the first two categories are candidates for the ordinary public API.
Private sums remain real sums—they are not converted to flags—but their names
do not become concepts users must import.

This document is not a second runtime-state specification. The sole normative
ownership equations and legal transitions are in
[`atomic-actor-solution.md`](atomic-actor-solution.md). If a candidate public
protocol described here cannot be projected from those equations without
losing ownership or provenance, the candidate is invalid; the inventory does
not amend the runtime state machine.

## Minimization test

A new public type is allowed only when all four answers are concrete:

1. Which distinct semantic state, outcome, or capability does it own?
2. Which invalid exchange or transition becomes impossible because it is a
   separate type?
3. Why can an existing core type or a variant of an existing actor protocol
   not express it?
4. Which caller-written type, alias, marker, route, or callback does it remove?

The following do not justify a public type: naming a generic parameter,
shortening a nested type, satisfying a bound, forwarding an event, hiding a
structural path, or making rustdoc look symmetrical.

## Visibility budget

There are four visibility bands.

| Band | Contents | Ordinary user names it? |
|---|---|---|
| Public semantic | actor builder/definition, command/outcome sums, real policy sums | Yes |
| Public but inferred | protocol identity and typestate return forms whose exact spelling is compiler-prototype dependent | Normally no |
| Crate-private semantic | exhaustive runtime phases, joins, correlations, tickets, drain states | No |
| Generated/interpreter | named effect products, occurrence positions, request lanes, lowering traits | No |

If the compile prototype makes the last two bands appear in ordinary source,
the DevX is rejected. They are not promoted to public concepts to make the
prototype compile.

## Existing core: seven retained concepts

The exhaustive decision is in
[`atomic-actor-retained-core.md`](atomic-actor-retained-core.md). The atomic
actors reuse only these irreducible concepts: `Protocol`, `Behavior`,
`Actions`, `Step`, logical/exact recipients, staged `CreateChild`, and
`TerminalOutcome`. Current support and interpreter carrier types may remain
internally, but the actor modules do not duplicate or re-export them.

## Shared public semantic values

These are shared only because every consuming actor gives them the same law.
There is no shared runtime ownership engine.

### `Recovery`

```text
Permanent { strategy, limit, release }
Transient { strategy, limit, release }
Temporary
```

- Owns the complete eligibility and replacement decision policy.
- `Temporary` cannot accidentally carry unused restart settings.
- Used by fixed supervisor, FIFO pool, and keyed pool.
- Dynamic supervision does not use it; replacement there is an explicit
  management operation.

### `Strategy`

```text
OneForOne | OneForAll | RestForOne
```

- Selects an ordered candidate set from one immutable topology snapshot.
- Shared by the three actors that use `Recovery`.

### `RestartLimit`

```text
{ maximum: u32, window: Duration }
```

- Owns atomic sliding-window admission.
- The catalogue defines the exact zero, boundary, monotonic-time, and
  out-of-order laws; no boolean “within limit” escapes the fold.

### `RestartRelease`

```text
Immediate
Constant { delay }
Linear { initial, maximum }
Exponential { initial, maximum }
```

- Combines timing and backoff in one sum; there is no optional backoff or
  separate timing flag.
- Checked construction rejects zero delay and maximum below initial.
- Delay arithmetic is checked and its failure is a typed denial outcome.

### `ActorDrainPolicy`

```text
WaitForActorGraph
RetireActorGraphAfter { deadline }
```

- `WaitForActorGraph` makes potentially indefinite actor-graph waiting an
  explicit choice.
- `RetireActorGraphAfter` schedules an interpreter-clock deadline and produces
  exact forced-retirement facts for every unresolved child/job. It bounds only
  actor-graph retirement; transferred uncancellable work keeps the root run
  future alive.
- If later requirements prove more than one deadline-retirement action, this sum gains
  semantic variants; it does not gain booleans.

This is not a process-exit policy. Final application completion separately
requires residual root settlement to become empty. No bounded process-exit or
emergency-abandonment policy is selected.

### Activation and diagnostic choices

Activation has two semantic modes: immediate readiness after initialization,
or a concrete statically dispatched activation plan resolved through the
exact request/fact capability in the solution. The plan type is inferred from
the value supplied to `.activation(...)`; the inventory does not authorize a
boxed callback, erased future, universal activation trait object, or a public
type per activation phase. Nor does it authorize
`Activation<Plan>::Immediate`, which would leave an unused plan type to infer;
immediate and plan-bearing builder states must both remain inferred.

The activation-authorization limit uses `ActivationPolicy`, constructed from a
plain `usize` with typed `ZeroCapacity` rejection. Its one user-facing law is
“at most this many owner authorizations may be unresolved.” A supervisor
conservatively records authorization when it emits the opaque proxy install
input because the proxy exposes no progress facts. A direct pool records it
when initialization has settled and it emits `BeginActivation`. The counting
law is identical; the template-specific authorization boundary is explicit.
A separate activation-limit spelling is not justified. Nor is there a second
`ActivationAuthorization` capability: the exact
installation-issued permit authorizes `BeginActivation`, while crate-private
waiting variants and occupied tickets prove the concurrency bound.

Diagnostic disposition is one real semantic sum because its alternatives
carry different capabilities:

```text
Diagnostics<Route> = DeliverTo(Route) | Terminate
```

The notation does not authorize a literal generic enum: its `Terminate`
variant would leave `Route` unconstrained. The builder must infer a
route-bearing concrete state for `DeliverTo` or a route-free concrete state
for `Terminate`; whether either policy type is an ordinary public name remains
a compile-prototype question. It replaces mandatory `.diagnostics_to(...)`;
absence is not encoded by an `Option`, and autonomous actors terminate on a
diagnostic without a dummy route or type annotation.

Per-item action settlements, activation attempts, installed incarnations, and
rejection products are interpreter-facing typed capabilities. Ordinary
application code does not construct them, but they are not erased or
dynamically dispatched.

### Open terminal and dependency boundary

`TerminalSettlement` now has a semantic equation but is not yet an authorized
universal public type. One concrete application/root terminal sum must receive
total statically dispatched lifts from every heterogeneous child and wrapper
terminal sum while preserving exact provenance. Duplicate semantic roles do
not identify a source. The solution's **Static terminal-projection equation**
is normative; its Rust spelling remains Open until wrapper-depth diagnostics
and heterogeneous lifting are compile-proven.

`NonEmptyResidualSettlement` is crate-private root-run state. It keeps exact
external activation and late-drain ownership alive after the actor graph is
forcibly retired. The final runner result does not exist until that state is
empty; ordinary users receive no cloneable residual report or topology
product.

`CreationBundle<CreateRequest, AfterCreation>` and
`LoweredAction<Independent, Creations>` are semantic names for the selected
interpreter dependency equation, not approved ordinary imports. Each bundle
owns one creation and the closed current occurrence-dependent set:
`ObserveCreation`, `ObserveEstablishedCreation`, `ObserveChild`,
`ChildDelivery`, `ChildInput`, and `ShutdownChild`. Its sum selects effects
that depend directly on creation or also on required-observation acceptance.
Every required observation is attempted in named lane/item order; if several
reject, every rejection is retained and later `NotAttempted` effects reference
the first rejection in that order. This replaces positional dependency
discovery and runtime lookup. A core prototype must determine whether existing
named `Actions` products lower truthfully or need a minimal interpreter-facing
product change.

### Why there are no shared public builder markers

Missing/present and empty/non-empty proofs are typestate implementation
details. One internal proof family can serve every builder, but ordinary users
must not import `Unset`, `Set`, `NoMembers`, `Members`, `WithEvents`, or a type
per axis. The compile-pass/fail prototype must discover whether Rust can keep
those states inferred. If not, the builder spelling is still open.

## 1. Stable proxy

### Current public surface

- `StableProxy<Worker, Plan>` — the behavior definition and stable service
  identity. `StableProxy::immediate()` and `StableProxy::activated()` are its two
  inferred constructions.
- `ActivationPlan`, `ImmediateActivation`, and `WorkerSubmission` — application
  worker definition and its concrete activation work.
- `ProxyPhase`, `ProxyOutcome`, `InitialWorkerOutcome`, `ReplacementOutcome`,
  and `ProxyDiagnostic` — owner-observable lifecycle and diagnostic products.

The service protocol is exactly the worker protocol. Owner control,
initialization and activation requests, effect products, operation correlation,
drain custody, and settlement inputs remain technically public only where the
associated Behavior/interpreter contract requires them; they are doc-hidden and
are not part of Bombay's ordinary facade.

### Runtime ownership reference

The proxy performs one direct fold. Its complete `Dormant`, creation/stop,
initialization/stop, activation/stop, ready, replacement, and drain equations are
normative only in the solution's **Stable worker proxy solution**. In
particular, the worker definition is transferred when the creation action is
emitted; no inventory shorthand may place it back in a later creation state.
“Atomic” means one local state/action commit, not atomic cross-actor delivery.

## 2. Fixed supervisor

### Current public surface

- `fixed(...)` and `FixedSupervisor<…>` — the one inferred construction and
  resulting behavior. The returned `FixedBuilder` is doc-hidden because ordinary
  source never names it.
- `OrderedRoles` and `DuplicateRole` — validated non-empty unique topology.
- `ActivationPolicy`, `Recovery`, `FailureReaction`, and `ActorDrainPolicy` —
  every required supervisor policy.
- `FixedCommand` — status, capability, and shutdown management.
- `FixedSnapshot` — read-only role/order/phase/availability projection.
- `FixedLifecycle` — durable lifecycle outcomes only when
  `.publish_lifecycle(route)` is selected.
- `FixedDiagnostic` — exact operational failures selected by
  `DiagnosticDisposition`.
- `InitialWorkerRejection` — shared construction-only worker rejection with the
  function, prepared prefix, exact role/reason, and untouched suffix.
- `FixedConstructionRejected` — fixed policies plus one
  `InitialWorkerRejection`.
- `FailureReaction` — `RetireMember | StopSupervisor`; one exhaustive response
  when fixed topology can no longer be preserved.

Lifecycle publication is optional. An autonomous supervisor is a complete valid actor
and provides no discard callback or dummy recipient.

The selected names are `FixedCommand`, `FixedSnapshot`, `FixedLifecycle`, and
`FixedDiagnostic`. Construction and preparation rejections retain their exact
typed values. Request products, internal events, lifecycle-route traits, and
worker-preparation action products are doc-hidden interpreter contracts.

### Builder semantic state

```text
factory
ordered non-empty role declarations
Recovery
fixed-topology failure reaction: RetireMember | StopSupervisor
activation policy
positive activation-authorization limit
ActorDrainPolicy
diagnostic disposition
optional lifecycle DeliveryRoute
```

Only typestate proof markers are internal. Duplicate roles and factory
rejection are value failures returned by `build`; construction performs no
actor effect.

### Runtime ownership reference

The solution's **Fixed supervisor solution** is the sole member, recovery
transaction, activation-authorization, and drain equation. The decision is
prepared completely before the supervisor commits any member reservation. Factory
rejection, budget denial, overlap, clock regression, or timer exhaustion
returns every value still owned at that phase in one typed outcome.

Different workers may be different behavior variants behind one closed worker
sum only when they expose one common public service protocol. Workers with
different public protocols require separate supervisors; the design does not
erase them into a universal envelope.

## 3. Dynamic supervisor

### Current public surface

- `DynamicSupervisor<…>` — one keyed behavior that creates proxies.
- `DynamicCommand<Key, Worker>` — `Start`, `Replace`, `Stop`, `Query`,
  and `Cancel`, each retaining submitted values.
- `WorkerChangeReceipt<Key>` and `WorkerChangeRejection<Key, Worker, Reason>` —
  the shared start/replace result products; the rejection reason remains
  operation-specific.
- `CancellationReceipt<Worker>` — the exhaustive affine cancellation result.
- `QueryReply<Key>` — the exhaustive known/unknown read-only result.
- `DynamicLifecycle<Key, Worker, Plan>` — durable later readiness,
  unavailability, cancellation, and retirement reports sent to the configured
  lifecycle owner.
- `DynamicDiagnostic<Key, Worker, Plan>` — exact operational contradictions and
  runtime rejections handled by the independently selected diagnostic policy.
- `UnexpectedExit` — `KeepEmpty | Retire`; the complete policy after an
  unrequested exact worker stop.
- `EntryCapacity` — the positive maximum retained keyed entries. It is distinct
  from `ActivationPolicy`, so exchanging the two policies does not compile.

Request replies and durable lifecycle events use distinct protocol
capabilities. A temporary request caller never becomes the long-lived owner by
accident.

### Command and operation sums

```text
Start { key, worker, reply_to }
Stop { key, reply_to }
Replace { key, worker, reply_to }
Query { key, reply_to }
Cancel { operation, reply_to }
```

Start and replace use the same result equation without a naming alias:

```text
Result<WorkerChangeReceipt<Key>, WorkerChangeRejection<Key, Worker, Reason>>
```

Accepted operations return a supervisor-issued opaque cancellation authority.
Cancellation is then exhaustive within `CancellationReceipt`:

```text
Returned { authority, worker }
Pending { authority, phase }
Committed { authority, resulting_phase }
Cancelled { authority }
Stale { authority }
Draining { authority }
```

### Runtime ownership reference

AA-20 and the normalized dynamic document are the sole entry-state and
cancellation ownership equation. Transaction-local preparation reserves every
correlation before committing the creating-proxy entry and its creation action;
there is no stored or queryable reservation phase. The state distinguishes
emitted proxy creation, committed proxy waiting for activation authorization,
emitted install, awaiting one atomic proxy outcome, readiness, cancellation,
drain, and retirement. Worker creation and activation remain proxy-private.
Only phases that still own a worker definition may return it.

The table has a configured maximum. `Stop` drains a ready or empty entry and
eventually removes it; reuse of the same key creates a fresh generation. Facts
from earlier generations are stale diagnostics and cannot mutate the new entry.
No permanent tombstone is required.

## 4. Shared pool values

Only values with identical FIFO and keyed meaning are shared.

### `Assignment<Job>`

```text
{ payload, opaque_completion_authority }
```

- Delivered to one exact worker incarnation.
- Token fields and construction are private.
- Consuming `assignment.complete(result)` creates the only valid completion.
- Issuance creates an affine worker-held authority and pool-retained
  non-authorizing comparison evidence; retained evidence cannot manufacture a
  completion.
- The authority is privately bound to one worker-birth evidence value. Worker
  code cannot inspect it; rejected assignment delivery returns the authority
  for typed reunion with the pool half.
- Contains no customer recipient, parent path, role, job ID, or public token
  constructor unless the pool itself needs those fields privately.

### `Completion<Result>`

```text
{ result, consumed_completion_authority }
```

- Moves from worker to owning pool.
- Cannot select or substitute a customer destination.
- The locked parent report supplies a creator-local child nonce. That nonce,
  authority-carried private worker-birth evidence, and the retained pool half
  must all match the active assignment before completion commits.

### `Interruption`

```text
Fail | Retry
```

- `Fail` returns the accepted payload to the customer once.
- `Retry` is explicitly at-least-once execution because the pool retained a
  retryable payload before assignment.

### Customer and diagnostic protocols

The customer outcome is one closed sum:

```text
Accepted { request: RequestCorrelation, job: JobId }
Rejected { request: RequestCorrelation, payload, reason }
Completed { job: JobId, role, result }
ReturnedQueued { job: JobId, payload, reason }
ReturnedAssigned { job: JobId, role, payload, reason }
```

A stale/duplicate/foreign completion is not another customer outcome. It goes
to a separate operational diagnostic sum carrying the complete result,
authority context, child nonce, private birth evidence, and reason. Its disposition is the builder's
`DeliverTo(route) | Terminate` choice; it is never an unnamed lane, mandatory
dummy destination, or no-op callback.

Whether customer acceptance and terminal outcome are one protocol or two
capabilities remains a DX/type-safety prototype question. The law is fixed:
one accepted job produces at most one terminal customer outcome.

`RequestCorrelation` is caller-authored and echoed only; the pool never trusts
it as job identity or retains it after moving the admission outcome.
`JobId` is an opaque public correlation value because a
shared customer route must distinguish several accepted jobs; its constructor
is private. `AdmissionOrdinal`, assignment IDs, completion authorities,
private worker-birth evidence, and generations are private newtypes. Pool-owned
issuers are non-wrapping, and the values remain distinct so one correlation kind
cannot substitute for another.

## 5. FIFO pool

### Current public surface

- `fifo(…) -> Result<FifoPool<…>, FifoConstructionRejected<…>>` is the sole
  production construction spelling.
- `FifoPool<…>` — one direct worker-owning fold.
- `FifoCommand`, `FifoOutcome`, and their exact admission and return reason
  sums.
- shared `Assignment`, `Completion`, `Interruption`, direct-pool recovery,
  drain, and diagnostics values above, plus the FIFO-specific
  `BacklogCapacity`.
- one immutable private admission ordinal per accepted job; retry reinserts by
  ordinal rather than by stop-arrival order.

There is no `Fifo` marker, distribution mode, shared pool builder, policy bag,
or build stage. The aggregate name and constructor already select FIFO.

### Runtime ownership reference

The solution's **FIFO pool solution** is the sole worker initialization,
activation authorization, job, completion, return, fairness-cursor, and drain
equation. Outcome
delivery is staged in the action settlement rather than represented as an
invented `Completing` or `Returning` actor phase. Only active jobs exist in
the table. After the single terminal customer action is committed, the active
correlation is removed; settlement owns a rejected terminal outcome without
recreating an active job or an unbounded tombstone.

## 6. Keyed pool

### Current public surface

- `keyed(…) -> Result<KeyedPool<…>, KeyedConstructionRejected<…>>` is the sole
  production construction spelling and does not reuse FIFO's constructor or a
  shared pool builder;
- `KeyedPool<…>` — a separate direct fold, not a `FifoPool` wrapper;
- `KeyedCommand<Key, Job>` — `Submit { key, payload, reply_to }`,
  `Rebalance { key, expected, target, reply_to }`, and
  `Unbind { key, expected, reply_to }`, where `expected` is
  `Absent | Exact(binding_generation)`;
- keyed management/customer outcome sum; and
- one inferred concrete closure `Fn(&Key) -> Role` stored by the pool.

There is no public `KeyedJob` or `SelectWorker` trait. Submission supplies the
key explicitly. The constructor statically stores one key-to-role selector
without a boxed callback, selector trait, or second application-defined type.

### Runtime ownership reference

The solution's **Keyed pool solution** is the sole binding, rebalance,
unbinding, admitted-partition, and role-retirement equation. First accepted
submission may create a generation-tagged binding after selector validation;
an explicit rebalance with `Absent` expectation may also create a binding
without work. The table stores only live bound entries, never a persistent
unbound value. The binding is the sole owner of its key. Queued and assigned
jobs retain copyable, non-authorizing `{ binding_generation, admitted_role }`
evidence rather than cloning or owning the key. Rebalance and unbind affect
only future admission. Unbinding an absent key with `Absent` expectation
returns the selected idempotent `AlreadyUnbound { key }` outcome.

Existing rebalance and unbind require `Exact(current_generation)`. A
role-changing rebalance prepares a fresh generation before replacing the old
binding; a same-role rebalance is an accepted no-op preserving the generation.
Every stale expectation returns the complete command unchanged. Rebalance
uses its own total target projection: temporary recovery and pre-ready drain
with retained recovery are bindable, while stopping, terminal drain, retained
retirement, and shutdown ownership are unavailable.

### Capacity and retention

- There is no shared public `Capacity` sum. These are different domain laws,
  not interchangeable configuration values.
- Dynamic-supervisor maximum entries is a mandatory positive `EntryCapacity`;
  an unbounded dynamic table is not a valid construction.
- Backlog capacity is explicitly per role.
- Pool backlog capacity is a `usize` because zero lawfully means immediate
  assignment only.
- Binding-table capacity is a separate positive `NonZeroUsize` bound.
- Unbind removes only future-admission affinity and releases binding capacity;
  already queued and assigned jobs retain their admitted role and generation.
- Reusing a key creates a fresh binding generation.
- Completion/rebalance facts from earlier generations are diagnostics only.
- The transition committing permanent role unavailability unbinds every
  retained binding to that role, releases capacity, and emits exact
  key/generation diagnostics. Irrecoverability is a cause, not a retained
  terminal phase beside `Retired`; temporary recovery retains bindings.
  Existing work keeps its admitted evidence only until its terminal outcome.

Worker, job, recovery, and drain states are keyed-pool-private even where
their variant names resemble FIFO. Sharing a runtime state type is allowed
only after both folds prove identical ownership and transitions; similarity is
not proof.

## Actor topology and dependency direction

```text
FixedSupervisor ──creates/controls──▶ StableProxy ──creates──▶ Worker
DynamicSupervisor ─creates/controls─▶ StableProxy ──creates──▶ Worker

FifoPool  ─────────────creates/assigns/observes directly─────▶ Worker
KeyedPool ─────────────creates/assigns/observes directly─────▶ Worker
```

- Proxy depends only on the retained behavior algebra and neutral interpreter
  carriers.
- Fixed and dynamic supervisors depend on proxy; neither depends on a pool.
- FIFO and keyed pools contain neither supervisor nor proxy and do not contain
  one another.
- Shared recovery/pool values are pure semantic values, never an engine.
- All five templates are direct `Behavior` folds. They are not assembled from
  `BehaviorLayer`s or a universal ownership utility.
- `crates/behavior` never depends on an actor template.

## Types deliberately absent

- no `Supervisor<Mode>` or `Pool<Mode>` runtime engine;
- no shared `Member`, `Slot`, `Fleet`, `Ownership`, or `Children` runtime;
- no `utils` module;
- no public `WithParent`, `Inner`, `AtPath`, occurrence, or effect-lane path;
- no public typestate marker per builder axis;
- no customer route inside an assignment/completion;
- no optional selector in FIFO;
- no stable proxy in either pool;
- no public nonce, timer, job, assignment, operation, token, or generation
  constructor;
- no callback/boolean where a semantic sum owns mutually exclusive choices;
- no compatibility wrapper retaining legacy and replacement templates.

## Selected public surfaces

| Family | Component construction | Status |
|---|---|---|
| Stable proxy | `StableProxy::immediate()` or `StableProxy::activated()` | selected |
| Fixed supervisor | `fixed(...)`, optional lifecycle publication, then `build()` | selected |
| Dynamic supervisor | `dynamic(entries, activation, unexpected_exit, actor_drain, lifecycle, diagnostics)` | selected |
| FIFO pool | `fifo(factory, roles, activation, recovery, backlog, interruption, actor_drain, diagnostics)` | selected |
| Keyed pool | `keyed(factory, roles, selector, activation, recovery, backlog, bindings, interruption, actor_drain, diagnostics)` | selected |

Generated component rustdoc lists 102 semantic items for the five implemented
families: 43 structs, 53 enums, two traits, and four construction functions.
These are construction, policies, commands, capabilities, typed rejections, and
domain outcomes—not structural actor machinery. Reduce this count only when
two names own the same law; hiding a real outcome is not surface curation.

There is one external component path: `behavior_actors::atomic::Name`; the crate
root exposes zero duplicate atomic paths. Interpreter reexports are doc-hidden
at the facade or at their definitions. Generated rustdoc excludes sampled
effect products, request products, internal events, worker preparation,
activation operations, proxy control, and settlement carriers. They remain
statically reachable but are not candidates for Bombay's ordinary facade.

Bombay must selectively re-export only the semantic categories in
[`atomic-actor-devx.md`](atomic-actor-devx.md). It must not glob the component
module or expose request products, internal events, effect products, preparation state,
activation requests, proxy control, or settlement carriers.

All five component construction paths are selected. Bombay's selective facade,
role-first application assembly, and runtime settlement/custody changes remain
upstream integration work rather than component public-name questions.
