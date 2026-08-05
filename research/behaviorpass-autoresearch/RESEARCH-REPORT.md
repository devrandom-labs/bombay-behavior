# behaviorpass adversarial autoresearch — session report

Session: `autoresearch/run-the-behaviorpass-adversarial-autoresearch-lo-20260805`
Baseline: `086328746bba` (harness snapshot) — production frozen throughout.
Scope: `research/behaviorpass-autoresearch/**` only. No production code touched.

## Summary

The public behaviorpass algebra survived every attack lane. 63 tests across
deterministic/exhaustive, model-based property, fuzz, and performance
workloads plus six coverage-guided fuzz targets (6.2M+ executions); **no
invariant violation was found** — but the campaign produced one
scaffold-test correction and a set of deterministic semantic observations
(below) that pin the algebra's actual behavior.

## Tests added (research crate)

| File | Count | Lane |
|---|---|---|
| `tests/core.rs` | 10 | deterministic (1 scaffold test corrected, see Finding 1) |
| `tests/boundaries.rs` | 14 | deterministic boundary/edge (incl. Spec defaults, inherent builders) |
| `tests/compositions.rs` | 8 | deep wrapper permutations + routing |
| `tests/cross_lane.rs` | 5 | supervised-stack lane isolation + full-stack randomized property |
| `tests/properties.rs` | 4 | model-based properties (scaffold) |
| `tests/fsm_properties.rs` | 4 | FSM no-drop/no-dup (2- and 3-phase) + exhaustive + Stop mid-drain |
| `tests/supervision_model.rs` | 4 | model properties (incl. births × window × budget) + boundedness |
| `tests/exhaustive.rs` | 1 | exhaustive small-state enumeration |
| `tests/stash_properties.rs` | 3 | filter-model property + exhaustive + driver |
| `tests/driver_accumulation.rs` | 5 | driver SendProduct accumulation + monoid law + full-stack drive |
| `tests/workers_fleet.rs` | 5 | `workers!` dispatch + 3-kind boundaries + RestForOne/OneForAll fleets |
| `src/model.rs` | — | shared independent supervision reference model |
| `benches/protocol_matrix.rs` | — | supervise/fsm/stash/nested workloads |
| `fuzz/fuzz_targets/` | 6 | protocol, supervision, fsm, birth, stash, stack sequences |

## Explored combinations

- **Supervision**: strategy × policy × budget × window × timestamp ordering
  (equal, duplicate, backwards) × fleet size; static fleets, dynamic births
  interleaved with deaths, birth-sequence (RestForOne) ordering, duplicate
  death redelivery, duplicate configured nonces, empty fleets, unknown
  nonces, window-edge inclusivity, budget recovery after stamp aging,
  future-stamp survival.
- **Exhaustive**: every sequence of ≤ 3 child-stopped events over a 2-slot
  fleet (alphabet 16 → 4,369 sequences) × 3 strategies × 3 policies × 3
  budgets × 2 windows = **236k model-vs-impl comparisons**; every stash
  sequence of ≤ 4 messages over Release/Deliver/Stash classes (121
  sequences) vs the filter model; every FSM sequence of ≤ 4 messages over
  four residue classes (341 sequences) under the no-drop/no-dup invariant.
- **Properties**: 256-case supervision model, 256-case mixed birth+death
  model **with a finite window** (births × window × budget — the only
  coverage of that cross product), 256-case stash filter model, 512-case
  FSM no-drop/no-dup (2-phase) + 256-case 3-phase, 256-case full-stack
  random lane-routing, 512-case scaffold properties.
