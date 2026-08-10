//! The explicit result of one actor behavior transition.

use super::sending::SendAlgebra;
use crate::Exit;
use crate::actor::{Address, BirthMode, Create};
use crate::verdict::{Never, Step};

pub type Become<A, Ph = Never> = Step<Ph, Exit<A>>;

/// Bombay's typed realization of the actor transition effects: communications,
/// fresh actor creation, and next behavior or termination.
///
/// An interpreter resolves every fresh creation in `creates` before
/// interpreting any ordinary delivery or [`crate::ServiceSends`] request in
/// `sends` from this value. A successful resolution installs and binds the
/// child; a rejected resolution binds nothing. This ordering lets a same-action
/// [`crate::ObserveCreation`] request return the committed result rather than
/// the behavior's intent. Creation order is vector order, and each concrete
/// send lane retains its own order; this contract does not impose an order
/// between independent lanes of a [`crate::SendProduct`]. Constructing a value
/// remains pure.
pub struct Actions<A: Address, Ph, Sends, Birth: BirthMode> {
    pub sends: Sends,
    pub creates: Vec<Create<A, Birth::Child>>,
    pub become_: Become<A, Ph>,
}

impl<A: Address, Ph, Sends, Birth: BirthMode> Actions<A, Ph, Sends, Birth> {
    /// Transform only the send algebra, preserving creation order and the
    /// next-behavior verdict exactly.
    #[must_use]
    pub fn map_sends<Mapped>(
        self,
        map: impl FnOnce(Sends) -> Mapped,
    ) -> Actions<A, Ph, Mapped, Birth> {
        Actions {
            sends: map(self.sends),
            creates: self.creates,
            become_: self.become_,
        }
    }

    /// Transform only the next-behavior verdict, preserving sends and
    /// creation order exactly.
    #[must_use]
    pub fn map_become<NextPh>(
        self,
        map: impl FnOnce(Become<A, Ph>) -> Become<A, NextPh>,
    ) -> Actions<A, NextPh, Sends, Birth> {
        Actions {
            sends: self.sends,
            creates: self.creates,
            become_: map(self.become_),
        }
    }
}

impl<A: Address, Ph, Sends: SendAlgebra, Birth: BirthMode> Actions<A, Ph, Sends, Birth> {
    #[must_use]
    pub const fn new(
        sends: Sends,
        creates: Vec<Create<A, Birth::Child>>,
        become_: Become<A, Ph>,
    ) -> Self {
        Self {
            sends,
            creates,
            become_,
        }
    }

    #[must_use]
    pub fn just(become_: Become<A, Ph>) -> Self {
        Self {
            sends: Sends::empty(),
            creates: Vec::new(),
            become_,
        }
    }

    #[must_use]
    pub fn cont() -> Self {
        Self::just(Step::Continue)
    }
    #[must_use]
    pub fn stop(exit: Exit<A>) -> Self {
        Self::just(Step::Stop(exit))
    }
    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }
}

impl<A: Address, Ph, Sends, Birth: BirthMode>
    From<(Sends, Vec<Create<A, Birth::Child>>, Become<A, Ph>)> for Actions<A, Ph, Sends, Birth>
{
    fn from(
        (sends, creates, become_): (Sends, Vec<Create<A, Birth::Child>>, Become<A, Ph>),
    ) -> Self {
        Self {
            sends,
            creates,
            become_,
        }
    }
}

pub type Acted<A, Ph, Sends, Birth, E> = Result<Actions<A, Ph, Sends, Birth>, E>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Births, CreationKind, MailAddr};

    #[test]
    fn mapping_sends_preserves_creation_order_and_verdict() {
        let actions: Actions<MailAddr, u8, Vec<u8>, Births<()>> = Actions::new(
            vec![1, 2],
            vec![
                Create::new(3, (), CreationKind::Birth),
                Create::new(4, (), CreationKind::replacement_of(3)),
            ],
            Step::Goto(7),
        );

        let mapped = actions.map_sends(|sends| sends.len());
        assert_eq!(mapped.sends, 2);
        assert_eq!(
            mapped
                .creates
                .iter()
                .map(|creation| creation.nonce)
                .collect::<Vec<_>>(),
            [3, 4]
        );
        assert!(matches!(mapped.become_, Step::Goto(7)));
    }

    #[test]
    fn mapping_become_preserves_sends_and_creation_order() {
        let actions: Actions<MailAddr, u8, Vec<u8>, Births<()>> = Actions::new(
            vec![1, 2],
            vec![Create::new(3, (), CreationKind::Birth)],
            Step::Goto(7),
        );

        let mapped: Actions<MailAddr, Never, Vec<u8>, Births<()>> =
            actions.map_become(|_| Step::Stop(Exit::Normal));
        assert_eq!(mapped.sends, [1, 2]);
        assert_eq!(mapped.creates[0].nonce, 3);
        assert!(matches!(mapped.become_, Step::Stop(Exit::Normal)));
    }
}
