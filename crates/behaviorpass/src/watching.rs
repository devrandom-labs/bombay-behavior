//! `Watching` — a `Behavior` that reacts to a watched peer's death. It handles
//! [`Envelope::LinkDied`] with a policy and forwards every other event inward.

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Become, Behavior, Envelope, lift};

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

/// The default policy: propagate an abnormal linked death, absorb everything
/// else (`OtpPropagation`).
///
/// # Errors
/// Never — the propagation decision is pure; the signature matches the seat.
pub fn otp_propagation<B: Behavior>(
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
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Become<B::Ph>, B::Error> {
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
    use super::{Watching, otp_propagation};
    use crate::behavior::{Behavior, Envelope};
    use crate::{Base, Exit};
    use bombay::capability::{Never, Step};

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

    #[tokio::test]
    async fn watching_propagates_abnormal_death_and_absorbs_normal() {
        let mut w = Watching::new(recorder(), otp_propagation);
        assert!(
            matches!(
                w.step(Envelope::LinkDied { peer: 42, abnormal: true }).await,
                Ok(Step::Stop(Exit::LinkDied(42)))
            ),
            "an abnormal linked death propagates with the carried reason",
        );

        let mut w2 = Watching::new(recorder(), otp_propagation);
        assert!(matches!(
            w2.step(Envelope::LinkDied { peer: 42, abnormal: false }).await,
            Ok(Step::Continue)
        ));
        assert!(matches!(w2.step(Envelope::User(2)).await, Ok(Step::Continue)));
        assert_eq!(w2.inner().state(), &vec![2], "a normal death is absorbed; user forwards");
    }
}