- **Composition**: all 6 orderings of {at, watch, at} init-protocol nesting;
  stash adds no init sends; environment lanes (Reached/PeerStopped) bypass a
  stash buffer while user messages are intercepted; supervision over stash,
  supervision over at, and the full four-layer stack (supervision ∘ at ∘
  watch ∘ stash) — user/child/time lanes never leak into each other
  (observable via the parent's echo lane and product-lane sends); the
  randomized full-stack property interleaves all four lanes; watch-of-watch
  peer routing; watch reaction re-invocation after Stop; nested At schedule
  collisions; FSM mid-drain reordering and mid-drain Stop.
- **Driver-level**: lossless `SendProduct` accumulation across a driven
  supervised trace (echo, replacement, observe lanes each keep their own
  order), the `SendAlgebra` monoid law (identity + associativity at `Vec`
  and `SendProduct` levels), `FnState`/`Base::from_fn` folding, and the
  full four-layer stack driven through the mailbox with a peer-death
  `Stop` leaving the tail unconsumed.
- **Fuzz × model**: supervision budget/window reference model, supervision
  birth+death slot-table model, FSM and stash black-box no-drop/no-dup
  reconciliation, and a capstone four-layer-stack target with per-lane
  models — all asserted per byte inside the targets.

## Discovered counterexamples / semantic observations

None are contract violations; all are deterministic behaviors pinned by
tests, several are surprising enough to matter to algebra consumers:

1. **`Stashing::Release` never replays held messages under the pure route
   API.** The drain re-routes each held message through the route fn, which
   is a pure function of the message, so a stashed message re-routes to
   `Stash` and stays held forever; the drain's `Deliver|Release` arm and its
   Stop-mid-drain path are unreachable. The frozen crate test
   `stashing_is_local_state_and_replay` pins this. The scaffold test
   `stash_preserves_origins_order_and_multiplicity` asserted the opposite
   (replay) and was corrected to assert the real invariants: no loss, no
   duplication, FIFO order and origins preserved across any number of
   releases. A 256-case filter-model property + 121-case exhaustive
   enumeration + 301k fuzz executions confirm delivered == route-admitted
   lane exactly.
2. **Duplicate `ChildStopped` redelivery triggers a second replacement.**
   Supervision does not deduplicate death notices; a redelivered death
   consumes budget and emits a fresh replacement send.
3. **Duplicate configured nonces are accepted.** `children_with_nonces`
   does not validate configured fleet nonces (two slots can share one
   route); the fresh-nonce guard exists only for dynamic births.
4. **Nested `At` schedules with identical (id, at) collapse onto the outer
   layer.** The inner schedule fires only on a second (duplicate) delivery.
   Deterministic, no loss, but the two schedules are not independently
   observable.
5. **FSM mid-drain deferral reorders relative to FIFO.** A message deferred
   during a drain, then replayed after a phase change inside the same drain,
   is processed after messages held behind it. Deterministic, lossless,
   no duplication (512-case + 341-case + 467k fuzz executions).
6. **Restart-window pruning keeps future stamps** (a death stamped earlier
   than a prior restart's stamp underflows `checked_duration_since` and is
   kept), inflating the budget count.
7. **OneForAll excludes previously-denied slots** from its candidate set and
   never resurrects them; the budget counts every replacement.
8. **RestForOne is birth-sequence ordered, not index ordered**: dynamic
   births (sequence ≥ fleet size) are replaced only alongside later-born
   slots.
9. **Transient policy treats `Exit::LinkDied` as abnormal** (restart
   eligible) and `Normal`/`Collected` as terminal.
10. **Panic guards pinned**: proxy double-init, `workers!` out-of-range
    build, unknown-nonce `ChildStopped`. Empty fleets initialize and step
    cleanly.
11. **Watch does not latch**: re-stepping after `Stop` re-invokes the
    reaction on each matching death; the fold continues to process user
    messages after stopping.
12. **`Spec::children` defaults to Transient policy with a budget of one
    restart per 5-second window.** A second abnormal death inside the
    window is denied even under a restart strategy that would otherwise
    replace; `when(RestartPolicy::Permanent)` / `within(u32::MAX, …)` must
    be configured explicitly for unbounded restarts. (Pinned by a
    deterministic test; the full-stack property initially failed on this.)
13. **`workers![(0, Kind, build)]` is a hard compile error.** The macro
    emits a `start..end` range arm and Rust rejects empty range patterns
    (`0..0`) with E0579; zero-count kinds are rejected by the compiler with
    an obscure message rather than a clear macro diagnostic. Three-kind
    fleets route each slot to exactly its declared variant.
14. **Full-stack randomized property + capstone fuzz target**: random
    interleavings of user/peer/time/child events through
    supervision ∘ at ∘ watch ∘ stash keep every effect in its own product
    lane (624k fuzz executions, no leakage).

## Performance observations (`benches/protocol_matrix.rs`, M1 Pro, release)

| Workload | rate |
|---|---|
| base (Base step) | ~390 M t/s |
| proxy (Forward/Replace mix) | ~46 M t/s (**score**) |
| supervise fleet 8 | ~19 M t/s |
| supervise fleet 256 | ~9 M t/s |
| FSM (alternating phases) | ~290 M t/s |
| stash passthrough | ~102 M t/s |
| nested At∘Watch∘Stash | ~51 M t/s |

- Supervision throughput is **sublinear in fleet size** at 8→256 slots
  (~2× slower for 32× slots): the `position()` scan is cache/memory-bound at
  these sizes, not element-count-bound.
- **Memory growth is strictly proportional to emitted algebra**: with
  `window = Duration::MAX` the `restarts` stamp vector grows one entry per
  emitted replacement (100k events → 100k stamps, no pruning, unbounded
  budget); with a finite window, pruning bounds it.

## Fuzzing

`cargo-fuzz 0.13.2`, pinned stable toolchain with `RUSTC_BOOTSTRAP=1`
(cargo-fuzz's `-Zsanitizer=address` requires it). Five targets:

| Target | Runs | Duration | exec/s | Corpus (files) |
|---|---|---|---|---|
| `protocol_sequences` | 2,857,694 | 91 s | ~31k | 1 → 25 |
| `supervision_sequences` | 1,168,809 (+ 541,151) | 91 s (+ 45 s) | ~13k | 1 → 62 |
| `fsm_sequences` | 467,410 | 91 s | ~5k | 62 → 123 |
| `birth_sequences` | 258,961 | 91 s | ~2.8k | 123 → 174 |
| `stash_sequences` | 301,250 | 91 s | ~3.3k | 174 → 228 |
| `stack_sequences` | 623,552 | 91 s | ~6.8k | 228 → 357 |

**~6.2M executions total; no crash, no counterexample in any target.**
Corpus (357 files, 1.4M) committed under `fuzz/corpus/`.

## Exact command results

- `bash autoresearch.sh` → exit 0; last full run:
  `METRIC score=50895765` (best kept 51.5 M t/s; score is a throughput
  ruler, variance is machine noise).
- `.auto/checks.sh` → `CHECK OK` (production/docs/`.auto` untouched;
  `cargo test --all-targets` 63 passed; clippy `-D warnings` clean; no
  fastpass in the research surface).
- `.auto/measure.sh` → `METRIC score=...` plus 7 secondary metrics.
- `cargo test --manifest-path research/behaviorpass-autoresearch/Cargo.toml --all-targets`
  → 63 passed, 0 failed, 0 ignored.
- `cargo fuzz build` (all six targets, fuzz workspace) → clean.
- Git: 14 focused commits (`c7f5bd58`, `00007460`, `1ea6a6d3`, `3eb839da`,
  `fe060589`, `0179edf0`, `c953880e`, `4675ed90`, `b290539a`, `59df757a`,
  + report iterations).
