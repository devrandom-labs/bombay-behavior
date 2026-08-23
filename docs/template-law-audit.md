# Behavior Actors template law audit

This audit covers every reusable behavior template exported by
`bombay-behavior-actors`. It distinguishes actor-model law from Bombay's
derived routing and lifecycle policies; an exact endpoint is useful, but it is
not required by the actor model merely because it exists.

## Semantic basis

Actor-model law requires that a transition communicate only through existing
acquaintances, names received in the current communication, or freshly created
names. It also requires fresh allocation and keeps behavior replacement
separate from creation. See Agha, *Actors: A Model of Concurrent Computation in
Distributed Systems*, and Agha, Mason, Smith, and Talcott, *A Foundation for
Actor Computation*.

Bombay derives three distinct communication capabilities:

| Capability | Meaning | Runtime work |
|---|---|---|
| `Recipient<P>` / `Delivery<P>` | logical protocol name | select according to logical-name policy |
| `ChildRoute<C, O>` / `ChildDelivery<P, O>` | one creator-local child occurrence and nonce | select the committed local binding |
| `EstablishedRecipient<P>` / `EstablishedDelivery<P>` | one exact installed incarnation | use the carried endpoint directly |

Using a logical name is therefore lawful when stable naming, discovery,
membership, transport addressing, or a caller-supplied reply address is the
declared protocol. It is a defect when a template fabricates a logical address
from a child nonce, discards a structural occurrence, degrades an exact
capability to an address, or asks an interpreter to discover a wrapper lane.

## Findings corrected

- `ShutdownChild`, `ObserveChild`, `ObserveCreation`, and `ChildTermination`
  now retain their structural occurrence. Duplicate declarations of the same
  behavior or protocol cannot select the same interpreter capability by type.
- Homogeneous and heterogeneous shutdown coordination preserve those
  occurrences through their complete request products.
- Fixed, delayed, dynamic, FIFO-pool, and keyed-pool proxy owners all expose a
  `ParentPath` form. Their proxy births carry the final
  `ProxyParentIngress<A, ParentPath>` selected for the complete outer event
  algebra, including an outer `Guardian`.
- `MessageAdapterWithRoute` statically selects logical, established, or
  creator-local child delivery through `DeliveryRoute`; its associated send
  effect remains concrete.
- `WatchWith` and `TerminationMonitorWith` statically select either the legacy
  late-bound address request or exact established observation. Exact mode
  correlates every fact by `ObservationId`; terminal monitor phases consume a
  relationship once and preserve controlled-error atomicity.
- `Guardian::coordinated` is the explicit root policy that delegates a root
  shutdown to an inner owner. It is proven with `FinalizeOnShutdown`, including
  preservation of final sends, creations, and stop.
- The circuit breaker no longer uses a production `expect` to advance its
  attempt counter; exhaustion is an ordinary typed transition.

These are Bombay construction or policy corrections. Structural occurrence,
observation IDs, shutdown ordering, proxy identity, and Guardian policy are
not presented as Agha laws.

## Complete template classification

### Pure wrappers and lifecycle

`Machine`, `Stash`, `StopOnShutdown`, `FinalizeOnShutdown`, `Guardian`,
`Deadline`, `OneShot`, `Periodic`, and `ReceiveTimeout` introduce no retained
destination. Wrapper-owned interpreter requests return through a structural
`Here`; nesting reindexes the request's return path through `Inside<Path>` and
does not rewrite any destination carried by inner effects.

`Watch` and `TerminationMonitor` deliberately retain their address-selected,
late-bound forms. `EstablishedWatch` and `EstablishedTerminationMonitor`
select an exact endpoint without lookup. `PropagateTermination` supports an
occurrence-aware local child target and the separately documented late-bound
peer target. `ShutdownCoordinator`, `TreeShutdown`, and
`HeterogeneousShutdownCoordinator` use occurrence-indexed child shutdown;
`ShutdownEstablished` remains the stronger operation for an already retained
concrete actor capability. `Task` preserves the logical reply address supplied
by its request.

