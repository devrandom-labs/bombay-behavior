# behaviorpass — perf loop: the `Supervising` children mechanism

## Objective
MAXIMIZE `METRIC score` — a SPACE fitness for how `Supervising` tracks its
children. Today the liveness table is a `Vec<Child>` (one heap byte per child +
a 24-byte control block + a per-slot `bool`). Make it as small and
allocation-free as possible. Smaller footprint ⇒ higher score. `measure.sh`
prints, every run:
- `score` = `1_000_000 / (1 + space_bytes)` (the number to grow),
- `info space_bytes / alloc_bytes / struct_size` (where the bytes are),
- `info step_throughput_per_s` (the hot path — must NOT regress; see gates).

The measurement is `crates/behaviorpass/examples/perf_supervising.rs`. It is
FROZEN and drives `Supervising` only through its public constructor and
`Behavior::step`. You do not edit the ruler — you shrink what it measures.

## What you MAY change
- **`crates/behaviorpass/src/supervising.rs`** — the whole point. The internal
  liveness REPRESENTATION is yours: bit-packed masks, an inline/stack buffer, a
  fixed bitset, a hybrid — anything. Add bespoke private helpers freely.
- **`crates/behaviorpass/src/lib.rs`** — only if you change Supervising's
  bespoke re-exports (e.g. drop `Child`, add `is_alive`).
- **Supervising's own tests** (`tests/adv_supervising.rs`, the Supervising cases
  in `tests/oracle.rs` / `tests/adv_composition.rs`, the `p_sup_*` examples) —
  ONLY to follow a bespoke-getter rename. Never weaken a behavior assertion.

## Hard rules — the interface is the contract
- **The generic `Behavior` interface stays IDENTICAL.** `impl Behavior for
  Supervising` — its associated types and the OBSERVABLE results of `step(...)`
  and `next_deadline()` — must not change. Processes drive supervisors through
  this generic surface; they must see the exact same behavior. Specifically, for
  every input these stay pinned (the existing tests assert them — keep them
  green): abnormal-stop-within-budget ⇒ exactly ONE create + budget−1; exhausted
  or normal stop ⇒ zero creates; out-of-range idx ⇒ zero creates, budget
  untouched; `next_deadline` forwards the inner deadline unchanged; sends/creates
  pass through composition untouched.
- **`Supervising::new(inner, n_children, build, restarts_left)` keeps its
  signature.** It is how every process constructs a supervisor.
- **Only the getters an owner uses when it KNOWS it holds a Supervising may
  change** (`children`/`Child::alive` → e.g. `is_alive(idx)` / `child_count()`).
  A generic process never calls these; a Supervising-aware one does. If you
  rename them, update their call sites in the Supervising tests so the suite
  stays green.
- **Everything else is FROZEN** (`checks.sh` reverts it): the other capability
  modules, both `Cargo.toml`s, the perf example, this harness.
- Use the workspace toolchain only. No new dependency without one in the
  workspace already (bombay, fastpass, thiserror, tokio, proptest) — a manifest
  edit is a frozen-surface violation. If a representation wants a bitset, hand-roll
  it or use `u64`/`[u64; N]` words; do not add a crate.

## The gates (checks.sh)
1. Frozen-surface BASELINE diff is clean.
2. `cargo test -p behaviorpass --all-targets` is green — the whole existing suite
   passes on the real code and every example/bench still builds. This is what
   pins the generic Behavior contract while you reshape the representation.

Later phases add: an allocation bound (a counting-allocator test asserting the
table is zero-heap for small fleets), and a `miri` UB pass over the Supervising
step. Keep the score; do not regress `step_throughput_per_s`.
