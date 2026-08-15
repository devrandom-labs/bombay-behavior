# Progress

## Current state

Fifteen retained loop changes (E01-E04, E06-E15). E05 was reverted by the
independent audit because unused convenience conversion APIs violate the
repository contract. Authoritative `nix flake check` passes
7/7 (build, nextest, doc, fmt, toml-fmt, audit, deny). Full verification:
283 nextest, 10 doc, clippy clean on production crates, fmt clean, diff --check
clean, 0 unsafe, deny licenses ok (0 warnings). Production panic surface: 2
provably-unreachable guarded `expect` (circuit_breaker counter guards), 0
`unwrap`/`panic!`/`unreachable!`.

## Retained improvements

- E01: `IncarnationError` ⇄ `ProxyError` duplicate authority consolidated.
- E02: `BreakerOutcome::Admitted { probe: bool }` → `Admitted` + `ProbeAdmitted`.
- E03: `RestartBudget` `unwrap_or(u32::MAX)` sentinel removed.
- E04: `IncarnationEffects` 3-Option → enum `{None, Create, Deliver, Report}`.
- E06/E06b: forgeable report/state types → private-or-derived (6 types).
- E07: `Buffer` stray doc fragment; `Supervisor::new` false allow-reason.
- E08: `mem::take` instead of `drain(..).collect()` in Machine/Stash.
- E09: circuit_breaker dead `unwrap_or` + `TimerGeneration(0)` dummy.
- E10: circuit_breaker `Exhausted` boundary tests (2).
- E11: supervisor `wrap()` intermediate `born` Vec removed.
- E12: deny.toml stale `ring` clarify + comments removed; allow list trimmed.

## Convergence status

Three fresh whole-repository audit passes (7 scouts) + manual sweeps covered all
13 audit lenses. Findings trended HIGH→LOW (duplicated authority/boolean
blindness/sentinels/forgeable types → allocation micro-wins → config hygiene).
The later adapter-ergonomics pass closed the previously deferred constructor
and type-nameability findings:

- E14 was subsequently removed: generic `B: Behavior` boundaries preserve
  inference without introducing a second authoring macro.
- E15: `ChildTopology`, `RestartConfiguration`, and `PoolConfiguration`
  replace the eight- and nine-argument supervisor/pool constructors.
- E11a — `order_gate` watermark clone: rejected (clean fix doesn't typecheck for
  generic K; verbose `Bound` form costs more than the clone).

## Completed experiment

E13 — removed the mandatory `Compose<B>` container while retaining typed
wrapper composition. Direct catalogue definitions activate through `Activate`;
`Compose` is now a wrapper-only extension trait. Normal construction, including
the two-buffer `Machine`/`Stash`/`Deadline` stack, relies on inference instead
of an explicit nested alias. The workspace, doctests, architecture review,
fuzz-target build, and authoritative Nix gate pass.

Independent audit correction: E05 reverted; generated `.serena/` local state
is ignored rather than entering the repository. Pinned-Nix format, all-target
check, and 10 documentation tests pass after the correction.

## Exact next actions

1. Review and commit the verified behavior-owned adapter ergonomics changes.
