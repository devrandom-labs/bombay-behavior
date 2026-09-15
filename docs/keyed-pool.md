# Keyed pool

Status: `feature-complete` locally. The aggregate implementation and both bounded independent
models are executable. Separate retained stateful targets isolate binding
management from assignment/retirement custody. The assignment/quarantine
partition has all 18 viable candidates caught, including exact nonzero-role
completion selection; the binding value/table partition has all 22 viable
candidates caught, including public evidence privacy and exact caller request
identity. The recovery partition has all 13 viable candidates caught, including
exact delayed recovery of a nonzero role through preparation, scheduling, and
timer release. The binding and recovery partitions retain 49 and twelve
compiler-unviable substitutions as inventory only. The shutdown partition has
both viable candidates caught; thirteen compiler-unviable defaults remain
inventory only. The complete fresh 232-candidate owner inventory is classified:
all 72 executable candidates are caught and 160 compiler-unviable substitutions
remain inventory only. Final campaign audits and Bombay root custody remain
open.

Production includes canonical construction, pre-ready admission,
generation-exact binding management, exact successful direct-worker startup,
role-local dispatch, both accepted-delivery/completion orders, foreign
completion rejection, exact busy-worker interruption, permanent role
extraction, rejected-delivery authority reunion, worker quarantine, direct
worker preparation, immediate and delayed replacement release, exact restart
schedule/timer correlation, and shutdown custody for creating, initializing,
waiting for activation capacity, activating, ready, busy, quarantined, waiting
for the recovery source, preparing, scheduling a restart, and waiting for its
timer. An unused activation plan remains in the terminal diagnostic action
until the interpreter settles that action; it is not destroyed when the worker
exits. Forced retirement retains the unchanged unresolved worker roster and
exact cause in the final concrete behavior; a non-cloneable pending worker
value proves that custody survives `Step::Stop`. Generic settlement and FIFO's
complete direct-worker consumer are retained, and Bombay owns the documented
transfer of final behavior and actions through its retirement barrier. The
private diagnostic-cause sum carries a checked dead-code expectation because
the selected custodian receives it intact; KeyedPool does not expose lifecycle
internals merely to make the producer inspect them. The
complete transition law is owned only by
[`actor-laws/keyed-pool.md`](actor-laws/keyed-pool.md).

## Responsibility

`KeyedPool` owns direct workers, one bounded queue per semantic role, stable
future-admission affinity, bounded bindings, non-reused binding generations,
exact generation-or-current-absence management expectations, rebalance/unbind,
assignment and completion, permanent role retirement/binding removal, exactly
one terminal customer outcome per accepted job, and complete shutdown drain.

It has no global FIFO queue, circular selector, stable proxy, or supervisor. It
is neither a wrapper nor mode of `FifoPool`.

## Collaboration and policy

Fresh worker allocation is the actor-model law. Concrete key selection,
binding capacity, fresh global generation, same-role rebalance semantics,
immutable admitted affinity, per-role admission, and automatic binding
extraction are deliberate keyed-pool policies.

The binding table solely owns each retained key. Accepted work retains only
opaque generation/role evidence and its immutable admitted role. Unbind or
rebalance affects future admission; it cannot retarget queued, assigned, or
retried work. Permanent role unavailability removes every binding and returns
all role-owned work in one transition. Temporary recovery retains bindings.

The initial factory is construction-only. `KeyedConstructionRejected` embeds
the shared `InitialWorkerRejection` described in
[`atomic-actor-architecture.md`](engineering/atomic-actor-architecture.md) beside keyed
policy. Permanent and transient recovery use
the same generic one-role worker-source request as FIFO and fixed supervision;
Bombay executes it outside `Behavior`, while KeyedPool owns eligibility, budget,
timing, binding consequences, and topology disposition. Temporary recovery has
no worker source.

FIFO and KeyedPool use one passive direct-worker component for successful child
creation, initialization, activation admission, and readiness. That component
cannot inspect a queue or binding and cannot emit `Actions`. KeyedPool alone
selects the correlated role, applies the global activation capacity, and drains
only that role's oldest queued job when its worker becomes ready. Creation
identity and lifecycle kind are selected once by the shared ordered mapping;
the member transition does not repeat that correlation test.

They also use one direct-worker retirement transition. Its
`Option<WorkerStartupCustody>` owns only a concrete startup value still local to
the pool; absence means none remains. An exact stopped initialization joins the
authoritative worker stop with shutdown and returns the unused activation plan.
An exact activation `Started` receipt advances the retained attempt without a
diagnostic; a later terminal or duplicate activation remains complete for the
owning pool's diagnostics. This component chooses no binding, queue, customer,
deadline, recovery, or final actor outcome.

