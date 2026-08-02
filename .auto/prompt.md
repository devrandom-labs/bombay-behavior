# behaviorpass — adversarial invariant loop (mutation-kill)

## Objective
MAXIMIZE `METRIC score` = mutants **CAUGHT** by the test suite over
`crates/behaviorpass/src` (cargo-mutants). You break the code by **adding
tests**: cargo-mutants generates broken versions of the source, and your tests
must catch each one. A **MISSED (surviving) mutant is an invariant no test
pins** — that is the gap to close.

## Hard rules
- **NEVER edit `crates/behaviorpass/src/**`, `crates/behaviorpass/tests/oracle.rs`,
  or any `Cargo.toml`.** They are FROZEN — `checks.sh` reverts any change. You
  ONLY add new files: `crates/behaviorpass/tests/adv_<topic>.rs`.
- Tests use the PUBLIC API only. `proptest = "1.11"` is available.
- Every test asserts EXACT values (`assert_eq!`), calls the real API, and
  **passes on the real code** (a green baseline is required to measure).
- If a test catches a real BUG — it *should* pass but **FAILS on the real
  code** — that is a **FINDING**. PRESERVE it, never revert or weaken it:
  put it in `crates/behaviorpass/tests/findings.rs`, keep the exact failing
  `assert_eq!`, and mark the test `#[ignore = "FINDING: <what breaks; expected
  vs actual>"]`. `#[ignore]` keeps it visible and runnable
  (`cargo test -- --ignored`) WITHOUT reverting the run or breaking the mutation
  baseline — so we can see it and fix the code. Then report it. Never turn a
  finding into a green test.

## Public API
`Actions{ sends: Vec<(MailAddr, Out)>, creates: Vec<New>, become_: Step<Ph,Exit> }`
(`Actions::{just,cont,stop,goto}`); `MailAddr(u64)`;
`Transcript{ sends, creates, exit }`; `Base<S,M,P,E,O=Never,N=Never>`
(`Base::new`, `.state()`); trait `Behavior` (assoc `Msg/Ph/Error/Outbound/Offspring`;
`async step(Envelope<Msg>) -> Result<Actions<..>, Error>`; `next_deadline()`);
`Envelope::{ User(M), Deadline, LinkDied{peer:u64,abnormal:bool}, ChildStopped{idx:usize,abnormal:bool} }`;
`run<B,C>(b, fastpass::Consumer<C,B::Msg>) -> Result<Transcript<B::Outbound,B::Offspring>, B::Error>`
(`B: Behavior<Ph=Never>`); `Deadlined`, `Watching`+`otp_propagation`,
`Stashing`+`StashRoute`, `Supervising`+`Child`, `Fsm`+`Move`, `Exit::{Normal,Collected,LinkDied(u64)}`.
Driver pattern:
`use fastpass::{Config, channel}; let (ctl,usr,rx)=channel::<Never,M>(Config::new(8)); let h=tokio::spawn(run(b,rx)); usr.send(m).await.unwrap(); drop(usr); drop(ctl); let t=h.await.unwrap().unwrap();`

## Test methods — use ALL of them (mirror the fastpass sibling)
- **Handcrafted edge/boundary**: empty inputs, budget 0, `MailAddr(0)`/`MailAddr(u64::MAX)`, single vs many, capacity.
- **Sequence/protocol + lifecycle**: multi-step interactions on ONE object; build → drive → drain → exit.
- **proptest — property**: random `Envelope` sequences asserted against an independent, obviously-correct hand model you fold by hand in the test (differential). Like fastpass `tests/property_suite.rs`. Ranges MUST include boundaries (0, 1, MAX-1, MAX; empty / max-length / max-length+1).
- **proptest — fuzz**: long randomized interleavings and large inputs to surface livelock (Stashing/Fsm replay), ordering (Transcript sends), and drain bounds. Like fastpass `tests/proptest_interleavings.rs`.

## Invariants to pin — attack every one with the methods above
- **Base**: floor emits no sends/creates for ANY input; framework events are no-ops.
- **driver**: `Transcript` records sends/creates in exact emission order; `Collected` iff full close, else the `Stop`'s exit; nothing folds after a Stop.
- **Deadlined**: `next_deadline` = min(own, inner); fires once then clears; forwards the rest.
- **Watching**: abnormal death → `Stop(LinkDied(peer))`; normal absorbed.
- **Stashing**: release drains the held SNAPSHOT (no livelock on all-restash); FIFO ahead of backlog; Stop mid-drain abandons the rest; delivered steps' effects accumulate in order; `next_deadline` forwards inner.
- **Fsm**: an `Err` transition never half-commits the phase (D3); defer → replay FIFO on a real phase change; the `Stop` arm is reached; re-defer snapshot bound.
- **Supervising**: abnormal-within-budget → exactly ONE `Offspring` create + slot alive + budget−1; exhausted/normal → zero creates + dead; `next_deadline` forwards inner.
- **Composition**: sends/creates pass unchanged through EVERY layer; the deadline min-fold surfaces through outer layers.

## Current survivors — start here (5 MISSED)
- `stashing.rs:60 drain_into -> Ok(())` — the release-drain's effect accumulation is unasserted (drive a Stashing whose inner emits sends, Release, assert the Transcript/Actions sends).
- `stashing.rs:118 Stashing::next_deadline -> None` — deadline forwarding through Stashing untested (compose `Stashing<Deadlined<Base>>`, assert `next_deadline`).
- `supervising.rs:108 Supervising::next_deadline -> None` — same for Supervising.
- `fsm.rs:123 delete arm Step::Stop(exit)` — Fsm `Stop` via `step` not exercised (a `Move::Stop` must yield `Actions.become_ == Stop`).
- `fsm.rs:124 guard changed -> true` — replay must happen only on an ACTUAL phase change (Goto to the SAME phase must NOT replay).

Kill these first, then keep hunting until no mutant survives.
