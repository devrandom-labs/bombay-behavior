//! The user-message lane and its composition contracts.

use crate::actor::Address;

/// The user-message event at the Agha floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User<A, M> {
    pub from: A,
    pub message: M,
}

/// A statically proven injection of one semantic input into a concrete event sum.
///
/// Implementations must select exactly one constructor and preserve `input`
/// unchanged. Absence of an implementation means that the protocol does not
/// accept that input.
pub trait EventInput<Input>: Sized {
    fn inject(input: Input) -> Self;
}

/// Attempt to route one input through a nested event product.
///
/// Unlike [`EventInput`], this is not an acceptance capability. It is the
/// lossless routing operation used by an outer wrapper when it does not own an
/// input itself. Rejection returns the original input unchanged.
pub trait RouteInput<Input>: Sized {
    fn route(input: Input) -> Result<Self, Input>;
}

impl<A, M> EventInput<User<A, M>> for User<A, M> {
    fn inject(input: User<A, M>) -> Self {
        input
    }
}

impl<A, M, Input> RouteInput<Input> for User<A, M> {
    fn route(input: Input) -> Result<Self, Input> {
        Err(input)
    }
}

impl<A, M> User<A, M> {
    #[must_use]
    pub const fn new(from: A, message: M) -> Self {
        Self { from, message }
    }
}

impl<A, M> From<(A, M)> for User<A, M> {
    fn from((from, message): (A, M)) -> Self {
        Self::new(from, message)
    }
}

/// Construction/extraction of the user lane through a composed event type.
pub trait UserEvent: Sized {
    type Addr: Address;
    type Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self;

    /// # Errors
    /// Returns the unchanged event when it belongs to another composed lane.
    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self>;
}

impl<A: Address, M> UserEvent for User<A, M> {
    type Addr = A;
    type Message = M;

    fn user(from: A, message: M) -> Self {
        Self::new(from, message)
    }
    fn into_user(self) -> Result<Self, Self> {
        Ok(self)
    }
}
