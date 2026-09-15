# Atomic actor architecture map (engineering record)

Status: feature-complete normalization map. This document owns dependency
direction and responsibility placement only. It does not restate aggregate
state machines, runtime settlement, public construction syntax, or verification
matrices.

## Protocol vocabulary and reuse

Atomic protocols use names that expose timing and ownership. An `input` enters
a behavior transition. A `command` carries application intent. A runtime
`request` remains owned until interpretation returns either an exact `receipt`
or a `rejection` containing the complete request and reason. An `outcome` or
`report` is a later domain or lifecycle result, not proof of send acceptance. A
`reply` is an application-level answer to a command.

When several operations have the same complete ownership and timing equation,
that equation is represented once and parameterized by its domain request,
receipt, rejection, or outcome values. `Result<Receipt, (Request, Rejection)>`
is preferred for the exact two-alternative case. A separate sum is justified
only by different alternatives, ordering, correlation, or retirement—not by a
different aggregate name or message suffix.

## Semantic authority map

| Concern | One normative owner |
|---|---|
| Stable-proxy transition law | [`actor-laws/proxy.md`](../actor-laws/proxy.md) |
| Fixed-supervisor transition law | [`actor-laws/fixed-supervisor.md`](../actor-laws/fixed-supervisor.md) |
| Dynamic-supervisor transition law | [`actor-laws/dynamic-supervisor.md`](../actor-laws/dynamic-supervisor.md) |
| FIFO-pool transition law | [`actor-laws/fifo-pool.md`](../actor-laws/fifo-pool.md) |
| Keyed-pool transition law | [`actor-laws/keyed-pool.md`](../actor-laws/keyed-pool.md) |
| Generic interpretation, settlement, and terminal custody | [`atomic-runtime-settlement.md`](../atomic-runtime-settlement.md) |
| Application construction and usage | [`atomic-actor-devx.md`](atomic-actor-devx.md) |
| Verification and evidence gates | [`atomic-actor-verification.md`](atomic-actor-verification.md) |

The family documents in this directory own collaboration boundaries, policy
classification, and realization gates. They link to the authoritative law
matrices instead of copying them.

`Worker` means the concrete application-defined `W: Behavior`, not a Bombay
trait or wrapper. Atomic templates may privately host that value through fresh
creation, initialization, activation, exact capability ownership, and stop.
`WorkerSubmission<W, P>` is the affine construction product pairing the worker
with its activation work; it is not another actor abstraction. Stable service
identity and replacement belong to `StableProxy`; roster and recovery policy
belong to supervisors; assignments belong to pools; bindings belong to
`KeyedPool`. Shared worker-hosting data may encode only equations that are
identical in every real consumer and may never select those contextual policies.

`InitialWorkerRejection` is the one shared construction result used by Fixed,
FIFO, and Keyed construction. It owns the callable, prepared prefix, unavailable
role and reason, and untouched suffix. Each aggregate construction error embeds
that value beside only its own policies. The private preparation operation runs
before a behavior exists; it cannot run application code during a transition.

The StableProxy source location does not make intrinsic worker values
proxy-owned. `ActivationPlan`, `WorkerSubmission`, opaque worker/
initialization/activation correlations, initialization and activation requests
and results, exact established-worker custody, and pre-commit worker rejection
are aggregate-neutral data. FIFO and KeyedPool now share only their proven
direct-worker relationship beneath the private `atomic::pool::worker`
hierarchy, while retaining the canonical `atomic::*`
application/interpreter spelling.
`PendingWorker`, `WorkerStartResult`, and `ProxyDrain` remain StableProxy
concerns in their present form; a pool must not translate or ignore their proxy
outcomes.

## Dependency direction

```text
Behavior interpretation law
    -> validated values and boundary records
    -> StableProxy
        -> FixedSupervisor
        -> DynamicSupervisor
    -> direct-pool semantic values
        -> FifoPool
        -> KeyedPool
    -> inferred construction and concrete protocols

Bombay Engine
    -> Behavior + Behavior Actors
    -> Address + Communication + Observe + Timers
```

