# Card 1 — behaviorpass evolution: Address, nonce-keyed creation, general supervision

> **Context:** approved design in
> `~/Code/devrandom/actorpass/docs/2026-08-04-actorpass-create-at-address-design.md`
> ("Runtime talk #2", items 1–7 + fork rulings). This card is the FIRST of
> three (2: fastpass asks; 3: actorpass spine). Work happens in
> `~/Code/devrandom/behaviorpass`. Read the design doc section "Runtime
> talk #2" before touching code — every change below is derived there.

## Non-negotiable constraints (the layering law)

1. **The pure fold is inviolable.** `step` performs no I/O, no entropy, no
   `Instant::now()`, no channels. The receipt `Instant` arrives VIA the
   envelope stamp (data), never from a clock call inside a layer.
2. **Not ugly.** The algebra stays minimal and self-describing: one trait,
   one floor, wrappers. No runtime types, no framework jargon.
3. **No new dependencies.** `thiserror`/`tokio`/`fastpass` only, as today.
4. The clippy `[lints]` table is law: `all` deny + `pedantic` warn; zero
   errors, ZERO new warnings. Every `#[allow]` carries a `reason`.
5. No new `unsafe`. `supervising.rs`'s existing bitset `unsafe` stays as
   is; the spill map is safe code (`Vec`).
6. Naming: explicit Rust-style behavioral names. Citations (Agha, AMST,
   OTP) in doc comments as prose, NEVER in identifiers.
7. Panics are for programmer bugs only (an inner emitting
   `Create::Restart`, a `ChildStopped` naming an unknown nonce, a birth
   nonce collision). Folding never panics on data.
8. **No boolean blindness in decision vocabulary.** Death outcomes ride as
   `Result<Exit<A>, Crash>`; classification is pure policy inside the
   layer, never a driver-side pre-digested flag.

## Target files

- `crates/behaviorpass/src/behavior.rs` — the core grammar (most of the
  work).
- `crates/behaviorpass/src/exit.rs` — `Exit` gains the address parameter;
  the `Crash` marker.
- `crates/behaviorpass/src/supervising.rs` — the generalization (the rest
  of the work).
- `crates/behaviorpass/src/watching.rs`, `deadlined.rs`, `stashing.rs`,
  `fsm.rs` — mechanical threading only (no semantics change).
- `crates/behaviorpass/src/lib.rs` — exports.
- `crates/behaviorpass/tests/oracle.rs` — re-gated traces (deliberate,
  user-approved; see Acceptance).
- `crates/behaviorpass/tests/adv_*.rs`, `examples/p*.rs`,
  `examples/perf_supervising.rs` — mechanical compile fixes.

**Non-goals (explicit):** actorpass code (separate repo, card 3); fastpass
changes (card 2); the watch leg (`links: Vec<Link<Addr>>`) — DEFERRED
(fork E); any `Started` envelope variant; per-birth restart policies;
escalate-on-budget-exhaustion; typed crash reasons on the envelope
(homogeneous-fleet door, deferred); fastpass interface distillation
(queued).

## Change 1 — the `Address` trait + `Behavior::Addr` (behavior.rs)

New vocabulary, exactly:

```rust
/// An abstract mail address (Agha): the fold names send recipients and
/// derives child addresses as a PURE function — `birth` is AMST's
/// `newadr()` discharged by derivation, not computation. The nonce TYPE is
/// the address type's own business (how it mixes into the derivation is
/// the impl's concern — no encoding trait).
pub trait Address: Copy + Eq {
    /// The creator-minted birth nonce namespace (`Copy + Eq` only — no
    /// `Hash` bound leaks into the pure layer).
    type Nonce: Copy + Eq;
    /// The address of the child born of this address at `nonce`.
    fn birth(self, nonce: Self::Nonce) -> Self;
}
```

`MailAddr` (already in behavior.rs) implements it with `type Nonce = u64`
and a toy pure mixing (e.g. `MailAddr(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))`
— any deterministic pure fn; it is golf vocabulary, not crypto).

`Behavior` gains one associated type:

```rust
pub trait Behavior {
    type Addr: Address;
    type Msg;
    // … Ph / Error / Outbound / Offspring unchanged …
```

