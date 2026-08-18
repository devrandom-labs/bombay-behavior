# Worker Pool Semantics

Pool wiring follows the orthogonal protocol model described in
[Protocol, ingress, behavior, and effect algebras](protocol-algebra.md).
`WorkerPoolProtocol` is stable public pool identity; `PoolEvent` includes
internal supervision facts; `WorkerPool` owns state and the fold; `PoolSends`
is its explicit effect product. A worker assignment carries the established
pool recipient needed for completion, so workers never reconstruct a global
address or require the pool's behavior type.

Bombay's worker pool is a derived behavior construction. It is not an
actor-model primitive and it does not extend the actor transition effects.
The actor-model law used here is only that an actor handles one communication
at a time and may return communications, fresh creations, and its next
behavior. FIFO admission, bounded backlog, worker selection, interruption
handling, and the ordering below are deliberate Bombay policies.

The pool remains a deterministic fold:

```text
(pool state, one typed event) -> Result<(pool state, Actions), PoolError>
```

`Actions` remains the complete effect boundary. The pool does not inspect a
mailbox, spawn a task, allocate an actor, read a clock, or query worker
liveness. The Bombay runtime gives the pool's typed creations, deliveries, creation
resolutions, and worker-stop observations their runtime meaning.

## Ownership law

Every accepted job is owned by exactly one semantic state until a terminal
response is emitted:

```text
submitted -> queued | assigned
queued -> assigned
assigned -> completed | interrupted | queued-for-retry
```

Rejected jobs were never accepted. A job cannot simultaneously be queued and
assigned. An assignment is made visible in the retained state before its
delivery appears in the same `Actions` value. Completion must name the exact
assignment token currently owned by the worker slot; stale, duplicate, and
cross-slot completions are typed errors and cannot complete another job.

Admission prepares one cloned dispatch payload before committing ownership.
The queued state owns the caller's retained recovery payload and that prepared
worker payload as a named product; dispatch moves the prepared value without
cloning or another fallible policy call. Retry prepares its next dispatch value
before leaving `Assigned`. A panicking application `Clone` therefore leaves
the prior semantic state unchanged. This copying is the explicit cost of the
pool's payload-returning failure and at-least-once retry contract.

## Worker phases

Each stable supervised worker slot is exactly one of:

- `Installing`: no assignment may be sent;
- `Idle`: eligible for one assignment;
- `Assigned`: owns one accepted job and its assignment token; or
- `Retired`: creation was rejected and the slot is not eligible.

A replacement request does not make a slot idle. Only a matching successful
`WorkerCreationResolved` event does so. A rejected creation retires the slot.
The stable proxy and fresh worker-incarnation protocol are the existing
Bombay supervision construction; fresh creation is still interpreted by
the Bombay runtime.

## Ordering policy

The backlog is FIFO and idle slots are visited in configured order. On every
event the pool first updates semantic ownership, then folds the same event
through supervision, then fills idle slots from the backlog. Consequently a
successful worker installation may dispatch work in the same transition, but
an installing or rejected worker never receives an assignment.

Creation-before-send interpretation remains the existing Bombay policy. The
pool does not add an ordering guarantee between independent send-product
lanes.

## Key-persistent routing

`KeyedWorkerPool` adds a typed key `K` and an explicit affinity table to the
same pool fold. This is a derived Bombay policy, not an actor-model property.
The complete binding law is:

```text
unbound key + accepted submission -> bound to select(&key)
unbound key + Rebalance           -> explicitly bound to named slot
bound key + submission            -> same stable worker slot
bound key + worker replacement    -> unchanged binding
bound key + Rebalance             -> named new stable worker slot
```

The statically dispatched selector value is consulted only for an unbound key.
It may capture immutable configuration without a trait object, registry, or
free-standing function requirement. A submission is not admitted and creates
no binding if the selected route is unknown, retired, or lacks backlog
capacity; the owned payload is returned in a typed
`PoolResponse::Rejected`. Equality is concrete through `K: Eq`; there is no
hashing service, erased key, registry, or global membership lookup.

Affinity names the stable supervised proxy nonce, never a fresh worker
incarnation. A successful replacement therefore preserves affinity without
address reuse inference. A rejected replacement retires the slot, and later
submissions for its keys are explicitly refused until a valid `Rebalance`
names another non-retired slot.

Rebalance affects only future admission. Each accepted job stores its selected
slot, so queued and interrupted/retried work cannot migrate implicitly when a
binding changes. Work for a busy or installing affinity slot remains queued
even if another slot is idle. FIFO order is preserved among jobs eligible for
the same slot; there is deliberately no global FIFO claim across independent
affinity lanes.

