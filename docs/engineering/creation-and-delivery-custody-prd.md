# PRD: Complete interpreter ownership and startup contracts

Date: 2026-09-29. Status: Behavior-owned contracts implemented on the
repository-quality release branch; cross-repository P5 acceptance pending.
The [implementation ledger](interpreter-contract-implementation-ledger.md)
separates local proof from the future Bombay/Address runtime witnesses. This
revision supersedes the 2026-09-28
creation-and-delivery proposal in this file.

## 1. Objective and implementation baseline

Make the **current Behavior and Behavior Actors code** fully interpretable by
Bombay through small, concrete capability interpreters. Complete rejected
request return, retained acceptance, and startup custody before attempting a
broader redundancy refactor. Preserve the existing pure policies and aggregate
transition authorities.

The implementation baseline is current Behavior commit
`6ef850f47852b4a90a6eae7633757a9e5a1c9d6b`, not the older registry release.
The review began at `de8f1bf264366da29992c0061d5bae9888353bd5` with 37 modified
files; those pending implementation/test changes were committed during review
as `b46923c`; `6ef850f` then updated the audit's follow-on work description.
They are part of this baseline, not changes to revert or recreate.
At implementation start, record the then-current merged commit or complete tree
snapshot and recheck the seven contract files fingerprinted in section 13.

The adjacent Bombay working tree was also inspected, based on HEAD
`a7a66e3912731923015560463cd2c9e5e76fc041` with substantial pending changes.
Observations below about Driver, launch, and local hosting refer to that current
source. Its lock still selects Behavior/Actors 0.17.0 and Address 0.2.0. That is
an integration constraint to update, not the upstream design baseline. Do not
make current source conform to an obsolete locked signature.

The deliverable of this review is this PRD. It does not claim that its proposed
methods compile, that startup has already been repaired, or that all catalogue
machinery is redundant. A prescribed API below must first pass its specified
external regression; compiler fallout cannot invent additional architecture.

### Success criteria

- A runtime can attempt real delivery of `AssignWorker` and `ProxyOperation`
  and return the complete original request on rejection, without cloning during
  settlement or assembling private correlations. FIFO's existing retry policy
  deliberately copies a `Clone` job before delivery and retains the original.
- Explicitly retained acceptance survives continuing turns through the existing
  generic custody path. Ordinary discharged acceptance does not accumulate.
- Fresh commitment, initialization completion, endpoint publication, and worker
  activation have distinct, implementable meanings. Initialization runs once.
- Every supported failure has an exact outcome and a surviving owner for every
  value the protocol still owns. Consumed actions are never reconstructed.
- A downstream fixture using public APIs, real Communication, and real Driver
  proves the contracts. Manually manufactured successful host results do not
  count as that evidence.
- Runtime code contains no duplicate pool, proxy, or supervisor transition law.

### Scope boundary

| Behavior / Actors owns | Bombay and primitive runtimes own |
|---|---|
| Pure policies and aggregate state | Delivery and admission attempts |
| Typed requests and source correlations | Tasks, clocks, endpoint publication |
| Consuming settlement and complete rejection return | Concrete initialization execution and startup evidence |
| Accepted-value retention declaration and static composition | Retained runtime values and parent-to-root retirement |
| Readiness, recovery, assignment, and supervision decisions | Observation of actual execution and termination |

Do not add a supervisor abstraction, runtime registry, erased envelope, second
Driver, callback-driven behavior, or lifecycle engine. Do not change FIFO/keyed
selection, fixed/dynamic supervision policy, or actor cardinality to make their
implementations look alike. Distributed execution, Entity redesign, Mnesis,
Selo, and transport replacement are outside this project.

## 2. Current-code findings

### Verified contract defects

| ID | Current owner and evidence | Consequence |
|---|---|---|
| D1 | `actors/src/atomic/pool/assignment.rs`: `AssignWorker::into_parts` exposes target, assignment, receipt; `returned` is restricted to `crate::atomic`. `receipt()` can separately reproduce receipt evidence. | Actual transport rejection returns the assignment but an external interpreter cannot reconstruct the complete request lawfully. Public decomposition also separates evidence that ought to stay together during settlement. |
| D2 | `actors/src/atomic/stable_proxy/operation.rs`: private request fields, public `into_parts`, and public `ProxyInputReceipt::new`. | A runtime can issue acceptance, but cannot return a rejected control after consuming the operation. Independently supplied creation, endpoint, and operation evidence can be assembled in the accepted constructor. |
| D3 | `behavior/src/effects/sending.rs`: `SourceSettlementCustody` for `Vec<ActionItemResult<Item>>` unconditionally returns `Exhausted(self)`. `DiagnosticAccepted::Terminal` owns its diagnostic. | Continuing Driver execution drops terminal evidence. The same blanket path also warrants correction for rejected, blocked, corrupt, and untouched requests; they are residual ownership, not discharged receipts. |
| D4 | `behavior/src/actor/creation.rs`: `HostRejected` requires current child plus untouched initialization `Actions`. | It cannot represent a failure after consuming those actions. Adding a reason or exposing a constructor cannot solve this ownership mismatch. |
| D5 | Current Bombay `establish_child` waits for `spawn_owned_with` publication; its panic/cancel/early-end branch panics. `SpawnError::from_local` treats settlement failure as unreachable. | The runtime cannot currently realize the documented established-child/initialization-result distinction for all startup outcomes. |
| D6 | Current Bombay `LocalEnvironment::activate` calls Address `try_claim` before `commit(actions)`. Address makes a claimed endpoint resolvable immediately. | Publication callback delay does not hide the endpoint. Moving the fallible claim after effects would create the D4 failure instead. |
| D7 | Current Driver calls `environment.publish()` in rejected-initialization, corrupt-initialization, and accepted-initialization-stop branches. | Hidden Address reservation alone cannot satisfy the selected no-publication law; these generic startup branches must retire without publishing. |

Paths in this table are relative to `crates/` in the named repository.

### Existing machinery to retain

- `ItemSettlement`, `SettledItem`, `Interpretation`, and `ActionSettlement`
  already own total ordered interpretation, rejection, corruption, and suffixes.
- `SourceActions`/`SourceSettlements` already return exact results to a live
  emitter. `SourceCustody::{Exhausted, Retained, Admitted, Closed}` already
  distinguishes terminal retention from live ingress.
- Driver already keeps `Retained` settlements across continuing turns and
  sends them to retirement. Fix D3 upstream; do not special-case diagnostics in
  Driver or retain every successful receipt there.
- `InitializeWorker::resolve` already reunites a host result with the original
  plan and issues the correlated activation permit. It is not permission to
  initialize a definition a second time.
