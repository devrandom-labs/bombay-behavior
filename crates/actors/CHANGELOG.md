# Changelog

All notable changes to `bombay-behavior-actors` are documented here.

## [Unreleased]

### Changed

- Make `Proxy<C>` an orderly subtree owner: shutdown now emits a typed
  `ShutdownChild<C>` for the exact installed incarnation and stops only after
  its matching `ChildStopped`. Shutdown during installation waits for creation
  resolution, and rejection remains a typed `ProxyError`.
- Make every runtime callback lane part of its concrete template protocol:
  `CircuitBreaker`, `Lease`, and `Presence` now receive `TimerElapsed` through
  `TimedEvent`; `DynamicSupervisor` installs a shutdown-capable `DynamicProxy`
  and accepts `WorkerStopped`; and `ShutdownCoordinator` requires its concrete
  child protocol to accept `ShutdownRequested`.
- Preserve the concrete child behavior protocol in `ShutdownChild<C>` and in
  the homogeneous `ShutdownCoordinator<B, C>` effect lane, allowing runtimes
  to select the hosted namespace without ambient lookup or type erasure.
- Make every semantic wrapper constructor public and remove the parallel
  `Compose` extension API, including its policy-bearing `children` shortcut.
- Require supervisor restart strategy, eligibility, and budget to enter
  together through `RestartConfiguration`; remove the split policy setters.

### Added

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
  `BackoffSupervisor`, which withholds replacement commands until the exact
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