FIFO and KeyedPool now also consume the same private `AssignedJob` join. A
completion received before its delivery receipt remains attached to the
assignment; the exact receipt releases one customer completion. When the
receipt arrives first, the exact completion releases that outcome directly.
Worker identity alone is insufficient: the completion authority must identify
the retained assignment. An exact worker stop applies the configured
interruption policy once. If retry is selected but recovery retires the role,
the job remains an assigned obligation and is returned as
`RolePermanentlyUnavailable`; it is never reclassified as queued. Role
retirement extracts that role's queue and every binding in the same aggregate
transition.

FIFO and KeyedPool also use one private `ForcedRetirementCause`: exhausted
worker-shutdown identifiers, a deadline that was not scheduled with its
complete interpreter result, or the exact elapsed deadline. Each pool still
owns its own unresolved workers, customer extraction, and terminal transition;
there is no common pool or retirement engine.

FIFO and KeyedPool consume one direct-worker preparation settlement and one
replacement-release law. The shared worker value correlates the exact
preparation request, retains the stopped worker and complete replacement
submission, and changes from waiting for schedule acceptance to waiting for
the exact timer. It cannot choose a role, touch a queue or binding, apply a
failure policy, emit an action, or create a worker. KeyedPool alone serializes
its affine recovery source, applies restart limits and release timing, returns
work, removes bindings when a role becomes permanently unavailable, and selects
role retirement or pool shutdown.

Rejected delivery returns the complete assignment to the shared authority
reunion. An exact return restores the existing customer obligation to its
original role queue at its immutable admission order, then quarantines that
worker. The role remains unavailable until both the shutdown response and the
exact worker exit arrive in either order. Only then may recovery or permanent
role retirement begin. A returned assignment that contradicts an already
received completion returns the customer obligation once, preserves both
contradictory inputs in diagnostics, and drains the pool.

The independent `keyed_model` checks two keys/roles, zero and one capacities,
all eligibility classes, exact management expectations, immutable affinity,
every delivery/completion/stop permutation, retry order, retirement, shutdown,
22,621 bounded prefixes, and five deliberate inversions in debug and optimized
builds. The production `keyed_binding_sequences` fuzz target separately drives
only management commands through the real aggregate. Its scenario stores one
absent-or-bound value, issued and stale generations, and one foreign generation;
it owns no assignment, customer, or worker-retirement state. Five thousand
development and five thousand optimized inputs preserve same-role stability,
role-changing and post-unbind freshness, exact stale rejection, and foreign
generation rejection.

The separate `keyed_assignment_sequences` target begins with one ready worker
and one accepted binding, then drives accepted or rejected delivery, completion,
exact and foreign stops, shutdown settlement, and replay. Five thousand
development and five thousand optimized sequences preserve the original job,
payload or result, role, generation, logical customer, and one terminal outcome.
Its model retains the current post-stop phase because a late receipt can release
terminal custody, and permits `ReturnedQueued` when rejected delivery restores
an assignment to its admitted role immediately before that role retires. It
delivers no input after `Step::Stop`, matching the interpreter lifecycle law.

## Realization gate

Canonical construction, submission, management, outcomes, and worker completion
syntax are owned by [`atomic-actor-devx.md`](engineering/atomic-actor-devx.md). The exact
production-source binding harness proves final-generation issuance, permanent
exhaustion without wrap, complete rejected-key return, and preservation of an
existing binding when rebinding cannot issue a generation. Real aggregate
traces now cover shutdown from every reachable direct-worker and recovery
phase, including activation-capacity and recovery-source waiters. A separate
account-directory model now matches the production aggregate's automatic and
explicit binding, capacity rejection and release, same-role preservation,
cross-role generation, stale expectation, and unbind results without reading
production state. The same independent customer-custody model now compares all
24 delivery-acceptance, completion, exact-exit, and shutdown orders against FIFO
and KeyedPool. The keyed adapter also proves that every customer outcome retains
the original binding generation and role. Recovery/shutdown parity is now
closed by direct public traces: concurrent failures serialize the one recovery
source in role order; every accepted, rejected, corrupt, or unattempted worker
preparation and restart-schedule result returned after shutdown emits no new
worker work; an accepted restart timer is cancelled; and accepted or rejected
worker shutdown settlement reunites with the exact worker exit in either order.
The separate workshop oracle checks the same ownership equations and explores
22,621 generated recovery/shutdown prefixes in debug and optimized builds. It
uses no production state projection or second actor transition. Bombay's
separately documented root transfer remains an upstream integration
requirement.

No generic pool engine, optional key, selector mode, cloned key evidence,
generation inference, permanent tombstone, stable proxy, structural completion
route, helper alias, or compatibility spelling may enter the replacement.
Repository-wide verification status and remaining gates are owned by
[`atomic-actor-verification.md`](engineering/atomic-actor-verification.md).
