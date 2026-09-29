# PRD: Complete creation and delivery custody

Date: 2026-09-28. Status: proposed; research and contract review completed,
implementation and cross-repository acceptance pending.

## Decision

Solve the two reported integration gaps by completing the contracts at their
semantic owners:

1. **Separate exclusive address reservation from public endpoint visibility.**
   Prefer an Address-owned, affine, non-resolvable reservation over moving
   initialization effects before a fallible address claim. Preserve Behavior's
   existing distinction between rejected creation and an established child
   whose initialization effects subsequently fail.
2. **Let Behavior Actors preserve and settle its complete delivery requests.**
   Give interpreters a consuming, statically dispatched settlement operation
   for `AssignWorker` and `ProxyOperation`. The owner retains correlation and
   reconstructs rejection internally. Reuse existing delivery interpretation
   where its complete ownership equation matches.
3. **Keep complete action settlements in the existing runtime custody path.**
   Do not thread a child's send/birth settlement product through every proxy,
   supervisor, and pool merely to accommodate one hosting sequence.

These are design recommendations, not claims that the proposed reservation or
settlement methods already exist. The reservation design must pass the real
Bombay witness before it is retained. A fully typed post-interpretation
creation failure remains an alternative if that witness falsifies the selected
ordering. It is not an additional feature to implement alongside reservation.

The common requirement is **ownership completeness at every irreversible
transfer**: every request, accepted effect, returned payload, and unfinished
operation has one truthful surviving owner. Returning an earlier request is
valid only while that request remains unconsumed.

## Problem and verified baseline

The reported symptoms are accurate, but “two missing owner APIs” is not yet a
complete diagnosis. The creation symptom also exposes disagreement about what
creation commitment, address visibility, initialization completion, and worker
readiness mean.

Reviewed baselines:

- Behavior checkout HEAD `0274dab842f913c58c33c473d2a20adec6a7944e`, with
  pre-existing working-tree changes. Those changes are not this PRD's work.
- Bombay checkout HEAD `a7a66e3912731923015560463cd2c9e5e76fc041`, also with
  pre-existing changes; runtime observations below refer to that working tree.
- Bombay's inspected lock selects registry artifacts `bombay-behavior 0.17.0`,
  `bombay-behavior-actors 0.17.0`, and `bombay-address 0.2.0`. The relevant
  signatures were also inspected in the cached published artifacts. A local
  source change does not alter those immutable dependencies.

| Finding | Evidence and consequence |
|---|---|
| The creation rejection carrier is incomplete for commit-before-claim | `ChildCreationOutcome<C, Occurrence>::HostRejected` owns `RoutedCreation<BehaviorAddr<C>, C>`, **uninterpreted** `Actions<BehaviorAddr<C>, C::Ph, C::Sends, C::Birth>`, and `CreationRejection`. It cannot truthfully contain `Interpretation<C::Settlements>`. |
| The current runtime claims first | `LocalEnvironment::activate` calls `AddressSpace::try_claim` before `CommitActions::commit`. The reported late-claim hole arises when implementing the desired reorder; it is not the current execution order. |
| Claim success is immediately visible | Address `try_claim` inserts the endpoint before returning its lease. `resolve` can observe that registration. Allocation of a candidate address alone is not an exclusive reservation. |
| Assignment delivery cannot return its envelope externally | `AssignWorker::into_parts` is public and returns target, assignment, and receipt; `AssignWorker::returned` is restricted to `crate::atomic`. The non-cloneable assignment cannot be recreated after a real rejected delivery. |
| Proxy input has the same ownership gap | `ProxyOperation::into_parts` returns creation correlation, control, and operation ID. Its fields and issuing constructors are private; there is no public inverse. `ProxyInputReceipt::new` handles acceptance only. |
| The common settlement machinery already exists | `ItemSettlement`, `SettledItem`, `Interpretation`, `ActionSettlement`, `SourceAction`, and source/retirement custody already distinguish acceptance, rejection, blocking, corruption, and unattempted work. A second settlement framework is unnecessary. |
| Runtime failure projection also needs attention | `SpawnError::from_local` treats settlement failure before publication as unreachable, and child establishment panics for several terminal spawn outcomes. Merely adding an enum variant upstream does not repair these paths. |