The arrows denote use, never ownership reversal. There are no aggregate cycles.
`FixedSupervisor` and `DynamicSupervisor` are independent folds that collaborate
with the one `StableProxy` law. `FifoPool` and `KeyedPool` are independent direct
worker folds; neither contains a proxy or supervisor and neither implements the
other.

## Repository responsibility

- `crates/behavior` owns the pure `Behavior` fold, `Actions`, and only generic
  statically typed interpretation products proven useful across the catalogue.
- `crates/actors` owns the five reusable aggregate laws and their concrete
  protocols, policies, builders, and pure folds.
- Bombay owns the single Engine `Driver`, tasks, mailboxes, actor installation,
  activation, capability interpretation, observation, timers, and retirement.
- Communication owns mailboxes; Address owns identity and leases; Observe owns
  completion publication; Timers owns scheduling state.
- `crates/behavior-testkit` and the nested atomic experiment own independent
  models and falsification evidence, never production semantics.

## Module ownership

Aggregate directories expose the ownership hierarchy directly. A child file
uses the shortest name that is meaningful beneath its parent, and the aggregate
`mod.rs` alone selects the public surface. For example:

```text
stable_proxy/
    mod.rs
    state.rs
    protocol.rs
    effects.rs
    operation.rs
    worker/
        mod.rs
        initialization.rs
        activation.rs
```

Thus `stable_proxy/worker/mod.rs` reads as the proxy's exact worker ownership, without a
redundant `proxy_worker_start.rs` filename. Nested modules do not widen
visibility or create another transition owner; their parents control what may
be used by sibling concerns and what reaches applications.

The fixed supervisor follows the same rule without decorative directories:

```text
fixed_supervisor/
    mod.rs
    member.rs
    role.rs
    proxy/
        mod.rs
        start.rs
        stop.rs
        outcome.rs
    recovery/
        mod.rs
        correlation.rs
        preparation.rs
        stop.rs
    restart/
        mod.rs
        admission.rs
    shutdown/
        mod.rs
        member.rs
```

Each group uses its existing principal concern as `mod.rs`; there is no empty
module whose only job is to make the tree deeper.

FIFO uses the same ownership rule with fewer concerns:

```text
fifo_pool/
    mod.rs
    member.rs
    job.rs
    policy.rs
    protocol.rs
    requests.rs
```

`mod.rs` is the only transition authority. `member.rs` stores the current
direct-worker relationship, `job.rs` owns customer custody and assignment
reunion, and the other three modules separate validated application policy,
application protocol, and interpreter requests. None receives arbitrary pool
inputs or emits `Actions` independently. Creation, activation, readiness,
assignment, recovery, and retirement are member alternatives, not one child
module per arrival path.

Forbidden edges include Behavior depending on Behavior Actors, pools depending
on StableProxy, supervisors depending on pool-private models, any Behavior fold
depending on runtime services, or any component introducing a second Driver,
mailbox, registry, lifecycle service, erased envelope, or dynamic capability
map.

## Law classification

Fresh allocation, isolated processing of one communication, sending to known
recipients, and explicit next behavior are actor-model laws. Stable identity,
supervision strategy, admission, affinity, retry, settlement products, and
terminal customer outcomes are derived constructions. Creation-before-dependent
operations, initialization settlement before ordinary ingress, diagnostic
disposition, drain policy, and root terminal custody are deliberate Bombay
policies. Family documents label policy choices without presenting them as Agha
guarantees.

## Campaign boundary

Entity/Mnesis persistence, `ActorInterface`, external actors, receptionists,
HTTP, discovery, clustering, and process-exit policy are downstream constraints,
not part of the five-fold implementation. Their migration requirements are
recorded separately and cannot justify a compatibility alias in this campaign.

The normalized catalogue, canonical syntax prototype, exact dependency
contracts, and total-settlement representation gates pass. Bombay owns the
documented four-link terminal-custody change externally. Local production begins
after the independent evidence commit and explicit clean-room deletion commit.
