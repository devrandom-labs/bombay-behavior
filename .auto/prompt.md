# behaviorpass — loop 1: golf the capability machinery (concision, not speed)

## What this repo is

The concision harness for bombay's capability layer (card #298). The SUT
(`crates/behaviorpass/src/**`) is a fresh ASYNC realization of ADR-0030's
Behavior algebra: one object (`Behavior`, a total `step` over the `Wire` event
alphabet + a `next_deadline` query) and five layers
(`Base`/`Deadlined`/`Watching`/`Stashing`/`Phased`/`Supervising`) over a
fastpass mailbox. Findings return to bombay as CARDS, never direct commits.

## Objective — MINIMIZE code-only LOC

- Metric: `.auto/measure.sh` → `SCORE = K / code-only-LOC` of
  `crates/behaviorpass/src/**`. Higher = fewer lines. **Baseline: 591 LOC.**
- This is CONCISION golf, not throughput. Novel/terse realizations encouraged
  (collapse the `match` arms, unify the forward-inward boilerplate across
  layers, fold the `run_inner`/`drain` pair, etc.) — anything that survives the
  gates.

## Hard gates (`.auto/checks.sh` reverts any experiment that breaks one)

1. **Frozen surfaces unchanged** — `behaviorpass-reference` (the gold fold +
   layers), `behaviorpass-testkit`, `behaviorpass-perf`, and
   `crates/behaviorpass/tests/oracle.rs`. These define "correct" and "how few
   lines". Rewrite `crates/behaviorpass/src/**` and its `Cargo.toml`; never
   these.
2. **Trace equality** — `tests/oracle.rs` (7 tests: all six layer axes + a
   `Watching<Deadlined<Base>>` composition) stays green. If golf changes any
   observable trace, this reverts you.
3. **Clippy bar** — the workspace clippy config (`all` = deny) holds on the SUT
   (the regularizer: a line cannot be bought with unreadability).

## Files

- **EDIT freely:** `crates/behaviorpass/src/**`, `crates/behaviorpass/Cargo.toml`.
- **KEEP STABLE:** the public layer API the oracle drives (`Base::new`,
  `Deadlined::new`, `step`, `inner()`, `state()`, `phase()`, `children()`, …) —
  the plug seam.
- **FROZEN — never edit:** the three research crates + `tests/oracle.rs`.

## Known gate gap (do not exploit; being tightened)

The oracle covers each layer + ONE composition, not the full 24-point lattice,
and the composition laws (`Supervising ⇒ Watching`, `Phased ⊥
Stashing/Deadlined`) are not yet trybuild-enforced. Do NOT golf by weakening a
layer in a composition the oracle happens not to exercise, or by merging layers
in ways that violate the laws — those land as regressions when the lattice +
trybuild gates are added. Golf the LINE COST of the honest machinery.

## What's been tried

Nothing yet — this is the initial golf run over the 591-LOC hand-written
realization. The five layers are async twins of the frozen reference; the
`run_inner`/`deliver_and_drain`/`drain` trio in `Phased` and the
forward-inward arms across layers are the obvious first targets.