Exact source owners:

- [Creation and child outcomes](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/behavior/src/actor/creation.rs),
  [action settlement](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/behavior/src/effects/actions.rs), and
  [item interpretation](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/behavior/src/effects/sending.rs).
- [Assignment custody](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/actors/src/atomic/pool/assignment.rs),
  [proxy operations](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/actors/src/atomic/stable_proxy/operation.rs),
  [worker creation](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/actors/src/atomic/worker/mod.rs), and
  [worker initialization](https://github.com/devrandom-labs/bombay-behavior/blob/main/crates/actors/src/atomic/worker/initialization.rs).
- Downstream: `bombay/crates/bombay/src/{local,launch,application_runtime,terminal}.rs`,
  `bombay/crates/bombay-engine/src/driver.rs`, and
  `bombay-address/crates/address/src/lib.rs`.
- Downstream decision records: `bombay/docs/open-design-ledger.md`, sections
  “ARC-006 — transactional activation ownership blocker” and
  “BEH3 — exact atomic source-action rejection return”.

## Research and authority

The research supplies constraints; the selected Rust API and publication policy
are Bombay decisions.

| Authority | Source and applicable conclusion |
|---|---|
| Actor research | Agha, Mason, Smith, and Talcott, *A Foundation for Actor Computation*, §3, pp. 14–20: `newadr` allocates a fresh actor name; `initbeh` separately initializes the new actor. Receptionists describe external visibility. The calculus does not specify Bombay's address table, rejection carriers, readiness protocol, or transactional rollback. [Primary paper](https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf) |
| Information hiding | Parnas, *On the Criteria To Be Used in Decomposing Systems into Modules*, discussion of the second decomposition: organize around hidden design decisions. Address owns registration; Actors owns its request representation; the interpreter owns execution. This supports completing those interfaces instead of exposing their fields throughout Bombay. [Paper text](https://www.cs.lafayette.edu/~gexia/cs301/resources/parnas.html) |
| Aggregate ownership | Evans, *DDD Reference*, “Aggregates”: a root is responsible for aggregate invariants. Proxy and pool transitions remain the authorities for readiness, recovery, and job ownership. A delivery settlement operation must not become a second lifecycle dispatcher. [Author's reference](https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf) |
| Rust ownership | Moves invalidate the previous owner; they do not retain a recoverable copy. A consuming port must return owned values on rejection. Domain alternatives belong in explicit types. [Ownership](https://doc.rust-lang.org/stable/book/ch04-01-what-is-ownership.html), [type safety guidance](https://rust-lang.github.io/api-guidelines/type-safety.html) |

**Derived construction:** pure `Behavior` transitions and concrete `Actions`
realize communications, fresh creation, and next behavior or termination.
Creator-local IDs, exact endpoints, and affine receipts implement additional
typed guarantees.

**Bombay policy:** creation precedes dependent same-action operations;
initialization effects settle before ordinary mailbox processing; public
resolution must not expose an initializing actor; rejected work retains its
complete ownership; terminal custody outlives source admission. None is a
claim that the research defines Bombay's exact effect product or Rust API.

The visibility recommendation is an inference from those ownership constraints
and Bombay's desired observations. The actor paper permits reasoning about
allocation and visibility separately; it does not mandate this implementation.

## Holistic contract review

The reviewed path spans the public algebra, both request owners, their
aggregate consumers, generated effect products, wrapper composition, testkit
witnesses, and the actual downstream host. This is a contract-focused review,
not a claim that every unrelated actor transition has been reverified.

| Surface | Required preservation or correction |
|---|---|
| Behavior | Keep one pure transition authority and the existing total settlement products. Preserve `CreationKind`, occurrence identity, ordered creation batches, and complete rejected values. |
| StableProxy | `Established` is distinct from initialized and ready. Its `InitializeWorker` request observes a host-owned settlement; it must not rerun the definition fold or acquire the worker's entire effect product. |
| Fixed/DynamicSupervisor | Proxy input acceptance is distinct from the later proxy outcome. A rejected input returns the exact submission/control and operation authority. A replacement decision is not `Restarted`. |
| FIFO/KeyedPool | Assignment delivery acceptance is distinct from completion. Preserve the assignment's opaque completion authority, original customer ownership, admission ordering, and stale/foreign-input treatment. |
| Other request families | Compare `CustomerDelivery`, `ShutdownEstablished::settle`, ordinary established delivery, and `PrepareWorkers` before inventing a reusable abstraction. Similar names do not establish identical ownership equations. |
| Layers and macros | Handwritten and generated named effect products must retain the same ordered settlements. No new application event, path-counting syntax, no-op input, or template-specific macro. |
| Runtime and Engine | Host, claim, interpretation, publication, and retirement must each have a named commitment point. Engine remains generic; no pool/proxy branches, detached custody tasks, or erased payloads. |
| Verification | Existing pure witnesses do not prove Address visibility or actual Communication rejection recovery. External consumer and real runtime tests are required. |

There is a concrete documentation conflict. The sole normative
[atomic runtime contract](../atomic-runtime-settlement.md) and
[transition algebra](../actor-transition-algebra.md) place host/binding commit
before initialization-effect interpretation, and assign subsequent failure to
the committed child's drain. Parts of the [adapter contract](../adapter-contract.md)
describe effect interpretation before endpoint installation and still describe
short-circuit error behavior. Bombay's ARC-006 record selects absence from
logical resolution until effects settle. These statements need one explicit
interpretation before production changes.

The selected interpretation is **private host commitment before effects,
public visibility after successful initialization**. Update the canonical
documents together during implementation. Do not silently redefine
`Established` as application readiness or make a failed effect undo a birth.

## Required creation contract

### Commitment and visibility

The following is the proposed ordering, preserving the current Behavior law:

```text
reserve a fresh identity exclusively; logical resolution remains absent
    -> run the pure initialization fold once
    -> commit the private child host and creator-local binding
    -> interpret initialization Actions once, retaining complete settlement
    -> on successful continuing initialization, publish the logical endpoint
    -> permit ordinary ingress according to the existing actor policy
```

For atomic workers, initialization settlement additionally authorizes the
existing activation protocol. The owning proxy or pool publishes worker
availability only after activation succeeds. The stable proxy's service
address can exist while its worker is unavailable; its documented unavailable
reply remains the service policy. Do not conflate proxy visibility with worker
readiness.

Address must own the exclusive reservation. Its proposed consuming publication
operation must not perform another collision check or allocate another
registration identity after effects have run. Those fallible decisions happen
when reservation succeeds. The token cannot be cloned, publicly forged,
published twice, or used in another address space. It does not authorize
ordinary delivery by itself.

Unpublished retirement consumes/releases the same reservation. Published
retirement releases the exact lease generation. Neither may remove a later
registration. No second Bombay address table, pending-address registry, or
boolean visibility gate is an acceptable substitute for this Address contract.

The private host owns the actual child and its lifecycle control before it
acknowledges committed creation. A reservation alone cannot produce
`Established`, a birth report, or a restart diagnostic. A committed child that
later fails initialization has an actual lifecycle to drain, even if it was
never publicly resolvable.

### Complete outcomes and custody

| Point reached | Required retained value and observable result |
|---|---|
| Routing refused | Original whole creation batch and namespace reason; no child initialization. |
| Reservation refused | Original routed creation and typed allocation/host reason; no initialization effects and no successful birth. Retain exact Address error in runtime custody where Behavior's reason is a projection. |
| Pure initialization rejected | Current child, unchanged correlation/provenance, and exact `C::Error`; release unused reservation. |
| Host refused before commitment | Current child and complete uninterpreted `Actions`; existing `HostRejected` remains truthful. |
| Private host committed | Exact child capability and binding; host owns initialization actions/settlement and all descendants. This is the creation commitment. |
| Initialization effects rejected or corrupted | Preserve the factual committed prefix, complete rejection/fault, and untouched suffix. No public visibility; drain the committed child. Return the existing worker initialization classification where applicable. |
| Initialization selected `Stop` | Interpret final actions, retain their settlement, drain; no ordinary ingress and no transient public endpoint. |
| Continuing initialization succeeded | Publish the reserved endpoint without a new fallible claim; retain/offer settlements through the existing source-custody protocol. |
| Shutdown/cancellation overlaps any stage | Preserve current child, reservation/lease, complete settlement, activation work, descendant custody, and admitted inputs through the existing retirement barrier. |

For ordinary creating behaviors as well as atomic workers, the runtime must
deliver the committed birth and eventual failure/stop under the existing
creation and lifecycle contracts. It must not wait for readiness and then
manufacture a pre-commit rejection. Inspect `SpawnError::from_local`, child
establishment, root publication, and terminal projection together: the current
“unreachable” failure projection is not an accepted implementation.

Private parent reports, child control, observations, and initialization-created
descendants must work without global resolution of the parent. Public logical
self-delivery during unpublished initialization follows the ordinary absent
address rule and returns its typed rejection; do not add a secret resolver
exception. Prove this policy and document its compatibility impact. Bounded
control traffic and nested creation must not deadlock while the creator waits
for initialization settlement.

No rollback of an accepted send, child creation, or external activation is
promised. Releasing an unpublished address is not retirement of those effects.
The existing root custodian must continue owning them until settled.

### Alternative if reservation is falsified

A post-interpretation creation rejection is lawful only if the project
explicitly changes its creation commitment policy. Such a result must own the
current routed child, complete `Interpretation` of its initialization actions,
and the truthful failure reason. Runtime residuals must additionally retain
the concrete claim error and all unfinished host work. A coarse
`CreationRejection` alone cannot replace that custody.

This would affect `ChildCreationOutcome`, `ChildCreationProduct`,
`BehaviorSettlements`, `WorkerRecovery`, worker creation rejection, proxy
creation, aggregate terminal products, and downstream interpretation. A
`C::Settlements` field can introduce recursive trait/type obligations through
birth products; its legality and consumer cost require a focused compile
witness. Adding a blanket bound to every actor is not a proof.

Select this alternative only after recording why the reservation witness fails
and proving its complete algebra through ordinary creation, a pool, a proxy,
heterogeneous children, and two wrapper orders. Do not implement both designs
as permanent policy flags.

## Required delivery contract

The interpreter-facing operation consumes one `AssignWorker` or
`ProxyOperation` and returns that action's existing `ItemSettlement` equation.
Suggested consuming method name: `settle`. Its exact Rust signature is a
design-stage deliverable, not a pre-approved new generic framework.

| Outcome | Assignment | Proxy operation |
|---|---|---|
| Accepted | Transfer the exact `Assignment<Job>` once; return the original `AssignmentReceipt`. | Admit the exact `ProxyControl` once; return a `ProxyInputReceipt` with the original operation ID and actual resolved proxy capability. |
| Rejected | Recover the original assignment and reason; owner reunites it with its original target and receipt. | Recover the original control and reason; owner reunites it with its original creation ID, operation ID, and source type. |
| Corrupt before payload consumption | Return the complete request and exact interpreter fault. | Return the complete request and exact interpreter fault. |
| Unattempted | Product traversal retains the original request unchanged. | Product traversal retains the original request unchanged. |

`Blocked` remains uninhabited for these two source actions under their current
`ActionItem::Prerequisite = Never`. Missing proxy binding uses the existing
`ChildInputReason`; it does not create another prerequisite vocabulary.
Corruption after payload consumption cannot claim to own an untouched request:
the lower transport contract must retain its actual accepted/pending custody.

Implementation requirements:

1. Hold envelope correlation inside the owning implementation across the
   transport attempt. Do not expose public field constructors that allow
   independently supplied receipts, endpoints, or operation IDs to be mixed.
2. For assignment, first attempt composition with existing
   `InterpretItem<EstablishedDelivery<P>, ...>` and its complete returned
   delivery. Owner code can reconstruct its envelope from that exact value.
3. For proxy control, inspect existing `ChildInput` interpretation. A successful
   proxy receipt also needs the exact resolved `EstablishedActor<StableProxy<...>>`;
   a unit receipt alone is insufficient. Reuse a port only if it supplies that
   evidence. If a new static interpreter seam is necessary, it owns this exact
   private-control admission contract and has no aggregate policy methods.
4. Calls occur solely during effect interpretation. A request's settlement
   method may transfer its payload and reunite returned ownership; it may not
   advance a pool/proxy state, retry, create actors, or emit aggregate `Actions`.
5. No `Clone` requirement on jobs, worker definitions, activation plans,
   controls, or affine authority. No boxed futures, dynamic dispatch, callbacks
   in the behavior fold, or runtime type lookup.
6. Return the same request type on failure, preserving `Source`, protocol,
   occurrence, and worker type. Downstream interpreters remain trusted to
   return the exact runtime payload; static typing alone cannot distinguish
   two values of the same Rust type. State that trust obligation explicitly.
7. Inventory and remove public decomposition/receipt constructors that become
   unnecessary only after all legitimate interpreters use the complete port.
   Keep private aggregate custody access where it is still meaningful. An
   API break is preferable to maintaining two competing external contracts.

Acceptance is mailbox/control admission, not worker execution, job completion,
successful replacement, or readiness. Rejection does not rewind the aggregate
transition that emitted the action. Independent later effects still run;
closed source admission transfers the complete result outward.

Cancellation must preserve an in-flight transfer's current custody. Dropping
the settlement future while its locals own the only request is not a terminal
handoff. The real Driver witness must show how its existing retirement barrier
keeps the operation owned until delivery settles or transfers its complete
pending value; this applies to the owner port as well as the transport.

## Alternatives and code reduction

| Candidate | Decision |
|---|---|
| Make `returned` and raw proxy constructors public | Small source patch, but exposes assembly of correlated authority and leaves reconstruction repeated in each interpreter. Rejected as the preferred contract. |
| Add only a post-commit `HostRejected` variant | Represents the stated runtime reorder, but conflicts with the retained creation policy and propagates child settlement types. Keep only as the explicit alternative above. |
| Non-resolvable reservation plus complete owner settlement | Selected hypothesis: removes the late claim race at Address and keeps request reunion inside Actors. Requires Address and Bombay changes and real integration proof. |
| Preflight `resolve`, then claim later | Racy; proves neither exclusivity nor registration capacity. Rejected. |
| Clone/rebuild consumed actions, log failures, or map them to generic corruption | Falsifies effects or loses affine ownership. Rejected. |
| Universal request/reconstruction trait or new lifecycle engine | No independent law requires it. Rejected unless focused evidence demonstrates meaningful reuse and deletion. |

Reduction opportunities are specific and conditional:

- remove external split/send/rebuild code once the owner performs settlement;
- avoid new worker/proxy/pool variants carrying a runtime action settlement;
- reuse one reservation/lease lifecycle for root and child hosting;
- use existing item/product/source/retirement settlement rather than adding
  parallel return enums;
- remove runtime panic/empty-residual assumptions by preserving actual custody.

Do not claim net code reduction in advance. Reservation adds capability code;
removing duplicated transfer logic may offset it. Measure capability additions
and simplification separately. Do not merge FIFO/keyed selection rules or
supervision recovery states merely to reduce line counts.

## Acceptance criteria

| ID | Required evidence |
|---|---|
| C1 | Real Address contention: a reserved name is unresolvable and cannot be claimed by a competitor. Exhaustion fails before effect interpretation. |
| C2 | Publication consumes reservation exactly once, uses its already-issued registration identity, and has no recoverable late collision/exhaustion branch. Release preserves generation identity. |
| C3 | Real root and nested-child traces cover reservation refusal, fold rejection, host refusal, accepted initialization, rejected effect, corrupt prefix/suffix, and `Stop` with final actions. No failed or stopping initialization is transiently publicly resolvable. |
| C4 | A committed but never-published child drains with its exact effects and descendants; no fabricated creation rejection, false readiness, or successful restart on failed creation. Distinguish a committed replacement's later failure from an uncommitted replacement request. |
| C5 | External consumer compilation and real Communication rejection recover a non-cloneable assignment, exact target, and original receipt. Success consumes the payload once. |
| C6 | Real proxy control acceptance/rejection preserves original submission/control, creation ID, operation ID, source type, and resolved endpoint. Cover start, replacement, shutdown, missing binding, and closed control. |
| C7 | Invalid construction and duplicate consumption fail to compile; wrong protocol/source/occurrence remain incompatible. Negative tests fail for the intended error, not missing unrelated bounds. |
| C8 | At least two unrelated templates and two legal wrapper orders preserve every named effect lane, initialization ordering, generic source admission, and terminal custody without placeholders. Include ordinary creation plus atomic owners, and handwritten plus generated products. |
| C9 | Rejection does not skip an independent later action. Corruption preserves the exact prefix, faulting value, and untouched suffix. Source closure at each transfer preserves the result through root retirement. |
| C10 | Exercise nested creation, early private reports, public self-delivery, stop during initialization, and cancellation under bounded capacity. No circular readiness wait, implicit retry, or detached residual owner. |
| C11 | Existing FIFO/keyed customer conservation, retry ordering, and supervisor/proxy readiness laws remain true under rejection/completion/exit permutations. A successful send alone emits no completion or restart. |
| C12 | Published artifacts are selected by one downstream lock graph; the actual Bombay interpreter witnesses pass against them. No path-patched build is represented as acceptance of the old locked release. |

Use deterministic gates and explicit events, never wall-clock sleeps. Write
the focused failing law/consumer regressions before production edits. Run them
in debug and optimized builds and restore/simulate each original defect to
prove their sensitivity. Extend independent models and existing fuzz targets
for changed ordering/custody sequences; use Address's concurrency verification
for reservation/publication/release interleavings.

After focused proof, run the repository's required `cargo nextest run --workspace`
and `nix flake check`, plus the applicable Address and Bombay gates. A local
generic testkit host does not substitute for real Address and Driver evidence.

## Delivery plan and architecture checkpoints

1. **Contract stage:** reconcile commitment versus visibility in canonical
   documents; record exact ownership tables and failing external regressions.
   Include the existing downstream visibility tests without weakening their
   observations. Establish the Address reservation witness before retaining
   the creation design.
2. **Actors stage:** prove complete settlement ports with one real assignment
   and one real proxy input interpreter. Preserve aggregate policy. Do not
   bulk-migrate callers while inventing signatures.
3. **Host stage:** implement Address reservation and Bombay private host /
   public visibility separation through the existing Driver and residuals.
   Resolve creation success, initialization failure, `Stop`, and root return
   semantics together. No template-specific Engine change.
4. **Migration stage:** only after the witnesses pass, mechanically migrate
   callers and delete superseded external reconstruction APIs. Any new
   semantic requirement reopens the design stage.
5. **Release stage:** publish compatible owner versions, update the downstream
   lock, run the complete witnesses, and close ARC-006/BEH3 only with that
   immutable evidence. Release numbering follows the actual source breaks;
   do not assume this is a patch release.

The expected Behavior/Actors design has **zero new aggregate control states**.
`ProxyState` currently has eight top-level alternatives; FIFO and keyed pool
control sums each have five in production. The normalized fixed/dynamic and
pool models distinguish operating, draining, and stopped custody with their
owned member/entry products. Their internal phases are not interchangeable,
and these counts are not an acceptance argument or a complete drift audit.

Before each semantic experiment, record the full applicable control sum and
all subordinate alternatives, exact retained values, transition branches,
production lines, modules, and public spellings. Repeat after the batch. Scan
for arrival-history state, duplicated causes, false cardinality, nested
transition authority, semantic booleans, and structural application syntax.
Cross-check all five [normalized actor laws](https://github.com/devrandom-labs/bombay-behavior/blob/main/docs/actor-laws/README.md), the
runtime contract, and the relevant catalogue pages. Record `pass` or `reopen`;
missing measurements mean `reopen`, not provisional acceptance.

The implementation ledger must identify each new symbol's law, prior failing
regression, intended callers, and machinery replaced. Initial estimates:

| Area | Expected source scope | Public surface target |
|---|---|---|
| Actors | Assignment and proxy-operation owners plus required re-exports | Two consuming methods; reuse delivery ports; at most one new proxy interpreter trait if required by the exact receipt law. |
| Behavior | Documentation and existing interpreter contracts; production change only if the witness proves a gap | No new creation outcome or universal settlement trait on the selected path. |
| Address | Registration storage and reservation/lease lifecycle | One affine reservation capability; typed reserve and consuming publication. |
| Bombay | Local hosting, child establishment, spawn/terminal projection, concrete item interpreters | Reuse existing residuals and Driver; no new aggregate engine. |

These are scope estimates, not a waiver of repository limits. Before production
work, replace them with the exact expected files and line deltas. The cumulative
limits remain: more than 15 changed files, more than 500 net new production
lines, or more than three new public types requires the explicit scope review
specified in `AGENTS.md`. Count across stages; do not reset the ledger to evade
the checkpoint. Track unrelated pre-existing changes separately.

## Review evidence and limits

This PRD is documentation work. No production design has been retained and no
new API implementation is claimed. Source inspection verified the three owner
signature gaps in both the checkout and published 0.17.0 artifacts. The
existing creation-order testkit witness was read; it models reservation and
commit and does not implement a real Address reservation. The downstream
visibility regressions are currently explicitly ignored in the inspected
runtime and therefore are not green acceptance evidence.

External compile probes were run with Cargo/Rust 1.95.0 in a disposable crate,
using exact registry dependencies `bombay-behavior = 0.17.0` and
`bombay-behavior-actors = 0.17.0`, through `cargo check --offline`:

| Probe body under the existing generic bounds | Observed diagnostic |
|---|---|
| Call `AssignWorker::returned(target, assignment, receipt)` from an external crate | `E0624`: the associated function is private. |
| Supply `Interpretation<C::Settlements>` as `HostRejected.initialization` | `E0308`: expected `Actions<...>`, found `Interpretation<...>`. |
| Construct `ProxyOperation { creation, control, operation, source }` externally | `E0451`: all four fields are private. Run separately so earlier type errors did not suppress the privacy check. |

These are expected compile failures verifying the original gaps, not passing
implementation tests. The temporary probes were not added to the repository;
implementation must add durable law regressions before its production edits.

The reservation recommendation remains subject to C1–C4 and C10, especially
private child reporting and bounded-capacity progress. The proposed delivery
methods remain subject to external caller compilation and actual rejected
payload recovery. There is no numerical code-reduction promise and no claim
that complete workspace or cross-repository verification was run for this
document-only review.

Task-attributable production delta: `+0 / -0 / net 0`; retained test delta:
`+0 / -0 / net 0`; public API: `+0 types / -0 types`. This document and its
index links are the deliverables. Implementation checkpoints above remain
pending rather than being marked passed by this PRD.