## Change 2 — the alphabet and the actions grammar (behavior.rs, exit.rs)

`Exit` gains the address parameter (`LinkDied(u64)` → `LinkDied(A)`) and a
death-category marker joins the vocabulary:

```rust
pub enum Exit<A: Address> {
    Normal,
    Collected,
    LinkDied(A),
}

/// How the fold crashed. The reason VALUE stays with the driver in both
/// cases (heterogeneous error types and panic payloads are runtime
/// plumbing — typed reasons are the homogeneous-fleet door, deferred);
/// the fold receives the DOMAIN. Both variants classify as abnormal; the
/// distinction is preserved for future policy (e.g. poison-message
/// handling) and for trace truth — the death site knows which one
/// happened, and the report must not collapse it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crash {
    /// `step` returned `Err` — the behavior's declared controlled crash.
    Failed,
    /// The fold panicked — an undeclared programmer bug.
    Panicked,
}
```

`Become`/`Step` thread it: `pub type Become<A, Ph = Never> = Step<Ph, Exit<A>>`.
`Transcript.exit` becomes `Exit<A>`; `run` mints `Exit::Collected` as
today. (`run`'s error path stays `Err(B::Error)` — a crash is NOT an
`Exit`; that is exactly why the envelope below needs the `Result`.)

`Envelope` gains the address parameter; `User` gains the driver-stamped
sender (ADR-0015: the sender is the authority); the two death legs carry
the OUTCOME, not a bool (classification moves into the pure layer);
`ChildStopped` names the child by nonce and gains the receipt stamp
(windowed budgets — the DRIVER mints the stamp at envelope construction;
the fold never reads a clock):

```rust
pub enum Envelope<A: Address, M> {
    /// A user-lane message with the driver-stamped sender address.
    User { from: A, msg: M },
    /// The single-shot deadline arm fired.
    Deadline,
    /// A watched/linked peer stopped, with how it ended.
    LinkDied { peer: A, outcome: Result<Exit<A>, Crash> },
    /// A supervised child fold ended, received at `at` (the budget-window
    /// stamp), with how it ended.
    ChildStopped { nonce: A::Nonce, outcome: Result<Exit<A>, Crash>, at: Instant },
}
```

`Behavior::step`'s argument becomes `Envelope<Self::Addr, Self::Msg>`.

Sends become targeted (fork I): global addresses or a child slot (slot =
nonce):

```rust
/// A send target: a global mail address, or one of the sender's own
/// children by birth nonce (the driver resolves the nonce against its
/// child table — symmetric with `Envelope::ChildStopped`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<A: Address> {
    /// A global address (Agha mobility: addresses travel in messages).
    Global(A),
    /// A child of the sender, by the nonce its birth carried.
    Child(A::Nonce),
}
```

`Actions.sends` becomes `Vec<(Target<A>, Out)>`; `Actions` therefore gains
the address parameter: `Actions<A: Address, Ph, Out, New>` (put `A` FIRST —
it is the namespace the other three live in). `Acted`, `Handler`, `lift`,
`Actions::{just, cont, stop, goto}`, `Transcript`, and `run` thread the
parameter mechanically. `run` (the test driver) takes a `from: B::Addr`
argument used for every `Envelope::User` it fabricates.

`Create` gains the address parameter; **birth carries the creator-minted
nonce** (the ONLY birth shape — the tree is total: every parent, up to the
System/main, is a supervisor namespace); restart renames `slot` → `nonce`
(slots ARE nonces):

```rust
pub enum Create<A: Address, New> {
    /// A fresh actor at `Address::birth(self, nonce)` (creator-minted,
    /// framework freshness-validated): the driver spawns at the derived
    /// address.
    Birth { nonce: A::Nonce, child: New },
    /// A restart decision for the child born at `nonce`: the address and
    /// mailbox SURVIVE, only the behavior is swapped (keep-address).
    Restart { nonce: A::Nonce, child: New },
}
```

## Change 3 — the `fleet()` query (behavior.rs + all wrappers)