If a selected slot retires, every queued job still targeting it terminates in
the same fold. A never-assigned job reports `AffinityRetired`; a retried job
preserves its exact earlier `WorkerStopped` cause. Another live slot cannot
hide or strand those jobs. A later rebalance can restore admission for the key
but cannot resurrect already terminated ownership.

The selector does not resolve an actor address. The fold emits an ordinary
typed `Recipient::child(stable_nonce)` assignment. The Bombay runtime derives the child
address from that relative route and Bombay Address resolves the resulting
address to the exact live registered endpoint. Thus Behavior owns selection
policy while Bombay Address retains registration and endpoint-routing
authority; neither duplicates the other.

## Construction products

Pool construction separates topology from operating policy:

```rust,ignore
let topology = ChildTopology::indexed(nonce_for, worker_count, build_worker);
let configuration = PoolConfiguration::new(
    backlog_capacity,
    InterruptionPolicy::Retry,
    RestartPolicy::Permanent,
    maximum_restarts,
    restart_window,
);
let pool = WorkerPool::new(topology, configuration)?;
```

`ChildTopology` owns the ordered creator-local nonces and the pure slot
factory. `PoolConfiguration` owns backlog, interruption, and restart policy.
`KeyedWorkerPool::new` accepts those same two products plus its selector. No
positional argument carries meaning by its location, and construction still
rejects empty or duplicate topology before a behavior exists.

## Admission

Submission while an idle worker exists is accepted and assigned. Otherwise it
is accepted into the backlog only while the configured backlog capacity has
room. A full backlog returns the owned job in `PoolResponse::Rejected`; it is
never silently dropped. `Accepted` means that the pool fold accepted ownership,
not that a worker has completed or even received the job.

## Worker interruption

Interruption behavior is a declared Bombay policy:

- `Fail`: emit `PoolResponse::Interrupted` and end pool ownership of the job;
- `Retry`: return the job to the front of the backlog. This is at-least-once
  assignment and may duplicate external effects if the failed incarnation
  acted before stopping.

If every fixed slot is retired, the pool returns every still-accepted backlog
entry as `Interrupted`; accepted ownership is never stranded in a pool that
can no longer dispatch. A retried job preserves its worker-stop cause, while a
job that had not yet been assigned reports `NoRecoverableWorkers`.

The pool does not claim exactly-once execution. Such a guarantee requires an
application protocol for idempotency, deduplication, or transactional effects.
Normal and abnormal worker outcomes are both interruptions when an assignment
is outstanding; supervision's independent restart policy still decides
whether a replacement is requested.

## Boundary ownership

Bombay Behavior owns the pool protocol, slot/backlog state, selection,
admission, assignment correlation, interruption decision, and resulting
`Actions`. The behavior executor owns exclusive turn execution and complete
effect interpretation before the next input. The Bombay runtime owns mailbox ingress,
routing, fresh installation, typed creation resolution, worker termination
observation, timers, and retirement. Neither the executor nor the Bombay runtime may
select a worker, mutate pool accounting, or infer retry policy.

`PoolBehaviorSends` exposes the pool's two ordinary delivery lanes by semantic
name: `responses` and `assignments`. An interpreter integrates this concrete
product by routing both lanes in that order through its existing typed
delivery router. It must not flatten them into an erased envelope or interpret
either lane as a interpreter request. Wrapper composition continues through the
standard `SendInput` implementation and cannot consume or reorder either lane.

Construction rejects a zero-worker topology with `PoolConfigError::NoWorkers`,
so a pool cannot accept ownership without any possible worker slot.

Assignment correlation uses a monotonic `u64` counter. The final counter value
is reserved so a successful dispatch batch always has a representable successor
counter. Reaching that physical boundary poisons the actor turn before the
dispatch plan is committed; this is a Bombay implementation limit, not an actor
model guarantee or ordinary recoverable pool outcome.

## Research classification

Gul Agha's actor semantics supplies asynchronous communication, fresh actor
creation, and behavior replacement as the actor's reaction capabilities. It
does not specify worker pools or their scheduling and delivery guarantees.
The construction here follows that semantic boundary while treating all pool
decisions as explicit Bombay policy. See Agha, Mason, Smith, and Talcott,
“A Foundation for Actor Computation,” *Journal of Functional Programming* 7(1),
1997, and Karmani and Agha, “Actors,” Open Systems Laboratory, 2011.