### Composition

`MessageAdapter` is the logical-name alias. `MessageAdapterWithRoute` accepts
`Recipient`, `EstablishedRecipient`, or `ChildRoute`, producing exactly
`Delivery`, `EstablishedDelivery`, or `ChildDelivery` respectively. It does
not use a runtime route enum or protocol registry.

### Supervision and pools

`Proxy`, `Supervise`, `Supervisor`, both backoff forms,
`DynamicSupervisor`, `WorkerPool`, and `KeyedWorkerPool` send to owned workers
and stable proxies with `ChildDelivery<_, ChildHead>`. `ChildHead` is truthful
here: each topology-changing owner declares the proxy or worker as its first
direct structural child. Observation, creation resolution, and shutdown carry
that same occurrence.

Every proxy-owning form has a corresponding `WithParent` form where wrapping
can change the parent's event path. The direct aliases use `Here`; callers
constructing an outer event layer supply `Inside<...>` explicitly. No runtime
payload search repairs an incorrectly authored path.

Dynamic-supervisor `Started` results expose the stable proxy's logical
recipient by design. That value denotes the derived stable identity, not the
replaceable worker incarnation. Pool completion destinations and command
reply addresses are likewise logical values supplied by the surrounding
protocol. No exact capability is converted into either value.

### Routing

`Router`, `RoundRobin`, `Broadcast`, `LeastLoaded`, `ConsistentHash`, and
`RendezvousHash` define membership over logical recipients; equality,
removal, stable member evidence, and stable routing names are their domain.
`WorkQueue` similarly models logical worker availability.

`Buffer`, `PriorityQueue`, `OrderGate`, `Sequencer`, `Deduplicator`, and
`RateLimiter` preserve the logical target and reply capabilities supplied in
each accepted message. `CircuitBreaker`, `Correlator`, and
`Acknowledgements` preserve caller-supplied logical reply capabilities. None
derives a recipient from a nonce, address arithmetic, timing, or adjacency.

The buffer contains one documented production invariant: positive validated
capacity plus the full-offer branch proves a non-empty queue before
`DropOldest` removes an element. Its state is private, and its tests exhaust
all overflow policies and value conservation.

### Discovery

`Registry`, `Resolver`, `Topic`, and `PubSub` intentionally store and compare
logical recipients. Discovery and subscription are stable-name domains, so
silently changing them to exact incarnations would change unbind,
unsubscribe, replacement, and equality semantics. `Presence` also retains the
logical notifier supplied at registration while using explicit version and
timer-generation evidence for stale-event rejection.

### Workflow, operations, and persistence

`Latch`, `Barrier`, and `Workflow` retain logical waiters or command reply
addresses supplied by their messages. `Configuration`/`Features`, `Health`,
and `Readiness` do the same for query replies. `Cache` returns results through
the caller's logical reply capability. These templates neither own child
topology nor receive an exact endpoint that they could accidentally degrade.

## Hard-coding checks

- No template uses `dyn Trait`, `Any`, `TypeId`, `unsafe`, erased messages, or
  runtime protocol lookup.
- No production template derives an actor address from a child nonce.
- `From<u64>` bounds in supervision and pools convert configured fleet indexes
  only into creator-local nonces; they do not allocate addresses or infer
  freshness.
- Lifecycle and overlap state is represented by exhaustive enums rather than
  coordinated booleans.
- Public multi-lane effects use named products. `SendLayer` positional access
  remains composition infrastructure and is not exposed as domain meaning.
- No exact recipient is converted back to a logical recipient anywhere in the
  actors crate.

This classification is the review boundary for future template changes. A
new destination must declare whether it is logical, creator-local, or exact;
changing that choice is a semantic API change, not a convenience refactor.
