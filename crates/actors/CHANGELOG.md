# Changelog

All notable changes to `bombay-behavior-actors` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Publish a crate-specific README and both license texts with the package.
- Use the shared `TimedEvent` event algebra for every timer wrapper and one
  shared `TimedReaction` callback contract for ReceiveTimeout, Periodic, and
  OneShot.

### Removed

- Remove the actors-crate glob reexport of the complete foundational behavior
  algebra. Depend on `bombay-behavior` for core types and macros, and on
  `bombay-behavior-actors` for catalogue actors and wrappers.
- Remove the four type-identical timer event aliases and the three
  type-identical complete-action reaction aliases. `DeadlineReaction` remains
  separate because its result is only the next-behavior verdict.

## [0.14.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-actors-v0.13.1...bombay-behavior-actors-v0.14.0) - 2026-08-21

### Added

- Add the source-selected fixed-supervisor preparation diagnostic protocol:
  `WorkerPreparationFailure` retains the complete owned failure while
  `WorkerPreparationFailureReason` exposes one exhaustive borrowed cause. Roles
  remain borrowed, the source value stays with recovery, and
  `FixedDiagnostic<Role, Worker, Plan, Source>` requires no structural alias.
- Add exact FixedSupervisor unavailable-command admission. One owned
  `WorkerUnavailable` product preserves role, original sender, proxy phase, and
  command for configured lifecycle publication or operational diagnostics;
  absent and foreign proxy reports return complete.

- Add exact FixedSupervisor restart-schedule failure diagnostics. A rejected
  relative timer request leaves recovery immediately, retains its request and
  rejection reason, and applies terminal, member-retirement, or
  supervisor-shutdown policy without a Bombay-specific branch.
- Give absolute and relative timer scheduling generic total-action contracts.
  Acceptance returns the exact timer identity and generation; rejection retains
  the complete request and distinguishes relative deadline overflow from the
  two timer-queue exhaustion cases.
- Retain every late FixedSupervisor worker-preparation result through shutdown.
  Its exact private ticket joins independently with the existing StableProxy
  drain, both arrival orders converge, and the stopped supervisor preserves the
  complete source result for Bombay's generic retirement transfer.
- Add exact successful FixedSupervisor worker-preparation admission. A private
  ticket and immutable role name correlate the result, the sole source returns
  to recovery policy ownership, and the complete prepared submission remains
  owned through shutdown without issuing replacement before budget/release.
- Add the first FixedSupervisor one-for-one recovery handoff. An eligible exact
  worker stop becomes one roster recovery owner and emits one affine worker
  preparation request; shutdown drains the proxy without retiring while that
  source action remains outstanding.
- Add exact fixed-supervisor worker-stop admission for ineligible recovery.
  The empty member retains its live StableProxy and complete prior worker
  ownership, and shutdown reuses the existing exact proxy-stop join.
- Add exact sequential fixed-supervisor worker preparation. The doc-hidden
  runtime progress value exposes one immutable `&Role` at a time, derives every
  returned role name from the emitted request, and cannot complete before each
  selected role owns one `WorkerSubmission`. A private non-reused ticket follows
  every request, progress value, accepted result, and returned action.
- Add the clean-room `StableProxy` initial-worker path from fresh creation
  through initialization, started activation, readiness, exact service
  forwarding, and complete pre-ready worker return. Its owner outcomes retain
  every rejected plan, request, settlement, shutdown result, and worker stop.
- Add affine child-route selection requests. Bombay selects an unused nonce
  from the current creator namespace before `StableProxy` stages creation;
  applications no longer need an integer-convertible nonce.
- Add clean-room `StableProxy` replacement. The proxy retains the complete
  successor, closes service admission, waits for Bombay route selection,
  returns the exact predecessor before fresh successor creation, and joins
  predecessor stop, shutdown settlement, and successor result without losing
  late values.
