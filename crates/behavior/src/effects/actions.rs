//! The explicit result of one actor behavior transition.

use super::sending::SendAlgebra;
use crate::actor::{Address, BirthMode, Create};
use crate::next::{Never, Step, Stopped};

pub type Become<Ph = Never> = Step<Ph, Stopped>;

/// Bombay's typed realization of the actor transition effects: communications,
/// fresh actor creation, and next behavior or termination.
///
/// An interpreter resolves every fresh creation in `creates` before
/// interpreting any ordinary delivery or [`crate::ServiceSends`] request in
/// `sends` from this value. A successful resolution installs and binds the
/// child; a rejected resolution binds nothing. This ordering lets a same-action
/// [`crate::ObserveCreation`] request return the committed result rather than
/// the behavior's intent. When creation is rejected, a same-action
/// [`crate::ObserveChild`] for its nonce is consumed without installing an
/// observation or emitting [`crate::ChildStopped`], while
/// [`crate::ObserveCreation`] reports the rejection. A later creation cannot
/// inherit that consumed observation. Creation order is vector order, and each
/// concrete named send lane retains its own order; this contract does not
/// impose an order between independent lanes. Constructing a value remains
/// pure.
pub struct Actions<A: Address, Ph, Sends, Birth: BirthMode> {
    pub sends: Sends,
    pub creates: Vec<Create<A, Birth::Child>>,
    pub become_: Become<Ph>,
}

impl<A, Ph, Sends, Birth> core::fmt::Debug for Actions<A, Ph, Sends, Birth>
where
    A: Address + core::fmt::Debug,
    A::Nonce: core::fmt::Debug,
    Ph: core::fmt::Debug,
    Sends: core::fmt::Debug,
    Birth: BirthMode,
    Birth::Child: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Actions")
            .field("sends", &self.sends)
            .field("creates", &self.creates)
            .field("become", &self.become_)
            .finish()
    }
}

impl<A, Ph, Sends, Birth> PartialEq for Actions<A, Ph, Sends, Birth>
where
    A: Address + PartialEq,
    A::Nonce: PartialEq,
    Ph: PartialEq,
    Sends: PartialEq,
    Birth: BirthMode,
    Birth::Child: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.sends == other.sends && self.creates == other.creates && self.become_ == other.become_
    }
}

impl<A, Ph, Sends, Birth> Eq for Actions<A, Ph, Sends, Birth>
where
    A: Address + Eq,
    A::Nonce: Eq,
    Ph: Eq,
    Sends: Eq,
    Birth: BirthMode,
    Birth::Child: Eq,
{
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
        map: impl FnOnce(Become<Ph>) -> Become<NextPh>,
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
        become_: Become<Ph>,
    ) -> Self {
        Self {
            sends,
            creates,
            become_,
        }
    }

    #[must_use]
    pub fn just(become_: Become<Ph>) -> Self {
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
    pub fn stop() -> Self {
        Self::just(Step::Stop(Stopped))
    }
    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }

    /// Continue after emitting the complete declared send product.
    #[must_use]
    pub fn send(sends: Sends) -> Self {
        Self::new(sends, Vec::new(), Step::Continue)
    }

    /// Continue after staging the complete declared creation product.
    #[must_use]
    pub fn create(creates: Vec<Create<A, Birth::Child>>) -> Self {
        Self::new(Sends::empty(), creates, Step::Continue)
    }
}

impl<A: Address, Ph, Sends, Birth: BirthMode>
    From<(Sends, Vec<Create<A, Birth::Child>>, Become<Ph>)> for Actions<A, Ph, Sends, Birth>
{
    fn from((sends, creates, become_): (Sends, Vec<Create<A, Birth::Child>>, Become<Ph>)) -> Self {
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
    use crate::{Births, CreationKind, MailAddr, NoBirths};

    #[test]
    fn equality_and_debug_cover_every_named_effect_leg() {
        type Plain = Actions<MailAddr, u8, Vec<u8>, NoBirths>;

        let value = Plain::new(vec![1], Vec::new(), Step::Goto(3));
        assert_eq!(value, Plain::new(vec![1], Vec::new(), Step::Goto(3)));
        assert_ne!(value, Plain::new(vec![2], Vec::new(), Step::Goto(3)));
        assert_ne!(value, Plain::new(vec![1], Vec::new(), Step::Goto(4)));
        assert_ne!(value, Plain::new(vec![1], Vec::new(), Step::Continue));
        assert_eq!(
            format!("{value:?}"),
            "Actions { sends: [1], creates: [], become: Goto(3) }"
        );

        type Creating = Actions<MailAddr, Never, Vec<u8>, Births<u8>>;
        let created = Creating::new(
            Vec::new(),
            vec![Create::new(7, 9, CreationKind::Birth)],
            Step::Continue,
        );
        let other_creation = Creating::new(
            Vec::new(),
            vec![Create::new(8, 9, CreationKind::Birth)],
            Step::Continue,
        );
        assert_ne!(created, other_creation);
    }

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
            actions.map_become(|_| Step::Stop(Stopped));
        assert_eq!(mapped.sends, [1, 2]);
        assert_eq!(mapped.creates[0].nonce, 3);
        assert!(matches!(mapped.become_, Step::Stop(Stopped)));
    }
}
