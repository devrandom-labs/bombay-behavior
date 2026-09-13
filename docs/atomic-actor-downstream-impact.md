# Atomic actor downstream impact

Status: current read-only impact report after H591. This document assigns the
remaining integration work; it does not authorize writes to a sibling
repository and does not add a compatibility layer to Behavior Actors.

## Result

The finalized behavior algebra has one direct runtime migration owner: Bombay.
Bombay must interpret the complete typed `Actions` value and carry terminal
behavior/environment custody through its existing Driver. Bombay Entity has a
later owner-side migration away from the pre-0.13 Behavior protocol wrapper.
Mnesis-Bombay has no direct atomic-actor call site; after Bombay's Entity surface
is released, it changes only its typed Entity projection and keeps durable
command completion and retry independent from actor lifecycle.

Consequently no downstream evidence requires restoring `Proxy`, `Supervise`,
`WorkerPool`, `KeyedWorkerPool`, `WorkerStopped`,
`WorkerCreationResolved`, or another removed atomic compatibility spelling in
this repository. `ActorInterface` is also not a prerequisite for action
settlement, terminal custody, Entity migration, or Mnesis durability.

## Audited checkouts

The source trees were inspected in place and remained read-only. Because each
sibling contained pre-existing work, both the Git identity and dirty-path count
are part of the evidence boundary.

| Checkout | Audited revision and worktree | Selected Behavior/runtime generation |
|---|---|---|
| Bombay | `ccfb8f695c475e5d812ae735d610de59b1d127aa`, dirty `refactor/address-integration`, 86 paths | worktree manifest/lock select Behavior and Behavior Actors 0.14.0 plus macros 0.11.4 at `75a317235710f5e5ae2dba02f0600673764cf728` |
| Bombay Entity | `68d0f503205a569ddda88124f5add8e8a652e18f`, dirty `release-plz-2026-08-12T18-06-22Z`, 2 paths | Behavior `0.9.1` |
| Bombay Entity Runtime | `442e735523f2591fa9d2c735834435afa8f51530`, dirty `feat/entity-runtime-v1`, 9 paths | Behavior `=0.9.1` |
| Mnesis-Bombay | `9d567f6d4f113bf3ac5ffeb569c219efb0ecf160`, dirty `main`, 5 paths | released Bombay `0.1.0`, Behavior `0.9.5`, Bombay Entity `0.1.0`, Mnesis `0.3.1` |

The finalized atomic worktree is at `50a226803ccf26673a6f08e99d189fe77870cb09`.
Bombay's selected `75a3172` revision is an ancestor of it and therefore is not
the finalized build contract. Bombay's manifest/lock selection must first move
to the retained revision or a release containing it. Some dirty Bombay design
records mention other interim revisions; the manifest and lock are the actual
audited build selection.

## Exact source trace

### Bombay

Bombay is already the sole concrete runtime owner, but the audited source still
implements the older effect contract:

- `crates/bombay/src/interpret.rs` defines a two-leg
  `InterpretationError<Creation, Send>`. `CommitActions::commit` loops over
  creations, then calls `InterpretSends`, returns `Result<(), _>`, and retains
  no complete accepted/rejected/blocked/corrupt/unattempted settlement.
- `crates/bombay/src/application_runtime.rs` contains 21 legacy
  `SendInterpreter`, `InterpretRequest`, delivery, and child-input
  implementations. `CreationResults` correlates by a raw nonce in a `HashMap`,
  rather than by typed occurrence plus `CreationId`. `spawn_child` reports the
  older `CreationResolved` result and publishes a child only after the old
  launch path completes.
- Seven production sends discard the `ControlSender::send` result across
  `application_runtime.rs`, `reports.rs`, and `observation.rs`. Those sites can
  lose the exact source result, child fact, parent report, or observation event
  when admission has closed.
- `crates/bombay/src/local.rs` makes `CommitActions` return only
  `Result<(), E>`. `ActiveEnvironment::retire` drains and drops resources and
  returns unit.
- `crates/bombay-engine/src/driver.rs` returns only `Completion` or
  `DriverError`; `crates/bombay-engine/src/environment.rs` fixes retirement to
  unit. The final concrete behavior and environment residual therefore do not
  cross the retirement barrier.
