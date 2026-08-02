//! The DECOMPOSITION prototype (bombay card #298 → a #295/ADR-0028 finding):
//! `Phased` and `Stashing` are the SAME capability — a **stateful gated
//! buffer** — differing only in their routing policy. This module defines the
//! one `Gated<B, G>` capability and recovers `Phased`/`Stashing` as two
//! policies over it.
//!
//! What this dissolves (the accidental structure the tower model exposed):
//! - **The `Phased ⊥ Stashing/Deadlined` exclusion law** — Phased and Stashing
//!   are now the same capability, so they simply STACK (`Stashing<Phased<..>>`);
//!   there is nothing to forbid.
//! - **Phased's inner seats** (`NoDefer`/`Bounded` × `NoTimeout`/seat, the 9
//!   extra lattice points) — deferral is the buffer (a policy that never defers
//!   is `NoDefer`); the phase timeout is a composed `Deadlined`. The seats were
//!   bombay bundling three concerns into one monolith.

use std::collections::VecDeque;

use bombay::capability::{Deferred, Disposition, Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Behavior, Envelope};

/// What a gate policy decides for one user message.
pub enum Admit {
    /// Hand it to the inner behavior now.
    Deliver,
    /// Hold it in the buffer; re-classify on the next drain.
    Defer,
    /// Drop it by declaration.
    Ignore,
    /// Deliver now, then drain the whole held buffer (message-triggered
    /// release — the `Stashing` shape).
    Release,
}

/// A gated-buffer policy: classify a message against its own state, and react
/// to the inner handler's phase transition. The two core capabilities are two
/// impls; a third-party gate is a third impl.
pub trait Gate {
    /// The user-message type classified.
    type Msg;
    /// The phase menu the inner handler transitions over (`Never` = stateless).
    type Ph;
    /// Classify a message against current policy state.
    fn admit(&self, msg: &Self::Msg) -> Admit;
    /// The inner handler returned `Goto(to)`. Update policy state; return `true`
    /// if the transition should release + re-gate the held buffer. A stateless
    /// policy has `Ph = Never`, so this is never reached.
    fn advance(&mut self, to: Self::Ph) -> bool;
}

/// THE unified capability: an inner behavior + a held buffer + a gate policy.
/// Erases the inner become-menu (`Ph = Never` upward — the gate consumes it as
/// buffer releases).
pub struct Gated<B: Behavior, G> {
    inner: B,
    gate: G,
    held: VecDeque<B::Msg>,
}

