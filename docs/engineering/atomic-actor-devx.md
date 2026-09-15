# Developer-experience acceptance contract (engineering record)

Normative ownership: this is the single owner of canonical application
construction and usage syntax for all five families. Aggregate law documents
own transitions; [`atomic-runtime-settlement.md`](../atomic-runtime-settlement.md)
owns interpretation/custody; [`atomic-actor-verification.md`](atomic-actor-verification.md)
owns evidence gates. Other documents link here rather than repeating builder
orders or worker authoring syntax.

This document specifies what ordinary source must express for the five atomic
actors. All five component construction syntaxes are selected and implemented.
Bombay's role-first application assembly and runtime custody changes remain
upstream work; component construction does not claim that integration complete.

The earlier code snippets are withdrawn. They looked copy-pasteable while
leaving hosting, activation, effect lowering, replies, rejected delivery, and
shutdown undefined. They also exposed or assumed manual `Protocol`
implementations, runtime addresses, turbofish, handwritten behavior dispatch,
pseudo-`send`, and unproven initialization syntax. None is an accepted API.

## Application-defined workers

The application defines the actual worker as an ordinary concrete `Behavior`.
It may be one domain behavior or an application-defined closed sum of domain
behaviors. No `Worker` trait, runtime context, wrapper base class, callback, or
atomic worker behavior is required. A template receives that behavior through a
typed `WorkerSubmission` and owns only its relationship to the actor. In a pool,
the worker receives `Assignment<Job>` and returns its result with
`assignment.complete(result)`; it does not know the pool, customer, completion
route, structural parent path, or assignment identity.

## Canonical public path

The Behavior Actors component has one external spelling for this family:
`behavior_actors::atomic::Name`. Atomic names are not repeated at the component
crate root. This module path communicates ownership; it is not structural actor
routing.

Bombay must not glob-re-export `behavior_actors::atomic`. Its ordinary facade
selects only these categories from the implemented families:

- construction and behavior: `StableProxy`, `fixed`, `FixedSupervisor`,
  `dynamic`, `DynamicSupervisor`, `fifo`, `FifoPool`, `keyed`, and `KeyedPool`;
- worker definition and activation: `ActivationPlan`, `ImmediateActivation`,
  `WorkerSubmission`, `ActivationPolicy`, `EntryCapacity`, `BacklogCapacity`,
  `BindingCapacity`, and `ZeroCapacity`;
- required policy: `ActorDrainPolicy`, `DiagnosticDisposition`, `Recovery`,
  `Strategy`, `RestartLimit`, `RestartRelease`, `FailureReaction`,
  `UnexpectedExit`, `PoolRecovery`, `PoolFailureReaction`, and `Interruption`;
- application protocols: `FixedCommand`, `DynamicCommand`, their reply values,
  query/status values, cancellation authority and outcomes, lifecycle values,
  diagnostics, FIFO/keyed customer outcomes, binding commands and replies, and
  typed policy errors; and
- validated fixed topology: `OrderedRoles`, `DuplicateRole`, and the complete
  construction rejection.

Bombay's interpreter implementation imports doc-hidden request products,
effect products, internal events, worker preparation, activation requests,
proxy control, and settlements directly from the component module when their
associated types require it. Bombay does not re-export those names. Therefore
ordinary application autocomplete and rustdoc contain domain construction,
policies, commands, capabilities, and outcomes rather than runtime plumbing.

## Status and acceptance gate

The five component construction paths are `feature-complete`: each is
implemented, inferred from domain values, and covered by a compile-pass
contract. Bombay's role-first assembly and complete runtime journey remain
`active`. That upstream journey requires focused proof of all the following
together:

1. imports and an ordinary concrete worker definition;
2. construction with an inferred final behavior type;
3. hosting through the current concrete interpreter boundary;
4. initialization before activation;
5. immediate and asynchronous readiness;
6. command/job submission with concrete typed recipients;
7. request replies, durable lifecycle events, customer outcomes, and the
   selected diagnostic disposition;