- `crates/bombay/src/incarnation.rs`, `retirement.rs`, `generation.rs`, and
  `launch.rs` reduce the terminal result to an exit/crash classification. Child
  tasks are awaited, but their complete stopped behaviors and unresolved
  settlements are not transferred to a parent or root custodian.
- `crates/bombay/src/application.rs` begins with `Application::new(root)` and
  appends application children afterward. Atomic templates whose root
  construction needs an installed lifecycle or diagnostic capability require
  the inverse, role-first order.

This is a contract migration, not evidence for a second runtime. Bombay's one
`Driver`, Communication mailbox, Address lease, Observe facts, Timers queue,
child host, and executor task hierarchy remain the concrete owners to extend.

### Bombay Entity and Entity Runtime

The canonical Bombay Entity checkout pins Behavior 0.9.1 and its
`crates/entity/src/protocol.rs` directly composes `WorkerStopped` and
`WorkerCreationResolved` with the old `Addr`/`Msg`, turn-less transition, and
positional-send Behavior contract. The secondary Entity Runtime worktree has
the same dependency generation and direct spellings in
`crates/entity/src/behavior.rs`.

Those wrappers are not atomic actor owners. The migration retains the
runtime-neutral Entity laws—stable `EntityId`, single-flight activation,
generation-safe replacement, bounded admission, ordered drain fence,
passivation, and exact-incarnation retirement—but deletes the obsolete protocol
wrapper. Bombay then hosts the application's current concrete `Protocol`
directly and owns the private FIFO fence ingress. Updating either Entity
checkout to current Behavior in isolation would create two runtime generations
and is rejected.

The canonical migration target is the Bombay-owned Entity surface. The
secondary Entity Runtime worktree is comparison evidence, not an additional
runtime to preserve or migrate independently.

### Mnesis-Bombay

No scoped atomic aggregate or removed atomic spelling occurs in Mnesis-Bombay
production source. Its only current Bombay-facing production boundary is:

- `crates/bombay/src/routing.rs`: `Addressed<A::Id, Message>` becomes
  `(bombay_entity::EntityId<A::Id>, Message)` without changing the payload; and
- `crates/bombay/src/transport.rs`: `ExecuteRequest<Request, Reply>` keeps the
  runtime-neutral command request separate from Bombay's typed reply
  capability.

The behavior dependency is present in the released graph but no production
module interprets `Actions` or names an atomic actor. Mnesis owns load, decide,
append, optimistic-conflict handling, command identity, and durable outcome.
`CommitFailure::Ambiguous` and the
`uncertain_append_failure_returns_command_identity_without_retry` regression
already prove the critical separation: a restart may restore availability, but
cannot prove whether an append committed and cannot authorize transparent
retry.

After Bombay releases its owned Entity family surface, Mnesis-Bombay replaces
the standalone `EntityId` projection/dependency with the Bombay-owned typed
Entity reference projection. It must preserve the command unchanged and keep
Bombay admission/termination outcomes distinct from `CommandOutcome`. It does
not gain a supervisor, atomic actor, behavior interpreter, Entity directory,
mailbox, or persistence effect inside `Behavior`.

## Required Bombay work by owner and order

The normative details remain in
[`atomic-runtime-settlement.md`](atomic-runtime-settlement.md). The executable
downstream sequence is:

