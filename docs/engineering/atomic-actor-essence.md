# Atomic actor essence and clean-room boundary (engineering record)

## Status

This document defines a **non-production falsification and model experiment**.
It is intentionally smaller than the existing atomic-actor catalogue. It may
test whether a simpler template state machine and ordinary authoring surface
can satisfy selected laws, but it does not authorize production actor types,
change implementation status, establish catalogue parity, or permit migration.

[`atomic-actor-solution.md`](atomic-actor-solution.md) remains the production
design authority. Its foundational prototype dependency order and coverage
matrix remain binding for production implementation. If this experiment
falsifies that order or supplies a replacement law, the solution, coverage
matrix, retained-core decision, and research audit must be updated together
before any production edit uses the result.

No Rust API is selected here. Capitalized names in equations name semantic
states, not authorized public types.

## Why this extraction exists

The detailed documents contain important discoveries, but they currently mix
four different designs:

1. the irreducible actor transition algebra;
2. interpreter settlement, hosting, and lifecycle mechanics;
3. the identity of each reusable actor template; and
4. a maximal catalogue of policies and failure handling.

That mixture makes a local template change appear to require a universal
activation service, a dependency-aware action language, recursive terminal
lifting, residual root ownership, typestate builders, and every final policy
at once. It recreates the conditions for compiler-driven design: a compiler
error in one layer appears to authorize another wrapper, path, marker, alias,
or associated type in every other layer.

The clean-room experiment begins from the semantic identity of each actor and
tests one independently stated policy at a time. This is an experimental
ordering, not a replacement production dependency order. An omitted policy is
recorded as outside that experimental slice; it is never represented by a
no-op callback, dummy route, default generic, or placeholder variant. A slice
that omits a foundational solution law cannot count as implementation evidence
for any coverage row that depends on that law.

## Authority labels

Every law used by the experiment is labelled with one of these authorities:

- **Actor-model law** — required by the actor semantics used by Bombay.
- **Bombay derivation** — a typed construction used to realize an actor-model
  law without effects inside the behavior fold.
- **Template law** — the defining contract of one reusable actor.
- **Policy choice** — one deliberately selected answer where actor research
  does not prescribe a result.

