//! `Stashing` — a `Behavior`: the gated-buffer engine with a message-only
//! routing policy. It routes each message (stash / deliver / release), holds
//! the stashed ones, and drains the buffer on a release.

use bombay::capability::Never;

use crate::behavior::Behavior;
use crate::gated::{Admit, Gate, Gated};

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

/// The message-only routing policy (stateless — `Ph = Never`, never transitions).
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

/// A stashing behavior: the gated buffer with a [`StashGate`].
pub type Stashing<B> = Gated<B, StashGate<B>>;

impl<B: Behavior<Ph = Never>> Gated<B, StashGate<B>> {
    /// Builds a stashing behavior with a message route.
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
        Gated::build(inner, StashGate { route })
    }
}

#[cfg(test)]
mod tests {
    use super::StashRoute;
    use crate::behavior::{Behavior, Envelope};
    use crate::{Base, Exit, Phased, Stashing};
    use bombay::capability::{Deferred, Disposition, Never, Step};

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

    /// A release delivers its trigger then drains the held batch; re-stashed
    /// messages return to held (the snapshot bound — no livelock).
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

    /// THE decomposition payoff: `Stashing<Phased<Base>>` — a stashing behavior
    /// over a phased behavior — TYPE-CHECKS and RUNS. bombay's
    /// `Phased ⊥ Stashing` exclusion law is DISSOLVED: they are the same
    /// engine, so they simply STACK, each with its own independent buffer.
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
            (Ph::Loading, Msg::Work(_)) => Disposition::Defer(Deferred),
            _ => Disposition::Deliver,
        });
        let mut stack = Stashing::new(phased, |m: &Msg| match m {
            Msg::Work(id) if *id >= 100 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });

        let _ = stack.step(Envelope::User(Msg::Work(1))).await; // → inner Phased defers
        let _ = stack.step(Envelope::User(Msg::Work(100))).await; // → outer Stashing holds

        assert_eq!(stack.held(), 1, "the OUTER stashing buffer holds the big id");
        assert_eq!(stack.inner().held(), 1, "the INNER phased buffer independently holds");

        let _ = stack.step(Envelope::User(Msg::Promote)).await; // → inner transitions + releases
        assert_eq!(stack.inner().inner().state(), &vec![1], "inner released on goto");
        assert_eq!(stack.inner().held(), 0, "inner buffer drained");
        assert_eq!(stack.held(), 1, "outer still holds Work(100) — independent");
    }
}