The initial fleet is a SOURCE, delivered by query — the `next_deadline`
pattern verbatim (source = query, firing = event). On `Behavior`:

```rust
/// The static child fleet this behavior declares at construction, as a
/// pure function of state (`None` = no fleet — a plain actor). The
/// driver's fleet birth happens at process START only; restarts do NOT
/// re-spawn (the child table survives the slot swap). The static fleet's
/// nonces are minted `0..n` by the creator (slot = nonce).
fn fleet(&self) -> Option<(usize, fn(usize) -> Self::Offspring)> {
    None
}
```

`Supervising` overrides returning `Some((self.n_children as usize,
self.build))`. `Watching`/`Deadlined`/`Stashing`/`Fsm` forward to `inner`
exactly like their `next_deadline` forwards (one line each). `Base` keeps
the default.

## Change 4 — `Supervising` generalized (supervising.rs)

Four hardcodings removed; the layer becomes the full strategy space as
PURE decisions (the driver executes). New vocabulary:

```rust
pub enum Strategy {
    /// Restart only the dead child.
    OneForOne,
    /// Restart every live child when one triggers.
    OneForAll,
    /// Restart the dead child and every child born AFTER it (birth
    /// SEQUENCE order, not nonce order).
    RestForOne,
}

pub enum RestartPolicy {
    /// Restart on any stop, normal or abnormal.
    Permanent,
    /// Restart only on abnormal outcome (today's behavior).
    Transient,
    /// Never restart; a stop only marks the slot dead.
    Temporary,
}
```

`Supervising::new(inner, n_children, build, strategy, policy, max_restarts,
window: Duration)`. Semantics (the LAW — simple, deterministic, ours; do
not cite OTP for edge semantics this card defines):

1. **Inner bound relaxes** from `B: Behavior<Offspring = Never>` to
   `B: Behavior<Offspring = C>`. The layer's `forward` maps the inner's
   creates: each `Create::Birth { nonce, child }` is **freshness-validated**
   against the liveness table (every known nonce — static or dynamic,
   alive or dead — is taken); a collision is a programmer bug → panic. A
   validated birth is recorded (spill entry, next sequence number) and
   emitted as-is. An inner-emitted `Create::Restart` is a programmer
   bug → panic (restart decisions belong to the layer).