8. rejected-delivery settlement after the source actor has stopped;
9. orderly and forced shutdown; and
10. pure-fold testing of the complete named `Actions` product.

The same prototype must include compile failures for each missing semantic
choice and each invalid capability exchange. A snippet containing `ignore`,
pseudocode, an undisclosed helper, or a support alias supplied only to appease
inference is not evidence.

## What users choose

Users choose only domain and policy values:

- semantic roles and dynamic keys;
- concrete worker definitions, jobs, and results;
- a fixed topology or dynamic entry capacity;
- immediate activation or a concrete statically dispatched activation plan;
- a positive maximum number of unresolved activation authorizations;
- recovery, strategy, budget, release, interruption, and `ActorDrainPolicy`
  values where those policies are meaningful;
- concrete request, lifecycle, customer, and diagnostic routes; and
- FIFO distribution, or keyed distribution with one key-to-role selector.

Users do not choose or construct child nonces, actor addresses, installation
attempts, generations, activation attempts, timer identities, job identities,
assignment identities, completion tokens, structural parent paths, effect
lane positions, or settlement owners.

## Construction laws

- A constructor or complete builder produces one concrete actor; users do not
  name its full generic return type.
- When a family has optional construction transformations, incomplete builders
  do not implement `Behavior` and have no `build` method.
- One documented order exists for each actor. Repeatable topology entries are
  the only deliberately repeatable step.
- A builder exposes no choice that its actor does not use. Dynamic supervision
  has no fixed factory or recovery placeholder; FIFO has no selector; pools
  have no proxy choice.
- Missing required choices are typestate, not runtime flags.
- Duplicate roles and other value-dependent failures are typed build errors.
- No valid construction supplies a no-op callback, dummy recipient, empty
  factory, marker route, reply alias, output alias, or explicit generic merely
  to satisfy implementation plumbing.
- Every route preserves its concrete logical, established, exact, or mixed
  delivery kind.

Diagnostic construction has exactly two semantic choices:

```text
DeliverTo(concrete route)
Terminate
```

`Terminate` is the autonomous case. There is no mandatory
`.diagnostics_to(...)`, optional route plus hidden default, or recursive
diagnostic fallback. A literal `Diagnostics<Route>::Terminate` is also
rejected because it leaves an unused route type to infer. The selected builder
must offer one route-bearing choice and one route-free choice without asking
the user to name their typestate result.

## Shared activation journey

Each fixed/dynamic supervisor and pool builder selects activation mode and a
positive limit on unresolved activation authorizations. A proxy has the
structural limit of its single pending worker. “Activation authorization” has
one user-facing meaning:
an owner has decided that a worker may progress toward `Ready`. A supervisor
reserves conservatively when it emits the opaque proxy install because it sees
no later progress facts; a direct pool reserves when it emits
`BeginActivation` after initialization settlement. The difference is a
documented template boundary, not two counting laws. The resulting journey is:

```text
build definition
  -> supervisor records activation authorization before proxy install
     (a direct pool waits until initialization settles)
  -> run the pure initialization fold
  -> establish the host and commit installed-but-not-ready
  -> interpret and settle initialization Actions
  -> direct pool records activation authorization
  -> activation consumes the installation permit and plan
  -> exact Ready or ActivationRejected fact
  -> only Ready publishes or accepts work
```

The ordinary worker author supplies a concrete activation value; they do not
send `BeginActivation`, mint an attempt, or route `ActivationResolved`.
Immediate activation uses a route/plan-free builder choice; it must not require
an annotation for an otherwise unused plan type.
Initialization failure, rejected initialization effects, and stopping during
initialization are observable pre-readiness outcomes; activation is never used
to hide or rerun them.
Cancellation recovers a worker definition only before the install/create
action that transfers it. Cancellation after that ownership boundary is
reported as pending—even if activation has not begun—and never claims that
external hydration stopped. Late readiness is drained and never published.

