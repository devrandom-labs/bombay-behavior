# Changelog

All notable changes to `bombay-behavior-actors` are documented here.

## [Unreleased]

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

### Changed

- Derive the actor crate's public error types with `thiserror`; wrapped fleet
  and behavior failures participate in typed source chains.
