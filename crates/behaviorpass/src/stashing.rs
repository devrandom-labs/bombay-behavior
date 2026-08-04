//! `Stashing` — a plain `Behavior` (Agha-pure): no gate trait, no engine. It
//! keeps a buffer of stashed messages IN ITS OWN STATE. Stashing a message is
//! `become` a fuller version of itself; a release replays the buffer.
//!
//! Same replay-ordering knob as `phased.rs`: a release replays the held batch
//! *ahead of* the mailbox backlog, in this step. The *behind* variant is a
//! plain self-send at the driver (Agha `send`-to-self, back of queue).

use std::collections::VecDeque;

use crate::verdict::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Acted, Actions, Address, Behavior, Envelope, Fleet};

/// What a stashing behavior does with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    /// Hold for a later release.
    Stash,
    /// Deliver now.
    Deliver,
    /// Deliver now, then replay the whole held batch in this step.
    Release,
}

/// A `Behavior` that routes messages, holding the stashed ones in its own state
/// and replaying them on a release. The buffer holds the WHOLE envelope leg —
/// sender stamp included — so a replayed message arrives with its original
/// `from`.
pub struct Stashing<B: Behavior> {
    inner: B,
    route: fn(&B::Msg) -> StashRoute,
    held: VecDeque<(B::Addr, B::Msg)>,
}

impl<B: Behavior<Ph = Never>> Stashing<B> {
    /// Builds a stashing behavior with a message route.
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
        Self { inner, route, held: VecDeque::new() }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// How many messages are held (test observability).
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Replay the held batch AHEAD of the backlog, ACCUMULATING each delivered
    /// inner step's sends and creates into `acc`: re-route each — re-stashed
    /// messages return to `held` (snapshot bound, no livelock); a `Stop` sets
    /// `acc.become_` and re-extends `held` with the remaining batch, abandoning
    /// the rest.
    async fn drain_into(
        &mut self,
        acc: &mut Actions<B::Addr, Never, B::Outbound, B::Offspring>,
    ) -> Result<(), B::Error> {
        let mut batch: VecDeque<(B::Addr, B::Msg)> = self.held.drain(..).collect();
        while let Some((from, m)) = batch.pop_front() {
            match (self.route)(&m) {
                StashRoute::Stash => self.held.push_back((from, m)),
                StashRoute::Deliver | StashRoute::Release => {
                    let actions = self.inner.step(Envelope::User { from, msg: m }).await?;
                    acc.sends.extend(actions.sends);
                    acc.creates.extend(actions.creates);
                    if let Step::Stop(exit) = actions.become_ {
                        self.held.extend(batch);
                        acc.become_ = Step::Stop(exit);
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }
}

impl<B> Behavior for Stashing<B>
where
    B: Behavior<Ph = Never> + Send,
    B::Addr: Send,
    <B::Addr as Address>::Nonce: Send,
    B::Msg: Send,
    B::Outbound: Send,
    B::Offspring: Send,
{
    type Addr = B::Addr;
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = B::Offspring;
    async fn step(
        &mut self,
        ev: Envelope<B::Addr, B::Msg>,
    ) -> Acted<B::Addr, Never, B::Outbound, B::Offspring, B::Error> {
        let Envelope::User { from, msg: m } = ev else {
            return self.inner.step(ev).await;
        };
        match (self.route)(&m) {
            // Stash = `become` a fuller self: the message joins the buffer.
            StashRoute::Stash => {
                self.held.push_back((from, m));
                Ok(Actions::cont())
            }
            StashRoute::Deliver => self.inner.step(Envelope::User { from, msg: m }).await,
            StashRoute::Release => {
                let mut acc = self.inner.step(Envelope::User { from, msg: m }).await?;
                if matches!(acc.become_, Step::Stop(_)) {
                    return Ok(acc);
                }
                self.drain_into(&mut acc).await?;
                Ok(acc)
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }

    fn fleet(&self) -> Option<Fleet<Self::Addr, Self::Offspring>> {
        self.inner.fleet()
    }
}

#[cfg(test)]
mod tests {
    use super::StashRoute;
    use crate::behavior::{Actions, Behavior, Envelope};
    use crate::{Base, FnState, Fsm, MailAddr, Move, Stashing};
    use crate::verdict::Never;

    type Rec = Base<FnState<Vec<u64>, MailAddr, u64, Never, Never, &'static str>, Never, Never, &'static str>;

    fn recorder() -> Rec {
        Base::from_fn(Vec::<u64>::new(), |seen: &mut Vec<u64>, _from: MailAddr, id: u64| {
            seen.push(id);
            Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
        })
    }

    fn user(msg: u64) -> Envelope<MailAddr, u64> {
        Envelope::User { from: MailAddr(1), msg }
    }

    #[tokio::test]
    async fn stashing_holds_and_re_stashes_under_the_snapshot_bound() {
        let mut s = Stashing::new(recorder(), |&id| match id {
            0 => StashRoute::Release,
            n if n % 2 == 1 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });
        for id in [1_u64, 2, 3, 0, 4] {
            let _ = s.step(user(id)).await;
        }
        assert_eq!(s.inner().state().state, vec![2, 0, 4]);
        assert_eq!(s.held(), 2, "re-stashed messages land in held, not the batch");
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
    }

    /// `Stashing` (the buffer primitive) composes over an `Fsm` (a state machine
    /// built from core): `Stashing<Fsm>`, each with its own independent buffer.
    #[tokio::test]
    async fn stashing_composes_over_an_fsm_each_with_its_own_buffer() {
        let fsm = Fsm::new(Vec::<u64>::new(), Ph::Loading, |phase, seen: &mut Vec<u64>, m: &Msg| {
            Ok::<Move<Ph>, &'static str>(match (phase, m) {
                (Ph::Loading, Msg::Work(_)) => Move::Defer,
                (_, Msg::Work(id)) => {
                    seen.push(*id);
                    Move::Stay
                }
                (_, Msg::Promote) => Move::Goto(Ph::Ready),
            })
        });
        let mut stack = Stashing::new(fsm, |m: &Msg| match m {
            Msg::Work(id) if *id >= 100 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });
        let from = MailAddr(1);
        let _ = stack.step(Envelope::User { from, msg: Msg::Work(1) }).await; // Stashing delivers → Fsm defers
        let _ = stack.step(Envelope::User { from, msg: Msg::Work(100) }).await; // Stashing holds

        assert_eq!(stack.held(), 1, "the OUTER stashing buffer holds the big id");
        assert_eq!(stack.inner().held(), 1, "the INNER fsm buffer independently holds");

        let _ = stack.step(Envelope::User { from, msg: Msg::Promote }).await; // Fsm transitions + replays
        assert_eq!(stack.inner().state(), &vec![1], "fsm released on goto");
        assert_eq!(stack.inner().held(), 0, "fsm buffer drained");
        assert_eq!(stack.held(), 1, "outer still holds Work(100) — independent");
    }
}
