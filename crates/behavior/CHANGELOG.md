# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.14.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.13.1...bombay-behavior-v0.14.0) - 2026-08-21

### Added

- Add runtime-owned `EndpointAddress` families and inert
  `EstablishedRecipient<P>`, `EstablishedActor<B>`, and
  `EstablishedDelivery<P>` capabilities for exact installed incarnations.
- Add protocol-and-occurrence-indexed `EstablishedCreation<P, Occurrence>`
  facts with exhaustive allocation, initialization, installation, and binding
  rejection reasons. Rejected facts carry no endpoint capability.
- Add `ChildDelivery<P, Occurrence>` for same-action local communication
  through a committed creator-instance binding.

### Changed

- Make fresh address allocation wholly interpreter-owned. `Address` now
  declares only its creator-local nonce type; a nonce is no longer convertible
  into an address.
- Index concrete birth installers by structural child position with
  `InstallBirth<Position, Child, ...>`. Duplicate occurrences remain distinct
  without making the parent behavior or role another protocol identity.
- Keep `Recipient<P>` as logical addressed intent and separate its resolver
  path from exact endpoint and creator-local child delivery.
- Require established endpoints to be cloneable but not unconditionally
  `Send`. Sendability is enforced on concrete asynchronous delivery and
  interpreter-request paths, allowing sound closure-owned local endpoints
  without imposing `P::Msg: Send` on every exact capability value.

### Removed

- Remove deterministic `Address::birth`, `ChildRecipient`, `DeliveryTarget`,
  and local-child construction on ordinary `Delivery`.
- Remove research probes and superseded architecture/audit documents now that
  the established-capability contract is part of the production API.

## [0.13.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.13.0...bombay-behavior-v0.13.1) - 2026-08-21

### Other

