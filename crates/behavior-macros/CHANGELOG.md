# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Resolve an unrenamed `bombay-rs` façade dependency through its actual
  `bombay` library target while preserving explicit dependency renames.

### Added

- Generate nominal child routes for every named `births` declaration.
- Generate inhabited `ActorChild::Role` selectors and exact parent/child role
  proofs, including their structural closed-sum positions, from the same named
  `births` declaration.
- Generate one actor-specific actions extension trait whose `send_lane`
  methods reuse Behavior's `AppendSend` capability and existing nominal lane
  proofs.

## [0.11.2](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.11.1...bombay-behavior-macros-v0.11.2) - 2026-08-19

### Added

- *(actors)* add lifecycle topology templates ([#49](https://github.com/devrandom-labs/bombay-behavior/pull/49))

### Added

- Extend the single `#[behavior]` attribute with named `sends = { ... }` and
  `births = { ... }` declarations. It generates nominal send products, unique
  typed lane selectors, structural interpretation, and closed `ChildChoice`
  products while leaving exact `Actions` construction in authored methods.
- Generate the actor template's nominal `Protocol` and bind
  `Behavior::Protocol = Self`, keeping public identity distinct from generated
  event/effect wiring.

- Resolve generated paths through either a direct `bombay-behavior` dependency
  or the `bombay-rs` façade, including Cargo dependency renames; a direct
  dependency takes precedence when both are present.

### Removed

- Remove the narrower `#[actor]` attribute. Capability-free behaviors use the
  safe `#[behavior(addr = ..., message = ...)]` defaults instead.
- Remove the unreleased `#[births]` attribute. The foundational `Children`
  product and generic `ChildChoice` dispatch provide heterogeneous creation
  without per-application procedural generation.
- Remove `workers!` and the unreleased `#[behavior_stack]` experiment. The
  remaining attributes generate only nominal behavior wiring.

## [0.11.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.11.0...bombay-behavior-macros-v0.11.1) - 2026-08-15

### Other

- Split and complete the reusable behavior actor catalogue ([#45](https://github.com/devrandom-labs/bombay-behavior/pull/45))

## [0.11.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.10.0...bombay-behavior-macros-v0.11.0) - 2026-08-15

### Other

- Refactor behavior algebra and typed lifecycle ([#43](https://github.com/devrandom-labs/bombay-behavior/pull/43))

## [0.9.5](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.9.4...bombay-behavior-macros-v0.9.5) - 2026-08-13

### Added

- add typed worker pools and nominal behaviors ([#37](https://github.com/devrandom-labs/bombay-behavior/pull/37))

## [0.9.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.8.2...bombay-behavior-macros-v0.9.0) - 2026-08-10

### Other

- Refactor behavior core into pure typed folds ([#25](https://github.com/devrandom-labs/bombay-behavior/pull/25))

## [0.7.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.6.0...bombay-behavior-macros-v0.7.0) - 2026-08-08

### Other

- Distill creation provenance and behavior algebra ([#18](https://github.com/devrandom-labs/bombay-behavior/pull/18))

## [0.2.1](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.2.0...bombay-behavior-macros-v0.2.1) - 2026-08-07

### Other

- release v0.2.0 ([#5](https://github.com/devrandom-labs/bombay-behavior/pull/5))

## [0.2.0](https://github.com/devrandom-labs/bombay-behavior/compare/bombay-behavior-macros-v0.1.0...bombay-behavior-macros-v0.2.0) - 2026-08-07

### Other

- release v0.2.0 ([#4](https://github.com/devrandom-labs/bombay-behavior/pull/4))