2. **Outcome classification (the bool's replacement):** the NORMAL subset
   is `{Exit::Normal, Exit::Collected}`; ABNORMAL ≡ `Err(Crash)` or
   `Ok(_)` outside that subset. Classification is a `match` in the layer —
   never a driver-computed flag.
3. **`on_child_stopped(nonce, outcome, at)`** (replacing the usize/bool
   version): unknown nonce → panic (driver/behavior desync is a programmer
   bug). The dead child's OWN policy gates strategy evaluation:
   `Temporary` → mark dead, no evaluation; `Transient` → evaluate only if
   abnormal (per point 2); `Permanent` → always evaluate. Candidate set by
   strategy: `OneForOne` → {nonce}; `OneForAll` → all live nonces;
   `RestForOne` → live nonces with birth-sequence ≥ the dead child's
   sequence.
4. **Windowed budget** replaces `restarts_left`: keep the restart
   timestamps (a `Vec<Instant>` is fine — prune entries with
   `at - ts > window` on each evaluation; `Duration::MAX` is the
   count-only case). All-or-nothing per event: if
   `restarts_in_window + candidates.len() <= max_restarts`, emit one
   `Create::Restart` per candidate (the dead child FIRST), push `at` per
   restart, re-mark the dead child live; else mark the dead child dead and
   emit nothing. (Escalate-on-exhaustion is a non-goal — noted door.)
5. **Liveness representation:** one uniform `Vec<(A::Nonce, SlotRec)>`
   table, `SlotRec { alive: bool, seq: u64 }` — assoc scans (fleets are
   small; the golf's perf ruler measures `Supervising` CONSTRUCTION and
   decisions, and a Vec scan beats the old bitset at golf sizes anyway).
   Static slots are inserted at construction with `seq = 0..n`; dynamic
   births take the next sequence from a `next_seq: u64` counter starting
   at `n_children`. The lazy-bitset `unsafe` block goes away WITH the
   bitset — the table is plain safe code. (If the perf loop later proves
   the bitset worth it, it returns as a representation-only change behind
   identical semantics — that is the loop's job, not this card's.)

**Static-prefix nonce question — RESOLVED:** the fleet's nonces are the
`usize` indices `0..n` only at the DRIVER level. The BEHAVIOR never sees
static nonces as a special range — the layer learns every static nonce the
same way it learns dynamic ones: recorded at construction. But the
constructor takes only `n_children` today, so the layer cannot know the
static nonces unless they are derivable — therefore `Supervising` REQUIRES
`A::Nonce: From<usize>`… — NO. Final call (main agent, recorded): the
constructor gains the nonce minter:
`Supervising::new(inner, nonces: fn(usize) -> A::Nonce, n_children, build, …)`
and the golf passes `|i| i as u64`. Explicit, no extra trait bounds, no
special ranges. `fleet()`'s shape is unchanged (the driver mints child
ADDRESSES from indices; the BEHAVIOR-side nonces come from the same
minter, so driver table and behavior table agree by construction — the
design doc's "slot = nonce" pin).

## Change 5 — mechanical threading (watching / deadlined / stashing / fsm)

Each wrapper: add the `A: Address` parameter alongside `B`, set
`type Addr = B::Addr`, forward `fleet()` (one line), thread
`Envelope<A, M>` through `step`. `Watching`'s `LinkReaction` becomes
`fn(&mut B, A, Result<Exit<A>, Crash>) -> Result<Become<A>, <B as Behavior>::Error>`
and `stop_on_abnormal_death` classifies by match (propagate on abnormal,
absorb `Ok(Normal | Collected)`). NO other semantics change in these
files; if you find yourself changing logic, stop — the change belongs in
behavior.rs or supervising.rs.

## Change 6 — exports (lib.rs)

Add `Address`, `Target` to the `behavior` re-export; `Crash` from `exit`;
add `Strategy`, `RestartPolicy` from `supervising`. Update the module doc
line for supervising to say it decides the full strategy space.

## Acceptance (the gates — all must hold)

1. `cargo test --workspace --all-targets` green.
2. `cargo clippy --workspace --all-targets`: zero errors, zero NEW
   warnings.
3. **Oracle re-gated, deliberately** (`tests/oracle.rs`): every scenario
   updated for the new shapes (`from`, stamps, nonces, outcomes,
   `Target`), and the supervising scenario extended to assert the NEW
   laws: outcome classification (crash vs abnormal-`Exit` vs normal —
   NO bools anywhere), transient gate, window eviction (a restart older
   than the window does not count), all-or-nothing exhaustion,
   `RestForOne` birth-sequence order (NOT nonce order), inner-birth
   freshness panic, inner-`Restart` panic. The oracle stays frozen in
   spirit: after this pass the file is frozen AGAIN against the golf loop.
4. **Mutation probes** for each new invariant: flip the production code
   (freshness check, window prune, sequence comparison, policy gate,
   classification match arm, all-or-nothing), watch the corresponding
   test fail, revert. Report the probe results.
5. `cargo test -p behaviorpass --all-targets` under `.auto/checks.sh`
   gate 2 passes; then re-point `.auto/BASELINE` to the evolution HEAD and
   confirm `CHECK OK` (the FROZEN diff now passes at the new baseline).
   LOC growth is EXPECTED (capability addition, not golf regression) —
   note the before/after `code_loc` from `.auto/measure.sh` in the report.
6. No `Instant::now()` anywhere in `src/` (grep it). No `unsafe` remains
   in `supervising.rs` (the bitset is gone). No new dependencies. No
   `abnormal: bool` (or any bare bool) in `Envelope`, `Create`, `Actions`,
   or any public signature — grep the public surface for `bool` and
   justify every hit in the report.

## Report back

- The diff summary per file.
- The mutation-probe table (invariant → flipped line → failing test).
- The `code_loc` before/after.
- Anything the card got wrong (quote the card text and what the code
  forced instead) — the card is a hypothesis, the design doc is the truth;
  divergences go back to the main agent, never silent improvisation.