## Stable proxy journey

Most users never construct a proxy. Fixed and dynamic supervisors create and
control one per stable service identity.

A direct proxy owner must be able to:

1. construct an empty proxy without repeating the worker type;
2. retain a private exact control capability;
3. install the initial worker or request replacement by moving a definition;
4. give clients only the stable service capability;
5. exhaustively receive readiness, replacement, stop, unavailability, and
   contradiction reports; and
6. shut down while creation or activation is unresolved.

Application clients cannot address install, replacement, readiness, or
shutdown control. No public method sets a nonce, incarnation, or ready flag.

## Fixed-supervisor journey

The selected semantic order is:

```text
fixed(
    factory,
    ordered non-empty roles,
    activation policy,
    recovery policy,
    topology-failure reaction,
    ActorDrainPolicy,
    diagnostic disposition,
)
  -> optional lifecycle publication
  -> build and prepare every initial worker
```

The construction-only worker factory returns one `WorkerSubmission`: the worker
definition and the concrete activation work for that exact attempt. Immediate
workers use the inferred immediate construction; activated workers provide one
owned plan per factory result. A multi-role actor never clones or rediscovers
one plan stored beside the roster.

The successful supervisor does not store this callable or carry its type.
Automatic recovery obtains submissions through a typed batch worker source in
`Actions`. H38 selects the Bombay-interpreted typed worker-source action. An
ordinary factory actor remains valid application architecture, but is not
mandatory because accepted delivery still needs a later reply join. Canonical
recovery-source syntax therefore names the application worker source and its
typed rejection only. Bombay will implement the generic static interpreter and
retirement transfer; no runtime path, source-result route, callback, or composed
Behavior type appears in application code.

H52 proves the candidate diagnostic protocol for operating preparation failure.
The diagnostic actor names
`FixedDiagnostic<Role, Worker, Plan, Source>` directly as its domain message;
no application alias is required. `Source` is the already-selected recovery
capability type and statically selects its worker and source rejection types;
the source value itself is never placed in a diagnostic. Applications inspect
one owned `WorkerPreparationFailure` through `role()`, `prepared()`,
`remaining_roles()`, and one exhaustive borrowed
`WorkerPreparationFailureReason`. They never name immutable role storage,
preparation tickets, action settlements, request lanes, or parent paths. This
shape is implemented by the current fixed supervisor.

H53 lowers this protocol surface into the actors crate. Construction of the
payload from an operating recovery remains the next independent transition
stage; the public syntax no longer depends on that implementation detail.

The source implements method-free `WorkerSource<Role, Worker, Plan>` by naming
only its worker-level and whole-source rejection types. Applications never
construct `PrepareWorkers`, inspect `WorkerPreparation`, or name their generic
types. Those interpreter-facing products remain hidden behind the supervisor's
own recovery transition and the generic action-settlement path.

`Recovery` owns the complete automatic-recovery choice. Canonical construction
is `Recovery::permanent(source, strategy, limit, release)`,
`Recovery::transient(source, strategy, limit, release)`, or
`Recovery::temporary()`. Permanent and transient recovery retain the concrete
worker source that Bombay will invoke outside Behavior. Temporary recovery
carries neither a source nor ignored strategy, budget, or release values. The
activation policy owns only its positive authorization maximum; callers use
`ActivationPolicy::new(usize)` and receive `ZeroCapacity` instead of constructing
`NonZeroUsize`. The plan belongs to each worker submission. Passing all required values to
`fixed` avoids a user-visible chain of structural builder stages; omitting any
value is an ordinary missing-argument compile error.

The seven direct arguments were compared with a named policy product and another
typestate builder stage. The product would merely regroup unrelated choices and
the builder would expose more construction machinery without preventing another
invalid exchange. The direct form remains canonical because the factory, roles,
activation, recovery, topology reaction, drain policy, and diagnostics all have
distinct semantic types. Lifecycle publication remains the one truthful builder
transformation because it changes the resulting action protocol.

