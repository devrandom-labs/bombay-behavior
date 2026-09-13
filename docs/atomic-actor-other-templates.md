# Disposition of every other actor template

The five supervisor/pool actors are not a reason to delete the rest of
`crates/actors`. Most other templates own different, smaller transition laws.
They should remain independent. This audit classifies the current source and
prevents the supervisor/pool rewrite from becoming an accidental catalogue
rewrite.

No production deletion or migration is authorized by this document. Each
`Update` row needs its own law and regression after the five atomic actors are
designed; it must not be folded into their implementation stage.

## Dispositions

- **Keep** — distinct law; no dependency on legacy supervisor/pool machinery.
- **Keep, cross-cutting update** — distinct law, but it inherits a newly found
  delivery, capacity, retention, readiness-name, or lifecycle gap.
- **Replace** — its feature belongs to one of the five new atomic actors.
- **Remove after migration** — structural plumbing exists only to support the
  legacy supervisor/pool architecture.
- **Internal infrastructure** — not an actor template and not ordinary DevX;
  retain only while a concrete interpreter/composition proof needs it.

## Composition and lifecycle

| Current family | Disposition | Reason / required change |
|---|---|---|
| `Activate` / `Initialized` / `Active` | Keep as advanced pure-fold tooling | It proves one-time synchronous `Behavior` initialization only. Rename no type, but documentation must say it is not worker activation/readiness. |
| `MessageAdapter` | Keep, cross-cutting update | A real typed message transformation. It must inherit rejected-delivery ownership for logical, established/exact, and occurrence-aware creator-local child destinations. An adapter with no birth capability cannot fabricate or reconstruct a `ChildRoute`; the creating owner must carry the exact occurrence proof through the named effect lane. |
| `Machine` | Keep | A direct finite-state/defer/replay actor with its own law. It is not supervisor behavior composition. |
| `Stash` | Keep | A genuine transparent wrapper with replay and initialization-order laws. It is not a component of the new atomic actors. |
| `StopOnShutdown` | Keep | Orthogonal shutdown-ingress wrapper. Supervisors/pools own their internal drains and may themselves be wrapped only when the composition law is explicit. |
| `FinalizeOnShutdown` | Keep, cross-cutting update | Orthogonal finalization wrapper; final-send rejection must not be confused with successful finalization. |
| `Watch` / `Link` | Keep, cross-cutting update | Exact/logical peer observation is a distinct capability. Observation and reaction delivery rejection need typed ownership. |
| termination monitors | Keep, cross-cutting update | Exact/logical observation plus cleanup/publication is distinct. Publication rejection must preserve its fact. |
| terminal propagation | Keep, update | Generic propagation remains valid, but the current supervision-specific terminal reason sum must be reshaped around the new recovery/forced-retirement outcomes. |
| homogeneous shutdown coordinator | Keep, update separately | It owns dependency-ordered child shutdown. It needs an explicit stuck-child/deadline law if it promises completion; it is not reused as the internal pool/supervisor drain engine. |
| heterogeneous shutdown coordinator and child-shutdown builder | Keep, update separately | Static heterogeneous application shutdown is a different topology problem. Keep its exact-role law; do not copy its public proof-state taxonomy into atomic builders. |
| `Task` | Keep, cross-cutting update | One-shot typed task/result actor. Result delivery rejection must preserve the owned result. |

## Discovery

| Current family | Disposition | Reason / required change |
|---|---|---|
| `Resolver` | Keep, cross-cutting update | Immutable finite bindings supplied at construction. Only reply-delivery rejection is newly shared. |
| `Registry` | Keep, update | Mutable key bindings are a distinct discovery law, but the current `Vec` has no capacity policy. Add bounded admission/removal semantics and rejected-reply ownership in its own stage. |
| `Topic` | Keep, update | Typed membership/broadcast is distinct. Current subscriber growth is unbounded; add membership capacity and delivery-failure disposition. |
| `PubSub` | Keep, update | Typed topics/subscribers are distinct. Current topic table is retained even when empty and both dimensions are unbounded; add topic/member capacity and topic-retirement law. |
| `Presence` | Keep, update | Versioned presence/expiry is distinct. Current expired tombstones and participant table can grow forever; add bounded retirement/generation rules and reply-delivery rejection. |

