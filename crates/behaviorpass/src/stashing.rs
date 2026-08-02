//! `Stashing` — a plain `Behavior` (Agha-pure): no gate trait, no engine. It
//! keeps a buffer of stashed messages IN ITS OWN STATE. Stashing a message is
//! `become` a fuller version of itself; a release replays the buffer.
//!
//! Same replay-ordering knob as `phased.rs`: a release replays the held batch
//! *ahead of* the mailbox backlog, in this step. The *behind* variant is a
//! plain self-send at the driver (Agha `send`-to-self, back of queue).

use std::collections::VecDeque;

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Become, Behavior, Envelope};

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
/// and replaying them on a release.
pub struct Stashing<B: Behavior> {
    inner: B,
    route: fn(&B::Msg) -> StashRoute,
    held: VecDeque<B::Msg>,
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

    /// Replay the held batch AHEAD of the backlog: re-route each — re-stashed
    /// messages return to `held` (snapshot bound, no livelock); a `Stop`
    /// abandons the rest.
    async fn drain(&mut self) -> Result<Become<Never>, B::Error> {
        let mut batch: VecDeque<B::Msg> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            match (self.route)(&m) {
                StashRoute::Stash => self.held.push_back(m),
                StashRoute::Deliver | StashRoute::Release => {
                    if let Step::Stop(exit) = self.inner.step(Envelope::User(m)).await? {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B> Behavior for Stashing<B>
where
    B: Behavior<Ph = Never> + Send,
    B::Msg: Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Become<Never>, B::Error> {
        let Envelope::User(m) = ev else {
            return self.inner.step(ev).await;
        };
        match (self.route)(&m) {
            // Stash = `become` a fuller self: the message joins the buffer.
            StashRoute::Stash => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            StashRoute::Deliver => self.inner.step(Envelope::User(m)).await,
            StashRoute::Release => {
                if let Step::Stop(exit) = self.inner.step(Envelope::User(m)).await? {
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

#[cfg(test)]
mod tests {
    use super::StashRoute;
    use crate::behavior::{Behavior, Envelope};
    use crate::{Admit, Base, Exit, Phased, Stashing};
    use bombay::capability::{Never, Step};

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
    }

    /// `Phased` and `Stashing` are two plain behaviors now — no shared engine —
    /// and they still STACK freely: `Stashing<Phased<Base>>`, each holding its
    /// own buffer. bombay's `Phased ⊥ Stashing` exclusion law stays dissolved.
    #[tokio::test]
    async fn phased_and_stashing_compose_with_independent_buffers() {
        let base = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, m: Msg| match m {
            Msg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            Msg::Promote => Ok(Step::Goto(Ph::Ready)),
        });
        let phased = Phased::new(base, Ph::Loading, |ph, m| match (ph, m) {
            (Ph::Loading, Msg::Work(_)) => Admit::Defer,
            _ => Admit::Deliver,
        });
        let mut stack = Stashing::new(phased, |m: &Msg| match m {
            Msg::Work(id) if *id >= 100 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });

        let _ = stack.step(Envelope::User(Msg::Work(1))).await; // inner Phased defers
        let _ = stack.step(Envelope::User(Msg::Work(100))).await; // outer Stashing holds

        assert_eq!(stack.held(), 1, "the OUTER stashing buffer holds the big id");
        assert_eq!(stack.inner().held(), 1, "the INNER phased buffer independently holds");

        let _ = stack.step(Envelope::User(Msg::Promote)).await; // inner transitions + replays
        assert_eq!(stack.inner().inner().state(), &vec![1], "inner released on goto");
        assert_eq!(stack.inner().held(), 0, "inner buffer drained");
        assert_eq!(stack.held(), 1, "outer still holds Work(100) — independent");
    }
}