Roles are application domain values such as `SearchRole::Index`, not static
strings, child positions, or runtime addresses. The factory receives a borrowed
role and returns that role's concrete worker or a typed rejection. `build`
prepares roles in declaration order. `FixedConstructionRejected.workers` is one
`InitialWorkerRejection` containing the factory, every prepared member, the
unavailable role and reason, and all untouched roles; the outer value contains
only fixed policy. No partial supervisor exists.

The external lifecycle route is genuinely optional. Omitting it creates an
autonomous supervisor; it does not create a discard sink. Operational failures
still follow the selected diagnostic disposition. Route-free diagnostic
termination is inferred directly; it requires neither a dummy route nor a
route-type annotation.

All workers in one fixed supervisor expose one common public service protocol.
Different behavior implementations may be variants of a closed concrete sum,
but ordinary users should not need to hand-write forwarding solely because the
supervisor API failed to compose existing behavior forms. Capability-safe
heterogeneous public protocols remain a separate, unselected feature.

The lifecycle consumer, when configured, receives one flat semantic event sum
containing exact ready proxy capabilities and complete failure facts. It never
walks `.inner`, `.own`, `WithParent`, a slot number, or wrapper depth.

## Dynamic-supervisor journey

The canonical constructor is one `dynamic(entries, activation, unexpected_exit,
actor_drain, lifecycle, diagnostics)` call. Applications construct `entries`
with `EntryCapacity::new(usize)` and `activation` with
`ActivationPolicy::new(usize)`. Both return `ZeroCapacity`, but their values
cannot be exchanged. Every executable semantic policy appears once; there is no
staged builder, factory, callback, default, marker, or public proxy type.
`ActorDrainPolicy` is mandatory because global shutdown is executable.

A named policy product or another builder would only move these six unrelated
choices behind an extra public name, so the direct form remains canonical.
Bombay constructs the lifecycle and diagnostic peers first, passes their typed
capabilities to this call, and then activates the returned supervisor;
applications never name the final behavior type. H148 historically derived the
role-first inference shape with stable Rust. The real
`role_first_construction_returns_the_complete_behavior` integration test now
owns that syntax; H589 removes the superseded private facsimile. The earlier
H135/H137 fragments remain rejected because they fabricated capabilities before
the code under test.

Bombay must declare and establish application actors by semantic role before it
constructs the root. Its one-time application assembly supplies typed logical
or exact capabilities for those actors to the root constructor. The lifecycle
actor's concrete protocol then supplies the dynamic supervisor's key, worker,
and activation types; the diagnostic actor supplies its own concrete protocol.
The resulting root capability makes every command, including `Stop { key }`,
infer without an alias, annotation, turbofish, raw runtime address, structural
path, dummy route, or fixture capability. Bombay performs this assembly outside
`Behavior`; the constructor performs no I/O and is never stored in the
supervisor.

Bombay's assembly method names remain an upstream choice, but the required
source shape is role-first topology, typed peer capabilities, one pure root
construction, then ordinary delivery through the returned root handle. A
root-first API that asks the user to repair missing types is not compatible.
H148's frozen proof compiled all five commands through the returned typed
capability. Current dynamic integration tests exercise all five real commands,
and the real construction test retains the inferred behavior syntax. Removing
worker and activation provenance from the lifecycle capability failed at
construction; detaching `Stop` from that capability failed with E0282.

The durable lifecycle route is mandatory and configured once. A start,
replace, stop, query, or cancel request carries only its temporary reply route.
No request may become, replace, or transfer the durable owner. `Stop` is the
single public operation that drains and removes a ready or empty entry; there
is no second retirement command.

Start and replace replies distinguish admission from realization. Admission
returns an opaque `CancelAuthority`. Later readiness or failure goes only to
the configured durable lifecycle route. Cancellation has the complete
phase-sensitive outcomes selected by AA-20; only cancellation before
definition transfer returns the definition.