These actors do not become Entity, supervisor, or pool services. Their logical
host requirements remain a separate application-topology concern.

## Operational and state actors

| Current family | Disposition | Reason / required change |
|---|---|---|
| `Configuration` and `Features` alias | Keep, cross-cutting update | Single versioned configuration state; only reply-delivery rejection is shared. `Features` remains an alias, not a second actor. |
| `Health` | Keep, cross-cutting update | Fixed component-health evidence is independent. Preserve exact evidence and rejected report ownership. |
| current `Readiness` actor | Keep, rename/update | It means versioned readiness of a declared dependency set, not incarnation activation. Prefer the public name `DependencyReadiness` so the new `Installed → Activating → Ready` lifecycle cannot be confused with it. |
| `Cache` | Keep, cross-cutting update | Bounded in-memory cache is independent. It is not durable persistence; module/docs should stop implying durability, and result-delivery rejection must preserve values. |

Moving `Cache` out of a `persistence` taxonomy is a documentation/module API
decision for a later contained stage. It is not necessary to implement the
atomic actors.

## Routing and admission actors

| Current family | Disposition | Reason / required change |
|---|---|---|
| `Acknowledgements` | Keep, update | Multi-party acknowledgement is distinct. Completed/cancelled records are currently retained forever; add capacity, retirement, and key-reuse generation laws. |
| `Buffer` | Keep, cross-cutting update | Bounded deferred delivery is distinct. Update target/outcome rejection ownership. |
| `CircuitBreaker` | Keep, cross-cutting update | Closed/open/probe and timer correlation are independent. Update attempt/outcome delivery rejection; do not merge its timer logic into supervisor recovery. |
| `Correlator` | Keep, update | Begin/resolve/cancel correlation is independent. Terminal keys are currently retained forever; add bounded retirement/key-reuse laws and rejected-result ownership. |
| `Deduplicator` | Keep, cross-cutting update | Already has explicit positive capacity/eviction. Update target/outcome delivery rejection only. |
| `OrderGate` | Keep, update | Monotonic ordered release is independent. Its held `BTreeMap` is unbounded; add admission capacity and rejected-release ownership. |
| `PriorityQueue` | Keep, cross-cutting update | Already bounded with typed token exhaustion. Update target/outcome delivery rejection. |
| `RateLimiter` | Keep, cross-cutting update | Fixed token-bucket state is independent. Update target/outcome delivery rejection. |
| `Router` and static strategies | Keep, update | Recipient routing is not worker ownership. Current mutable membership can grow unbounded; add capacity/removal policy and delivery rejection. Strategies remain pure values, not actors. |
| `Sequencer` | Keep, update | Ordered gap release is independent. Future-position retention is unbounded; add gap/capacity/retirement law and target/outcome rejection. |
| `WorkQueue` | Keep, clearly distinguish, update | It routes values to externally announced one-use worker routes. It creates no workers, observes no lifecycle, owns no completion, and performs no recovery. Therefore it is not a pool and the new pool must not contain it. Add delivery-rejection ownership; consider a less ambiguous public name only in a separate migration. |

The shared observation is not “make all routing actors use queue/correlation
utils.” It is that several independent actors currently omit capacity or
delivery-rejection laws. Each keeps its own direct fold and uses only a truly
general interpreter capability once that capability is designed.

## Timing

| Current family | Disposition | Reason / required change |
|---|---|---|
| `OneShot` | Keep | Transparent single-timer wrapper with exact identity/generation. |
| `Periodic` | Keep | Transparent repeated-timer wrapper with its own rescheduling law. |
| `Deadline` | Keep | Absolute-deadline wrapper; distinct from supervisor drain policy. |
| `ReceiveTimeout` | Keep | Mailbox-activity-reset timer wrapper; distinct from backoff or drain deadline. |
| `Lease` | Keep, cross-cutting update | One bounded lease state machine with exact timer generation. Update outcome-delivery rejection. |

Timer IDs/generations and scheduling requests remain neutral interpreter
carriers. The new actors reuse them; they do not copy these wrappers or form a
generic scheduler component.

## Workflow

