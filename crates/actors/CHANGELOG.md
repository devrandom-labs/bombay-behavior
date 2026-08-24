# Changelog

All notable changes to `bombay-behavior-actors` are documented here.

## [Unreleased]

### Added

- Add exact-capability monitoring to `TerminationMonitor`, and a statically
  selected logical or established destination for `MessageAdapter`.
- Add explicit parent-path forms for delayed supervisors and keyed worker
  pools so proxy reports remain correctly indexed through outer wrappers.
- Add `EstablishedChild` and `established_child`, preserving an exact
  installed-actor capability together with its occurrence-aware creator-local
  route so committed children can enter typed heterogeneous shutdown plans.
- Add typed proxy-command unavailability returns, generic operations for the
  existing child-shutdown builder, and owner-authored transitive logical-host
  metadata.

### Changed

- Preserve structural child occurrence in shutdown, observation, creation
  observation, termination propagation, and homogeneous or heterogeneous
  coordinated shutdown effects.
- Prove that root shutdown reaches `FinalizeOnShutdown`, and document the
  complete catalogue audit of logical, exact, and creator-local routing
  choices.
- Make circuit-breaker attempt exhaustion a panic-free typed transition.
- Make every standalone proxy and pool topology owner expose
  `BehaviorBase<Base = Self>`, and reject creator-local `MessageAdapter`
  destinations whose `NoBirths` algebra cannot resolve a child binding.
- Preserve the circuit breaker's successful and failed completion alternatives
  as a private sum instead of collapsing them to a boolean helper argument.
- Give `Watch`, fixed supervision, both worker-pool forms, and both shutdown
  coordinators direct folds for their distinct public transition laws.
- Join initial dynamic-supervisor proxy and worker creation facts in either
  arrival order, and keep pool assignments recoverable when worker-stop and
  proxy-unavailability facts race.
- Make `Router` a single-recipient ownership transfer so round-robin,
  least-loaded, consistent-hash, and rendezvous routing accept non-`Clone`
  commands such as `ProxyCommand`.

### Removed

- Remove guardian and established-watch aliases, feature aliases,
  selector-policy and backoff supervisor wrappers, and forwarding recipe
  functions that did not own distinct transition laws. Applications now use
  the retained concrete actors and ordinary typed composition.
- Remove the `Broadcast` router policy. `Topic` and `PubSub` retain fan-out as
  their distinct, explicitly clone-requiring membership-snapshot law.

## [0.14.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-actors-v0.13.1...bombay-behavior-actors-v0.14.0) - 2026-08-21

### Added

- Add `ObserveEstablishedCreation`, `ObserveEstablished`, and
  `CancelObservation` with explicit relationship IDs and complete
  started/cancelled/rejected/stopped fact variants.
- Add `ShutdownEstablished` and `EstablishedShutdownResolved`, preserving the
  exact concrete behavior and typed shutdown ingress without an ambient
  lifecycle side channel.

### Changed

- Preserve generated child-role resolution through topology-transparent
  lifecycle, timing, stash, and observation wrapper compositions, while
  topology-changing supervision exposes its own proxy child position.
- Route built-in same-action proxy and worker communication through
  occurrence-indexed `ChildDelivery` rather than nonce-derived logical
  addresses.
- Reuse the foundational `CreationRejection` domain and remove false parent or
  wrapper `Behavior` bounds from creation facts, pool send products, and
  replacement bookkeeping.
- Retain legacy address-based observation and lifecycle requests as distinct
  logical-name operations; they are not aliases for exact endpoint
  capabilities.

## [0.13.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-actors-v0.13.0...bombay-behavior-actors-v0.13.1) - 2026-08-21

### Other

- add typed child topology roles and routes ([#52](https://github.com/devrandom-labs/bombay-behavior/pull/52))

### Changed

- Route stable-proxy and worker-incarnation creation, delivery, observation,
  and shutdown through the same typed `ChildRoute` correlation source. Existing
  nonce constructors remain available for independently authored lifecycle
  requests.
- Lower generated named child roles and routes through their sealed structural
  positions into the existing `ShutdownChoice` sum. Plan validation,
  coordinator phase transitions, and terminal provenance remain owned by their
  existing lifecycle components.
- Expose `InstallationRequirements` as the closed ordered product of a
  behavior's canonical protocol and every transitive staged-birth protocol.
  Structural membership distinguishes repeated occurrences; external-only
  delivery destinations are excluded.

## [0.13.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-actors-v0.12.1...bombay-behavior-actors-v0.13.0) - 2026-08-19

### Added

- *(actors)* add lifecycle topology templates ([#49](https://github.com/devrandom-labs/bombay-behavior/pull/49))

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
