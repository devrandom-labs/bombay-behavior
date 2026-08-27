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

/// Owner-selected event ingress for one statically identified source.
///
/// `Source` distinguishes otherwise identical inputs without an absolute
/// wrapper path. A child report uses the existing
/// [`ChildRoute`](crate::ChildRoute) type as its source; a current-actor policy
/// input uses [`Here`]. Outer [`EventLayer`] composition lifts the selected
/// ingress automatically, so adding a [`BehaviorLayer`](crate::BehaviorLayer)
/// never requires a caller to recount structural event depth.
///
/// This is a derived Bombay composition contract, not an actor-model
/// primitive. It constructs a typed event only and performs no delivery,
/// lookup, or interpreter effect.
pub trait EventIngress<Source, Input>: Sized {
    /// Construct the event selected for `Source` and `Input`.
    fn ingress(input: Input) -> Self;
}

/// Construction of one private input sent by an established parent to its
/// concrete direct child.
///
/// This is the event-side contract of [`ChildInput`](crate::ChildInput). It is
/// deliberately distinct from [`EventIngress`], which selects a parent event
/// for an incoming child report or a same-actor owner input. Keeping the two
/// directions distinct lets a report-owning behavior transformation preserve
/// every inner child-input capability without knowing its source or payload.
///
/// `Source` identifies the behavior law that owns `Input`; it is neither an
/// actor address nor runtime lookup key. Constructing the event performs no
/// delivery or other actor effect.
pub trait ChildInputIngress<Source, Input>: Sized {
    /// Construct the concrete child event selected by `Source` and `Input`.
    fn child_input(input: Input) -> Self;
}

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

impl<Owned, Inner, Source, Input> EventIngress<Source, Input> for EventLayer<Owned, Inner>
where
    Inner: EventIngress<Source, Input>,
{
    fn ingress(input: Input) -> Self {
        Self::Inner(Inner::ingress(input))
    }
}

/// A complete event algebra formed by adding owned inputs around an inner
/// behavior event algebra.
///
/// `from_inner` is the structure-preserving injection used by effect
/// composition. A single owned lane uses [`EventLayer`]; a domain with several
/// genuinely coexisting owned inputs may use a named exhaustive sum.
pub trait ComposedEvent: UserEvent {
    type Inner: UserEvent<Addr = Self::Addr, Message = Self::Message>;

    fn from_inner(event: Self::Inner) -> Self;
}

impl<Owned, Inner> ComposedEvent for EventLayer<Owned, Inner>
where
    Inner: UserEvent,
{
    type Inner = Inner;

    fn from_inner(event: Inner) -> Self {
        Self::Inner(event)
    }
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

    #[test]
    #[allow(
        clippy::clone_on_copy,
        reason = "the contract test independently exercises both promised construction traits"
    )]
    fn ingress_is_a_copyable_equal_capability_with_stable_debug_identity() {
        let ingress = Ingress::<u16, Here>::new();
        let copied = ingress;
        let cloned = ingress.clone();

        assert_eq!(ingress, copied);
        assert_eq!(ingress, cloned);
        assert_eq!(format!("{ingress:?}"), "Ingress");
    }
}
