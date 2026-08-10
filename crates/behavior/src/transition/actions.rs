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