- Add exact-capability monitoring to `TerminationMonitor`, and a statically
  selected logical or established destination for `MessageAdapter`.
- Add structural parent reports and private child inputs so supervision and
  pool facts remain correctly typed through outer behavior composition.
- Add `EstablishedChild` and `established_child`, preserving an exact
  installed-actor capability together with its occurrence-aware creator-local
  route so committed children can enter typed heterogeneous shutdown plans.
- Add typed proxy-command unavailability returns and generic operations for
  the existing child-shutdown builder.
- Add `ObserveEstablishedCreation`, `ObserveEstablished`, and
  `CancelObservation` with explicit relationship IDs and complete
  started/cancelled/rejected/stopped fact variants.
- Add `ShutdownEstablished` and `EstablishedShutdownResolved`, preserving the
  exact concrete behavior and typed shutdown ingress without an ambient
  lifecycle side channel.

### Changed

- StableProxy now has one private exact worker-departure join shared by
  pre-readiness return and predecessor replacement; the duplicate internal
  state machines were removed without changing the public API.
- Replace DynamicSupervisor's start-only `StartInterrupted` lifecycle case
  with `WorkerChangeInterrupted` and a closed `WorkerChangeInterruption` cause.
  Explicit Stop retains both operation correlations; the same value can
  truthfully represent global shutdown of an accepted start or replacement.
  No compatibility spelling remains.

- Remove the rejected-recovery lifecycle alternative. Recovery denial has one
  diagnostic owner; lifecycle continues to publish ineligible and admitted
  worker stops.

- Give every fixed-supervisor member one immutable private roster position and
  store admitted recovery as one positioned participant collection. The former
  trigger/peer sum and three-part admitted traversal are removed; construction,
  recovery actions, diagnostics, and shutdown order are unchanged.
- Give FixedSupervisor one concrete worker-preparation result input and one
  named `worker_preparations` action lane without making proxy lifecycle state
  depend on the recovery source. Rename its controlled behavior error to
  `FixedSupervisorError`; the obsolete initialization-only spelling is removed.
- FixedSupervisor proxy-operation results now use the existing same-actor
  `Here` source selection instead of recursively naming the aggregate event;
  exact settlement ownership and application syntax are unchanged.

- Put every concrete `StableProxy` action lane on the generic total
  interpretation contract. Readiness opens service only after matching
  activation start, and exact shutdown resolution joins worker stop in either
  arrival order before an owner result is published.
- Keep worker-attempt exhaustion distinct from Bombay child-namespace
  exhaustion, returning the complete worker submission in either case.
- Put `ShutdownEstablished` on the generic total action settlement: acceptance
  leaves `ShutdownId`, both exact rejections return the complete request, and
  the concrete endpoint port no longer selects an arbitrary output type.
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
- Require `RestartTiming` in both restart configuration constructors so
  immediate and delayed replacement are always explicit policy choices.
- Make `Router` a single-recipient ownership transfer so round-robin,
  least-loaded, consistent-hash, and rendezvous routing accept non-`Clone`
  domain commands routed through a proxy-preserved protocol.
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

### Removed

- Remove the former proxy, fixed-supervisor, dynamic-supervisor, FIFO-pool, and
  keyed-pool implementations, their atomic-only wrapper stacks, structural
  send projections, compatibility aliases, and alternate construction paths.
  Replacement families are added only in clean-room vertical slices.
- Remove guardian and established-watch aliases, feature aliases,
  selector-policy and backoff supervisor wrappers, and forwarding recipe
  functions that did not own distinct transition laws. Applications now use
  the retained concrete actors and ordinary typed composition.
- Remove the `Broadcast` router policy. `Topic` and `PubSub` retain fan-out as
  their distinct, explicitly clone-requiring membership-snapshot law.
- Remove the duplicate delayed restart constructors and the crate-root relay
  re-exports; the relay feature remains public in `actors::composition`.

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
- Expose Behavior Core's `BirthProtocols` as the closed ordered product of a
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