- add typed child topology roles and routes ([#52](https://github.com/devrandom-labs/bombay-behavior/pull/52))

### Added

- Add `ChildRoute` and generated `ActorChildrenRoutes` products so one
  nominal child-role route stages creation and addresses the same
  creator-local child without repeated nonce correlation.
- Add `ChildRole` proofs and generated `ActorChild::Role` selectors for static
  application topology builders, including sealed structural positions in the
  existing closed child sum and reusing the same nominal roles as child routes.
- Add `AppendSend` and generated `ActorActions::send_lane` methods so authored
  folds can fluently append to named send lanes while preserving creations and
  the exact next-behavior verdict.
- Add the closed `BirthProtocols` projection over the existing staged-birth
  algebra so consumers can traverse concrete behavior leaves,
  `ChildChoice` alternatives, and transitive births without reclassifying
  upstream types or inspecting send destinations.

### Changed

- Clarify and test that `Behavior::Protocol` remains the sole canonical hosting
  identity through typed delivery and birth installation. Distinct semantic
  child roles or concrete behaviors may share that protocol without creating
  role-keyed storage identities.

## [0.13.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.12.1...bombay-behavior-v0.13.0) - 2026-08-19

### Added

- *(actors)* add lifecycle topology templates ([#49](https://github.com/devrandom-labs/bombay-behavior/pull/49))

### Other

- Strengthen foundational value contract tests ([#51](https://github.com/devrandom-labs/bombay-behavior/pull/51))

### Added

- Add `EventLayer`, `InjectEvent`, `Here`, `Inside`, and `Ingress` as the
  structural, statically selected algebra for interpreter-originated inputs.
  Equal payload types at nested layers remain distinct capabilities and
  unsupported paths fail to compile.
- Add `Behavior::Protocol`, `BehaviorAddr`, and `BehaviorMessage` so public
  destination identity is orthogonal to the current behavior implementation,
  internal event algebra, and explicit effects.
- Add `MessageProtocol<A, M>` as the zero-state structural endpoint for public
  message signatures that have no nominal actor template.

- Extend the single `#[behavior]` authoring macro with nominal heterogeneous
  send products, typed lane selectors, closed `Children`/`ChildChoice` birth
  products, and capability-denying defaults.

- Add `InstallBirth` and `DispatchBirth` as the statically dispatched
  interpreter contract for closed heterogeneous creation products.
- Implement `DispatchBirth` for every concrete `Behavior`, so homogeneous
  `Births<C>` and recursive `ChildChoice` sums share the same interpreter leg.
- Add `Children` and `ChildChoice` as the ordered heterogeneous creation
  product and its closed, generic child sum, without procedural generation.
- Require `InstallBirth` and `DispatchBirth` to return `Send` futures so
  recursive runtime drivers can remain eligible for thread-safe spawning.

### Changed

- Remove the redundant `#[actor]` authoring macro; the capability-free
  `#[behavior]` form now covers that subset without a second generated path.

- Remove payload-driven `EventInput`/`RouteInput` routing. Event ownership is
  now selected by a concrete compile-time path and never discovered by
  searching nested behavior types.
- Remove recursive `SendInput` forwarding into structurally nested send
  products. `SendLayer` exposes emission only through its owned lane; products
  with several application effects must provide nominal, semantically named
  lanes instead of making callers select effects by an `Inner<Path>` nesting
  position. The recursive forwarding recorded for 0.9.2 is historical and is
  not part of the current API.
- Remove `Protocol` as a `Behavior` supertrait. `Recipient<P>` and
  `Delivery<P>` now require only the stable public protocol, while activation,
  creation, wrapping, and interpretation project it through
  `Behavior::Protocol`.

- Move reusable actors, protocols, composition, and worker pools to the new
  `bombay-behavior-actors` package, leaving this package focused on the typed
  behavior boundary and explicit transition effects.
- Make the terminal alternative of `become` a payload-free `Stopped` marker;
  lifecycle, supervision, collection, and runtime-failure provenance no longer
  appears in the foundational behavior API.

## [0.12.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.12.0...bombay-behavior-v0.12.1) - 2026-08-15

### Other

- Test foundational behavior value contracts ([#48](https://github.com/devrandom-labs/bombay-behavior/pull/48))

## [0.11.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.10.0...bombay-behavior-v0.11.0) - 2026-08-15

### Other

- Refactor behavior algebra and typed lifecycle ([#43](https://github.com/devrandom-labs/bombay-behavior/pull/43))

## [0.10.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.5...bombay-behavior-v0.10.0) - 2026-08-14

### Added

- *(behavior)* type sends by destination protocol ([#41](https://github.com/devrandom-labs/bombay-behavior/pull/41))

## [0.9.5](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.4...bombay-behavior-v0.9.5) - 2026-08-13

### Added

- add typed worker pools and nominal behaviors ([#37](https://github.com/devrandom-labs/bombay-behavior/pull/37))

### Other

- *(behavior)* restore mutation gate ([#40](https://github.com/devrandom-labs/bombay-behavior/pull/40))
- *(bombay-behavior)* release v0.9.4 ([#35](https://github.com/devrandom-labs/bombay-behavior/pull/35))

### Added

- index `Recipient` and `Delivery` by the concrete destination `Behavior`, so
  protocols sharing an address namespace and payload remain statically
  distinct and runtimes can derive topology without endpoint aliases or
  dynamic registries
- make `Handler`/`Pure` carry their complete send effects instead of deriving
  an ambiguous delivery lane from an outbound payload type

- add a pure typed bounded FIFO worker pool with stable supervised workers,
  explicit admission and interruption outcomes, exact assignment correlation,
  independent model/property tests, compile-fail coverage, and a stateful fuzz
  target
- add `ReplacementResolution` as a consumer-facing, interpreter-neutral
  projection of explicit replacement provenance and installation outcome
- add `#[behavior]` for nominal user-message behaviors while preserving normal
  inherent `&mut self` methods, generics, state, and explicit protocol types
- add `KeyedWorkerPool` with persistent stable-slot affinity, explicit
  future-only rebalancing, payload-returning refusal, model/property tests, and
  stateful fuzz coverage
- harden pool ownership with zero-worker rejection, per-affinity retirement,
  precommitted dispatch copies, captured static selectors, named response and
  assignment lanes, and adversarial panic/retirement tests
- make `#[behavior]` and `workers!` expansions resolve renamed
  `bombay-behavior` dependencies

## [0.9.4](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.3...bombay-behavior-v0.9.4) - 2026-08-12

### Other

- Expose pure nested behavior transition delegation ([#34](https://github.com/devrandom-labs/bombay-behavior/pull/34))

### Added

- add canonical exact-once nested behavior transition delegation for semantic
  wrappers, keeping top-level runtime execution distinct

## [0.9.3](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.2...bombay-behavior-v0.9.3) - 2026-08-11

### Added

- add concrete behavior function adapter ([#32](https://github.com/devrandom-labs/bombay-behavior/pull/32))

## [0.9.2](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.1...bombay-behavior-v0.9.2) - 2026-08-11

### Other

- Add recursive typed send forwarding ([#29](https://github.com/devrandom-labs/bombay-behavior/pull/29))

### Added

- add recursive typed `SendInput` forwarding through `SendEffects`, allowing
  callers to select arbitrarily nested send lanes with `Own` and `Inner<Path>`
  without positional field mutation

## [0.9.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.9.0...bombay-behavior-v0.9.1) - 2026-08-10

### Other

- Add typed peer observation cancellation ([#27](https://github.com/devrandom-labs/bombay-behavior/pull/27))

### Added

- add the pure `UnwatchPeer` cancellation request and define authoritative
  already-stopped resolution through the existing `ObservePeer` / `PeerStopped`
  protocol

### Changed

- define rejected same-action child observations as inert while preserving the
  matching typed creation rejection

## [0.9.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.8.2...bombay-behavior-v0.9.0) - 2026-08-10

### Other

- Refactor behavior core into pure typed folds ([#25](https://github.com/devrandom-labs/bombay-behavior/pull/25))

### Added

- add typed creation-result observation so a behavior can distinguish a staged
  creation request from successful installation or explicit rejection
- add explicit incarnation, supervised-fleet, restart-admission, and timer
  lifecycle domains with independently testable transitions
- add canonical constructors and lossless conversions for protocol events,
  lifecycle reports, routes, actions, and named effect products

### Changed

- replacement creation provenance now names the exact prior incarnation, and
  supervised proxies remain unroutable until installation is committed
- supervision effect lanes are named by meaning instead of exposed through
  positional `SendEffects` nesting

## [0.8.2](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.8.1...bombay-behavior-v0.8.2) - 2026-08-09

### Other

- Ground the behavior algebra in actor research ([#23](https://github.com/devrandom-labs/bombay-behavior/pull/23))

## [0.8.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.8.0...bombay-behavior-v0.8.1) - 2026-08-08

### Other

- *(bombay-behavior)* release v0.8.0 ([#21](https://github.com/devrandom-labs/bombay-behavior/pull/21))

## [0.8.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.7.0...bombay-behavior-v0.8.0) - 2026-08-08

### Other

- Add pure receive-timeout behavior ([#20](https://github.com/devrandom-labs/bombay-behavior/pull/20))

## [0.7.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.6.0...bombay-behavior-v0.7.0) - 2026-08-08

### Other

- Distill creation provenance and behavior algebra ([#18](https://github.com/devrandom-labs/bombay-behavior/pull/18))

## [0.6.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.5.1...bombay-behavior-v0.6.0) - 2026-08-07

### Other

- Add typed supervision failure reactions ([#16](https://github.com/devrandom-labs/bombay-behavior/pull/16))

### Added

- add explicit `CreationKind` provenance so interpreters can distinguish an
  ordinary fresh birth from a fresh replacement incarnation after installation
- add exhaustive supervision-failure reasons and a pure configurable reaction
  that can retire a failed slot or stop for ordinary parent observation

### Changed

- make `Behavior` return its declared `Actions` algebra directly, removing the
  unconstrained `Effect` and `Done` associated-type escape seats
- centralize interpreter-originated event and service vocabulary in a neutral
  protocol layer while retaining concrete wrapper event sums

## [0.5.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.5.0...bombay-behavior-v0.5.1) - 2026-08-07

### Other

- Add typed graceful shutdown behavior ([#13](https://github.com/devrandom-labs/bombay-behavior/pull/13))

## [0.5.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.4.0...bombay-behavior-v0.5.0) - 2026-08-07

### Other

- Preserve proxy identity across worker restarts ([#12](https://github.com/devrandom-labs/bombay-behavior/pull/12))
- release v0.3.1 ([#9](https://github.com/devrandom-labs/bombay-behavior/pull/9))

## [0.4.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.3.0...bombay-behavior-v0.4.0) - 2026-08-07

### Added

- separate local service sends ([#10](https://github.com/devrandom-labs/bombay-behavior/pull/10))

### Other

- *(bombay-behavior)* release v0.3.0 ([#8](https://github.com/devrandom-labs/bombay-behavior/pull/8))

### Changed

- separate interpreter-local service requests from ordinary address-routed
  deliveries with the `InterpreterRequests` algebra

## [0.3.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.2.1...bombay-behavior-v0.3.0) - 2026-08-07

### Added

- classify runtime terminal outcomes ([#7](https://github.com/devrandom-labs/bombay-behavior/pull/7))

### Added

- classify fatal effect-interpretation failures and executor cancellation as
  distinct abnormal actor outcomes

### Changed

- define that interpreters install one transition's fresh creations before
  interpreting that transition's sends

## [0.2.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-v0.1.0...bombay-behavior-v0.2.0) - 2026-08-07

### Fixed

- distinguish nested timer generations ([#3](https://github.com/devrandom-labs/bombay-behavior/pull/3))

### Other

- release v0.1.0 ([#1](https://github.com/devrandom-labs/bombay-behavior/pull/1))

## [0.1.0](https://github.com/devrandom-labs/bombay-behavior/releases/tag/bombay-behavior-v0.1.0) - 2026-08-07

### Other

- transform project into bombay-behavior
