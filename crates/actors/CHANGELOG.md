# Changelog

All notable changes to `bombay-behavior-actors` are documented here.

## [Unreleased]

### Changed

- Extract fixed stable-proxy topology, installation, restart, failure, and
  shutdown-drain transitions into one `FixedFleetOwnership` domain fold used
  by composed supervision, standalone supervision, `WorkerPool`, and
  `KeyedWorkerPool`. Remove the inert internal `PoolKernel` behavior and its
  fabricated `Births<C>` capability witness.
- Rename the application-behavior composition to `BackoffSupervise<B, C>` and
  reserve `BackoffSupervisor<A, C>` for the standalone fixed-fleet template.
  Both forms share the same checked attempt, timer-generation, pending-batch,
  collision, cancellation, and stale-timer state machine.
- Make `Supervise`, `BackoffSupervise`, `DynamicSupervisor`, `WorkerPool`, and
  `KeyedWorkerPool` orderly subtree owners. Typed shutdown now drains every
  owned stable proxy, waits through proxy-installation races, stops only after
  the final matching `ChildStopped`, and reports child shutdown rejection as a
  typed error. Delayed supervision cancels pending restart batches; both pool
  forms return every accepted queued or assigned job with the distinct
  `PoolShutdown` interruption before draining their proxies.
- Give `BackoffSupervise` a concrete event coproduct dual to its layered send
  product: timer and coordinated-shutdown inputs are direct, supervisor return
  facts retain the inner supervisor path, and wrapped behavior inputs remain
  one path deeper. It can therefore be named as a coordinated shutdown child
  without reindexing its timer or supervision capabilities.
- Give stable proxies an explicit two-lane `ProxyParentIngress` acquaintance.
  `WorkerStopped` and `WorkerCreationResolved` reports now retain the exact
  parent event path chosen when the proxy is created; wrapped dynamic
  supervisors can therefore select `Inside<Here>` without runtime parent or
  payload-lane discovery.
- Replace template-specific event forwarding lists with structural ingress.
  Timer, watch, lifecycle, supervision, creation, parent-report, and shutdown
  requests now carry their exact relative fact destination; stale facts are
  inert at the selected owner instead of falling through by payload type.
- Make shutdown wrappers own the lane they add, so compositions such as
  `StopOnShutdown<DynamicSupervisor<...>>` require no fabricated shutdown
  variant in the inner actor. Guardian construction now selects either direct
  root stop or coordinated inner shutdown as an explicit policy.
- Make `Proxy<C>` an orderly subtree owner: shutdown now emits a typed
  `ShutdownChild<C>` for the exact installed incarnation and stops only after
  its matching `ChildStopped`. Shutdown during installation waits for creation
  resolution, and rejection remains a typed `ProxyError`.
- Make every runtime callback lane part of its concrete behavior event algebra:
  `CircuitBreaker`, `Lease`, and `Presence` now receive `TimerElapsed` through
  `TimedEvent`; `DynamicSupervisor` installs a shutdown-capable `DynamicProxy`
  and accepts `WorkerStopped`; and `ShutdownCoordinator` requires its concrete
  child protocol to accept `ShutdownRequested`.
- Preserve the concrete child's declared public protocol in `ShutdownChild<C>` and in
  the homogeneous `ShutdownCoordinator<B, C>` effect lane, allowing runtimes
  to select the hosted namespace without ambient lookup or type erasure.
- Make every semantic wrapper constructor public and remove the parallel
  `Compose` extension API, including its policy-bearing `children` shortcut.
- Require supervisor restart strategy, eligibility, and budget to enter
  together through `RestartConfiguration`; remove the split policy setters.

### Added

- Add `Supervisor<A, C>` and `BackoffSupervisor<A, C>` as nominal,
  standalone actor templates that own their proxy creation capability without
  requiring an unrelated inner behavior. `TopologyFailurePolicy`
  selects the exhaustive retire-or-stop reaction to an unpreservable topology.
- Add `WorkerPoolWithParent`, backed by the shared fixed-fleet ownership fold, so every stable
  pool proxy reports worker termination and creation resolution through one
  caller-supplied `ProxyParentIngress` path.
- Add arbitrary heterogeneous coordinated shutdown through the closed recursive
  `ShutdownChoice<C, Tail>` sum. Each root names its complete direct-child
  topology, and interpretation preserves phase declaration order while
  dispatching every `ShutdownChild<C>` request statically.

- Separate stable public `Protocol` identity from `Behavior` state/fold,
  internal event sums, and effect products. Transparent wrappers preserve the
  inner protocol and cannot become alternative recipient identities.
- Add concrete `WorkerPoolProtocol` and `KeyedWorkerPoolProtocol` products for
  recursive assignment/completion seams without exposing pool state or worker
  topology.

- Extract Bombay's reusable actors, protocols, and composition API from
  `bombay-behavior`.
- Own lifecycle outcomes, crash classifications, restart denials, and
  supervision failures above the foundational behavior algebra.
- Publish exact supervision failures through the named `failure_reports` lane
  before a supervisor's payload-free terminal `become` is interpreted.
- Add the modular reusable catalogue: lifecycle tasks; deterministic routing,
  queueing, correlation, acknowledgement, ordering, retention, circuit and
  rate policies; discovery, pub/sub and presence; generation-safe timers and
  leases; bounded cache policy; dependency workflows and coordination; and
  typed health, readiness and configuration boundaries.
- Add `ChildTopology`, `RestartConfiguration`, and `PoolConfiguration` so
  supervisor and pool construction uses named semantic products.
- Add `Guardian<B>` as the application or subtree lifecycle boundary that
  preserves bootstrap effects and adds normal shutdown without supervision
  policy.
- Add `TerminationMonitor<B>` for consuming one exact peer-terminal fact into
  complete behavior actions without runtime-owned cleanup or publication
  policy.
- Add validated phased and dependency-ordered `ShutdownCoordinator` /
  `TreeShutdown` folds with explicit child-shutdown rejection.
- Add checked constant, linear, and exponential `Backoff` plus
  `BackoffSupervise`, which withholds replacement commands until the exact
  scheduled timer generation is observed.
- Add `DynamicSupervisor` with typed start, stop, replace, and query commands;
  command acceptance remains distinct from committed creation, replacement,
  and termination facts.
- Add `Link` as the honest named specialization of `Watch`; reciprocal linking
  is two statically typed endpoint compositions, not a hidden runtime table.

### Changed

- Allow application roots and wrapped behaviors to use the foundational
  `Children` heterogeneous creation product without turning child protocols
  into a forwarding behavior enum.

- Derive the actor crate's public error types with `thiserror`; wrapped fleet
  and behavior failures participate in typed source chains.
- Change supervisor and pool constructors to accept named topology and
  configuration products instead of long positional argument lists.
- Remove the `workers!` and `#[behavior_stack]` convenience macros. The sole
  behavior authoring macro is `#[behavior]`; wrapper stacks use inference and
  heterogeneous fleets use explicit exhaustive sums.