| Current family | Disposition | Reason / required change |
|---|---|---|
| `Barrier` | Keep, cross-cutting update | Finite declared membership and generation law are independent. Release-delivery rejection must preserve participant outcomes. |
| `Latch` | Keep, cross-cutting update | Finite countdown/waiter law is independent. Release-delivery rejection must preserve waiters. |
| `Workflow` | Keep, cross-cutting update | Finite dependency graph and step-state machine are independent. Outcome-delivery rejection must preserve the exact workflow result. |

## Legacy supervision and pool surface

| Current source/surface | Disposition | Replacement |
|---|---|---|
| `supervision/adapter/proxy.rs`, current `Proxy`, `ProxyEvent`, `ProxySends`, `ProxyUnavailable` | Replace | New stable proxy direct fold and its minimal control/report sums. |
| `Supervise` wrapper and fixed/backoff recipes | Remove after migration | Fixed supervisor already owns supervision; applications should not assemble it from a wrapper/proxy/report stack. |
| current fixed `Supervisor` and `fixed_supervisor.rs` | Replace | New fixed supervisor typestate definition and direct fold. |
| current `DynamicSupervisor` | Replace | New dynamic supervisor with order-independent readiness, capacity, cancellation, generations, and durable lifecycle ownership. |
| `supervision/domain/{fleet,incarnation,ownership,restart_budget}.rs` | Remove after migration | Private state belongs in each new direct actor. Pure recovery policy arithmetic may be rewritten once and shared only if two independent folds prove identical laws. |
| `ChildTopology`, `FixedFleetOwnership`, slot/fleet/ownership types | Remove | Runtime roles/members are actor-private values, not a universal ownership model. |
| current `RestartConfiguration`, `RestartPolicy`, `RestartTiming`, `Backoff` split | Replace/consolidate | Minimal `Recovery`, `Strategy`, `RestartLimit`, and `RestartRelease` sums. |
| current `pool.rs`, `WorkerPool`, `KeyedWorkerPool`, aliases and events | Replace | Separate direct FIFO and keyed pool folds with no supervisor/proxy nesting. |
| `protocol/pool.rs`, `WorkerPoolProtocol`, `KeyedWorkerPoolProtocol` | Remove after migration | New nominal public pool protocols selected by compile prototypes. |
| current `PoolAssignment`, `PoolCompletion`, `PoolCustomer`, job/assignment IDs, pool response/rejection/failure sums | Replace | Opaque completion token, pool-retained customer route, one customer terminal outcome, separate diagnostics, private correlations. |
| `composition/report_relay.rs` | Remove after migration | Its only production consumer is the legacy pool's proxy/report stack. New direct pool workers complete through the pool-issued token; supervisors receive direct proxy reports. |
| supervision-specific protocol values in `protocol/mod.rs` (`WorkerCreationResolved`, `WorkerStopped`, replacement/report wrappers) | Remove/replace | Neutral `CreationResolved`, `ChildStopped`, exact activation facts, and actor-private control/report sums. |
| supervision/pool-specific `LogicalDeliveryProtocols` implementations in `requirements.rs` | Remove/replace | New named actor products receive only their exact static requirement projections. The generic projection mechanism remains internal infrastructure. |
| `SupervisionFailureReason`, `RestartDenial`, and supervision terminal reporting | Update, do not blindly delete | Exact terminal provenance is consumed outside supervision. Reshape it to the new recovery, activation, delivery, and forced-retirement outcomes; remove legacy-only variants and coarse factory failure. |

## Neutral infrastructure kept under audit

The following are not “other atomic templates”:

- `DeliveryRoute`/`DeliveryRouteFor` and exact/logical recipient products;
- neutral creation, observation, child shutdown, timer, and terminal facts;
- generated event/send products and static logical-host projections; and
- occurrence/position machinery required by closed child products.

They remain in the minimal-core/interpreter audit. Atomic actor builders and
ordinary examples must not name their structural paths or product nesting.

## Result

The catalogue does **not** need a mass deletion. The clean boundary is:

1. replace the complete legacy supervision/pool island;
2. remove structural relays and supervision-specific protocols left with no
   consumer;
3. retain independent actors and wrappers;
4. later give every route-emitting actor the same foundational
   rejected-delivery law; and
5. separately repair unbounded retention where this audit identified it.

Those later catalogue repairs must be staged independently. They cannot grow
the supervisor/pool production change or become another universal utility
module.
