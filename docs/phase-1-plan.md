# behaviorpass phase-1 — build the harness, then hand it to the loop (card #298)

> **Status:** phase-0 scaffold shipped (frozen reference + `.auto` contract,
> compiles + `CHECK OK`). Phase-1 authors the parts the loop CANNOT (the frozen
> oracle + the SUT public seam + the trybuild cases); only after the oracle is
> green at all 24 points do we dispatch the golf loop.

**Goal:** a fresh **async** realization of the five ADR-0030 layers, spawnable,
trace-equal to the frozen sync fold at every legal lattice point — plus the
frozen oracle that proves it and the 17 `compile_fail` cases that enforce the
laws. The champion realization becomes the blueprint for #295's implementation
pass.

## Key decisions (resolved; alternatives noted)

1. **SUT = fresh async layers, not a port of bombay's capability apparatus.**
   `crates/behaviorpass/src` gets an `async fn step` `Behavior` trait and five
   async layer wrappers — the async twins of `behaviorpass-reference`. No
   `CapSet`/`Provide`/`Shell`/derive. This is the golf target.
2. **Runtime: reuse bombay's mailbox primitive, own the select driver.** The
   SUT spawns a tokio task that owns a merged `select!` over
   `bombay::mailbox` (the receiver) + a deadline arm + a link arm, folding the
   composed `Behavior`. The driver (the guarded select arms #298 wants golfed)
   lives IN the SUT — golfable. *Alternative rejected for phase-1:* adapting to
   bombay's `actor::Actor` loop (that reintroduces the machinery we're trying
   to out-concise).
3. **Lattice generator = a declarative macro** `machine! { Stashing, Phased(...) }`
   emitting one minimal actor per point; illegal combinations don't expand
   (the laws are `where`-bounds on the layer constructors, so illegal stacks
   fail to typecheck — that IS the `compile_fail` proof). *Alternative:* 24
   hand-written actors — rejected (LOC noise, drift risk).
4. **Oracle = one abstract `Script` → two runners.** The sync runner folds the
   script into reference events; the async runner translates the script into
   tells / `tokio::time` advances (`start_paused`) / synthetic link-deaths and
   collects probes. Probe sequences + stop kinds must be identical. Mirrors
   bombay's `phase_equivalence` (#266).

## File structure

- FROZEN `crates/behaviorpass-reference/src/lib.rs` — DONE (the sync model).
- FROZEN `crates/behaviorpass-testkit/src/lib.rs` — the `Probe` vocab, the
  `Script`/`Op` types, `same_stop`, and the `behavior_suite!` macro.
- SUT `crates/behaviorpass/src/`
  - `behavior.rs` — the async `Behavior` trait + the `run`/`spawn` driver.
  - `layers.rs` — the five async layers (`Base`/`Deadlined`/`Watching`/
    `Stashing`/`Phased`/`Supervising`) with law `where`-bounds.
  - `lattice.rs` — the `machine!` generator + the 24 legal point actors.
  - `lib.rs` — module wiring + the public seam the oracle drives.
- SUT `crates/behaviorpass/tests/`
  - `oracle.rs` — instantiates `behavior_suite!` per legal point.
  - `compile_fail.rs` + `tests/ui/*.rs` — the 17 illegal stacks (trybuild).

## Tasks (bite-sized, TDD; commit scope `[#298]`)

### Task 1 — async `Behavior` trait + minimal driver (SUT)
- [ ] `behavior.rs`: `trait Behavior { type Event; type Ph; type Error; async fn step(&mut self, Event) -> Result<Step<Ph, Exit>, Error>; }` (reuse `bombay::capability::Step`/`Never`; `Exit` re-exported from the reference).
- [ ] A `run<B: Behavior<Ph = Never>>(b, sources) -> Result<Exit, B::Error>` driver: a `tokio::select!` over a mailbox arm + a deadline arm (`sleep_until`, fires-once guard, ADR-0025 placement) + a link arm, folding `step`. Test with a plain `Base` first: one unit test spawns it, tells 2 messages + a stop, asserts probes — RED, then implement to GREEN.
- [ ] Commit.

### Task 2 — the five async layers (SUT)
- [ ] `layers.rs`: async twins of the reference layers. Each is a `C<B>: Behavior where B: Behavior`; routing/gate/drain logic mirrors the reference exactly (the reference tests already pin the semantics). Compose-law `where`-bounds: `Supervising<B>` requires `B: Watches`; `Phased` and `Stashing`/`Deadlined` are mutually exclusive via a sealed marker trait.
- [ ] One unit test per layer mirroring the reference's layer tests (same assertions, async). RED→GREEN→commit each.

### Task 3 — the frozen oracle (testkit)
- [ ] `Probe` enum (`Applied`/`Processed`/`Refused`/`ShedFull`/`TimedOut`/`StaleTimeoutLeaked`), `Script`(`Vec<Op>`), `Op` (mode-blind), `same_stop(Exit, &ActorStopReason)`.
- [ ] `behavior_suite!($point)` macro: for the axes the point HAS — FIFO+exactly-once (all), defer/replay (Bounded), fires-once/left-phase (armed), death/restart (Watching/Supervising) — drive the reference fold AND the SUT actor from the same `Script`, assert identical probes+stop. Every awaited step under a timeout.
- [ ] Freeze: add the testkit test files to `.auto/checks.sh` FROZEN list.

### Task 4 — the lattice generator + 24 legal points (SUT)
- [ ] `machine!` macro emitting one minimal actor per cap-set; `lattice.rs` instantiates the 24 legal points (15 stacks × Phased inner seats).
- [ ] `tests/oracle.rs`: `behavior_suite!` per point. This is the gate-2 payload — it makes `checks.sh` trace-equality load-bearing. RED (SUT incomplete) → GREEN.

### Task 5 — the 17 illegal points (trybuild)
- [ ] `tests/ui/*.rs`: the 17 illegal stacks; each must fail with the law's readable error. `tests/compile_fail.rs` runs trybuild. Wire into `.auto/checks.sh` gate 3 (replace the placeholder).
- [ ] Freeze the ui files.

### Task 6 — re-freeze + dispatch
- [ ] Re-point `.auto/BASELINE` to the phase-1 HEAD; confirm `measure.sh` now reports a real `code_loc`/score and `checks.sh` = `CHECK OK` at all 24 points.
- [ ] Rewrite `.auto/prompt.md` to the golf brief (the harness is built; minimize LOC).
- [ ] Dispatch: `pueue add --label phase1 -- omp --profile autoresearch --auto-approve --max-time 8h -p "/autoresearch <golf brief>"`; `tail -f .auto/log.jsonl`.

## Gotchas (from the scaffold + bombay history)
- Frozen crates must clear `clippy::all` (deny): factor `fn(...)->Result<..>`
  fields into `type` aliases (hit on the reference already).
- Deadline arm: `start_paused` + the fires-once guard, or the wrapping-timeout
  never fires (bombay ADR-0025 / the paused-clock memory).
- The oracle is FROZEN from the moment it lands — the loop can't edit it, so it
  must be right before dispatch. An empty/loose oracle = a green lane over the
  wrong surface (#149).