Start and replace share one direct result equation because their accepted
ownership is identical: `WorkerChangeReceipt` or `WorkerChangeRejection` with
an operation-specific reason. Stop uses its own direct `Result`. Cancellation
returns `CancellationReceipt::{Returned, Pending, Committed, Cancelled, Stale,
Draining}`; query returns its independent known/unknown reply. There is no
catch-all `DynamicReply` and no alias that merely renames `Result`.

Start preparation is transaction-local: successful admission commits the
creating-proxy entry and emits proxy creation in the same transition. Query
therefore exposes creating proxy, waiting for activation authorization,
awaiting proxy outcome, ready, empty, stopping, replacing, cancelling,
draining, or retiring—never a stored `Reserved` phase. It does not invent
worker progress absent from the proxy report protocol. An unknown key is a
distinct outcome.

## FIFO-pool journey

The selected construction spelling is one direct call:

```text
fifo(
    initial_factory,
    ordered unique roles,
    ActivationPolicy,
    PoolRecovery,
    BacklogCapacity,
    Interruption,
    ActorDrainPolicy,
    DiagnosticDisposition,
)
```

Every argument has a distinct domain type, so exchanging backlog and interruption is a compile
error. `BacklogCapacity::new(0)` is valid because zero means no waiting work. A named policy
product owns no new invariant; a fluent form adds missing-state machinery. Neither is retained.

The call prepares the complete roster before a pool exists. Success returns a
`FifoPool` whose type does not contain the callable. Failure returns
`FifoConstructionRejected`: its `workers` field is the shared
`InitialWorkerRejection`, beside only FIFO policy.
Automatic `PoolRecovery` owns its source, limit, release, and `RetireRole | StopPool` reaction.
Temporary recovery owns only the reaction and needs no source placeholder. No transition invokes
the factory, and users never name preparation or interpreter machinery.

Submission moves a caller request correlation, payload, and concrete customer
route to the pool. `Accepted` and `Rejected` both echo the request correlation;
the pool never trusts it as its internal job identity. Admission either returns
the payload in `Rejected` or creates one authoritative customer obligation
identified by a fresh opaque job value and immutable admission ordinal.
Terminal customer outcomes are exactly:

```text
Completed { job, role, result }
ReturnedQueued { job, payload, reason }
ReturnedAssigned { job, role, payload, reason }
```

The pool, not the worker, retains and selects the customer destination. Retry
is explicitly at-least-once execution: the pool keeps the authoritative
obligation and retained retry payload while a cloned execution payload is at
the worker. An interrupted retry is reinserted by its immutable admission
ordinal. This preserves admission order even when several active workers stop
in a different order; unconditional front insertion would reverse those jobs.

### Pool-worker authoring

The intended worker expression is now a production compile contract in
`crates/actors/tests/fifo_pool/compile.rs`:

```rust
fn transition(&mut self, assignment: Assignment<Job>) -> WorkerActed<Self> {
    let result = self.process(assignment.payload());
    Ok(Actions::cont().with_send(assignment.complete(result)))
}
```

The `behavior_actors::atomic::pool_worker` attribute is syntax derivation only.
It reads the authored `Assignment<Job>` input and declared result type, rewrites
the intentionally unresolved `WorkerActed<Self>` spelling, and emits the sole
ordinary `Protocol`, `Behavior`, and `BehaviorBase` implementations. There is
no public `WorkerActed` type, worker trait, adapter actor, callback, result
marker, or alternate transition. The worker body names no parent path, lane,
assignment identity, completion route, composed behavior type, or alias.

Exact source inspection and the compile experiment distinguish the mechanisms:

