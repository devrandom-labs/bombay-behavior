# Worker pool semantics

`WorkerPool` and `KeyedWorkerPool` are pure derived Bombay behaviors, not
actor-model primitives. The actor-model laws used here are isolated turns,
communication to acquaintances, fresh creation, and explicit next behavior.
FIFO scheduling, bounded admission, affinity, interruption, and replacement
policy are Bombay choices.

```text
(pool state, one PoolEvent) -> Result<Actions, PoolError>
```

The fold never reads a mailbox or clock, allocates an actor, looks up an
endpoint, or observes liveness. It emits typed creation, delivery,
observation, and timing effects for an interpreter.

## Identity and effects

`WorkerPoolProtocol` is canonical public pool identity. `PoolEvent` is the
larger internal event algebra. `PoolBehaviorSends` exposes two named ordinary
delivery lanes:

- `responses` to submitters; and
- `assignments` to supervised worker proxies.

The protocol, event, behavior, and sends types remain distinct. A pool
assignment carries a typed logical `Recipient<PoolProtocol>` for completion;
it does not require the pool's behavior type. That logical recipient is
appropriate because completion crosses an ordinary address boundary and is
resolved by the runtime.

Assignments to the pool's own child proxies use
`ChildDelivery<ProxyProtocol, ChildHead>`. The delivery retains the stable
proxy nonce and direct structural occurrence. The active pool instance supplies
the creator namespace. No child address is derived from the nonce and no
application-wide protocol space is required for this local path.

## Ownership law

Every accepted job is owned by exactly one semantic state until one terminal
response is emitted:

```text
submitted -> queued | assigned
queued -> assigned
assigned -> completed | interrupted | queued-for-retry
```

Rejected jobs were never accepted. A job is never both queued and assigned.
An assignment enters retained state before its delivery appears in the same
`Actions`. Completion must name the exact `AssignmentId` currently owned by
the worker slot; stale, duplicate, and cross-slot completions are typed errors.

Admission prepares a cloned dispatch payload before committing ownership. The
queued state owns both the recovery payload and prepared worker payload.
Dispatch moves the prepared value without another fallible policy call. Retry
prepares its next dispatch value before leaving `Assigned`, so a panicking
application `Clone` cannot leave a partially changed semantic state.

## Worker phases

Each fixed stable proxy slot is exactly one of:

- `Installing`;
- `Idle`;
- `Assigned { assignment, job }`;
- `Stopping`; or
- `Retired { reason }`.

An installing or retired slot cannot receive work. A replacement request does
not make a slot idle. Only its matching successful creation fact does so. A
creation rejection retires the slot and cannot be reported as a restart.

The stable proxy is a derived identity construction. Each worker incarnation
is still freshly allocated. Affinity and queued work name the proxy slot nonce,
never infer identity from an incarnation address.

## Admission and ordering

Idle slots are visited in configured order and the ordinary backlog is FIFO.
For each event the pool:

1. computes the ownership transition;
2. folds the same event through supervision; and
3. fills eligible idle slots from the backlog.

A successful installation may therefore dispatch work in the same transition.
The interpreter's creation-before-sends policy ensures the corresponding local
binding exists first.

Submission is immediately assigned when an eligible slot exists. Otherwise it
is queued only while backlog capacity remains. A full backlog returns the
owned payload in `PoolResponse::Rejected`; nothing is silently dropped.
`Accepted` means the pool accepted ownership, not that a worker received or
completed the job.

## Key-persistent routing

`KeyedWorkerPool<K, ...>` adds an explicit affinity table and a concrete
`AffinitySelector<K, Nonce>`:

```text
unbound key + accepted submission -> select and bind one stable slot
unbound key + Rebalance           -> bind the named live slot
bound key + submission            -> retain that stable slot
bound key + worker replacement    -> binding unchanged
bound key + Rebalance             -> named new stable slot for future jobs
```

The selector is consulted only for an unbound key. It is statically dispatched
and may carry immutable configuration. There is no trait object, global
registry, hashing service, or erased key.

Admission creates no binding if the selected slot is unknown, retired, or
cannot accept backlog ownership. Rebalance affects future submissions only;
accepted jobs retain their chosen slot. Work for a busy or installing affinity
slot remains queued even when another slot is idle, so no global FIFO claim is
made across independent affinity lanes.

When a selected slot retires, every queued job targeting it terminates in the
same fold. Never-assigned work reports `AffinityRetired`; retried work retains
its earlier `WorkerStopped` cause. A later rebalance cannot resurrect already
terminated ownership.

## Interruption

`InterruptionPolicy` is explicit:

- `Fail` emits `PoolResponse::Interrupted` and ends ownership;
- `Retry` returns the job to the front of its eligible backlog.

Retry is at-least-once assignment and may duplicate application effects if the
old incarnation acted before stopping. Exactly-once execution requires an
application idempotency, deduplication, or transaction protocol.

If every fixed slot is retired, all accepted backlog work is returned as
interrupted. Accepted ownership is never stranded. Supervision separately
decides whether an incarnation outcome is eligible for replacement.

## Construction

Topology and policy are named products:

```rust,ignore
let topology = ChildTopology::indexed(nonce_for, worker_count, build_worker);
let configuration = PoolConfiguration::new(
    backlog_capacity,
    InterruptionPolicy::Retry,
    RestartPolicy::Permanent,
    maximum_restarts,
    restart_window,
);
let pool = WorkerPool::new(topology, configuration, complete_to)?;
```

`ChildTopology` owns ordered creator-local nonces and the pure slot factory.
`PoolConfiguration` owns backlog, interruption, and restart policy.
Construction rejects empty topology, duplicate nonces, and exhausted nonce
sequences before a behavior exists.

## Boundary ownership

Behavior owns selection, job/slot state, assignment correlation, admission,
interruption, restart decisions, and resulting `Actions`. The interpreter owns
exclusive turns, fresh installation, creator-local binding, logical external
delivery, exact endpoint delivery where used, observation, timers, and effect
completion.

Neither side may infer the other's policy. In particular, the interpreter may
not choose workers or retries, and the behavior may not infer installation or
liveness from nonce arithmetic.
