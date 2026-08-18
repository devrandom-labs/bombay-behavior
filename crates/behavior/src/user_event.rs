//! The user-message lane and its composition contracts.

use crate::actor::Address;

/// The current layer of a structurally composed event algebra owns an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Here;

/// An inner event algebra owns an input at `Path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inside<Path>(core::marker::PhantomData<fn() -> Path>);

/// Address-free destination for one interpreter-originated input.
///
/// Unlike an actor [`Recipient`](crate::Recipient), this capability targets one
/// exact member of the current actor's composed ingress algebra. Service
/// requests retain it so the eventual fact does not have to search the final
/// behavior type for a matching payload. `Path` is compile-time evidence and
/// occupies no runtime storage.
pub struct Ingress<Input, Path> {
    marker: core::marker::PhantomData<fn(Input, Path)>,
}

impl<Input, Path> Copy for Ingress<Input, Path> {}

impl<Input, Path> Clone for Ingress<Input, Path> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Input, Path> Ingress<Input, Path> {
    /// Select a statically proven ingress member.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: core::marker::PhantomData,
        }
    }

    /// Construct the exact concrete event without runtime route discovery.
    #[must_use]
    pub fn event<Event>(self, input: Input) -> Event
    where
        Event: InjectEvent<Input, Path>,
    {
        Event::inject_at(input)
    }

    /// Lift this destination through one outer structural event layer.
    #[must_use]
    pub const fn inside(self) -> Ingress<Input, Inside<Path>> {
        Ingress {
            marker: core::marker::PhantomData,
        }
    }
}

impl<Input, Path> Default for Ingress<Input, Path> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Input, Path> core::fmt::Debug for Ingress<Input, Path> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Ingress")
    }
}

impl<Input, Path> PartialEq for Ingress<Input, Path> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<Input, Path> Eq for Ingress<Input, Path> {}

/// One statically owned input lane composed in front of an inner event algebra.
///
/// `EventLayer` is a concrete coproduct, not an erased envelope. `Owned` is the
/// lane introduced by the current behavior template and `Inner` preserves the
/// complete event algebra of the wrapped behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventLayer<Owned, Inner> {
    Owned(Owned),
    Inner(Inner),
}

/// Path-indexed injection into a structural event coproduct.
///
/// The path is compile-time routing evidence. It prevents the overlapping
/// implementations that arise when an outer layer owns the same input type as
/// an inner layer and makes that ownership choice explicit in the type system.
/// Unsupported input has no construction capability:
///
/// ```compile_fail
/// use behavior::{Here, InjectEvent, MailAddr, User};
/// let _ = <User<MailAddr, ()> as InjectEvent<u8, Here>>::inject_at(7);
/// ```
///
/// Repeated payload types require an explicit ownership path rather than an
/// outermost-first runtime guess:
///
/// ```compile_fail
/// use behavior::{EventLayer, InjectEvent, MailAddr, User};
/// type Duplicate = EventLayer<u8, EventLayer<u8, User<MailAddr, ()>>>;
/// let _ = <Duplicate as InjectEvent<u8, _>>::inject_at(7);
/// ```
pub trait InjectEvent<Input, Path>: Sized {
    fn inject_at(input: Input) -> Self;
}

impl<Owned, Inner> InjectEvent<Owned, Here> for EventLayer<Owned, Inner> {
    fn inject_at(input: Owned) -> Self {
        Self::Owned(input)
    }
}

impl<Owned, Inner, Input, Path> InjectEvent<Input, Inside<Path>> for EventLayer<Owned, Inner>
where
    Inner: InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Inner(Inner::inject_at(input))
    }
}

/// The user-message event at the Agha floor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User<A, M> {
    pub from: A,
    pub message: M,
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

impl<Owned, Inner> UserEvent for EventLayer<Owned, Inner>
where
    Inner: UserEvent,
{
    type Addr = Inner::Addr;
    type Message = Inner::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(Inner::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(inner) => inner.into_user().map_err(Self::Inner),
            owned @ Self::Owned(_) => Err(owned),
        }
    }
}

#[cfg(test)]
mod structural_tests {
    use super::*;
    use crate::MailAddr;

    type Nested = EventLayer<u8, EventLayer<u16, User<MailAddr, ()>>>;

    #[test]
    fn paths_select_exactly_one_layer_without_runtime_search() {
        let outer = <Nested as InjectEvent<u8, Here>>::inject_at(3);
        assert_eq!(outer, EventLayer::Owned(3));

        let inner = <Nested as InjectEvent<u16, Inside<Here>>>::inject_at(5);
        assert_eq!(inner, EventLayer::Inner(EventLayer::Owned(5)));
    }

    #[test]
    fn duplicate_payload_types_remain_distinct_ownership_capabilities() {
        type Duplicate = EventLayer<u8, EventLayer<u8, User<MailAddr, ()>>>;

        let outer = <Duplicate as InjectEvent<u8, Here>>::inject_at(7);
        let inner = <Duplicate as InjectEvent<u8, Inside<Here>>>::inject_at(7);

        assert_eq!(outer, EventLayer::Owned(7));
        assert_eq!(inner, EventLayer::Inner(EventLayer::Owned(7)));
    }

    #[test]
    fn a_unique_structural_path_is_inferred_at_a_concrete_composition() {
        let event = <Nested as InjectEvent<u16, _>>::inject_at(9);
        assert_eq!(event, EventLayer::Inner(EventLayer::Owned(9)));
    }

    #[test]
    fn ingress_identity_lifts_without_payload_or_runtime_route_search() {
        type Inner = EventLayer<u16, User<MailAddr, ()>>;
        type Outer = EventLayer<u8, Inner>;

        let inner = Ingress::<u16, Here>::new();
        let outer: Ingress<u16, Inside<Here>> = inner.inside();

        let event: Outer = outer.event(12);
        assert_eq!(event, EventLayer::Inner(EventLayer::Owned(12)));
    }
}
