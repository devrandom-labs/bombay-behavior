# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Add `#[births]` for closed creation-only heterogeneous child sums with
  exhaustive, statically bounded installation dispatch.
- Resolve generated paths through either a direct `bombay-behavior` dependency
  or the `bombay-rs` façade, including Cargo dependency renames; a direct
  dependency takes precedence when both are present.

### Removed

- Remove `workers!` and the unreleased `#[behavior_stack]` experiment. The
  remaining attributes generate nominal behavior wiring or creation-only
  installation dispatch, never a hidden forwarding protocol.

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
