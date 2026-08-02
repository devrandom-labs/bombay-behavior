//! `Phased` — a plain `Behavior` (Agha-pure): no gate trait, no engine. It keeps
//! a buffer of deferred messages IN ITS OWN STATE. Deferring a message is just
//! `become` a fuller version of itself (push to the buffer); a phase change
//! releases the batch (a replay).
//!
//! **The one knob — replay ordering.** When a phase change releases the held
//! batch, the messages are replayed *ahead of* the mailbox backlog, in this
//! same step (`drain` below). Agha's `send`-to-self would put them *behind*
//! instead. We pick ahead (bombay ADR-0022 / `gen_statem` postpone).

use std::collections::VecDeque;

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Become, Behavior, Envelope};

/// What a phased behavior does with a message in the current phase — its OWN
/// admission vocabulary (no longer borrowed from bombay's `Disposition`; part
/// of this crate's grammar).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admit {
    /// Hand it to the inner behavior now.
    Deliver,
    /// Hold it in the buffer — `become` a fuller self; re-gate on the next replay.
    Defer,
    /// Drop it by declaration.
    Ignore,
}

/// A `Behavior` that gates messages by phase, holding the deferred ones in its
/// own state and replaying them when the inner handler's `Goto` moves the phase.
pub struct Phased<B: Behavior> {
    inner: B,
    phase: B::Ph,
    gate: fn(B::Ph, &B::Msg) -> Admit,
    held: VecDeque<B::Msg>,
}

impl<B: Behavior> Phased<B>
where
    B::Ph: Copy + PartialEq,
{
    /// Builds a phased behavior in `initial` with a per-phase gate.
    pub fn new(inner: B, initial: B::Ph, gate: fn(B::Ph, &B::Msg) -> Admit) -> Self {
        Self { inner, phase: initial, gate, held: VecDeque::new() }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// The committed phase (test observability).
    pub fn phase(&self) -> B::Ph {
        self.phase
    }

    /// How many messages are held (test observability).
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Run one event through the inner behavior; a `Goto` commits the phase (D3:
    /// inside the `Ok`, so an `Err` can never half-switch). Returns whether the
    /// phase actually changed (which asks for a replay).
    async fn run_inner(
        &mut self,
        ev: Envelope<B::Msg>,
    ) -> Result<(Become<Never>, bool), B::Error> {
        Ok(match self.inner.step(ev).await? {
            Step::Continue => (Step::Continue, false),
            Step::Stop(exit) => (Step::Stop(exit), false),
            Step::Goto(next) => {
                let changed = next != self.phase;
                self.phase = next;
                (Step::Continue, changed)
            }
        })
    }

    /// Replay the held batch AHEAD of the backlog (the knob): re-gate each in
    /// the new phase — `Ignore` drops, `Defer` re-holds (snapshot bound),
    /// `Deliver` folds. A mid-replay transition folds fresh holds back in.
    async fn drain(&mut self) -> Result<Become<Never>, B::Error> {
        let mut batch: VecDeque<B::Msg> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            match (self.gate)(self.phase, &m) {
                Admit::Ignore => {}
                Admit::Defer => self.held.push_back(m),
                Admit::Deliver => {
                    let (verdict, changed) = self.run_inner(Envelope::User(m)).await?;
                    if let Step::Stop(exit) = verdict {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                    if changed {
                        batch.extend(self.held.drain(..));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }

    async fn deliver(&mut self, ev: Envelope<B::Msg>) -> Result<Become<Never>, B::Error> {
        let (verdict, changed) = self.run_inner(ev).await?;
        match verdict {
            Step::Stop(exit) => Ok(Step::Stop(exit)),
            _ if changed => self.drain().await,
            _ => Ok(Step::Continue),
        }
    }
}

impl<B> Behavior for Phased<B>
where
    B: Behavior + Send,
    B::Msg: Send,
    B::Ph: Copy + PartialEq + Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Become<Never>, B::Error> {
        let Envelope::User(m) = ev else {
            // Framework events are not gated; a reaction that transitions replays.
            return self.deliver(ev).await;
        };
        match (self.gate)(self.phase, &m) {
            Admit::Ignore => Ok(Step::Continue),
            // Defer = `become` a fuller self: the message joins the buffer.
            Admit::Defer => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            Admit::Deliver => self.deliver(Envelope::User(m)).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::{Admit, Phased};
    use crate::behavior::{Behavior, Envelope};
    use crate::{Base, Exit};
    use bombay::capability::Step;

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

    #[tokio::test]
    async fn phased_releases_the_deferred_batch_fifo_on_goto() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, msg: Msg| match msg {
            Msg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            Msg::Promote => Ok(Step::Goto(Ph::Ready)),
            Msg::Quit => Ok(Step::Stop(Exit::Normal)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |ph, msg| match (ph, msg) {
            (Ph::Loading, Msg::Work(_)) => Admit::Defer,
            _ => Admit::Deliver,
        });
        for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
            let _ = p.step(Envelope::User(m)).await;
        }
        assert_eq!(p.inner().state(), &vec![1, 2, 3], "batch replays FIFO ahead of the backlog");
        assert_eq!(p.phase(), Ph::Ready);
    }

    #[tokio::test]
    async fn phased_never_commits_a_failed_handlers_goto() {
        let inner = Base::new((), |(): &mut (), msg: Msg| match msg {
            Msg::Work(_) => Err("bang"),
            _ => Ok::<Step<Ph, Exit>, &'static str>(Step::Goto(Ph::Ready)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |_, _| Admit::Deliver);
        assert_eq!(p.step(Envelope::User(Msg::Work(1))).await, Err("bang"));
        assert_eq!(p.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    }
}