The primary actor reference is Agha, Mason, Smith, and Talcott,
[“A Foundation for Actor Computation”](https://doi.org/10.1017/S095679689700261X).
Its actor language identifies asynchronous `send`, fresh actor creation, and
`become` as coordination primitives, and makes an actor's current behavior a
deterministic function of the messages it has received. Agha's
[1986 monograph](https://mitpress.mit.edu/9780262511414/actors/) is the
foundational source for the open, dynamically reconfigurable actor model.

Those sources do **not** define supervisors, restart strategies, worker pools,
readiness handshakes, delivery-rejection APIs, shutdown deadlines, fluent
builders, or Rust effect products. Those are Bombay template or policy laws
and must be presented as such.

## Irreducible actor boundary

The clean-room experiment evaluates templates through the repository's locked
equation:

```text
initialize : Behavior -> Transition
receive    : (Behavior, one Communication) -> Transition

Transition = Communications
           * FreshCreations
           * (NextBehavior | Stop)
```

This means:

- **Actor-model law:** an actor reacts using only its local behavior and one
  received communication.
- **Actor-model law:** a reaction may send communications to known actors,
  create fresh actors, and designate its next behavior.
- **Bombay derivation:** `Behavior` is a deterministic, directly testable fold
  and `Actions` is the explicit typed representation of those consequences.
- **Bombay policy:** initialization is a separate fold interpreted before the
  first mailbox communication.
- **Bombay policy:** normal termination and controlled transition rejection
  are explicit results even though they are not part of Agha's primitive
  three-operation presentation.

The executable prototype must implement the real `Behavior` trait and return
the real `Actions` type from `crates/behavior`. A smaller local algebra may be
used only as an independent model oracle whose vocabulary and transitions are
kept separate from the executable prototype. Passing against a local algebra
is not evidence that a template composes with Bombay's actor contract or that
the interpreter can realize its effects.

Scheduling, allocation, endpoint installation, clocks, observation, transport,
and effect settlement are interpreter operations. The fold may request them
through a typed effect, or later receive a typed fact about them, but it may not
perform them.

`Actions` is not a transactional promise across actors. Atomicity means that
one local fold chooses one next state and one complete effect value. It does
not mean that several messages are delivered atomically, that emitted effects
succeed, or that a later rejection rolls back the source transition.

## What makes a template atomic

An atomic actor template is one standalone behavior with:

- one coherent domain responsibility;
- one private exhaustive state sum;
- one closed input sum containing public commands and exact runtime facts;
- one explicit `Actions` product containing only effects that actor can emit;
- one initialization transition;
- total transitions for every state/input pair; and
- no dependency on another wrapper to complete its own lifecycle law.

An atomic template may create another atomic actor. That is topology, not
behavioral decomposition. For example, a fixed supervisor may own stable
proxies, but its supervision decision is still one direct fold; it is not a
stack of `Supervise`, relay, fleet, and positional-ingress wrappers.

A generic wrapper remains appropriate only for a genuinely orthogonal
same-actor transformation such as stashing or a receive timeout. Adding a
wrapper must not be necessary to understand a supervisor or pool's internal
ownership.

## The common semantic spine

The templates share laws, not a universal runtime state machine.

### Ownership follows the phase

Every owned value has one visible owner in each phase. The important boundary
is emission of an action:

```text
ActorOwns(value)
    -> action not emitted: actor can reject and return value
    -> action emitted:     action/interpreter owns value
    -> settlement fact:    actor owns the returned result or exact capability
```

A cancellation or rejection may return a worker definition, job, result, or
request only if that phase still owns it. No later state reconstructs the value
from an id, clones it to escape ownership, or claims to return a value already
moved into an action.

### Creation is fresh and staged

- **Actor-model law:** actor creation allocates a fresh actor. Changing the
  behavior of an existing actor is a different operation.
- **Bombay derivation:** a behavior emits a staged create request correlated by
  a creator-local value, then receives an accepted or rejected creation fact.
- **Bombay policy:** the accepted fact carries the exact installed capability
  needed by the creator. A correlation value is not an actor identity and a
  collision is rejection, never replacement.
- **Template law:** replacement means creating a fresh successor and carrying
  explicit predecessor provenance. It never means overwriting an address.

The minimal lifecycle equation is:

```text
OwnedDefinition
    -> CreationEmitted
    -> Installed(exact incarnation) | CreationRejected(complete request)

Installed
    -> Ready(exact incarnation)      when the template requires readiness
    -> Stopping(exact incarnation)
    -> Stopped(exact terminal fact)
```

Installation and readiness remain different facts. The first experiment may
attempt to express readiness through ordinary typed protocol composition, but
it must prove the complete authority chain:

- which exact capability authorizes production of `Ready`;
- which actor or interpreter owns that capability in every phase;
- how `Ready` is correlated to the exact installed incarnation and one
  non-reused activation attempt or generation;
- which explicit `Actions` effect initiates asynchronous activation work;
- who owns the activation plan and exact incarnation while that work is in
  flight;
- how accepted, rejected, stopped, cancelled, stale, duplicate, and late
  outcomes settle; and
- why application code and an unrelated worker cannot forge readiness for the
  incarnation.

Injecting a typed `Ready` value directly from a test is model input only. It
does not falsify the need for an activation capability or demonstrate an
interpreter path. A universal activation plan, permit, task, authorization
ticket, or hydration service remains unselected by this experiment until the
ordinary-protocol attempt either proves this complete chain or fails with a
focused compile/interpreter witness. The production solution's existing
activation blocker remains authoritative meanwhile.

### Correlation and provenance are exact

Facts are accepted only for the exact outstanding operation and incarnation.
Role, key, address reuse, sequence adjacency, or arrival time cannot substitute
for provenance. Stale, duplicate, foreign, and contradictory facts are
explicit outcomes that preserve their owned data and leave valid state
unchanged.

Private operation, incarnation, timer, assignment, job, completion, and binding
correlations may be distinct types. Their constructors belong to the actor or
test interpreter. A type is added only when it prevents a demonstrated invalid
exchange; “the compiler needs a different name” is not evidence.

### Acceptance, realization, and termination are separate

These facts must never collapse:

```text
request admitted
effect accepted by interpreter
actor installed
actor ready
operation completed
outcome delivery attempted
actor terminated
```

Each template uses only the distinctions observable in its contract. A
dynamic `StartAccepted` does not mean `Started`; a pool `Accepted` does not mean
`Completed`; emitting a lifecycle report does not mean its destination
received it; and a terminal report is not proof that its source has stopped.

### Shutdown closes ownership

Shutdown is a typed input, not ambient cancellation. It closes new admission,
extracts or returns values that cannot complete, and resolves every actor or
creation still owned by the template before stopping. Duplicate shutdown does
not duplicate effects.

The initial experiment selects one complete policy: wait for all owned actor
facts. Deadline retirement, forced transfer, uncancellable external work, and
residual root-run ownership remain a separate interpreter experiment. They are
not encoded as flags or dummy variants in each template.

### Retained state is bounded by a domain rule

Every table, queue, correlation set, and tombstone family needs a capacity or
retirement law. “Keep it forever for stale detection” is not accepted. Old
facts are made harmless with non-reused correlation and generation evidence,
not unbounded historical state.

## The five template identities

The following table is the catalogue essence. Policies may enrich a row, but
they may not change the row's responsibility.

| Template | Unique responsibility | State it must own | Defining invariant |
|---|---|---|---|
| Stable proxy | Preserve one public service identity across fresh worker incarnations | zero or one exact worker incarnation, one install/replace operation, readiness, and drain | Only the exact ready current incarnation receives service commands; replacement never reuses an actor identity. |
| Fixed supervisor | Preserve an ordered, declared role topology under a selected recovery policy | one member state per declared role, exact stable-proxy ownership, and one recovery decision | One immutable topology snapshot prepares admission; its membership partition, correlations, prepared ownership, and budget commit together, while participant realization resolves independently. |
| Dynamic supervisor | Manage a bounded set of keyed stable services through explicit operations | keyed entries, fresh entry generations, operation ownership, exact stable proxies, and retirement | A key is management identity only; reuse after retirement creates a fresh generation and old facts cannot affect it. |
| FIFO pool | Own a fixed worker set and one customer obligation for each admitted job | worker lifecycle, FIFO backlog, active assignments, completion authority, and customer routes | No job is both queued and assigned; each admitted job has at most one terminal customer outcome; older serviceable work cannot coexist with eligible idle capacity. |
| Keyed pool | Add stable future-admission affinity to direct worker ownership | independent direct-worker lifecycle, one obligation per accepted job, bounded key-to-role bindings, and per-role queues | Selection is retained at admission; rebalance/unbind changes future admission only and never retargets accepted work. There is no global backlog or worker-selection cursor. |

### Stable proxy

The proxy is the first template because both supervisors depend on its one
atomic report boundary. Its minimum complete contract covers:

- empty construction;
- initial installation;
- exact readiness before routing;
- command return while unavailable;
- spontaneous worker stop;
- replacement by fresh creation;
- overlap, stale fact, and creation rejection; and
- shutdown during every live phase.

The owner receives one flat report sum. It does not reconstruct readiness from
separate creation and stop observations. Service clients cannot address the
owner-control protocol.

### Fixed supervisor

The fixed supervisor owns semantic roles and stable proxies. It does not own
application children and it does not convert a role into a nonce or address.
Recovery eligibility, strategy, budget, and release timing are separate policy
dimensions. They should first be implemented as local, exhaustive values in
the fixed supervisor. A shared recovery abstraction is extracted only after a
second direct actor demonstrates the identical transition law and the
extraction deletes code.

The essential recovery transaction is:

```text
observe exact terminal fact
    -> classify eligibility
    -> select roles from an immutable ordered snapshot
    -> prepare every replacement and fallible correlation
    -> admit or reject the complete decision
    -> commit member states and Actions once
```

### Dynamic supervisor

The dynamic supervisor is not a fixed supervisor with an optional role list.
It owns bounded keyed membership and explicit `Start`, `Replace`, `Stop`,
`Query`, and retirement operations. Request replies describe admission; a
separate durable route, if selected by the experiment, owns later lifecycle
facts. Cancellation is added only with an exhaustive ownership table showing
which phases still own and can return the submitted worker.

No fixed factory, restart strategy, restart budget, or no-op policy belongs in
this template.

### FIFO pool

The pool creates and observes workers directly. A stable proxy is unnecessary
because customers address the stable pool, not individual workers.

The pool retains the customer destination and issues opaque completion
authority with each assignment. The worker may consume an assignment to form
a completion, but cannot choose or substitute the customer. Completion must
match both the issued authority and exact worker incarnation.

Retry, if selected, is explicitly at-least-once execution. The pool retains
the canonical customer obligation and a retryable payload while the worker
owns an execution value. This is one semantic obligation represented by
multiple Rust values, not an exactly-once side-effect claim.

### Keyed pool

The keyed pool is a separate direct fold sharing only proven value laws with
FIFO. A submitted key is mapped to a semantic role by one concrete selector;
the selected role and fresh binding generation are retained as opaque admitted
evidence with accepted work, while the binding alone owns the key. Per-role
backlog and binding capacity are different bounds. Keyed pooling has no global
backlog, circular worker cursor, or cross-worker serviceability law.

Rebalance and unbind use exact absence/generation expectations and affect only
later submissions. Admission-created bindings require accepted work;
management may explicitly bind an absent key. Absence has no stored entry, a
role-changing rebalance issues a fresh generation, and a same-role rebalance
preserves the current generation as an accepted no-op. Worker replacement
keeps role affinity without keeping an address. Committing permanent role
unavailability returns that role's work once and removes every binding to it;
temporary recovery retains bindings.

## Concerns deliberately outside the first falsification slices

The detailed documents contain candidate answers for the following concerns.
The experiment may omit them from an early model or focused falsification
slice because they are not part of a template's identity:

- a universal activation plan/permit/request/fact protocol;
- a global activation-authorization limit shared across owners;
- per-item delivery settlement with source/host fallback;
- transitive settlement priority before later mailbox traffic;
- creation-scoped dependency bundles for every child operation;
- total terminal projection through arbitrary heterogeneous wrapper stacks;
- deadline retirement and residual root-run ownership;
- final builder syntax and one typestate marker per configuration axis;
- wrapper-depth diagnostic budgets; and
- catalogue-wide delivery-rejection migration.

Each remains a production prerequisite wherever the solution and coverage
matrix say it is. Omitting one limits the result to model/falsification
evidence; it does not make a dependent actor implemented. The experiment may
propose a simpler replacement only after a focused compile/interpreter witness
and composition through two unrelated actors without caller plumbing. That
proposal changes no production authority until all governing documents are
updated together.

Delivery failure deserves special care. The actor model specifies asynchronous
send and fairness assumptions at the semantic level; it does not prescribe
Bombay's capability-level transport rejection API. The clean-room folds must
preserve emitted values in the real `Actions`. Until accepted and rejected
interpreter traces prove the selected settlement law, those folds remain
non-production evidence and cannot satisfy dependent solution rows.

## Clean-room crate method

The temporary crate must not import `crates/actors`, its protocols, aliases,
builders, macros, tests, or helper state. Its executable prototypes must depend
on `crates/behavior` and implement the locked `Behavior`/`Actions` boundary.
A tiny local algebra is permitted only in tests as an independent model oracle
and must never be adapted into executable evidence or a second actor contract.

The crate is developed in this dependency order:

```text
real Behavior/Actions smoke witness + independent model vocabulary
    -> stable proxy
    -> fixed supervisor
    -> dynamic supervisor
    -> FIFO pool
    -> keyed pool
```

For each step:

1. Write one plain-language law and a complete state/event/ownership table.
2. Write the smallest pure-fold regression that fails because the actor does
   not yet exist, using domain vocabulary only.
3. Add the private state sum and public protocol before transition code.
4. Assert the complete real `Actions` result, including empty lanes and next
   verdict.
5. Add stale, duplicate, overlap, rejection, and shutdown cases before adding
   another policy.
6. Add deterministic accepted and rejected interpreter traces proving
   creation/effect order and retained ownership.
7. Add compile failures only for actual capability violations.
8. Audit every new prototype symbol against its pre-edit law and regression.

No common helper is extracted on first use. On second use, compare the two
ownership and transition equations. Extract only if they are identical, the
new abstraction owns a semantic law, and total prototype source decreases.
Similar variant names are not enough.

## Questions the experiment must answer

The clean-room crate exists to answer these questions with code rather than
another speculative type inventory:

1. What is the smallest creation result that returns exact installed
   capability and complete rejection ownership without structural paths?
2. Can explicit typed worker readiness be expressed as ordinary protocol
   composition while proving the exact producer authority, incarnation and
   attempt correlation, asynchronous initiation effect, in-flight ownership,
   and accepted/rejected/cancelled/late settlement needed to avoid a universal
   activation subsystem?
3. How does `assignment.complete(result)` produce a statically typed private
   parent report without exposing a parent path, customer route, helper trait,
   or generated alias?
4. Can each actor expose one named semantic action product while keeping its
   interpreter requirements inferred and closed?
5. Which public protocols genuinely need distinct acceptance, lifecycle,
   diagnostic, and terminal destinations?
6. Which recovery values have exactly the same law in fixed supervision and
   the two pools after all three folds exist?
7. Can shutdown be complete with the initial wait-for-owned-actors policy
   before deadline retirement is introduced as an independent extension?
8. Can ordinary construction infer the finished behavior type without a
   public marker for every missing choice?

A negative answer is useful evidence. It authorizes only the smallest semantic
surface named by that failed witness, not a general wrapper or utility family.

## Ready for comparison and policy enrichment

The non-production experiment is ready to compare with the governing solution
and enrich with omitted policy only when:

- all five prototype folds implement the real `Behavior`/`Actions` boundary
  and pass pure state-machine tests against independent models;
- each has accepted and rejected end-to-end interpreter traces;
- invalid protocol/capability exchanges fail at compile time;
- no ordinary example names a nonce, occurrence, path, generated effect
  product, typestate proof marker, or parent-report carrier;
- no valid example supplies a no-op policy or placeholder type;
- shared abstractions delete more prototype machinery than they add; and
- the legacy implementation has not been imported, wrapped, or retained by a
  compatibility layer.

This gate authorizes comparison and policy enrichment only. Safe migration
additionally requires:

- applicable feature-catalogue parity recorded in the solution matrix;
- the complete selected initialization-settlement and activation path;
- accepted/rejected delivery settlement with surviving ownership;
- wrapper-composition and initialization-order proofs in applicable orders;
- complete orderly/forced shutdown and residual ownership handling;
- compiler-pass/fail and ordinary DevX acceptance; and
- every production prerequisite in the solution marked implemented and
  verified through its required interpreter and test layers.

Only after those requirements compose and the governing documents are updated
together may the repository produce a migration/deletion proposal.

## Disposition of the existing atomic documents

- `atomic-actor-features.md` remains the exhaustive edge-case quarry. A row is
  promoted only when its policy enters an explicit implementation stage.
- `atomic-actor-solution.md` remains the production design authority. Its open
  foundational mechanisms and dependency order remain production blockers;
  this experiment may test or falsify them but cannot bypass them.
- `atomic-actor-type-inventory.md` is a list of hypotheses. No candidate name
  is authorized until the clean-room compiler and fold witnesses need its law.
- `atomic-actor-devx.md` remains the eventual usability acceptance target, not
  the first source of builder types.
- `atomic-actor-retained-core.md` governs the retained production-core audit.
  The temporary crate uses the real locked behavior boundary while testing
  whether template-level candidate machinery is necessary.
- `atomic-actor-research-audit.md` remains the dead-end and discovery record.
- `atomic-actor-other-templates.md` remains out of scope; the clean-room work
  does not trigger a catalogue-wide cleanup.

This keeps the detailed learning without making its accumulated candidate
machinery the architecture of the replacement.
