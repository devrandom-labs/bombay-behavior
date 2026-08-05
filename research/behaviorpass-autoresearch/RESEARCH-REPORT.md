# behaviorpass adversarial autoresearch — session report

Session: `autoresearch/run-the-behaviorpass-adversarial-autoresearch-lo-20260805`
Baseline: `086328746bba` (harness snapshot) — production frozen throughout.
Scope: `research/behaviorpass-autoresearch/**` only. No production code touched.

## Summary

The public behaviorpass algebra survived every attack lane. 40 tests across
deterministic/exhaustive, model-based property, fuzz, and performance
workloads; **no invariant violation was found** — but the campaign produced
one scaffold-test correction and a set of deterministic semantic
observations (below) that pin the algebra's actual behavior.

## Tests added (research crate)

| File | Count | Lane |
|---|---|---|
| `tests/core.rs` | 10 | deterministic (1 scaffold test corrected, see Finding 1) |
| `tests/boundaries.rs` | 12 | deterministic boundary/edge (new) |
| `tests/compositions.rs` | 5 | deep wrapper permutations + routing (new) |
| `tests/properties.rs` | 4 | model-based properties (scaffold) |
| `tests/supervision_model.rs` | 3 | model-based properties + deterministic budget recovery |
| `tests/exhaustive.rs` | 1 | exhaustive small-state enumeration |
| `tests/stash_properties.rs` | 2 | model-based filter property + driver variant |
| `tests/workers_fleet.rs` | 3 | `workers!` sum/variant dispatch + supervised fleet |
| `src/model.rs` | — | shared independent supervision reference model |
| `benches/protocol_matrix.rs` | — | extended: supervise/fsm/stash/nested workloads |
| `fuzz/fuzz_targets/protocol_sequences.rs` | — | scaffold proxy fuzz target |
| `fuzz/fuzz_targets/supervision_sequences.rs` | — | new: budget/window model fuzz target |

## Explored combinations

- **Supervision**: strategy × policy × budget × window × timestamp ordering
  (equal, duplicate, backwards) × fleet size; static fleets, dynamic births
  interleaved with deaths, birth-sequence (RestForOne) ordering, duplicate
  death redelivery, duplicate configured nonces, empty fleets, unknown
  nonces, window-edge inclusivity, budget recovery after stamp aging,
  future-stamp survival.
- **Exhaustive**: every sequence of ≤ 3 child-stopped events over a 2-slot
  fleet (alphabet 16 → 4,369 sequences) × 3 strategies × 3 policies × 3
  budgets × 2 windows = **236k model-vs-impl comparisons**.
- **Properties**: 256-case supervision model, 256-case mixed birth+death
  model, 256-case stash filter model, 512-case scaffold properties
  (proxy generations, FIFO prefix, time init product).
- **Composition**: all 6 orderings of {at, watch, at} init-protocol nesting;
  stash adds no init sends; environment lanes (Reached/PeerStopped) bypass a
  stash buffer while user messages are intercepted; watch-of-watch peer
  routing; watch reaction re-invocation after Stop; nested At schedule
  collisions; FSM mid-drain reordering.
- **Fuzz × model**: supervision budget/window reference model inside the
  fuzz target, asserted per byte.

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
   releases. A 256-case filter-model property confirms delivered ==
   route-admitted lane exactly.
2. **Duplicate `ChildStopped` redelivery triggers a second replacement.**
   Supervision does not deduplicate death notices; a redelivered death
   consumes budget and emits a fresh replacement send.
3. **Duplicate configured nonces are accepted.** `children_with_nonces`
   does not validate configured fleet nonces (two slots can share one
   route); the fresh-nonce guard exists only for dynamic births
   (`wrap()` panics on duplicates there).
4. **Nested `At` schedules with identical (id, at) collapse onto the outer
   layer.** The inner schedule fires only on a second (duplicate) delivery.
   Deterministic, no loss, but the two schedules are not independently
   observable.
5. **FSM mid-drain deferral reorders relative to FIFO.** A message deferred
   during a drain, then replayed after a phase change inside the same drain,
   is processed after messages held behind it (newly deferred messages are
   appended to the batch tail).
6. **Restart-window pruning keeps future stamps** (a death stamped earlier
   than a prior restart's stamp underflows `checked_duration_since` and is
   kept), inflating the budget count.
7. **OneForAll excludes previously-denied slots** from its candidate set and
   never resurrects them; the budget counts every replacement (a 3-child
   OneForAll restart consumes 3 stamps).
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

## Performance observations (`benches/protocol_matrix.rs`, M1 Pro, release)

| Workload | rate |
|---|---|
| base (Base step) | ~392 M t/s |
| proxy (Forward/Replace mix) | ~46 M t/s (**score**) |
| supervise fleet 8 | ~18 M t/s |
| supervise fleet 256 | ~9 M t/s |
| FSM (alternating phases) | ~283 M t/s |
| stash passthrough | ~103 M t/s |
| nested At∘Watch∘Stash | ~51 M t/s |

- Supervision throughput is **sublinear in fleet size** at 8→256 slots
  (~2× slower for 32× slots): the `position()` scan is cache/memory-bound at
  these sizes, not element-count-bound.
- **Memory growth is strictly proportional to emitted algebra**: with
  `window = Duration::MAX` the `restarts` stamp vector grows one entry per
  emitted replacement (100k events → 100k stamps, no pruning, unbounded
  budget); with a finite window, pruning bounds it (verified by
  `restarts_in_window()` in the budget-recovery test and fuzz assertions).

## Fuzzing

`cargo-fuzz 0.13.2`, pinned stable toolchain with `RUSTC_BOOTSTRAP=1`
(cargo-fuzz's `-Zsanitizer=address` requires it).

- `cargo fuzz run protocol_sequences -- -max_total_time=90`:
  **2,857,694 runs** in 91 s (~31 k exec/s), cov 463 / ft 672, corpus
  1 → 25 files, **no crash, no counterexample**.
- `cargo fuzz run supervision_sequences -- -max_total_time=90`:
  **1,168,809 runs** in 91 s, corpus 1 → 61 files, **no crash** — the
  inline budget/window model held on every byte sequence.
- Extra session: `supervision_sequences -max_total_time=45`: 541,151 runs,
  cov 583 / ft 1041, corpus → 62 files, no crash.

Corpus (62 files, 244K) committed under `fuzz/corpus/`.

## Exact command results

- `bash autoresearch.sh` → exit 0; last full run:
  `METRIC score=33588417`, `METRIC base_transitions_per_s=391849530`,
  `METRIC proxy_transitions_per_s=45918253` (best kept score 51.5 M t/s;
  score is a throughput ruler, variance is machine noise).
- `.auto/checks.sh` → `CHECK OK` (production/docs/`.auto` untouched;
  `cargo test --all-targets` 40 passed; clippy `-D warnings` clean; no
  fastpass in the research surface).
- `.auto/measure.sh` → `METRIC score=...` plus secondary metrics.
- `cargo test --manifest-path research/behaviorpass-autoresearch/Cargo.toml --all-targets`
  → 40 passed, 0 failed, 0 ignored.
- `cargo fuzz build` (both targets, fuzz workspace) → clean.
- Git: 5 focused commits (`c7f5bd58`, `00007460`, `1ea6a6d3`, `3eb839da`,
  `fe060589`, + this report).
