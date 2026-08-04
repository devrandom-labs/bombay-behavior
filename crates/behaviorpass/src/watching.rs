//! `Watching` — a `Behavior` that reacts to a watched peer's death. It handles
//! [`Envelope::LinkDied`] with a policy and forwards every other event inward.

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Acted, Become, Behavior, Envelope, lift};

/// The reaction a link-death runs: the inner behavior plus the dead peer's id
/// and abnormal flag, returning a verdict on the erased menu.
pub type LinkReaction<B> =
    fn(&mut B, u64, bool) -> Result<Become<Never>, <B as Behavior>::Error>;

/// A `Behavior` that reacts to a linked peer's death over its inner behavior.
pub struct Watching<B: Behavior> {
    inner: B,
    on_link_died: LinkReaction<B>,
}

impl<B: Behavior> Watching<B> {
    /// Builds the layer over `inner` with a death policy.
    pub fn new(inner: B, on_link_died: LinkReaction<B>) -> Self {
        Self { inner, on_link_died }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

/// The default link-death policy: a peer's ABNORMAL death stops this actor
/// with [`Exit::LinkDied`] carrying the dead peer (the death propagates down
/// the link — OTP's link semantics); a normal death is absorbed.
///
/// # Errors
/// Never — the propagation decision is pure; the signature matches the seat.
pub fn stop_on_abnormal_death<B: Behavior>(
    _: &mut B,
    peer: u64,
    abnormal: bool,
) -> Result<Become<Never>, B::Error> {
    if abnormal {
        Ok(Step::Stop(Exit::LinkDied(peer)))
    } else {
        Ok(Step::Continue)
    }
}

impl<B> Behavior for Watching<B>
where
    B: Behavior + Send,
    B::Msg: Send,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = B::Offspring;
    async fn step(
        &mut self,
        ev: Envelope<B::Msg>,
    ) -> Acted<B::Ph, B::Outbound, B::Offspring, B::Error> {
        match ev {
            Envelope::LinkDied { peer, abnormal } => {
                Ok(lift((self.on_link_died)(&mut self.inner, peer, abnormal)?))
            }
            other => self.inner.step(other).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::{Watching, stop_on_abnormal_death};
    use crate::behavior::{Actions, Behavior, Envelope};
    use crate::{Base, Exit};
    use bombay::capability::{Never, Step};

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        })
    }

    #[tokio::test]
    async fn watching_propagates_abnormal_death_and_absorbs_normal() {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        assert!(
            matches!(
                w.step(Envelope::LinkDied { peer: 42, abnormal: true }).await.unwrap().become_,
                Step::Stop(Exit::LinkDied(42))
            ),
            "an abnormal linked death propagates with the carried reason",
        );

        let mut w2 = Watching::new(recorder(), stop_on_abnormal_death);
        assert!(matches!(
            w2.step(Envelope::LinkDied { peer: 42, abnormal: false }).await.unwrap().become_,
            Step::Continue
        ));
        assert!(matches!(w2.step(Envelope::User(2)).await.unwrap().become_, Step::Continue));
        assert_eq!(w2.inner().state(), &vec![2], "a normal death is absorbed; user forwards");
    }
}
