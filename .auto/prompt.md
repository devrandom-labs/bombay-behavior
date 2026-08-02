# behaviorpass — phase 1: build the harness, then golf the capability machinery

## What this repo is

The pillar-pass sibling of fastpass, for bombay card #298. It golfs bombay's
**capability machinery** for CONCISION (code-only LOC), gated on trace-equality
to a frozen essence-fold reference (ADR-0028/0030). Findings return to bombay
as CARDS, never direct commits.

- **`behaviorpass-reference` (FROZEN)** — the gold model: the ~50-line
  synchronous essence-fold (`run`) + one model layer per capability
  (`Base`/`Deadlined`/`Watching`/`Stashing`/`Phased`/`Supervising`). This is
  the executable spec; trace equality to it defines "correct".
- **`behaviorpass-testkit` (FROZEN)** — the mode-blind oracle (#266 pattern):
  one abstract script drives a generated SUT actor AND the reference fold;
  probe sequences + stop kinds must match, per axis, at every lattice point.
- **`behaviorpass-perf` (FROZEN)** — the metric: `SCORE = K / code-only-LOC`
  of the SUT.
- **`behaviorpass` (SUT, EDIT-freely)** — the golf target: the ported
  capability machinery + the 24-point lattice generator.

## Phase 1 objective (this run is NOT yet the golf loop)

The scaffold ships the frozen reference + the .auto contract. Before the golf
loop can run, the harness content must land (author it, keeping the frozen
crates' PUBLIC shape stable — they are the oracle):

1. **Testkit oracle** (`behaviorpass-testkit`): the `Probe` vocabulary + the
   per-axis suites + a `behavior_suite!($sut)` macro (mirror fastpass's
   `property_suite!`), every awaited step under a timeout.
2. **SUT lattice generator** (`crates/behaviorpass/src/**`): the ported
   capability layers + a generator emitting one minimal actor per legal point
   (15 stacks × Phased inner seats = **24 legal**), driving bombay's runtime.
3. **Illegal points** (`crates/behaviorpass/tests/compile_fail.rs`): the **17**
   illegal stacks as trybuild `compile_fail` cases (Supervising⇒Watching;
   Phased ⊥ Stashing/Deadlined). Wire the runner into `.auto/checks.sh` gate 3.
4. **Marginal-cost curve**: per-point code-only LOC / compile time / binary
   size (the monomorphization slope #295 needs before the open source-set
   select).

## Then: the golf loop

Once the oracle is green at all 24 points and the 17 illegal points fail to
compile, MINIMIZE `SCORE = K / code-only-LOC` of `crates/behaviorpass/src`
under the gates. Novelty encouraged; the only hard constraints are the gates.

## Files

- **EDIT freely:** `crates/behaviorpass/src/**`, `crates/behaviorpass/Cargo.toml`,
  and CREATE `crates/behaviorpass/tests/**`.
- **KEEP STABLE:** the public shape the testkit drives (the plug seam).
- **FROZEN — never edit:** `crates/behaviorpass-reference/**`,
  `crates/behaviorpass-testkit/**`, `crates/behaviorpass-perf/**`.

## Metric / gate

- Metric: `.auto/measure.sh` → `METRIC score=<n>` (higher = fewer lines).
- Gate: `.auto/checks.sh` → must print `CHECK OK` (frozen + trace-equality +
  compile_fail + clippy). It is vacuously green on the empty scaffold; making
  it load-bearing (authoring the oracle + generator) IS phase 1.

## What's been tried

Nothing yet — this is the initial scaffold. The frozen reference is authored
and correct; the SUT is empty; the oracle + generator + trybuild cases are the
first work.