| Mechanism | Result |
|---|---|
| inherent `Assignment::complete` | cleanly constructs and consumes the affine completion value, but cannot select the worker's already-declared `Behavior::Sends` |
| extension trait or associated type | has the same associated-send boundary; a worker-specific associated trait would become a forbidden second actor trait |
| generic closure/builder | can infer a wrapper's output type, but changes the owning authoring form and does not make the shown inherent method a `Behavior` fold |
| typestate | proves builder completeness but does not solve send-product selection |
| stable return-position `impl Trait` | cannot supply the named `Behavior::Sends` associated type on stable Rust 1.95 |
| syntax-only macro derivation | eligible: it can preserve the authored method, rewrite `WorkerActed<Self>` during expansion, and emit the one concrete `Behavior` implementation with its completion send type |

The earlier private declarative stand-in remains historical evidence for two
unrelated workers and exact typed interpretation. Production now proves the
same generated completion capability through the actual
`ReceiveTimeout<Deadline<Worker>>` and `Deadline<ReceiveTimeout<Worker>>`
orders. Both composed send products satisfy total settlement and the one sealed
pool-completion capability without an adapter or placeholder reaction.

The production surface exposes only `Assignment<Job>` and opaque
`Completion<Result>` values. Rustdoc proves that external code cannot mint a
pool-issued job identifier, construct an assignment, forge a completion, or
complete the same assignment twice. It also rejects missing macro declarations
and a transition that receives an unrelated input. All 38 Actors rustdoc
contracts pass on stable Rust 1.95. Deliberately deriving the completion lane
from `Job` instead of the declared result fails at the authored
`assignment.complete(result)` expression with the exact `SearchJob` versus
`SearchResult` mismatch.

The complete canonical FIFO journey occupies 38 meaningful lines including
imports, domain types, worker, worker declaration, construction, and
initialization. The worker implementation occupies six meaningful lines and
the direct `fifo(...)` call ten. It needs no final aggregate annotation, helper
alias, turbofish, structural actor concept, interpreter request, or completion
route. The direct eight-domain-value constructor remains the single selected
construction spelling.

## Keyed-pool journey

The selected construction spelling is one direct call:

```text
keyed(
    initial_factory,
    ordered unique roles,
    key-to-role selector,
    ActivationPolicy,
    PoolRecovery,
    per-role BacklogCapacity,
    BindingCapacity,
    Interruption,
    ActorDrainPolicy,
    DiagnosticDisposition,
)
```

The callable factory exists only during construction; the successful
`KeyedPool` does not store it. `KeyedConstructionRejected.workers` is the shared
`InitialWorkerRejection`, beside the selector and keyed policy. The selector is
the one concrete `Fn(&Key) -> Role` value used by
future admission; it is not a boxed callback, runtime registry, or distribution
mode. `ActivationPolicy`, `BacklogCapacity`, and `BindingCapacity` are distinct
types, so their numeric storage cannot make them interchangeable.

Keyed admission has one selected model:

```text
Submit { key, payload, reply_to }
selector: concrete Fn(&Key) -> Role
```

There is no `KeyedJob` and no `SelectWorker` trait. The submitted key is
explicit; one statically stored concrete function or closure maps that key to
a role. FIFO accepts no selector.

The selected semantic order reuses FIFO's accepted-job and direct-worker value
laws but substitutes per-role backlog and keyed distribution for global FIFO
backlog and circular worker selection. Rebalance and unbind use typed
management replies, carry `Absent | Exact(binding_generation)` expectation,
and affect future admission only. Role-changing rebalance issues a fresh
generation; same-role rebalance is an accepted no-op preserving it. An absent
key may be bound either with accepted admission or by explicit rebalance.
Absence stores no `Unbound` entry, and accepted jobs retain only opaque
`{ generation, role }` evidence rather than a cloned key.

Committing permanent role unavailability automatically unbinds every retained
binding for that role, releases binding capacity, and emits exact
key/generation diagnostics. Temporary recovery retains bindings.
An idle ready target assigns immediately. Prepared, creating, initializing,
waiting-for-authorization, activation-dispatched, activating, busy, recovering,
or pre-ready-draining-with-retained-recovery targets may admit only to their
own backlog when capacity remains. Stopping, retired, terminally draining, and
shutdown-owned targets reject without retaining either a new binding or the
submitted job; zero backlog therefore accepts a fresh key only when its
selected target is ready-idle.

