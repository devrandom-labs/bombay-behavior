# Progress

## Current state

Twelve retained loop changes (E01-E04, E06-E12). E05 was reverted by the
independent audit because unused convenience conversion APIs violate the
repository contract. Authoritative `nix flake check` passes
7/7 (build, nextest, doc, fmt, toml-fmt, audit, deny). Full verification:
282 nextest, 10 doc, clippy clean on scoped crates, fmt clean, diff --check
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
Final pass found no remaining credible high-value hypothesis. The two surviving
items are recorded, not silently dropped:

- Lens 2 — positional constructors (`Supervisor::new` 8, `WorkerPool::new` 8,
  `KeyedWorkerPool::new` 9): ergonomic, `#[allow]`ed, deferred pending operator
  decision on churn vs. value (would be a `PoolConfig` product-type change).
- E11a — `order_gate` watermark clone: rejected (clean fix doesn't typecheck for
  generic K; verbose `Bound` form costs more than the clone).

## Active experiment

None — practical fixed point reached for high-value findings. The loop remains
open per operator directive; any fresh audit can reopen it.

Independent audit correction: E05 reverted; generated `.serena/` local state
is ignored rather than entering the repository. Pinned-Nix format, all-target
check, and 10 documentation tests pass after the correction.

## Exact next actions

1. Operator decision on Lens 2 (positional constructors) if the churn is wanted.
2. Otherwise continue re-auditing on demand; ledger is current and resumable.