| Order | Owner and exact files | Required observable change | Explicit non-requirement |
|---|---|---|---|
| 0 | Bombay `Cargo.toml`, `Cargo.lock`, Engine fuzz manifest/lock | select the retained Behavior/Core, Actors, and macros revision as one family | no mixed revision, local compatibility fork, or floating sibling path |
| 1 | `crates/bombay/src/interpret.rs`; `crates/bombay/src/application_runtime.rs` | replace the two-leg/unit commit with one `actions.interpret` call and exact `InterpretItem` results for every statically known item | no universal runtime error, aggregate switch, erased envelope, or positional traversal |
| 2 | `application_runtime.rs`, `child_bindings.rs`, `local.rs`, `launch.rs` | route each creation batch once; establish children independently; retain `ChildCreationOutcome`; commit initialization settlement before publication/ingress | no nonce-as-identity, overwrite, rollback of an accepted prefix, or atomic-template host |
| 3 | `interpret.rs`, `application_runtime.rs`, `reports.rs`, `observation.rs`, `local.rs` | admit source-action and creation settlements in declared order; recover a closed control event exactly; retain the current value and untouched suffix after closure | no ignored send result, recursive actor call, detached task, callback, or second mailbox |
| 4 | `application_runtime.rs`, `local.rs`, `launch.rs` | host `InitializeWorker` on the installed child and `BeginActivation` in the existing typed task product; enqueue `Started` before polling and return the exact terminal input | no activation work in a behavior and no erased future/task collection |
| 5 | `application_runtime.rs`, `time.rs`, selected `bombay-timers` owner | implement total scheduling receipts/rejections and preserve exact scheduling results through the generic source-result path | no FixedSupervisor timer branch and no panic/exhaustion sentinel |
| 6 | `crates/bombay-engine/src/environment.rs`, `driver.rs`; Bombay `local.rs`, `launch.rs`, `incarnation.rs`, `retirement.rs`, `generation.rs`, `application_runtime.rs` | carry the final concrete behavior plus typed environment residual through the existing retirement barrier, parent admission, and a non-rejecting root custodian | no dropped terminal behavior, log-as-custody, second Driver, or alternate lifecycle service |
| 7 | `crates/bombay/src/application.rs`, `application_runtime.rs` | declare/install application actors by semantic role, obtain typed capabilities, then construct and activate the pure root | no `MailAddr`, structural occurrence path, explicit composed root type, dummy field, registry, or capability lookup in application syntax |

Orders 1–6 are one coherent custody contract: applying only the interpreter
syntax while leaving unit retirement still loses affine values. Order 7 is a
separate Bombay responsibility and may be developed independently, but both
must converge before an end-to-end atomic application is claimed.

The generic item implementations in order 1 include logical/exact/child
delivery, observation, shutdown, scheduling, `ReportToParent`, source actions,
atomic diagnostics, `CustomerDelivery`, worker preparation, initialization,
and activation. They are capability-family implementations, not branches for
StableProxy, FixedSupervisor, DynamicSupervisor, FIFO, or KeyedPool.

## Downstream dependency order

```text
retained Behavior + Behavior Actors release
    -> Bombay total interpretation and retirement custody
    -> Bombay role-first application assembly
    -> Bombay-owned Entity lifecycle/application surface
    -> Mnesis-Bombay dependency and typed projection migration
```

The five local atomic aggregates do not wait for Entity or Mnesis. Their
end-to-end runtime claim waits only for the separately authorized Bombay
custody/application work. Entity and Mnesis then consume the released Bombay
surface in their own campaigns.

## Acceptance evidence for the downstream implementation

Bombay must provide caller-visible regressions before its production migration:

1. every accepted, rejected, blocked, corrupt, and unattempted item returns its
   exact complete value in both relevant wrapper orders;
2. semantic creation rejection permits independent later work but blocks only
   the exact child-dependent item;
3. heterogeneous creations preserve their branch, occurrence, `CreationId`,
   current child, initialization actions, and exact reason;
4. initialization actions settle before publication and ordinary ingress,
   including initialization that selects `Step::Stop`;
5. source results run to quiescence in declared order, and closed admission
   returns the current value plus untouched suffix;
6. child-to-parent and parent-to-root closure injections preserve exact
   terminal custody;
7. the unchanged single Engine Driver returns the final behavior and typed
   environment residual through its retirement barrier;
8. a role-first DynamicSupervisor `Stop { key }` application compiles without
   explicit worker/activation types or fabricated values; and
9. the selected Entity and Mnesis suites run separately against their released
   graphs until the owner-side release migration is complete.

## Excluded campaigns

`ActorInterface`, external actors, receptionists, HTTP, discovery, clustering,
process-exit policy, durable inboxes, committed-event relays, and distributed
Entity directories remain independently owned work. The dirty Bombay checkout
contains `ActorInterface` and native Entity experiments, but neither changes
this report's dependency assignment and neither authorizes an actor-algebra
compatibility seam.

## Aggregate-drift checkpoint

This audit changes no production state, protocol, transition, or public
spelling. H591's five transition authorities and measured family rows remain
unchanged: StableProxy 8 control alternatives; FixedSupervisor 5 roster
alternatives; DynamicSupervisor 2 availability plus 14 entry alternatives;
FIFO 5 pool states; KeyedPool 5 pool states. Every exact creation, settlement,
activation, lifecycle, command, and durable-outcome value retains its existing
owner. Arrival history, repeated causes, false cardinality, nested transition
authority, semantic booleans, dynamic escape, and structural user syntax remain
zero. Disposition: `pass`.
