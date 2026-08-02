//! `Fsm` — a state machine built ENTIRELY from core. This is the `gen_statem`
//! ergonomics without the weight: it is NOT a core capability, it's a thin,
//! transparent helper — a state, a current phase, a transition function, and a
//! held buffer for messages the current phase isn't ready for (that buffer is
//! the `Stash` primitive, which stays in core). Read it and you see it's just
//! `Behavior` + a buffer; in bombay this pattern IS the nexus aggregate.
//!
//! What left core with it: `Phased` and its `Admit` gate. Phases are the FSM's
//! (aggregate's) concern, so the "defer / goto / stay / stop" vocabulary lives
//! HERE, in the FSM's own [`Move`], not in the core `Become`.

use std::collections::VecDeque;

use bombay::capability::{Never, Step};

use crate::Exit;
use crate::behavior::{Acted, Actions, Behavior, Envelope};

/// One step of a state machine's transition function.
pub enum Move<P> {
    /// Handled; stay in the current phase.
    Stay,
    /// Transition to another phase — replays the held batch in the new phase.
    Goto(P),
    /// Not ready in this phase — hold the message (stash it).
    Defer,
    /// The machine is done.
    Stop,
}

/// A state machine over `S` with phase menu `P` and message `M`. Built from
/// core: it IS a [`Behavior`] (erasing its phase to `Never` upward). The
/// held buffer replays AHEAD of the backlog on a phase change (the `Stash`
/// primitive's one knob).
pub struct Fsm<S, M, P, E> {
    state: S,
    phase: P,
    on: fn(P, &mut S, &M) -> Result<Move<P>, E>,
    held: VecDeque<M>,
}

impl<S, M, P: Copy + PartialEq, E> Fsm<S, M, P, E> {
    /// Builds a state machine in `phase` with a transition function.
    pub fn new(state: S, phase: P, on: fn(P, &mut S, &M) -> Result<Move<P>, E>) -> Self {
        Self { state, phase, on, held: VecDeque::new() }
    }

    /// The accumulated state (test observability).
    pub fn state(&self) -> &S {
        &self.state
    }

    /// The current phase (test observability).
    pub fn phase(&self) -> P {
        self.phase
    }

    /// How many messages are held (test observability).
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Run one OWNED message through the transition function — the machine owns
    /// `m`, so `Defer` buffers it with no clone. A `Goto` commits the phase
    /// (inside the `Ok`, so an `Err` can never half-switch). Returns whether the
    /// phase changed (which asks for a replay). Non-recursive on purpose:
    /// `drain` drives the replay, so the two async fns never call each other in
    /// a cycle (which would need boxing).
    fn advance(&mut self, m: M) -> Result<(Step<Never, Exit>, bool), E> {
        Ok(match (self.on)(self.phase, &mut self.state, &m)? {
            Move::Stay => (Step::Continue, false),
            Move::Defer => {
                self.held.push_back(m);
                (Step::Continue, false)
            }
            Move::Stop => (Step::Stop(Exit::Normal), false),
            Move::Goto(next) => {
                let changed = next != self.phase;
                self.phase = next;
                (Step::Continue, changed)
            }
        })
    }

    /// Replay the held batch in the (new) phase — re-run each: `Defer` re-holds
    /// (snapshot bound, no livelock), the rest fold; a mid-replay transition
    /// folds fresh holds back in; a `Stop` abandons the rest.
    fn drain(&mut self) -> Result<Step<Never, Exit>, E> {
        let mut batch: VecDeque<M> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            let (verdict, changed) = self.advance(m)?;
            if let Step::Stop(exit) = verdict {
                self.held.extend(batch);
                return Ok(Step::Stop(exit));
            }
            if changed {
                batch.extend(self.held.drain(..));
            }
        }
        Ok(Step::Continue)
    }
}

impl<S, M, P, E> Behavior for Fsm<S, M, P, E>
where
    S: Send,
    M: Send,
    P: Copy + PartialEq + Send,
    E: Send,
{
    type Msg = M;
    type Ph = Never;
    type Error = E;
    type Outbound = Never;
    type Offspring = Never;
    async fn step(&mut self, ev: Envelope<M>) -> Acted<Never, Never, Never, E> {
        let Envelope::User(m) = ev else {
            // A framework event is a no-op for a plain state machine.
            return Ok(Actions::cont());
        };
        let (verdict, changed) = self.advance(m)?;
        match verdict {
            Step::Stop(exit) => Ok(Actions::stop(exit)),
            _ if changed => Ok(Actions::just(self.drain()?)),
            _ => Ok(Actions::cont()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Fsm, Move};
    use crate::behavior::{Behavior, Envelope};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
        Quit,
    }

    /// The classic phased-actor scenario, now as a plain state machine built
    /// from core: `Work` defers while `Loading`; `Promote` transitions to
    /// `Ready` and replays the deferred batch FIFO, ahead of the backlog.
    #[tokio::test]
    async fn fsm_defers_then_replays_on_transition() {
        let mut fsm = Fsm::new(
            Vec::<u64>::new(),
            Ph::Loading,
            |phase, seen: &mut Vec<u64>, msg: &Msg| {
                Ok::<Move<Ph>, &'static str>(match (phase, msg) {
                    (Ph::Loading, Msg::Work(_)) => Move::Defer,
                    (_, Msg::Work(id)) => {
                        seen.push(*id);
                        Move::Stay
                    }
                    (_, Msg::Promote) => Move::Goto(Ph::Ready),
                    (_, Msg::Quit) => Move::Stop,
                })
            },
        );
        for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
            let _ = fsm.step(Envelope::User(m)).await;
        }
        assert_eq!(fsm.state(), &vec![1, 2, 3], "deferred batch replays FIFO ahead of the backlog");
        assert_eq!(fsm.phase(), Ph::Ready);
        assert_eq!(fsm.held(), 0);
    }

    /// D3: a transition function that errors never half-switches the phase.
    #[tokio::test]
    async fn fsm_never_commits_a_failed_transition() {
        let mut fsm = Fsm::new((), Ph::Loading, |_phase, (): &mut (), msg: &Msg| match msg {
            Msg::Work(_) => Err("bang"),
            _ => Ok::<Move<Ph>, &'static str>(Move::Goto(Ph::Ready)),
        });
        assert_eq!(
            fsm.step(Envelope::User(Msg::Work(1))).await.err(),
            Some("bang"),
            "the failing transition surfaces its error",
        );
        assert_eq!(fsm.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    }
}