- `WorkerInitializationFailure` already distinguishes rejected effects from
  interpreter corruption. Child action settlements belong in runtime custody,
  not in a new supervisor error sum.
- `ShutdownEstablished::settle` demonstrates an owner consuming a request at
  an interpreter seam. `CustomerDelivery` demonstrates complete rejected-route
  custody. Neither proves a universal delivery wrapper is necessary.

### Documentation conflicts to fix with implementation

`atomic-runtime-settlement.md` and `actor-transition-algebra.md` commit a fresh
host before interpreting initialization effects. `adapter-contract.md` also
contains an effects-before-endpoint sequence and a general short-circuit
failure statement. Reconcile these with section 6, including the difference
between ordinary rejection and corruption. Do not treat stale historical
engineering proposals as another normative contract.

The generic testkit Driver explicitly accumulates actions without interpreting
them. `proxy_operation_settlement.rs` presently proves type compatibility using
an absent value, not rejected transport custody. Existing creation-order tests
use a model host. These are useful tests with narrower claims than downstream
execution; retain that distinction in reports.

## 3. Laws and authority

| Law | Classification | Required implementation consequence |
|---|---|---|
| Newly allocated actor identity is fresh; creation and behavior replacement are different operations. | Actor research | No collision overwrite, reused endpoint substitution, or inferred replacement provenance. |
| A pure transition returns explicit communication, creation, and next-behavior effects. | Bombay's typed realization of actor operations | Execution remains outside `Behavior`; no Tokio or hidden delivery in transitions. |
| Rejection returns every still-owned input; acceptance transfers the payload exactly once. | Derived affine protocol | The request owner retains correlation during interpretation and reconstructs only from the actual returned payload. |
| Success status and remaining custody are independent. | Derived composition law | An accepted terminal value can have `Accepted` status and `Retained` custody simultaneously. |
| Commit fresh creation before dependent same-action operations; interpret initialization before ordinary transitions. | Bombay policy | Dependent operations see the committed child, and initialization is never replayed. |
| Public logical resolution remains absent until successful continuing initialization. | Bombay publication policy selected by this PRD | Use exclusive hidden reservation; private host commitment is distinct from publication. |
| Rejection continues independent effects; corruption preserves prefix, faulting value, and untouched suffix. | Existing Bombay interpretation law | No `?` that loses siblings; no rollback fiction. |
| Each aggregate chooses its own complete next state and actions. | Library design constraint | Request settlement performs transfer/reunion only, never recovery, retry, or supervisor transitions. |

Primary research checked for this revision: Agha, Mason, Smith, and Talcott,
*A Foundation for Actor Computation*, §3, especially pp. 19–20, distinguishes
fresh `newadr`, initialization, and receptionists. It does not prescribe
Bombay's startup transaction, rejection sums, readiness, or publication API.
[Primary paper](https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf).

