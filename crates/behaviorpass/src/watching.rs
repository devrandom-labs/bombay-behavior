//! `Watching` — a `Behavior` that reacts to a watched peer's death. It handles
//! [`Envelope::LinkDied`] with a policy and forwards every other event inward.

use crate::verdict::Step;
use tokio::time::Instant;

use crate::behavior::{Acted, Address, Become, Behavior, Envelope, Fleet, lift};
use crate::{Crash, Exit};

/// The reaction a link-death runs: the inner behavior plus the dead peer's
/// address and the death OUTCOME (classification is the reaction's pure
/// policy — never a driver-pre-digested flag), returning a verdict on the
/// erased menu.
pub type LinkReaction<B> = fn(
    &mut B,
    <B as Behavior>::Addr,
    Result<Exit<<B as Behavior>::Addr>, Crash>,
) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

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
/// the link — OTP's link semantics); a normal death (the `{Normal,
/// Collected}` subset) is absorbed.
///
/// # Errors
/// Never — the propagation decision is pure; the signature matches the seat.
pub fn stop_on_abnormal_death<B: Behavior>(
    _: &mut B,
    peer: B::Addr,
    outcome: Result<Exit<B::Addr>, Crash>,
) -> Result<Become<B::Addr>, B::Error> {
    Ok(match outcome {
        Ok(Exit::Normal | Exit::Collected) => Step::Continue,
        Ok(Exit::LinkDied(_)) | Err(_) => Step::Stop(Exit::LinkDied(peer)),
    })
}

impl<B> Behavior for Watching<B>
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
            Envelope::LinkDied { peer, outcome } => {
                Ok(lift((self.on_link_died)(&mut self.inner, peer, outcome)?))
            }
            other => self.inner.step(other).await,
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
    use super::{Watching, stop_on_abnormal_death};
    use crate::behavior::{Actions, Behavior, Envelope};
    use crate::{Base, Crash, Exit, MailAddr};
    use crate::verdict::{Never, Step};

    fn recorder() -> Base<MailAddr, Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
        })
    }

    #[tokio::test]
    async fn watching_propagates_abnormal_death_and_absorbs_normal() {
        let mut w = Watching::new(recorder(), stop_on_abnormal_death);
        assert!(
            matches!(
                w.step(Envelope::LinkDied { peer: MailAddr(42), outcome: Err(Crash::Failed) })
                    .await
                    .unwrap()
                    .become_,
                Step::Stop(Exit::LinkDied(MailAddr(42)))
            ),
            "an abnormal linked death propagates with the carried reason",
        );

        let mut w2 = Watching::new(recorder(), stop_on_abnormal_death);
        assert!(matches!(
            w2.step(Envelope::LinkDied { peer: MailAddr(42), outcome: Ok(Exit::Normal) })
                .await
                .unwrap()
                .become_,
            Step::Continue
        ));
        assert!(matches!(
            w2.step(Envelope::User { from: MailAddr(1), msg: 2 }).await.unwrap().become_,
            Step::Continue
        ));
        assert_eq!(w2.inner().state(), &vec![2], "a normal death is absorbed; user forwards");
    }
}
