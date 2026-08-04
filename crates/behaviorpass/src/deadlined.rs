//! `Deadlined` — a `Behavior` that owns the single-shot deadline. It provides
//! a wake time via [`Behavior::next_deadline`], reacts when [`Envelope::Deadline`]
//! fires, and forwards every other event to the inner behavior.

use tokio::time::Instant;

use crate::behavior::{Acted, Address, Become, Behavior, Envelope, Fleet, lift};

/// The reaction a deadline fire runs: mutates the inner behavior, returns a
/// verdict on the erased menu (`Never` — a deadline reaction cannot `Goto`).
pub type DeadlineReaction<B> =
    fn(&mut B) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

/// A `Behavior` that adds a single-shot deadline over its inner behavior.
pub struct Deadlined<B: Behavior> {
    inner: B,
    due: Option<Instant>,
    on_deadline: DeadlineReaction<B>,
}

impl<B: Behavior> Deadlined<B> {
    /// Builds the layer with an initial deadline slot and its reaction.
    pub fn new(inner: B, due: Option<Instant>, on_deadline: DeadlineReaction<B>) -> Self {
        Self { inner, due, on_deadline }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B> Behavior for Deadlined<B>
where
    B: Behavior + Send,
    B::Addr: Send,
    <B::Addr as Address>::Nonce: Send,
    B::Msg: Send,
{
    type Addr = B::Addr;
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = B::Offspring;
    async fn step(
        &mut self,
        ev: Envelope<B::Addr, B::Msg>,
    ) -> Acted<B::Addr, B::Ph, B::Outbound, B::Offspring, B::Error> {
        match ev {
            Envelope::Deadline => {
                self.due = None; // fires once per armed value
                Ok(lift((self.on_deadline)(&mut self.inner)?))
            }
            other => self.inner.step(other).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        // The min-fold law (ADR-0030): the earliest of this slot and any inner
        // deadline arms the one timer.
        match (self.due, self.inner.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn fleet(&self) -> Option<Fleet<Self>> {
        self.inner.fleet()
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::Deadlined;
    use crate::behavior::{Actions, Behavior, Envelope};
    use crate::{Base, Exit, MailAddr};
    use crate::verdict::{Never, Step};
    use tokio::time::Instant;

    #[tokio::test]
    async fn deadlined_routes_the_fire_forwards_the_rest_and_arms_once() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
        });
        let due = Instant::now() + Duration::from_secs(5);
        let mut d = Deadlined::new(inner, Some(due), |_inner| Ok(Step::Stop(Exit::Normal)));

        assert_eq!(d.next_deadline(), Some(due), "the declared slot arms the timer");
        assert!(matches!(
            d.step(Envelope::User { from: MailAddr(1), msg: 7 }).await.unwrap().become_,
            Step::Continue
        ));
        assert_eq!(d.inner().state(), &vec![7], "non-deadline events forward inward");
        assert!(
            matches!(d.step(Envelope::Deadline).await.unwrap().become_, Step::Stop(Exit::Normal)),
            "the reaction's verdict rides out",
        );
        assert_eq!(d.next_deadline(), None, "fires once — the slot clears after firing");
    }
}
