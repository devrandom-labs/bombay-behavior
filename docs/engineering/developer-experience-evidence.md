# Developer experience source coverage and document disposition

Companion to the [PRD](developer-experience-prd.md). Source references are pinned to Behavior `d69f992b371c12ab34e73b18e45b8112c90a1508` and Bombay `c3afb3011090ac4c9112fd39e4a76980db856df0`. Repository documents supplied requirement names, not implementation verdicts. Source inventory and test-body inspection do not constitute a fresh passing test run.

Current implementation additions are tracked in the owning PRD's single
checklist. Published Behavior/Actors0.28.0 is verified at d9237dde; it does not
contain finite holding. Candidate [PR114/aafd1e9](https://github.com/devrandom-labs/bombay-behavior/pull/114)
owns finite admission and replay-panic custody in stash/{queue,limit}.rs, with
executable owner tests in stash.rs/stash_limits.rs and real source-selected
Bombay retirement witnesses. [PR113/809e7b3](https://github.com/devrandom-labs/bombay-behavior/pull/113)
owns generic send insertion and private-parameter naming. Both remain unpublished; the pinned inventory below
is the initial source inventory, not a claim about their current spellings.

## Source evidence

| Area | Inspected source and test witnesses | PRD owner |
| --- | --- | --- |
| Core transition and identity | [crates/behavior/src/transition.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior/src/transition.rs) | DX07–08, DX17 |
| Effects, settlement and partial custody | [crates/behavior/src/effects/sending.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior/src/effects/sending.rs) | DX08, DX15–16 |
| Complete actions and creation settlement | [crates/behavior/src/effects/actions.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior/src/effects/actions.rs) | DX08, DX12, DX16 |
| Child occurrences and birth projections | [crates/behavior/src/actor/creation.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior/src/actor/creation.rs) | DX02, DX12 |
| Exact destination and actor authority | [crates/behavior/src/actor/addressing.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior/src/actor/addressing.rs) | DX20–21 |
| Generated syntax and send products | [crates/behavior-macros/src/lib.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior-macros/src/lib.rs) | DX07–08 |
| Infallible stash and retained messages | [crates/actors/src/stash.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/stash.rs) | DX14 |
| Finite test trace and error loss | [crates/behavior-testkit/src/lib.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/behavior-testkit/src/lib.rs) | DX25 |
| External interpreter privacy and custody witnesses | [tests/interpreter-contract/tests/assignment_delivery.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/tests/interpreter-contract/tests/assignment_delivery.rs) | DX16, DX27 |
| Application assembly | [crates/bombay/src/application/mod.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/application/mod.rs) | DX01–02 |
| Application staging and role projection | [crates/bombay/src/application/composition.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/application/composition.rs) | DX02–03 |
| Execution and complete application result | [crates/bombay/src/application/execution.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/application/execution.rs) | DX04–06 |
| Actor retirement and origin custody | [crates/bombay/src/terminal.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/terminal.rs) | DX03–04 |
| External clients and lifecycle | [crates/bombay/src/application/interface.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/application/interface.rs) | DX06, DX20–21 |
| Template methods | [crates/bombay/src/actors/actor_ext.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/actors/actor_ext.rs) | DX09 |
| Worker source port | [crates/bombay/src/worker_preparation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/worker_preparation.rs) | DX11 |
| Standard interpreted request leaves | [crates/bombay/src/local/effects/delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) | DX15–16 |
| Creation and async capability interpretation | [crates/bombay/src/local/effects/creation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/creation.rs) | DX12, DX15–16 |
| Observation and shutdown | [crates/bombay/src/local/effects/observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) | DX21 |
| Engine custody port | [crates/bombay-engine/src/environment.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay-engine/src/environment.rs) | DX16 |
| Pure causal Driver | [crates/bombay-engine/src/driver.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay-engine/src/driver.rs) | DX05, DX16 |
| Acquired application values and failures | [crates/bombay/tests/completed_application_receiving.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/tests/completed_application_receiving.rs) | DX03–05 |
| Public API static denials | [crates/bombay/tests/compile](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/tests/compile) | DX19 |
| Semantic send methods | [crates/bombay/tests/semantic_send_authoring_probe.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/tests/semantic_send_authoring_probe.rs) | DX08 |
| FIFO live recovery | [crates/bombay/tests/fifo_pool_recovery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/tests/fifo_pool_recovery.rs) | DX10–11 |
| Fixed supervisor live recovery | [crates/bombay/tests/fixed_supervisor_recovery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/tests/fixed_supervisor_recovery.rs) | DX10–11 |

## All executable example packages

Physical Rust lines include comments, blanks and tests. These counts locate ceremony; they do not establish how many lines the domain fundamentally needs.

| Package | Rust files | Physical lines | Required simplification |
| --- | --- | --- | --- |
| [counter](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/counter) | 2 | 226 | Inferred terminal and runner; preserve overflow and exact reply |
| [application-topology](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/application-topology) | 1 | 406 | One role declaration; no structural child failure tuples or predicted nonces |
| [actor-templates](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/actor-templates) | 2 | 252 | Actual ActorExt policy with inferred runner and complete terminal |
| [supervision](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/supervision) | 1 | 389 | No composed supervisor/proxy aliases or spaces; preserve recovery and readiness |
| [worker-pool](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/worker-pool) | 2 | 273 | No concrete pool type or spaces; preserve assignment/customer outcomes |
| [axum](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/axum) | 4 | 521 | Same application result; original rejected command; HTTP admission semantics |
| [entity](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/entity) | 1 | 399 | Same assembly; typed family outcomes; preserve hydration/passivation fencing |

## Complete catalogue inventory

The manifest was used to locate source. Each listed source exists and each listed public spelling was checked against that file; manifest implementation marks were not accepted as proof. Whole-catalogue review is at interface/inventory depth; detailed custody tracing concentrates on the source evidence above. Family-specific runtime coverage must still be executed under DX25. The role column selects the target usage path; missing ActorExt methods are DX09 work, not existing API claims. Standalone constructors are also the canonical Behavior-only entry; Bombay decoration methods delegate to the existing owner.

| Composition | Source | Canonical role and acceptance focus |
| --- | --- | --- |
| `DynamicSupervisor`, `dynamic` | [src/atomic/dynamic_supervisor/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/dynamic_supervisor/mod.rs) | Owning constructor; DX10–12 and actual rejection/recovery/shutdown |
| `FifoPool`, `fifo` | [src/atomic/fifo_pool/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/fifo_pool/mod.rs) | Owning constructor; DX10–12 and actual rejection/recovery/shutdown |
| `FixedSupervisor`, `fixed` | [src/atomic/fixed_supervisor/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/fixed_supervisor/mod.rs) | Owning constructor; DX10–12 and actual rejection/recovery/shutdown |
| `KeyedPool`, `keyed` | [src/atomic/keyed_pool/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/keyed_pool/mod.rs) | Owning constructor; DX10–12 and actual rejection/recovery/shutdown |
| `StableProxy` | [src/atomic/stable_proxy/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/stable_proxy/mod.rs) | Owning constructor; DX10–12 and actual rejection/recovery/shutdown |
| `Machine` | [src/machine.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/machine.rs) | Owning `Machine` constructor; DX14 deferred custody and phases |
| `MessageAdapter`, `MessageAdapterWithRoute` | [src/composition/message_adapter.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/composition/message_adapter.rs) | Owning `MessageAdapterWithRoute::new`; DX17 retires duplicate ordinary spelling, preserves route semantics |
| `Stash` | [src/stash.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/stash.rs) | Bombay `ActorExt::with_stash`; DX14 statically infallible replay |
| `Presence` | [src/discovery/presence.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/discovery/presence.rs) | Owning constructor; exact replies, membership and expiry |
| `PubSub` | [src/discovery/pub_sub.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/discovery/pub_sub.rs) | Owning constructor; exact replies, membership and expiry |
| `Registry` | [src/discovery/registry.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/discovery/registry.rs) | Owning constructor; exact replies, membership and expiry |
| `Resolver` | [src/discovery/resolver.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/discovery/resolver.rs) | Owning constructor; exact replies, membership and expiry |
| `Topic` | [src/discovery/topic.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/discovery/topic.rs) | Owning constructor; exact replies, membership and expiry |
| `shutdown_after_children`, `ChildShutdownPhases::finish`, `ChildShutdownPlan` | [src/lifecycle/child_shutdown.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/child_shutdown.rs) | Owning `shutdown_after_children` phase declaration, then `finish`; DX12, DX21 committed child authority |
| `FinalizeOnShutdown` | [src/shutdown.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/shutdown.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `HeterogeneousShutdownCoordinator` | [src/lifecycle/shutdown_coordinator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/shutdown_coordinator.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `PropagateTermination` | [src/lifecycle/termination_propagation.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/termination_propagation.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `ShutdownCoordinator` | [src/lifecycle/shutdown_coordinator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/shutdown_coordinator.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `StopOnShutdown` | [src/shutdown.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/shutdown.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `Task` | [src/lifecycle/task.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/task.rs) | Owning `Task` constructor; exact task result and complete retirement |
| `TerminationMonitor`, `TerminationMonitorWith` | [src/lifecycle/termination_monitor.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/termination_monitor.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `Watch` | [src/watch.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/watch.rs) | Bombay ActorExt decoration backed by this owner; DX09, DX21 prove policy and complete retirement |
| `Configuration` | [src/operations/configuration.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/operations/configuration.rs) | Owning constructor; actual versioned health/readiness/configuration |
| `Health` | [src/operations/health.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/operations/health.rs) | Owning constructor; actual versioned health/readiness/configuration |
| `Readiness` | [src/operations/readiness.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/operations/readiness.rs) | Owning constructor; actual versioned health/readiness/configuration |
| `Cache` | [src/persistence/cache.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/persistence/cache.rs) | Owning cache constructor; capacity/eviction; no durability claim |
| `Acknowledgements` | [src/routing/acknowledgements.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/acknowledgements.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Buffer` | [src/routing/buffer.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/buffer.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `CircuitBreaker` | [src/routing/circuit_breaker.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/circuit_breaker.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Correlator` | [src/routing/correlator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/correlator.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Deduplicator` | [src/routing/deduplicator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/deduplicator.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `OrderGate` | [src/routing/order_gate.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/order_gate.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `PriorityQueue` | [src/routing/priority_queue.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/priority_queue.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `RateLimiter` | [src/routing/rate_limiter.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/rate_limiter.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Router` | [src/routing/router.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/router.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Sequencer` | [src/routing/sequencer.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/sequencer.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `WorkQueue` | [src/routing/work_queue.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/routing/work_queue.rs) | Owning constructor; pressure, exact rejection and ordering policy |
| `Deadline` | [src/time/deadline.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/time/deadline.rs) | Bombay ActorExt timer decoration; DX13 generations and schedule rejection |
| `Lease` | [src/time/lease.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/time/lease.rs) | Owning `Lease` constructor; DX13 exclusive ownership and expiry generations |
| `OneShot` | [src/time/one_shot.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/time/one_shot.rs) | Bombay ActorExt timer decoration; DX13 generations and schedule rejection |
| `Periodic` | [src/time/periodic.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/time/periodic.rs) | Bombay ActorExt timer decoration; DX13 generations and schedule rejection |
| `ReceiveTimeout` | [src/time/receive_timeout.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/time/receive_timeout.rs) | Bombay ActorExt timer decoration; DX13 generations and schedule rejection |
| `Barrier` | [src/workflow/barrier.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/workflow/barrier.rs) | Owning constructor; membership, phase and incomplete-work custody |
| `Latch` | [src/workflow/latch.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/workflow/latch.rs) | Owning constructor; membership, phase and incomplete-work custody |
| `Workflow` | [src/workflow/coordinator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/workflow/coordinator.rs) | Owning constructor; membership, phase and incomplete-work custody |

## Actor capability coverage

All 19 inventoried actor-owned leaves have source declarations and corresponding concrete Bombay interpretation paths. This establishes implementation presence, not exhaustive runtime correctness. Structural lanes `Delivery`, `EstablishedDelivery`, `ChildDelivery`, `ChildInput`, creations and `ReportToParent` are additional obligations, not part of the 19 count.

| Request | Owning source | Bombay interpreter source |
| --- | --- | --- |
| `BeginActivation` | [src/atomic/worker/activation.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/worker/activation.rs) | [creation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/creation.rs) |
| `CancelObservation` | [src/protocol/established.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/established.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `CustomerDelivery` | [src/atomic/pool/customer.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/pool/customer.rs) | [delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) |
| `DiagnosticAction` | [src/atomic/diagnostic.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/diagnostic.rs) | [delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) |
| `InitializeWorker` | [src/atomic/worker/initialization.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/worker/initialization.rs) | [delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) |
| `ObserveChild` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ObserveCreation` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ObserveEstablished` | [src/protocol/established.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/established.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ObserveEstablishedCreation` | [src/protocol/established.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/established.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ObservePeer` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ReportShutdownPlan` | [src/lifecycle/shutdown_coordinator.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/lifecycle/shutdown_coordinator.rs) | [reports.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/reports.rs) |
| `ReportTerminalOutcome` | [src/termination.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/termination.rs) | [reports.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/reports.rs) |
| `ScheduleAfter` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [timers.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/timers.rs) |
| `ScheduleAt` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [timers.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/timers.rs) |
| `ShutdownChild` | [src/protocol/mod.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/mod.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `ShutdownEstablished` | [src/protocol/established.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/protocol/established.rs) | [observation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/observation.rs) |
| `AssignWorker` | [src/atomic/pool/assignment.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/pool/assignment.rs) | [delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) |
| `PrepareWorkers` | [src/atomic/worker/preparation.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/worker/preparation.rs) | [creation.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/creation.rs) |
| `ProxyOperation` | [src/atomic/stable_proxy/operation.rs](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/crates/actors/src/atomic/stable_proxy/operation.rs) | [delivery.rs](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/crates/bombay/src/local/effects/delivery.rs) |

## Full backlog crosswalk

Every identifier in the four requirement inventories has exactly one group below: 270 unique rows, including the ten K8S rows. The old README count of 260 is incorrect. Requirements are retained; their historical M/P/V/D/X/O marks and status prose are not treated as source evidence. This programme preserves local requirements and defines the extension discipline for later work; it does not implement all downstream features.

| Requirement IDs inclusive | Count | Owning PRD requirements | Disposition |
| --- | --- | --- | --- |
| [ACT-01–ACT-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 8 | DX12, DX16, DX21, DX25 | Retain local acceptance; reverify changed code |
| [INT-01–INT-11](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 11 | DX08, DX15–16, DX25, DX27 | Retain local acceptance; reverify changed code |
| [SUP-01–SUP-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 10 | DX10–12, DX21, DX25 | Retain local acceptance; reverify changed code |
| [POOL-01–POOL-11](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 11 | DX10–12, DX20–21, DX25 | Retain local acceptance; reverify changed code |
| [EXEC-01–EXEC-09](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 9 | DX04–05, DX16, DX25–27 | Retain local acceptance; reverify changed code |
| [APP-01–APP-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 8 | DX01–07, DX19–22, DX28 | Retain local acceptance; reverify changed code |
| [CAP-01–CAP-07](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 7 | DX15–16, DX23, DX25 | Retain local acceptance; reverify changed code |
| [CAT-01–CAT-06](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/local-runtime.md) | 6 | DX08–14, DX17, DX25–28 | Retain local acceptance; reverify changed code |
| [WIRE-01–WIRE-12](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 12 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [NET-01–NET-14](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 14 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [ZEN-01–ZEN-14](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 14 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [DISC-01–DISC-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 8 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [QUERY-01–QUERY-06](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 6 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [TOPIC-01–TOPIC-06](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/networking.md) | 6 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [ID-01–ID-12](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 12 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [AUTH-01–AUTH-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 8 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [PLACE-01–PLACE-12](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 12 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [FAIL-01–FAIL-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 10 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [SIM-01–SIM-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 10 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [SELO-01–SELO-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/identity-and-placement.md) | 8 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [DUR-01–DUR-18](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 18 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [OUT-01–OUT-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 8 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [REM-01–REM-06](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 6 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [OBS-01–OBS-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 10 | DX24 | Explicit later product scope; preserve typed extension and outcome laws |
| [K8S-01–K8S-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 10 | DX23, DX25, DX27 | Explicit later product scope; preserve typed extension and outcome laws |
| [RELEASE-01–RELEASE-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 10 | DX19, DX23, DX25–28 | Explicit later product scope; preserve typed extension and outcome laws |
| [PERF-01–PERF-08](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 8 | DX06, DX23, DX26 | Explicit later product scope; preserve typed extension and outcome laws |
| [OPT-01–OPT-10](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/docs/prd-backlog/durability-and-operations.md) | 10 | DX23 | Optional decisions remain unselected; no speculative production surface |

Failure acceptance records are retained in Bombay: `LAW-01`–`LAW-22` (22 properties), `F-01`–`F-68` (68 scenarios), and `C-01`–`C-10` (10 combined campaigns). DX25 applies the relevant local rows now; DX23 carries remote, authority, durability and deployment rows to their owning PRDs. No historical status in that file establishes a passing implementation.

## Documentation changes

This cleanup removes historical implementation campaigns, not the underlying acceptance criteria or tests. Deleted text remains available at the immutable baseline. Keep active contract documents until their semantic content is reconciled with code; changing navigation does not verify their statements.

| Behavior document or group | Disposition and surviving owner |
| --- | --- |
| [arc010-owner-contract.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/arc010-owner-contract.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-downstream-impact.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-downstream-impact.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-essence.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-essence.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-other-templates.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-other-templates.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-research-audit.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-research-audit.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-retained-core.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-retained-core.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-solution.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-solution.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [atomic-actor-type-inventory.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/atomic-actor-type-inventory.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [interpreter-contract-implementation-ledger.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/interpreter-contract-implementation-ledger.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [interpreter-implementation-ledger.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/interpreter-implementation-ledger.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [public-surface-inventory.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/public-surface-inventory.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [shutdown-authority-change-ledger.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/shutdown-authority-change-ledger.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [shutdown-authority-prd.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/shutdown-authority-prd.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [template-composition-audit.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/template-composition-audit.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [template-law-audit.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/template-law-audit.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| [worker-preparation-interleaving.md](https://github.com/devrandom-labs/bombay-behavior/blob/d69f992b371c12ab34e73b18e45b8112c90a1508/docs/engineering/worker-preparation-interleaving.md) | Delete historical record. Current source/rustdoc owns representation; aggregate laws and DX requirements retain semantic acceptance. |
| `repository-quality-audit.md` | Replace chronological evidence with the remaining downstream acceptance obligations and immutable historical reference. |
| `atomic-actor-devx.md` | Keep only a component authoring reference and links to actual source; the new PRD owns proposed application simplification. |
| `atomic-actor-verification.md` | Remove dated completion counts; retain verification requirements and source/test locations. |
| `atomic-actor-architecture.md`, `atomic-actor-features.md`, `creation-and-delivery-custody-prd.md` | Retain architecture/requirement content pending law-by-law reconciliation; remove historical links as current authority. No implementation status inherited. |
| Canonical contracts and `actor-laws/` | Retain guarantees and independent-test obligations; replace references to deleted design machinery. Future condensation requires semantic comparison, not indiscriminate deletion. |
| `DEAD_ENDS.md` | Retain falsifiers required by AGENTS; historical source names do not become current API. |
| `SUMMARY.md`, README and guide links | Remove archive from normal navigation, point users at source/API and the remaining contracts. |

Bombay replaces the 13,280-line EXEC campaign with a compact execution contract and source map, deletes its nine chronological companion files, and repairs links to immutable historical evidence where the old decision is still cited. The 270 requirement rows and all 100 failure records remain. Its backlog gains one DevX coordination entry pointing to this owning PRD rather than copying the specification.

## Preserved obligations from retired records

- A17 and the follow-on custody campaign: real released-package interpretation of initial/replacement birth, collisions/exhaustion, initialization rejection/stop, activation, rejected effect followed by independent effects, closed emitter custody and parent/root transfer. DX12, DX16, DX21, DX25 and DX27 own the ongoing evidence; the existing custody PRD retains T01–T24.
- Atomic feature groups SH, PX, FS, DS, FP, KP, API, EFFECT and VERIFY remain in `atomic-actor-features.md` and family laws. Deleting their solution/type inventory does not close or weaken any requirement.
- Preparation overlap preserves admitted source, prepared prefix, remaining roles and late rejection during shutdown. DX11 and DX16 retain the obligation.
- Exact shutdown preserves behavior-indexed authority, stale target rejection and original rejected control. DX21 retains the obligation.
- Previously rejected placeholders, inferred provenance and structural caller work remain prohibited by AGENTS and DEAD_ENDS; replacing a dated audit does not reopen them as acceptable shortcuts.
- EXEC retains caller-host execution, exact work/output and cleanup custody, opacity of native panic payloads, independent observation authority, actual result acquisition before producer destruction, and cooperative-host limits. DX04–05, DX16 and DX21 must preserve those distinctions. Its archived delivery record is historical evidence, not a fresh test certificate.

## Initial documentation validation

Executed on the documentation branches, 2026-10-08:

| Check | Result |
| --- | --- |
| Both repositories: `git diff --check` and complete changed-path inspection | Passed; Markdown only, no production/test/dependency/API edits and no untracked residue. |
| Repository Markdown targets and headings into changed documents | Passed. The two Behavior API-directory links resolve in generated output, not the source tree. |
| Immutable Git evidence destinations | 114 unique pinned repository paths exist at the recorded revisions. External research pages were inspected separately. |
| Bombay requirement preservation | All 270 requirement rows and 100 LAW/F/C rows byte-identical to main. Count corrected in the index only. |
| `python3 -m unittest scripts/test_check_published_docs.py` | Four passed. |
| `python3 scripts/check_rustdoc_imports.py` | Passed. |
| `nix develop -c python3 scripts/check_rustdoc_error_codes.py` | Passed: 62 diagnostics. An initial direct helper invocation could not find Cargo; verification was repeated in the pinned shell. |
| Pinned-shell `RUSTDOCFLAGS="-D warnings" cargo doc -p bombay-behavior -p bombay-behavior-actors --no-deps` | Passed. |
| Pinned-shell `mdbook build docs` and `python3 scripts/check_published_docs.py target/doc` with the Pages index installed | Passed after repairing the new aggregate-law link. |
| Behavior `nix flake check` | Incomplete/failed: Darwin package install aborted in code signing with `std::runtime_error: rename failed` (exit 134). Package, deny, audit, Rust formatting and TOML formatting checks reported pass. No full nextest/Clippy/doctest success is inferred. |
| Bombay `nix flake check` | Incomplete/failed: dependency build reported `No space left on device`. Formatting reported pass. No workspace runtime-test success is inferred. |

No runtime source changed. The full gates must be rerun in an environment with
sufficient space and functioning Darwin signing before claiming full repository
verification. Documentation checks do not establish the proposed API or its
future semantic acceptance.

Complete documentation diff against the inspected main revisions:

| Repository | Changed paths | Deleted documents | Added lines | Removed lines | Net lines |
| --- | --- | --- | --- | --- | --- |
| Behavior | 32 | 16 | 1129 | 12968 | -11839 |
| Bombay | 17 | 9 | 214 | 22647 | -22433 |

Counts include this PRD and evidence, retained-contract condensation, navigation
and history-link repairs. Production, tests and public API deltas are all zero.
The Git diff is the authoritative change record; the deleted material remains
recoverable at the pinned baselines.

## Tooling review

#### Selected tools

The selected approach follows the
[Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html)
on conventional interfaces, useful diagnostics, documentation and appropriate
generality. It does not use guideline names to justify a new abstraction.

Use the already selected [`thiserror` 2.0.20](https://docs.rs/thiserror/2.0.20/thiserror/)
for standard error implementations and truthful source relationships.
Use [`trybuild` 1.0.121](https://docs.rs/trybuild/1.0.121/trybuild/) for caller
syntax and diagnostic regression fixtures. Keep the existing property,
concurrency, fuzz and mutation tools in their owning test layer. Tokio continues
to own executor construction; ordinary Rust `impl Future` keeps static dispatch.
Crate upgrades require a separate compatibility reason, not a desire to shorten
the manifest. No new crate is a prerequisite for the initial application and
terminal experiments.

The requested crates.io/[Awesome Rust](https://github.com/rust-unofficial/awesome-rust)
review inspected current registry metadata and downloaded published sources.
No dependency was added. The bounded comparisons are:

| Candidate | Responsibility and disposition |
| --- | --- |
| [`frunk_core` 0.5.0](https://docs.rs/frunk_core/0.5.0/frunk_core/) | Typed heterogeneous products/sums. Substituting actual HCons/Coproduct into the descendant eligibility equation still produces E0275 in both profiles; it does not prove recursive metadata. Core already owns the closed occurrence product. |
| [`recursion` 0.5.4](https://docs.rs/recursion/0.5.4/recursion/) | Stack-safe traversal of a defined recursive data family. Traversal is distinct from proving the family's types and child interpreter eligibility; no owner machinery deletion is demonstrated. |
| [`recursion-schemes` 0.1.3](https://docs.rs/recursion-schemes/0.1.3/src/recursion_schemes/recursive.rs.html) | `Fix` stores `Box<F::Layer<Fix<F>>>`. Keeping exact current child payloads retains the eligibility cycle; replacing recursive positions by X requires a different, proven closed actor frame. It supplies no child interpreter proof. Partial expansion uses an erased callback. No adoption. |
| [`enum_dispatch` 0.3.13](https://docs.rs/enum_dispatch/0.3.13/enum_dispatch/) | Generates static delegation, but its generated TryInto rejection returns a string and drops the other variant. An independent original-worker Drop witness fails in debug and optimized builds. Rejected for custody-bearing sums. |
| [`enum-as-inner` 0.7.0](https://docs.rs/enum-as-inner/0.7.0/enum_as_inner/) | Checked consuming access returns Err(original), but the derive also generates unchecked unsafe accessors. It expands the public surface and does not select recursive custody. No adoption. |
| [`auto_enums` 0.8.10](https://docs.rs/auto_enums/0.8.10/auto_enums/) | Generates closed Future/Iterator delegation. It does not discharge child-host capability requirements; no repeated owner implementation has been shown to justify another macro dependency. |
| [`derive_more` 2.1.1](https://docs.rs/derive_more/2.1.1/derive_more/) | Derives standard conversions and checked variant extraction. The three existing u64 newtypes MailAddr, TimerId and TimerGeneration have six five-line conversion impls; these are bounded boilerplate candidates, not an assembly fix. No dependency/build-cost or deletion experiment is retained; thiserror remains the error owner. |
| [`async-recursion` 1.2.0](https://crates.io/crates/async-recursion/1.2.0) | Fresh registry/source recheck: src/expand.rs still emits `Pin<Box<dyn Future>>`; excluded by the static-dispatch law. Ordinary Box allocation alone is not excluded. |
| [`bon` 3.10.2](https://docs.rs/bon/3.10.2/bon/) | Awesome Rust lists its checked builders. Named field construction does not establish recursive child custody or interpreter eligibility; adding a required-field typestate builder would conflict with the selected owning constructors and ordinary Application spine. No adoption. |
| [`lru`](https://docs.rs/lru/latest/lru/struct.LruCache.html) / [`priority-queue`](https://github.com/garro95/priority-queue) | Catalogue source check: Cache deliberately supports Eq keys and exposes a recency slice; lru requires Hash and would change that contract. PriorityQueue already uses BinaryHeap for immutable priorities and checked FIFO ties; priority changes are not its law. Reconsider only with a measured scale or mutable-priority requirement. Neither addresses Application assembly. |

Published-source comparisons and pinned 1.99 debug/optimized falsifiers are in
/private/tmp/bombay-devx-crate-review/. These are library evaluations, not
Bombay acceptance tests. Existing thiserror, trybuild, Tokio and standard owned
sums/products remain the selected tools.

Deeper stable-language review (user requested): primary compiler/API sources
and40 executable language cases across pinned Rust1.95/1.99 and debug/optimized
profiles distinguish useful techniques from missing proof. These are feature
availability/falsifier tests, not actor acceptance or production changes.

| Technique | Executable result and responsibility |
| --- | --- |
| Nominal coinduction versus conditional native predicates | Recursive nominal custody is Send and preserves its original payload; an ordinary trait whose applicability requires itself denies E0275 on both compilers. The [compiler coinduction guide](https://rustc-dev-guide.rust-lang.org/solve/coinduction.html) explicitly separates recursive calls inside concrete bodies from cycles in where clauses. This supports the existing native-port cut; it supplies no custom execution auto trait. |
| Stable autoref selection | Concrete NoBirths selects its empty case; the same value passed through a generic function selects the generic alternative on both compilers. The [original pattern author's limitation](https://github.com/dtolnay/case-studies/blob/master/autoref-specialization/README.md#limitations) matches this witness. Concrete macro-site selection cannot supply generic recursive eligibility. |
| GAT families and precise opaque capture | A declared type family preserves affine input allocation; use<> lets a returned Send/static task avoid capturing its temporary non-Send owner on both compilers. [GAT RFC](https://rust-lang.github.io/rfcs/1598-generic_associated_types.html), [capture rules](https://doc.rust-lang.org/edition-guide/rust-2024/rpit-lifetime-capture.html). These select/loan known values rather than discover a graph or prove child interpretation. |
| Concrete boxed async recursion | A concrete recursive future passes Send/static checking without erased futures on both compilers; [stable since1.77](https://blog.rust-lang.org/2024/03/21/Rust-1.77.0/). This repairs future size, independently of trait applicability. |
| Opaque aliases/associated outputs and return-type notation | Type-alias impl Trait, opaque associated types and return-type notation each deny E0658 on1.95/1.99; root item fields spelled _ deny E0121. [TAIT](https://doc.rust-lang.org/unstable-book/language-features/type-alias-impl-trait.html), [opaque associated types](https://doc.rust-lang.org/unstable-book/language-features/impl-trait-in-assoc-type.html), [RTN](https://doc.rust-lang.org/unstable-book/language-features/return-type-notation.html). No nightly switch or visibility/bounds workaround is selected. |

The language findings are bounded availability/falsifier evidence. The retained
application design now uses the approved explicit topology and nominal recursive
roles; none of these techniques alone proves automatic graph discovery.
