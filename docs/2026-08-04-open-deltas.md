# behaviorpass — open deltas (tracking doc, 2026-08-04)

> Every known gap between what this crate is and what it must become,
> each with its landing pass. Criteria: pure fold, algebraic shape,
> ergonomics. The actorpass design doc (actorpass/docs/2026-08-04-…)
> carries the full evidence trail; this is the working delta list.

## 1. Verification debt (unproven)

- **Never run live.** The crate is golf-proven only (`run` records; the
  trace oracle compares folds). The hardest promises — parked
  keep-address restart, panic reincarnation, fleet spawn at process
  start, anchor-based delivery, drain-stop observability — are
  design-doc claims until **card 3 (the actorpass spine)** drives them.
  Treat every live-semantics claim as provisional until then.
- **Perf representation regression unanswered.** The general-supervision
  pass traded the lazy zero-alloc bitset for a uniform nonce-keyed
  `Vec<(Nonce, SlotRec)>` (24 B/child): ruler score 30303 → 37.8,
  space 32 B → 26 KB @ 1097 children. Accepted deliberately (generality
  first); the **perf loop's next run** re-derives a representation
  (static-prefix bitset + dynamic spill is the recorded candidate)
  behind identical semantics.

## 2. Deferred algebra legs (the post-spine pass, one oracle re-gate)

- **Watch registration leg** — `Watching` handles `LinkDied` but nothing
  live *registers* a watch. Needs a pure `links: Vec<Link<Addr>>` leg on
  `Actions` + driver bookkeeping (the system's first type erasure: an
  erased `LinkSink` per heterogeneous watcher). Fork E, RULED deferred.
- **Kill / decommission leg** — `Create::Stop { nonce }`. Semantics
  recorded: abort = ADDRESS DEATH (the Consumer drops with the task; old
  handles error fast), replacement = fresh `Birth` at a NEW nonce, never
  a restart. Keep-address restart is for live/parked children only.
- **Hung-fold watchdog** — a fold may legally await; a hung fold hangs
  its process (control lane queues but isn't processed). Driver-side
  step-time policy; pairs with the kill leg.
- **`DeadLetters` observability** — v1 follows Erlang (sends to dead
  addresses drop silently). The envelope is the refinement door.
- **`Crash::Panicked` site marker** — if bombay proves it needs
  hook-panic-vs-handler-panic distinction (it has
  refuse-restart-on-hook-panic today).

## 3. Surface gaps (the builder era)

- **Mixed-protocol `workers!`** — v1 is shared-protocol. The `CrewMsg`
  sum + mismatch arm generation is the real test of the macro approach.
- **Policy trait-ification** — reactions (`LinkReaction`, time
  reactions) are still fn pointers; the `State`-move for policies
  (traits + closure blanket impls) is recorded, unimplemented.
- **`hold(n)`** — the locked intent for Stashing needs capacity
  semantics; the layer today is route-only (`StashRoute`). Deferred.
- **Relative and periodic time intentions** — `.at(...)` is the absolute
  one-shot intent. Relative, idle, and periodic compositions remain open.
- **`Effect` sugar** (`none()`, `reply()`, …) — floated to spare
  handlers spelling out `Actions { sends, creates, become_ }`. UNRULED.
- **`Fsm`'s transition fn** — the same loose (state, fn) pair shape as
  the old floor; phases are a separate concern, left for a later pass.

## 4. Structural smells (live with, or slim later)

- **`Ph` is nearly vestigial** — `Never` everywhere except `Fsm`
  ("not core"). If phases stay out of core, a future pass could slim
  `Behavior` from six assoc types toward five.
- **`Supervising::new` is positional (8 args)** — hidden by `Spec`;
  the raw layer keeps it deliberately.
- **Block-local `Crew`** — macro-generated fleet sums are unnameable
  outside their block; user-facing errors will show hygiene names.
  Accepted UX cost, unproven in practice.

## 5. Bombay-coverage deltas with behaviorpass-side work

- **Kill semantics** (bombay `stop`/`kill`/`PendingAbort`) — becomes
  the `Create::Stop` leg above; the driver side lands in card 3
  (child-table `JoinHandle`).
- **Weak registration** — the System table pins lanes open with strong
  senders; solved as the fastpass **`UserAnchor`** ask (sends while the
  consumer lives, doesn't count toward `closed()`), card 2.5, NOT the
  earlier weak-sender framing.
- **Backoff + jitter** — derivable in the proxy replacement protocol from
  restart history; deterministic jitter can use `H(nonce, restart_count)`.