Rebalance uses a separate exhaustive target projection because it carries no
job: every admission-backlog phase above, including temporary recovery, is
bindable without consulting queue capacity, while stopping, retired,
terminally draining, shutdown-owned, and unknown targets reject the complete
command. Stale absence or generation expectation is rejected before target
validation and cannot mutate a later binding.

## Bombay hosting and sending

The component construction and command spellings are selected. Bombay's
application-level hosting spelling remains upstream work. Its accepted example
must reuse the concrete interpreter contracts after the documented Bombay
changes; it may not
invent `RuntimeAddr::application()`, a global envelope, implicit ambient
runtime, or a template-specific host. Likewise, application communication
must use the existing typed delivery values rather than a pseudocode `send`.

This is a usability requirement, not permission to widen core constructors or
add convenience traits. If the current boundary cannot express the journey,
the prototype must identify the precise missing semantic law before any new
production symbol is proposed.

A forced actor-graph summary is not the final application return while an
external activation or late exact drain remains unresolved. The root run
future continues to own and process that residual settlement and resolves only
after it is empty. Ordinary source must not receive a cloneable residual
report and accidentally treat it as completed shutdown.

`ActorDrainPolicy` has exactly `WaitForActorGraph` and
`RetireActorGraphAfter { deadline }`. The latter retires the represented actor
graph after the deadline; it does not promise bounded process exit. No separate
process-exit policy is selected in this design.

## Pure-fold testing

Each template must expose its ordinary `Behavior` initialization and event
fold to tests. A test supplies concrete typed facts, consumes one state plus
one event, and asserts the complete named `Actions` product and next verdict.
It does not spawn tasks, sleep, consult a clock, predict nonces, discard
actions, or use a test-only dynamic envelope.

Activation, monotonic time, delivery rejection, late settlement, child stop,
completion, and shutdown are typed inputs. Interpreter witnesses separately
prove that those facts can be produced and that settlement survives the
source actor.

## Required compiler diagnostics

Focused compile-fail fixtures must reject:

- build before every mandatory semantic choice;
- fixed or pool topology with no role;
- zero activation-authorization limit;
- a worker with the wrong service or assignment protocol;
- completion with the wrong result or without the issued assignment;
- worker selection or substitution of the customer destination;
- a FIFO selector, or keyed construction without its one selector;
- cross-domain key, worker, reply, lifecycle, or operation capabilities when
  their semantic Rust types differ;
- use of an established proxy capability as another protocol; and
- application construction of attempts, generations, job IDs, completion
  tokens, parent paths, or settlement owners; and
- ordinary application errors that expose a topology-derived settlement
  product, occurrence path, or type whose diagnostic grows with wrapper depth;
- a terminal lift that omits a heterogeneous child variant or exact source
  provenance; and
- treating an actor-side forced summary as the final root-run result while
  residual activation ownership remains.

Diagnostics are measured at the missing or invalid semantic choice. An error
whose useful cause is hidden inside nested associated types, positional effect
paths, or a repeated turbofish fails the DevX gate even if the invalid program
eventually fails to compile.

Runtime instances with the same public protocol are deliberately not assigned
compiler-only owner brands. For DynamicSupervisor, an authority issued by
another same-signature instance is accepted as typed input and returns
`CancellationReceipt::Stale` by AA-20's exact operation correlation. Adding an
owner marker solely to turn that runtime law into a compiler denial would add a
generic parameter with no application substitution law.

## Copy-paste deliverables

Before the API can be called selected, the repository needs five complete,
warning-clean examples—proxy owner, fixed supervisor, dynamic supervisor,
FIFO pool, and keyed pool—plus a sixth example showing pool assignment
completion through two wrapper orders. Each must cover the entire acceptance
gate at the top of this document. At present none exists; this is recorded as
an implementation blocker, not concealed with aspirational snippets.