impl<B: Behavior, G> Gated<B, G>
where
    G: Gate<Msg = B::Msg, Ph = B::Ph>,
{
    fn build(inner: B, gate: G) -> Self {
        Self { inner, gate, held: VecDeque::new() }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// How many messages are currently held (test observability).
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Run one event through the inner behavior; a `Goto` drives the gate's
    /// `advance` (D3: the commit is inside the `Ok`, so an `Err` can never
    /// half-transition). Returns whether the transition asks for a drain.
    async fn run_inner(&mut self, ev: Envelope<B::Msg>) -> Result<(Step<Never, Exit>, bool), B::Error> {
        Ok(match self.inner.step(ev).await? {
            Step::Continue => (Step::Continue, false),
            Step::Stop(exit) => (Step::Stop(exit), false),
            Step::Goto(to) => (Step::Continue, self.gate.advance(to)),
        })
    }

    /// Drain a snapshot of the held buffer, re-gating each message in the
    /// current policy state: `Ignore` drops, `Defer` re-holds (the snapshot
    /// bound — re-held messages never re-enter this batch), `Deliver`/`Release`
    /// fold. A mid-drain transition folds the freshly-held messages back in;
    /// a `Stop` abandons the rest.
    async fn drain(&mut self) -> Result<Step<Never, Exit>, B::Error> {
        let mut batch: VecDeque<B::Msg> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            match self.gate.admit(&m) {
                Admit::Ignore => {}
                Admit::Defer => self.held.push_back(m),
                Admit::Deliver | Admit::Release => {
                    let (verdict, transitioned) = self.run_inner(Envelope::User(m)).await?;
                    if let Step::Stop(exit) = verdict {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                    if transitioned {
                        batch.extend(self.held.drain(..));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B, G> Behavior for Gated<B, G>
where
    B: Behavior + Send,
    B::Msg: Send,
    B::Ph: Send,
    G: Gate<Msg = B::Msg, Ph = B::Ph> + Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        let Envelope::User(m) = ev else {
            // Framework events are not gated; a reaction that transitions still
            // releases the buffer via `advance`.
            let (verdict, transitioned) = self.run_inner(ev).await?;
            return match verdict {
                Step::Stop(exit) => Ok(Step::Stop(exit)),
                _ if transitioned => self.drain().await,
                _ => Ok(Step::Continue),
            };
        };
        match self.gate.admit(&m) {
            Admit::Ignore => Ok(Step::Continue),
            Admit::Defer => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            Admit::Deliver => {
                let (verdict, transitioned) = self.run_inner(Envelope::User(m)).await?;
                match verdict {
                    Step::Stop(exit) => Ok(Step::Stop(exit)),
                    _ if transitioned => self.drain().await,
                    _ => Ok(Step::Continue),
                }
            }
            Admit::Release => {
                if let (Step::Stop(exit), _) = self.run_inner(Envelope::User(m)).await? {
                    return Ok(Step::Stop(exit));
                }
                self.drain().await
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

// --------------------------------------------------------------- Phased ----

/// The phase-aware routing policy: a gate function of the current phase, plus
/// the phase state it transitions on `Goto`.
pub struct PhaseGate<B: Behavior> {
    phase: B::Ph,
    gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>,
}

impl<B: Behavior> Gate for PhaseGate<B>
where
    B::Ph: Copy + PartialEq,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    fn admit(&self, msg: &B::Msg) -> Admit {
        match (self.gate)(self.phase, msg) {
            Disposition::Deliver => Admit::Deliver,
            Disposition::Defer(Deferred) => Admit::Defer,
            Disposition::Ignore => Admit::Ignore,
        }
    }
    fn advance(&mut self, to: B::Ph) -> bool {
        let changed = to != self.phase;
        self.phase = to;
        changed
    }
}

/// A phased behavior: [`Gated`] with a [`PhaseGate`] policy — the deferral
/// buffer and the phase gate, unified.
pub type Phased<B> = Gated<B, PhaseGate<B>>;

impl<B: Behavior> Gated<B, PhaseGate<B>>
where
    B::Ph: Copy + PartialEq,
{
    /// Builds a phased layer in `initial` with a per-phase gate.
    pub fn new(
        inner: B,
        initial: B::Ph,
        gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>,
    ) -> Self {
        Gated::build(inner, PhaseGate { phase: initial, gate })
    }

    /// The committed phase (test observability).
    pub fn phase(&self) -> B::Ph {
        self.gate.phase
    }
}

// ------------------------------------------------------------- Stashing ----

/// The stash routing verdict for a user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    /// Hold for a later release.
    Stash,
    /// Deliver now.
    Deliver,
    /// Deliver now, then replay the whole held batch in this step.
    Release,
}

/// The message-only routing policy (stateless — `Ph = Never`, so it never
/// transitions).
pub struct StashGate<B: Behavior> {
    route: fn(&B::Msg) -> StashRoute,
}

impl<B: Behavior<Ph = Never>> Gate for StashGate<B> {
    type Msg = B::Msg;
    type Ph = Never;
    fn admit(&self, msg: &B::Msg) -> Admit {
        match (self.route)(msg) {
            StashRoute::Stash => Admit::Defer,
            StashRoute::Deliver => Admit::Deliver,
            StashRoute::Release => Admit::Release,
        }
    }
    fn advance(&mut self, to: Never) -> bool {
        match to {}
    }
}

/// A stashing behavior: [`Gated`] with a [`StashGate`] policy — the same
/// buffer, a message-only route.
pub type Stashing<B> = Gated<B, StashGate<B>>;

impl<B: Behavior<Ph = Never>> Gated<B, StashGate<B>> {
    /// Builds a stashing layer with a message route.
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
        Gated::build(inner, StashGate { route })
    }
}

#[cfg(test)]
mod tests {
    use super::{Phased, StashRoute, Stashing};
    use crate::Base;
    use crate::Exit;
    use crate::behavior::{Behavior, Envelope};
    use bombay::capability::{Deferred, Disposition, Never, Step};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
    }

    /// THE decomposition payoff: `Stashing<Phased<Base>>` — a stashing layer
    /// over a phased layer — TYPE-CHECKS and RUNS. bombay's `Phased ⊥ Stashing`
    /// exclusion law is dissolved: they are the same `Gated` capability, so
    /// they simply STACK, each with its OWN independent buffer.
    #[tokio::test]
    async fn phased_and_stashing_compose_with_independent_buffers() {
        let base = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, m: Msg| match m {
            Msg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            Msg::Promote => Ok(Step::Goto(Ph::Ready)),
        });
        // inner: defers Work while Loading; outer: stashes ids >= 100.
        let phased = Phased::new(base, Ph::Loading, |ph, m| match (ph, m) {
            (Ph::Loading, Msg::Work(_)) => Disposition::Defer(Deferred),
            _ => Disposition::Deliver,
        });
        let mut stack = Stashing::new(phased, |m: &Msg| match m {
            Msg::Work(id) if *id >= 100 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });

        // Work(1): outer delivers → inner Phased defers it (Loading).
        let _ = stack.step(Envelope::User(Msg::Work(1))).await;
        // Work(100): outer Stashing holds it (never reaches the inner).
        let _ = stack.step(Envelope::User(Msg::Work(100))).await;

        assert_eq!(stack.held(), 1, "the OUTER stashing buffer holds the big id");
        assert_eq!(
            stack.inner().held(),
            1,
            "the INNER phased buffer independently holds the deferred Work",
        );

        // Promote reaches the inner (outer delivers it), transitioning to
        // Ready and releasing the inner's deferred batch — Work(1) delivered.
        let _ = stack.step(Envelope::User(Msg::Promote)).await;
        assert_eq!(stack.inner().inner().state(), &vec![1], "inner released on goto");
        assert_eq!(stack.inner().held(), 0, "inner buffer drained");
        assert_eq!(stack.held(), 1, "outer buffer still holds Work(100) — independent");
    }

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

    /// Stashing: a release delivers its trigger then drains the held batch;
    /// re-stashed messages return to held (the snapshot bound — no livelock).
    #[tokio::test]
    async fn stashing_holds_and_re_stashes_under_the_snapshot_bound() {
        let mut s = Stashing::new(recorder(), |&id| match id {
            0 => StashRoute::Release,
            n if n % 2 == 1 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });
        for id in [1_u64, 2, 3, 0, 4] {
            let _ = s.step(Envelope::User(id)).await;
        }
        assert_eq!(s.inner().state(), &vec![2, 0, 4]);
        assert_eq!(s.held(), 2, "re-stashed messages land in held, not the batch");
    }

    enum FsmMsg {
        Work(u64),
        Promote,
        Quit,
    }

    /// Phased: work defers in Loading; the promotion releases the deferred
    /// batch FIFO within the goto step, ahead of the backlog.
    #[tokio::test]
    async fn phased_releases_the_deferred_batch_fifo_on_goto() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, msg: FsmMsg| match msg {
            FsmMsg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            FsmMsg::Promote => Ok(Step::Goto(Ph::Ready)),
            FsmMsg::Quit => Ok(Step::Stop(Exit::Normal)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |ph, msg| match (ph, msg) {
            (Ph::Loading, FsmMsg::Work(_)) => Disposition::Defer(Deferred),
            _ => Disposition::Deliver,
        });
        for m in [
            FsmMsg::Work(1),
            FsmMsg::Work(2),
            FsmMsg::Promote,
            FsmMsg::Work(3),
            FsmMsg::Quit,
        ] {
            let _ = p.step(Envelope::User(m)).await;
        }
        assert_eq!(p.inner().state(), &vec![1, 2, 3], "batch replays FIFO inside the goto");
        assert_eq!(p.phase(), Ph::Ready);
    }

    /// D3: a failing handler never half-switches the phase.
    #[tokio::test]
    async fn phased_never_commits_a_failed_handlers_goto() {
        let inner = Base::new((), |(): &mut (), msg: FsmMsg| match msg {
            FsmMsg::Work(_) => Err("bang"),
            _ => Ok::<Step<Ph, Exit>, &'static str>(Step::Goto(Ph::Ready)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |_, _| Disposition::Deliver);
        assert_eq!(p.step(Envelope::User(FsmMsg::Work(1))).await, Err("bang"));
        assert_eq!(p.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    }
}