Owner-controlled reconstruction follows the information-hiding criterion:
request representation belongs to its owner, execution to its interpreter.
This is a design inference, not an actor theorem.
[Parnas](https://www.cs.lafayette.edu/~gexia/cs301/resources/parnas.html).
The aggregate remains responsible for its invariant; subordinate protocol
values do not become transition authorities.
[Evans, Aggregates](https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf).
Use concrete types to express the resulting capabilities and ownership.
[Rust API guidance](https://rust-lang.github.io/api-guidelines/type-safety.html).

## 4. Delivery settlement contract

### 4.1 Compare complete ownership equations first

| Dimension | `AssignWorker<P, Job>` | `ProxyOperation<Source, Worker, Plan>` |
|---|---|---|
| Payload | `Assignment<Job>`, including completion authority | `ProxyControl<Worker, Plan>`; start/replacement carries definition and plan, shutdown does not |
| Destination | Already-established exact worker recipient | Creator-local proxy creation resolved to an exact private control endpoint |
| Evidence held by owner | Assignment receipt/correlation | Creation ID, affine operation ID, and static source identity |
| Accepted receipt | Original assignment receipt | Original creation/operation evidence plus exact resolved `EstablishedActor<StableProxy<...>>` |
| Expected rejection | `ExactDeliveryReason` | `ChildInputReason`, including missing binding or closed control |
| Subsequent result | Worker completion, possibly racing exit/admission | Proxy outcome, possibly racing exit/admission |
| Live settlement destination | `Source = Self` | Declared `Source` |

They share the **owner retains evidence while a lower capability consumes or
returns the payload** law. They do not share the same destination resolution,
receipt, rejection, or later outcome. Keep both concrete request types. Reuse
`ItemSettlement` and existing delivery capability; do not introduce a shared
public envelope or a universal reconstruction trait.

FIFO's accepted-worker retry policy retains its original `Job` while the worker
receives a clone. A rejected assignment returns that retained original to the
pool; the settlement method must not create another copy. A job type without
`Clone` cannot satisfy this policy after accepted delivery and worker loss.
Support for move-only FIFO jobs would require an explicit at-most-once policy
or a worker protocol that returns the job, with different failure semantics.
That policy is outside this PRD.

### 4.2 Required external syntax and transfer

The intended interpreter call for each request is a consuming
`request.settle(&mut delivery_capability).await`. This operation exists only
at effect interpretation. It returns that request's existing
`ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>`.
It does not return `SettledItem`; product traversal owns attempted/untouched
provenance. No application constructor or wrapper-depth argument is added.

**Assignment implementation:** inside the owning module, separate the receipt
from `EstablishedDelivery<P>`, invoke the existing
`InterpretItem<EstablishedDelivery<P>, RootEvent, Path>` capability, and reunite
its returned delivery with the receipt. Root/event path generics belong only
on this interpreter method if needed; they must not enter pool construction.

**Proxy implementation:** the lower operation takes the original `CreationId`
and `ProxyControl`, resolves the exact private proxy, and returns either its
exact established actor after control admission, or the same control and its
reason/fault. It must never receive the private operation ID. The owner adds
that held ID and the held creation correlation to acceptance or rejection.

The existing `ChildInput` accepted unit does not provide the exact actor needed
by `ProxyInputReceipt`. Select one narrow static interpreter seam,
`ProxyControlAdmission<Worker, Plan>`, with one admission method taking those
creation/control values. Its return is the existing `ItemSettlement` shape
with `ProxyControl` as returned item, exact proxy actor as accepted value,
`ChildInputReason` as rejection, and `Never` as prerequisite. No new result
wrapper is needed. The seam is an intentional third-party interpreter port;
its generic parameters select concrete worker/plan protocols, not policy.

The port's implementation must use the containing `ProxyOperation` interpreter's
statically selected child occurrence. Equal numeric IDs in different creator
namespaces or occurrences must remain distinct. Do not add runtime type lookup
or guess the child role from the ID.

This specifies an API candidate with one new interpreter trait, not a proven
signature. Before production, compile the external syntax with two concrete
worker/plan substitutions and a real proxy control rejection. If existing
static child admission can return the same exact evidence without a new seam,
use it and delete this proposed trait from the ledger. Do not ship both ports.
No implementation agent may widen the seam to implement lifecycle policy.

### 4.3 Complete settlement table

| Lower-capability outcome | Assignment owner | Proxy owner |
|---|---|---|
| Accepted | Consume payload once; return the held receipt | Consume control once; join exact admitted actor with held creation/operation evidence |
| Rejected | Return exact target, actual returned assignment, and held receipt as `Self` | Return original creation, actual returned control, held operation, unchanged source as `Self` |
| Corrupt before transfer | Return complete request and exact fault | Return complete request and exact fault |
| No committed proxy binding | Not applicable to an exact recipient | Return complete request with existing missing-binding reason |
| Untouched after earlier product corruption | Product retains original request unchanged | Product retains original request unchanged |

Both current prerequisites are `Never`; do not add a synthetic blocked state.
Acceptance proves admission, not execution, completion, readiness, or restart.
A close between resolution and admission is a real rejection, not corruption
and not a preflight-liveness success.

The lower interpreter is trusted to return the actual payload and destination
it received. Rust cannot distinguish all same-typed runtime values or prove a
foreign interpreter honest. The owner API must make receipt substitution
unavailable to ordinary callers, keep evidence out of the lower port, and test
runtime identity preservation under two simultaneous requests. Do not claim a
compile-fail test proves same-type runtime identity.

### 4.4 Close the old assembly surface

After the external proof succeeds, remove or narrow external
`AssignWorker::receipt`, `AssignWorker::into_parts`,
`ProxyOperation::into_parts`, and `ProxyInputReceipt::new` wherever they permit
independent construction of evidence. Keep only necessary read-only inspection
and owner-internal terminal decomposition. `AssignWorker::returned` remains
owner-private or is inlined into its sole owner path; it must not become the
public fix. Inventory all legitimate downstream consumers before narrowing.

Negative fixtures must reject external receipt assembly, double settlement,
wrong protocol/source/occurrence, and using an operation after moving it.
Tests inside the owning module do not establish these privacy guarantees.
Cancellation must not drop the only settlement future while it owns the
request. Bombay keeps the transfer alive to a settled result or retains its
actual owned in-flight work through retirement; neither method creates a task.

## 5. Generic retained acceptance

### 5.1 Selected representation

Keep acceptance status separate from custody. Add an owner-controlled consuming
operation to the existing `ActionItem` contract, with the intended signature:

```rust,ignore
fn retain_accepted(accepted: Self::Accepted) -> Option<Self::Accepted>;
```

`Some(value)` is exactly one accepted value still requiring terminal custody;
`None` means that receipt has been discharged. This is a residual value, not a
boolean semantic flag or a second settlement vocabulary. Provide the ordinary
discharged default on this existing method so existing unit-receipt items do
not acquire a mandatory no-op policy, extra bound, or marker. The method's
documentation must say that its default permits destruction of the receipt.

`DiagnosticAction` overrides it: `Delivered` discharges; `Terminal(value)`
returns that exact accepted value. Review every current `type Accepted` owner
before retaining the default. Receipts carrying remaining authority must opt
in where they actually use the source-free custody path. Do not apply this
operation to `SourceSettlements`: their receipt must first return to the source.

The proof needs both retained and discharged real requests, not a new generic
accepted wrapper at every call site. Keep `DiagnosticAccepted` as its current
concrete product unless the proof demonstrates that replacing it removes more
machinery than it adds. No new public type is needed for this design.

### 5.2 Source-free collection algorithm

When offering a `Vec<ActionItemResult<Item>>` to custody:

1. Consume its members in their existing order.
2. For attempted acceptance, use `Item::retain_accepted`; omit only discharged
   receipts, retain accepted values returned as `Some` unchanged in kind.
3. Keep complete rejection, blocking, corruption, and untouched items.
4. Return `Exhausted` only for an empty residual; otherwise return `Retained`.
5. Perform no send, source admission, reinterpretation, or automatic retry.

Classification remains independent: retained acceptance is `Accepted`, ordinary
rejection is `Rejected`, and corruption/untouched work is `Corrupt`. Driver must
inspect interpretation status before custody compacts discharged values. A
residual is not a complete historical success log. Do not reinterpret its
absence of discharged receipts as evidence those effects never occurred.

The accepted-retention operation is idempotent on its retained values: once it
returns `Some`, reapplying it must preserve that same value as `Some`. It cannot
perform effects or consume authority that its result still promises to own.
Re-offering a retained residual preserves it. A product with an earlier retained
lane still offers a later source lane. If that later lane is admitted, the
product returns `Admitted` with the retained sibling; after processing the
source input, the next offer still returns that sibling. Source closure keeps
the entire remainder. Preserve existing inner-before-owned `SendLayer` order
and named product order.

Audit `offer_source_in_order`, `ActionSettlement` composition, Actors'
`send_product!`, and generated/handwritten products. Change only machinery whose
current implementation fails this law. Driver's current `Retained` path should
need regression coverage, not diagnostic-aware production code.

### 5.3 Memory and retirement law

After any number of continuing turns, stored custody grows only with values
whose ownership is still outstanding. Successful unit receipts and delivered
diagnostics cannot create a growing retained history. Deliberately retained
diagnostics do occupy memory until retirement; do not silently cap or drop them.
Changing diagnostic disposition or bounded-retention policy is separate work.

At stop, source exhaustion, transition error, source closure, or interpreter
failure, every retained value travels through the same typed retirement product
as final behavior and runtime residuals. No actor must stop merely because it
selected terminal custody for one diagnostic.

## 6. Creation, initialization, and publication

### 6.1 Commitment decision

Retain the current normative **host commitment before effect interpretation**
law. Reject the proposed shortcut of moving a fallible address claim after
consuming initialization actions. Select Address-owned hidden reservation and
consuming publication as the runtime realization. This work is a dependency
of complete startup acceptance, not code to place in Behavior.

The coherent sequence is:

```text
exclusive fresh reservation, absent from logical resolution
  -> pure initialization exactly once
  -> private host and exact creator-local binding committed
  -> Established becomes true; host owns child and initialization work
  -> total initialization-effect interpretation exactly once
  -> initialization status recorded; source settlements processed lawfully
  -> continuing successful child may be publicly published
  -> ordinary mailbox transitions permitted
```

For atomic workers, initialization additionally supplies one activation permit;
the aggregate's existing `BeginActivation` and activation result decide service
availability. Worker initialization success is not application readiness. A
stable proxy may be publicly present while its worker is unavailable, according
to its existing service policy.

`Established` proves a fresh concrete child host and committed occurrence/ID
binding with explicit `CreationKind`. It does **not** prove initialization
effects accepted, endpoint publicly resolvable, child still live, activation
finished, or service ready. A child may stop before the creator consumes that
creation result. Success must contain `EstablishedCreation::Installed`; the
existing nested rejected alternative must never be emitted as successful host
commitment.

Bombay must separate its internal child-host commitment acknowledgement from
the public spawn/publication result. Parent `establish_child` must not wait for
public readiness as its evidence of commitment. Root spawning can continue to
wait for publication, but its failure must return the full unpublished terminal
custody. Use the existing host/binding/task ownership; do not add another host
registry or recreate proxy state in a spawn service.

Correct Driver's three early terminal initialization branches to retire without
calling public `publish`. The private commitment acknowledgement must already
have transferred child ownership, so this does not strand a parent waiting for
its creation result. Retain public `publish` only on the continuing successful
path after required source-settlement processing. A source transition that
stops or fails before that point also remains unpublished.

Replace `SpawnError::from_local`'s unreachable settlement case with an owned
unpublished terminal return, preserving the existing concrete local outcome:
final behavior, all action settlements, admitted control/user inputs,
activation work, descendants, and factual disposition. Do not project it to
bare `Ended(Completion)` or a coarse reason while dropping the residual. The
root consumer either receives that product or its exact typed terminal
projection. The design-stage external fixture must compile this product before
changing broad spawn signatures or adding projection bounds across callers.

### 6.2 Address reservation requirements

The Address owner must supply an affine reservation that excludes competitors
without making the endpoint resolvable. Registration identity allocation,
collision checks, and generation exhaustion happen before irreversible effects.
Publication consumes that exact reservation without a second fallible claim or
new generation allocation. Release and published lease retirement affect only
their own generation; stale retirement cannot remove a later registration.

No endpoint reference granted by reservation alone proves a committed birth.
The host and creator-local binding must exist before issuing `Established`.
Private parent reports, exact child control, and lifecycle observation must work
without public logical resolution. Public logical self-send during unpublished
initialization gets the ordinary absent-address rejection; do not install a
hidden resolver exception. Exact/private traffic can be admitted according to
its capability, but must not run ordinary child transitions before initialization.

Prove bounded-capacity progress for nested creation and early child reports.
Never wait for a parent's initialization to complete while that parent waits
for the same child's public publication. If reservation cannot meet these
requirements, stop the startup stage, record the counterexample, and revise
this PRD before production; do not silently switch commitment semantics.

### 6.3 Ownership equation and complete outcomes

Before the pure fold, the interpreter owns the routed creation. After a
successful fold and before commitment it owns **current child + untouched
Actions**. After commitment the child host owns execution and its effects;
after interpretation it owns **current child + exact settlement + unfinished
runtime work**. These products are not interchangeable.

The outer `ItemSettlement::Accepted(ChildCreationOutcome::...)` records that
the establishment port accepted ownership and returned an outcome. Only the
inner `Established` result proves a birth; an outer accepted item containing
initialization rejection or panic must never be counted as a successful child.

| Event / milestone | Creation result | Initialization/terminal result and custody |
|---|---|---|
| Batch routing refuses | Existing whole-batch rejection | No child fold; unchanged batch and namespace reason remain owned. |
| Allocation/reservation refuses | Existing item rejection with complete routed creation | No child fold; exact runtime allocation failure remains available where projected reason is coarser. |
| Pure fold returns `Err` | `InitializationRejected { creation, error }` | Current child, exact error, original correlation/provenance; no Actions were returned and no binding committed. Release reservation. |
| Host refuses after successful pure fold | `HostRejected { creation, initialization, reason }` | Current child and untouched Actions only. Release reservation. No accepted effects, readiness, birth, or restart. |
| Host commits | `Established` with installed evidence | Host owns initialization once; later failure does not undo this birth. |
| Expected effect rejection after accepted prefix | Still committed `Established` | Interpret independent later effects. `EffectsRejected(EffectsRejected)` report; exact accepted/rejected settlement and descendants stay with host. No public publication. Drain child. |
| Interpreter returns typed corruption | Still committed `Established` | `EffectsRejected(InterpreterCorrupt)` report; complete prefix, faulting request, untouched suffix, and runtime work remain owned. No publication. |
| Initialization selects `Stop`, all effects accepted | Still committed `Established` | Settle final effects, produce exact normal stop, never publish or run ordinary ingress; `InitializeWorker` resolves `Stopped`. |
| Initialization selects `Stop` with rejected/corrupt effects | Still committed `Established` | Corruption outranks rejection; either outranks stop for initialization classification. Preserve the stop decision in settlement and independent termination evidence. |
| Successful continuing initialization | `Established` | Record success, settle source custody, permit publication; later request can resolve `ReadyForActivation` once with its plan and permit. |
| Cooperative owner shutdown before child fold | No false commitment | Return untouched creation using existing environment-failure projection; retain exact cancellation disposition in runtime custody. Do not relabel it as initialization failure. |
| Cooperative owner shutdown after fold, before commitment | `HostRejected` | Return current child and untouched Actions with environment-failure projection, retaining cancellation disposition downstream. |
| Shutdown after commitment, before publication | Committed birth remains true | Close publication/ingress, settle or retain accepted work, then exact cancellation/stop and complete retirement. Return plan/report if source has closed. |
| Child panics after commitment, before publication | Committed birth remains true | Authoritative panicked termination; no readiness or public endpoint. Preserve extant host-owned custody; apply the panic limits below. |

A committed replacement is a fresh birth explicitly designated as replacement.
Only commitment authorizes a creation-level restart diagnostic. Successful
service replacement still requires the proxy's existing ready outcome. Neither
an attempted replacement nor an uncommitted failure produces successful restart.

### 6.4 Pure-initialization panic and cancellation limits

A panic in the pure initialization fold is not `C::Error`, not an accepted
initialization, and not `HostRejected` with invented empty Actions. The current
creation sum lacks that distinction. The selected minimal extension is
`ChildCreationOutcome::InitializationPanicked { creation }`, retaining the
current routed child, ID, route, and kind. Mirror it as
`WorkerCreationRejection::WorkerPanicked { worker }` when settling a worker;
proxy creation retains the original generic settlement. It grants no actor
capability, retry permission, or guarantee that a partially mutated child is
safe to run again. It is a creation outcome, not a new aggregate control state.

Write a focused external panic witness before adding these variants. Bombay
must catch the pure fold while the current child remains in an outer owned
slot, release its reservation, and return this outcome. Do not downcast, erase,
or put a panic payload in Behavior. Any exact runtime panic evidence belongs
to the existing runtime termination reporting contract. Reconcile exhaustive
creation matching and both public rustdocs in the same design stage.

Pure initialization is synchronous. Cooperative cancellation is observed before
or after it, not by inventing a half-completed successful Actions value. During
asynchronous effect interpretation, cancellation must preserve an already-owned
transfer until it settles or is retained by the existing custodian. Aborting a
task or dropping a future that owns the sole payload is not successful custody
transfer.

The guarantee has a necessary limit: arbitrary Rust code can move an input
into a local and panic, destroying that value during unwind. Neither a new enum
nor `catch_unwind` can reconstruct it. Process abort and forced destruction of
the owner likewise cannot promise recovery. Tests must distinguish (a) typed
rejection/corruption with complete return, (b) cooperative shutdown with retained
work, and (c) panic with truthful termination and recovery of **extant** outer
custody. Never assert recovery of a payload already destroyed by arbitrary
panicking user code. Panic after a transfer must not fabricate the original
request or report successful receipt. If a runtime path loses extant custody
merely by where it stores its task locals, repair that runtime path.

### 6.5 `InitializeWorker` is settlement observation

Accepting this request transfers the original plan to the already committed
child's host. The host observes its actual initialization result, then calls
`resolve` once. It must never invoke `initialize`, replay effects, or create a
replacement child to satisfy this request.

| Available evidence when the request is resolved | Report |
|---|---|
| Initialization corrupt | `EffectsRejected(InterpreterCorrupt)` |
| Initialization rejected/blocked | `EffectsRejected(EffectsRejected)` |
| No initialization failure, exact termination already known | `Stopped` with that exact stop |
| Successful continuing initialization, no terminal evidence yet | `ReadyForActivation` with original plan and one permit |
| Initialization still executing | Keep the request/plan owned until an outcome is known; do not fabricate success |

Failure evidence takes priority over a simultaneous stop. If readiness was
already issued and termination arrives later, keep both facts; do not retract
or issue a second initialization report. Ordinary observation and initialization
observation are independent consumers of the same execution evidence. Neither
may steal the other's notification. Store evidence in the existing typed child
host, scoped to its concrete endpoint and retirement lifetime, not in a global
cache or protocol registry.

A request arriving after child termination must still settle using retained
host evidence. Closed source admission returns the complete report, including
plan or permit, to retirement alongside the child's settlement. Missing typed
host support is `InterpreterFault::MissingCapability` with the untouched
request, not successful `Stopped`, empty evidence, or a fabricated mailbox
rejection. Duplicate/foreign reports remain subject to the existing aggregate
correlation rules and must not issue another activation permit.

## 7. Repository-wide impact and simplification

| Surface | Required work | What must remain unchanged in meaning |
|---|---|---|
| `behavior/src/effects/sending.rs` | Accepted retention declaration; complete vector residual custody | SourceActions transfer, interpretation order and static result shape |
| `behavior/src/effects/actions.rs` | Verify creation/send custody joins; change only a failing join | Current pending creation-custody fixes and exact step |
| `behavior/src/actor/creation.rs` | Precise commitment docs; pure-fold panic outcome | Freshness, occurrence, ordered batches, owned failures |
| `behavior/src/{lib.rs,effects/mod.rs}` | Curate only necessary public exports/docs | No runtime dependency, umbrella request trait, or new user syntax |
| `actors/src/atomic/pool/assignment.rs` | Consuming settle and removal of independent receipt assembly | Customer/job conservation, completion authority, retry order |
| `actors/src/atomic/stable_proxy/operation.rs` | Consuming settle and private receipt construction; narrow interpreter seam | Source, operation, creation, control, exact endpoint |
| `actors/src/atomic/diagnostic.rs` | Declare retained versus discharged acceptance | Route-free terminal policy does not stop the actor |
| `actors/src/atomic/worker/{mod.rs,initialization.rs}` | Panic rejection mapping; settlement-observation documentation | Existing activation plan/permit and failure classification |
| `actors/src/{lib.rs,atomic/mod.rs,atomic/stable_proxy/mod.rs}` | Curate the actual interpreter port and remove superseded exports | Small application surface; no duplicate aliases for settlement mechanics |
| Atomic proxy, pool, supervisor consumers and `proxy_creation.rs` | Exhaustive panic outcome/custody handling after focused proof | No new aggregate engine, policy, or public constructor inputs |
| `actors/src/send_product.rs`, macros, handwritten equivalents | Prove identical retention, order, and source progression | Named products and inferred application syntax |
| Testkit, integration tests, compile fixtures, fuzz targets | Add external ownership oracles and changed-sequence coverage | Pure models stay independent of interpreter implementation |
| Canonical docs and five actor catalogue/law documents | Reconcile commitment, source-free custody, panic, publication | Distinct template policies and cardinalities |
| Bombay Driver | Continuing-turn/mixed-custody regressions; remove publication on failed or stopping initialization | One generic driver; no diagnostic/proxy dispatch |
| Bombay local/launch/application runtime/child bindings/terminal projection | Early private commitment, startup evidence, real leaf interpretation, full failure custody | Actual delivery/tasks/observation owned by runtime |
| Address | Exclusive hidden reservation, promotion, exact-generation release | One registration authority, no competing Bombay registry |

Before migration, enumerate exhaustive creation consumers with:

```sh
rg -l 'ChildCreationOutcome::|WorkerCreationRejection::' crates --glob '*.rs'
```

Current production matches include `atomic/pool/worker.rs`,
`atomic/stable_proxy/worker/mod.rs`, `atomic/proxy_creation.rs`,
`atomic/worker/mod.rs`, `atomic/fifo_pool/mod.rs`,
`atomic/keyed_pool/{mod.rs,shutdown.rs}`, and core creation dispatch.
The same search includes integration tests, benchmarks, and fuzz targets; keep
those in the migration ledger rather than treating a library-only build as
complete. Paths prefixed `atomic/` here are under `crates/actors/src/`.

Related request families must be checked: ordinary logical/exact deliveries,
`CustomerDelivery`, `PrepareWorkers`, `BeginActivation`, observation, shutdown,
parent reporting, timers, and `RetirementBirths`. They do not automatically need
new APIs. `BeginActivation` must respect the existing started-before-plan-poll
contract in the witness; no new activation framework is in scope.

The proposed reduction is specific: remove downstream envelope reconstruction,
public independent receipt assembly, duplicate initialization execution, and
startup panic/empty-residual assumptions. Keep one total settlement algebra,
one source-custody traversal, and one Driver retirement path. Do not call the
work code reduction if its measured production delta is positive.

After the blockers pass, a separate consolidation stage may compare repeated
named-product traversal or request transfer code. A merge requires identical
ownership, ordering, error, source, and retirement equations plus deletion of
the competing mechanism. Similar field names and matching branch counts are
insufficient. The five aggregate state machines are not a preapproved target.

## 8. External interpreter fixture

Add an isolated downstream Cargo fixture at
`tests/interpreter-contract/`, with its own `[workspace]` and lockfile. It must
not become a production workspace member or pull Bombay/Tokio into core. The
fixture depends on the **current candidate** Behavior, Actors, Bombay Engine,
Bombay, Address, and Communication graph. During development explicit path
patches are permitted and recorded; CI/release uses immutable compatible
revisions. A script rejects duplicate Behavior versions and mismatched sources
using Cargo metadata. Do not accidentally test registry Behavior inside Driver
against a different local Behavior in the fixture.

Use public exports only. Implement narrow real capabilities in this fixture
where production leaves are still absent; final acceptance also runs through
Bombay's completed production leaves. No `cfg(test)` access, test-only public
constructors, copied Driver, fake task host, erased actor, or manually successful
host receipt may substitute for that final run.

Required fixture files: `Cargo.toml`, `Cargo.lock`, `README.md`, test modules
`delivery.rs`, `custody.rs`, `startup.rs`, `shutdown.rs`, and compile fixtures
under `tests/compile/{pass,fail}/`. Group shared concrete test protocols in one
support module only when they own real common protocol definitions. Use domain
names in source; requirement IDs below belong only in documentation/manifests.

Use non-`Clone`, non-`Copy` plans, definitions, diagnostics, and completion
authority. FIFO/keyed jobs implement `Clone` under their existing retry law;
record the original and transferred allocations separately and require exact
return of the retained original on rejection. Get requests from real
FIFO/keyed/supervisor transitions. A second
same-typed request provides a substitution adversary. Observe payload identity
through owned unique values and final recovery, not invented IDs guessed from
sequence arithmetic. An independent drop ledger may instrument payload lifetime
but cannot supply authority or reconstruct missing payloads.

Deterministic control gates force close-after-resolution, settle-before-stop,
stop-before-report, and source-close races. No sleeps or probabilistic timing.
At each observable cut, each issued value is in exactly one allowed owner:
aggregate, pending request, accepted destination, returned rejection/report,
active runtime work, or terminal custodian. Final explicit discharge is recorded
separately. Count conservation after every event, not just at shutdown.

## 9. Mandatory acceptance matrix

All rows run in debug and optimized builds where executable. Compile failures
must fail for the named ownership violation, not unrelated missing bounds.

| ID | Required scenario and oracle |
|---|---|
| T01 | Accepted assignment reaches real exact worker once; original receipt returns to source; completion is still separate. |
| T02 | Closed assignment and close-after-resolution return the exact original FIFO job, target, move-only completion authority, and original correlation. Settlement makes no copy; the pool's documented retry policy already cloned the worker payload. Source may recover/retry under that law. |
| T03 | Two same-typed assignment requests settle in either order without receipt exchange; external assembly and double settlement fail to compile. |
| T04 | Proxy start, replacement, and shutdown each use real private-control admission; accepted receipt identifies original operation, creation, and exact proxy. |
| T05 | Proxy missing binding, closed control, and close race return complete definition/plan/control and original operation/source. Wrong occurrence and forged receipt fail to compile. |
| T06 | Emit terminal diagnostic on a continuing turn, process at least three later turns, then retire; recover the exact non-cloneable diagnostic once. No premature drop. |
| T07 | Many successful ordinary receipts and delivered diagnostics leave no retained receipt history. A routed diagnostic rejection remains complete in terminal custody. |
| T08 | Mixed product: retained acceptance, source action, creation receipt, independent request. Both legal wrapper orders preserve order; source admission progresses; closure retains every remaining lane. |
| T09 | Typed rejection at every position still attempts independent later items. Corruption at every position preserves exact prefix, fault, untouched suffix. Source-free rejected/blocked values survive continuation. |
| T10 | Pure initialization rejection returns current move-only child and exact error, no actions, binding, publication, or successful restart. |
| T11 | Reservation/host refusal returns the appropriate untouched creation or current child plus complete untouched Actions. No effect is attempted. |
| T12 | Initialization accepts one effect, rejects the next, accepts an independent later effect; no rollback, no reconstructed Actions, no publication; complete runtime settlement survives to root. |
| T13 | Initialization accepts a prefix then reports corruption; suffix remains untouched and owned. Ready/activation cannot be manufactured. |
| T14 | Initialization stops with final effects; effects settle once, no ordinary event runs, no endpoint becomes public, and exact stop reaches observation and initialization consumers. Include stop plus rejection and corruption. |
| T15 | Initialization succeeds once; `InitializeWorker` arrives before and after completion and after termination. Correct report/plan/permit each time; no second initialization or activation authority. |
| T16 | Panic during pure initialization returns typed uncommitted panic outcome and extant child custody. Panic after commitment is exact termination, not creation rejection or a parent panic. Test stated unwind limits explicitly. |
| T17 | Cooperative cancellation before fold, after fold, during an in-flight effect, after commitment, and just before publication retains actual ownership and prevents false success. |
| T18 | Termination and initialization report arrive in both orders; independent observer still receives exact terminal evidence. Parent closes before each admission; full values reach root. |
| T19 | Nested child startup and early private reports progress with bounded mailbox capacity; no parent/child publication deadlock. Public logical self-send while unpublished has the documented rejection. |
| T20 | Address reserve/competing claim/resolve/publish/release races, registration exhaustion, and stale retirement preserve exclusivity, hidden visibility, and exact generation. |
| T21 | Shutdown during assignment/proxy/initialization/activation preserves already-admitted work and pending reports/plans through the real retirement barrier. Never-ready work remains explicitly owned; cancellation is not fabricated completion. |
| T22 | Ordinary creating actor and an atomic owner both work; FIFO plus fixed supervisor provide unrelated template witnesses. Test both `Watch<ReceiveTimeout<B>>` and `ReceiveTimeout<Watch<B>>` with meaningful observation and timing policies. |
| T23 | Fixed/dynamic supervision, proxy replacement, FIFO/keyed work execute through public Bombay applications after leaf integration. Acceptance cannot masquerade as ready/completed/restarted. |
| T24 | Handwritten/generated equivalent products, heterogeneous child occurrences, source closure and `RetirementBirths` preserve identical custody without structural application syntax. |

Sensitivity proof: restore or simulate each original defect in an isolated
candidate and demonstrate the corresponding test fails for that law. T06 must
fail on the current unconditional `Exhausted` implementation even though
immediate-stop tests may pass. T02/T05 must reach actual rejection after payload
transfer; privacy-only compilation is insufficient.

Extend existing independent models and sequence targets for changed inputs,
including `fifo_pool_sequences`, `keyed_assignment_sequences`,
`keyed_binding_sequences`, and relevant lifecycle/catalogue sequences. Use
Address's concurrency verification for reservation interleavings. Do not copy
implementation matches into an alleged independent oracle.

## 10. Ordered implementation work packages

Each package starts with law, external syntax, failing regression, and reuse/
deletion ledger **before production**. Design and mechanical migration are
separate. Do not silently make an unresolved design choice during migration.

| Package | Work and prerequisites | Completion evidence |
|---|---|---|
| P0 — Baseline and red witnesses | Pin merged/current trees, snapshot dirty changes separately, inventory every affected symbol, create external fixture and dependency-source check. | Current-source D1–D7 evidence; focused failing ownership/retention/startup tests; exact anticipated change ledger. |
| P1 — Retained acceptance | Implement section 5 in core and diagnostic owner. Prove source-free failures and mixed product composition. | T06–T09, T22/T24 focused witnesses; no diagnostic-specific Driver logic or ordinary receipt accumulation. |
| P2 — Complete delivery | Implement section 4, assignment first and proxy second; compare existing ports before publishing the proxy seam. Narrow old assembly API only after real witnesses pass. | T01–T05; two same-typed concurrent requests; privacy, move, source and occurrence failures. |
| P3 — Startup ownership proof | Establish Address reservation and private child commitment witness; add pure-initialization panic outcome and smallest ordinary/atomic consumers; specify root unpublished return concretely. | T10–T20; one ownership trace for every row in section 6; no consumed Actions in `HostRejected`. |
| P4 — Current caller migration | Only after P1–P3 prove APIs, mechanically update all consumers, generated/handwritten products, tests, docs and runtime leaves. | No new abstraction discovered; fixed/dynamic/proxy/FIFO/keyed checks and public syntax preserved. Discovery of new semantic plumbing reopens its design package. |
| P5 — Integrated runtime and release | Complete actual Bombay leaf interpreters using proven contracts; run shutdown/activation races and all required gates; select immutable compatible dependency graph. | T01–T24 on production runtime; complete terminal ownership at root; reproducible versions and command logs. |
| P6 — Optional consolidation | Separate follow-up after blockers are closed. Compare complete ownership equations and delete demonstrably repeated machinery. | Independent law proof and measured deletion; no promise that the entire catalogue or Bombay runtime can be collapsed. |

No package may claim complete integration from a local path-patched compile.
P5 requires an immutable candidate graph and a coordinated release/pin plan.
Version according to the actual API break; do not assume narrowing constructors
or extending exhaustive public sums is patch-compatible. Release publication
itself is separate from this documentation deliverable.

### Required verification commands

Use the pinned toolchain through `nix develop -c` when Cargo is absent from the
shell. In the Behavior repository:

```sh
nix develop -c cargo nextest run --workspace
nix develop -c cargo test --workspace --doc
nix flake check
```

The fixture must document and run:

```sh
nix develop -c cargo test --locked --manifest-path tests/interpreter-contract/Cargo.toml
nix develop -c cargo test --locked --release --manifest-path tests/interpreter-contract/Cargo.toml
```

Wire fixture execution into the authoritative CI gate; an isolated workspace
is not covered by `--workspace`. Run applicable Bombay and Address gates too.
Activate the three currently ignored Bombay local-publication regressions.
Preserve absence while effects are pending and after rejection/corruption.
The current successful test expects visibility after `activate` but before
`publish`; move that successful visibility assertion to the explicit publication
milestone selected here and add an absence check before publication. Record this
intentional test-contract correction; never weaken the pending/failure checks.
Log actual command outcomes and environmental failures separately. Focused
proof comes before broad migration and repeated full-suite runs.

## 11. Architecture checkpoints and budgets

The target adds **zero aggregate control states**. The core acceptance method
adds no new public type. The proxy port is at most one new interpreter trait;
Address reservation is its own capability. The pure-fold panic extension adds
variants to existing creation/rejection sums, not a universal startup framework.
These are proposed limits, not measured implementation success.

Current aggregate top-level control sums relevant to review:

- Proxy: `Dormant`, `Starting`, `Ready`, `EmptyInitial`, `EmptyAfter`,
  `Replacing`, `ShuttingDown`, `Stopped` (8).
- FIFO: `Constructed`, `Operating`, `Draining`, `Stopped`,
  `ForcedRetirement` (5).
- Keyed pool: `Constructed`, `Operating`, `Retiring`, `Stopped`,
  `ForcedRetirement` (5).

Do not pretend those counts measure fixed/dynamic state held in their owned
member/entry products. Before each semantic experiment, write the complete
control sum and every subordinate alternative for the aggregate actually
changed. Record the exact current value each surviving alternative owns.
A protocol result is not an additional aggregate state or dispatcher.

Every retained batch must have this record, before and after:

| Required measurement | Required explanation |
|---|---|
| Aggregate control states | Complete domain sum, not just number |
| Subordinate state and result alternatives | Every retained value and the future decision needing it |
| Transition branches | Same counting method before/after; report measurement method |
| Production lines and modules | Separate production from inline/integration tests and generated code |
| Public spellings/types/traits/variants | List additions and removals, including doc-hidden exports |
| Residue scan | Arrival history, repeated causes, false cardinality, nested transition authority, semantic booleans, structural user syntax |
| Contract cross-check | Canonical runtime/transition/layer contracts and all five normalized actor laws |
| Provenance | Each new symbol linked to pre-edit law, failing regression, real consumers, and deleted/reused machinery |
| Disposition | `pass` or `reopen`; missing measurements mean `reopen` |

The required cross-check documents are [runtime settlement](../atomic-runtime-settlement.md),
[transition algebra](../actor-transition-algebra.md),
[layer laws](../behavior-layer-laws.md), and the normalized
[proxy](../actor-laws/proxy.md), [fixed supervisor](../actor-laws/fixed-supervisor.md),
[dynamic supervisor](../actor-laws/dynamic-supervisor.md),
[FIFO pool](../actor-laws/fifo-pool.md), and [keyed pool](../actor-laws/keyed-pool.md)
laws. Review their ownership equations; historical representation names are not
instructions to reintroduce removed machinery.

On `reopen`, remove only that experiment's production representation, record
its falsifier in `DEAD_ENDS.md`, and return to the last retained design. Never
revert pending user work. No blanket bounds, new no-op policies, hidden
compatibility wrappers, default generics, erased futures, or visibility fixes
may be introduced to silence compiler errors.

Expected Behavior design owners: the core sending/creation files, assignment,
proxy operation, diagnostic, worker creation, their curated export surfaces,
and focused tests. Runtime owners: local host, launch, child bindings,
application capabilities, terminal projection, and Address registration.
The implementation ledger must name the exact files and estimated deltas before
editing; this cross-repository scope will likely cross the repository's review
thresholds and must be presented as such.

Per `AGENTS.md`, more than 15 changed files, more than 500 net new production
lines, or more than three new public types requires explicit expanded-scope
authorization before further production edits. Count cumulatively across
packages; do not reset counts per commit. Report unrelated pre-existing changes
separately. This PRD does not waive that rule or mark an experiment retained.

## 12. Definition of done and prohibited shortcuts

The blockers are complete only when:

1. All T01–T24 rows have concrete evidence against one candidate dependency
   graph; external compile and real runtime results are reported separately.
2. Every ownership row in sections 4–6 has an executable witness and a truthful
   public contract; no startup outcome hits a known unconditional panic branch.
3. Generic custody retains requested evidence across continuing turns and
   retires it once; discharged receipts do not accumulate.
4. Initialization occurs once, creation commitment is explicit, and unpublished
   failure returns real custody instead of pre-interpretation fiction.
5. Complete source/API/doc migration and required gates pass; ignored tests or
   temporary source patches are not counted as completion.
6. Each retained batch passes the drift/provenance checkpoint and reports its
   actual capability additions separately from deletions.

Reject any patch that makes reconstruction fields public, clones jobs to recover
rejection, stores all receipts forever, treats diagnostics specially in Driver,
reinitializes a worker, converts consumed effects into empty Actions, conflates
admission with completion, discards startup residuals after logging, or moves
runtime mechanics into pure Behavior. Also reject arbitrary state-machine
unification justified solely by hoped-for line reduction.

## 13. Review evidence and reproducibility

The review traced current core action/item/creation composition, assignment and
proxy request ownership, diagnostics, worker startup, proxy/pool/supervisor
consumers, named send-product custody generation, testkit limitations, canonical
and normalized laws, and adjacent Bombay launch/Driver/host interpretation.
This is a holistic review of these contracts, not a claim that every unrelated
catalogue branch was executed or proved redundant.

Current physical source inventory (includes inline tests/comments, so these are
review-surface counts rather than production-only budgets):

| Crate | `src` Rust files / lines | `tests` Rust files / lines |
|---|---:|---:|
| Behavior | 10 / 6,764 | 15 / 3,796 |
| Actors | 125 / 57,860 | 44 / 35,604 |
| Macros | 1 / 1,499 | 10 / 255 |
| Testkit | 2 / 173 | 31 / 8,112 |
| Mutation gate | 1 / 612 | 0 / 0 |

Nested fuzz targets, benchmarks, and nested fixture workspaces are additional
verification surfaces, not included in this table.

Current-source SHA-256 fingerprints, captured before this document revision:

| File under `crates/` | SHA-256 |
|---|---|
| `behavior/src/effects/sending.rs` | `a768908396a6878be079290c4dda02b39297449ad34f3e48c349880dacc6c468` |
| `behavior/src/effects/actions.rs` | `189cb846f51c0b07d38789cf1bca5b291b73e86c58b9428063ab9bf61db78ae9` |
| `behavior/src/actor/creation.rs` | `9d490247c4e1e3b7252643bb262249fadf14c82b774d09290d31259127a7b181` |
| `actors/src/atomic/pool/assignment.rs` | `719e30c321f234ebbb1777c3c4fcb3c547a0a54111b4556a1564732972c14903` |
| `actors/src/atomic/stable_proxy/operation.rs` | `55fdb78ee174ce1b772fb7fd179600004411c3b07a2aa505207fd22e0a175df5` |
| `actors/src/atomic/diagnostic.rs` | `ef8f9f873284db7a534ad6536a77cd5a675f91e963b56674aa7f17fa2fe85b09` |
| `actors/src/atomic/worker/initialization.rs` | `09487c21e7368911242aceaf202fa81f4627037bd6f21a9f76bb56e42a46a3f6` |

The adjacent downstream record already contains debug/optimized diagnostic
reproductions and the ignored visibility failure. Those are prior evidence,
not a claim this review reran the full runtime.

This review also compiled a disposable **external** Cargo crate with path
dependencies on the current Behavior and Actors source, using Cargo 1.95.0.
Its non-cloneable `Diagnostic(Box<str>)` was placed in
`Vec<ActionItemResult<DiagnosticAction<Infallible, Diagnostic>>>` as
`Accepted(DiagnosticAccepted::Terminal(value))`. Calling the public
`SourceSettlementCustody::<(), ()>::offer_next_to_source` returned `Exhausted`.
The test demanding `Retained` failed with exactly
`terminal diagnostic was classified as exhausted` in the debug build.
This verifies D3 on the current code; it is not the T06 real-Driver witness.
The disposable probe is outside the repository and is not a retained test.

Documentation validation: `mdbook build docs` passed, the four published-doc
checker unit tests passed, the rustdoc-import scan passed, and acceptance-ID,
work-package, navigation, and diff-whitespace checks passed. Full runtime
verification of the proposed implementation remains P1–P5 work.

The reviewer also started `nix develop -c cargo nextest run --workspace`,
`nix flake check`, and an optimized build of the disposable diagnostic probe.
These did not produce final test verdicts during the review and were explicitly
interrupted by the reviewer. They are **unverified**, not passing checks and not
demonstrated code failures. The standalone rustdoc-error-code script also could
not run outside the Nix environment because Cargo was absent from that shell;
its authoritative flake invocation is included in the unverified full gate.
The optimized diagnostic failure recorded in the adjacent downstream review
must not be presented as a newly completed optimized run against this source.

Task-attributable implementation delta for this PRD: production `+0 / -0 / net
0`; retained tests `+0 / -0 / net 0`; public types `+0 / -0`. No production
experiment is marked retained by this documentation change.
