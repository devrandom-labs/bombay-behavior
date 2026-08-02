# behaviorpass

The **concision harness** for bombay's capability machinery (bombay card
#298) — the pillar-pass sibling of `fastpass`. Where fastpass golfs a two-lane
merge for **throughput**, behaviorpass golfs the capability layer for
**concision** (code-only LOC), gated on trace-equality to a frozen essence-fold
reference (ADR-0028/0030).

> Findings return to bombay as **cards**, never direct commits. bombay is
> pinned here as a path dependency (`../bombay/crates/core`).

## Layout

| Crate | Role | Frozen? |
|---|---|---|
| `behaviorpass-reference` | The gold model: the ~50-line essence-fold + one model layer per capability (`Base`/`Deadlined`/`Watching`/`Stashing`/`Phased`/`Supervising`). The executable spec. | **frozen** |
| `behaviorpass-testkit` | The mode-blind oracle (#266 pattern): one script drives a generated SUT actor AND the reference fold; probes must match. | **frozen** |
| `behaviorpass-perf` | The metric: `SCORE = K / code-only-LOC` of the SUT. | **frozen** |
| `behaviorpass` | The golf target: the ported capability machinery + the 24-point lattice generator. | edit-freely |

## The lattice

Cap-set subsets of {Stashing, Deadlined, Phased, Watching, Supervising} under
the composition laws (Supervising ⇒ Watching; Phased ⊥ Stashing/Deadlined) =
15 valid stacks × Phased's inner seats where present = **24 legal machines**;
the **17 illegal** points are trybuild `compile_fail` cases (laws enforced, not
documented).

## Status

**Phase-0 scaffold.** The frozen reference is authored and correct; the `.auto`
contract is wired; the SUT is empty and the oracle/generator/trybuild cases are
the first work (see `.auto/prompt.md`). The concision loop runs once the oracle
is green at all 24 points.

## Running the loop

```bash
cd ~/Code/devrandom/behaviorpass
pueue add --label phase1 -- omp --profile autoresearch --auto-approve \
  --max-time 8h -p "/autoresearch $(sed -n '1,3p' .auto/prompt.md | tr '\n' ' ')"
tail -f .auto/log.jsonl
```

Metric: `.auto/measure.sh` (`METRIC score=<n>`, higher = fewer lines). Gate:
`.auto/checks.sh` (`CHECK OK` = frozen + trace-equality + compile_fail +
clippy). Dual-licensed MIT OR Apache-2.0.
