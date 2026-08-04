# behaviorpass

The **behavior algebra**: the pure fold at the heart of the pass family —
the pillar-pass sibling of `fastpass` (the mailbox) and `actorpass` (the
runtime).

A `Behavior` folds an `Envelope` into `Actions` — the messages sent, the
actors created, the replacement verdict, all **data**, no I/O, no entropy,
no clocks. Every capability (`Deadlined`, `Watching`, `Supervising`,
`Stashing`) is a `Behavior` that wraps a `Behavior`; the floor is a
[`State`] coalgebra (a state type with its transition, bound in one type)
lifted by `Base`. Judged on three criteria only: the fold stays pure, the
shape stays algebraic, and the ergonomics stay good.

> Findings return to bombay as **cards**, never direct commits.

## Layout

| Path | Role |
|---|---|
| `crates/behaviorpass/src/behavior.rs` | The core: `Address`, `Behavior`, `State`, `Base`, `FnState`, `Envelope`, `Actions`, the `run` test driver |
| `src/{deadlined,watching,supervising,stashing}.rs` | The capability wrappers (one file per capability) |
| `src/fsm.rs` | `Fsm` — a thin helper built from core (not a capability) |
| `tests/oracle.rs` | **The correctness gate**: trace-equality to the frozen reference at every lattice point |
| `tests/adv_*.rs` | Adversarial/differential suites per capability |
| `examples/p*.rs` | The 24-point lattice fixtures |
| `examples/perf_supervising.rs` | The perf ruler: space + hot-path throughput of the `Supervising` children mechanism |
| `.auto/` | The perf loop harness (frozen-surface diff gate + conformance gate + score) |

## The perf loop

Representation optimization **behind a stable API** — the loop reshapes
`Supervising`'s internals (never the `Behavior` contract) against the
frozen ruler:

```bash
cd ~/Code/devrandom/behaviorpass
bash .auto/measure.sh   # METRIC score=<n> (space fitness) + info breakdown
bash .auto/checks.sh    # CHECK OK = frozen surfaces + full suite green
```

Dual-licensed MIT OR Apache-2.0.
