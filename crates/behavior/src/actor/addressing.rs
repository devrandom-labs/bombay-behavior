//! Protocol-indexed actor recipients and deliveries.

use core::marker::PhantomData;

use crate::Protocol;

#[cfg(test)]
use crate::Behavior;

/// A pure actor-address namespace.
pub trait Address: Copy + Eq {
    type Nonce: Copy + Eq;

    #[must_use]
    fn birth(self, nonce: Self::Nonce) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailAddr(pub u64);

impl From<u64> for MailAddr {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<MailAddr> for u64 {
    fn from(value: MailAddr) -> Self {
        value.0
    }
}

impl Address for MailAddr {
    type Nonce = u64;

    fn birth(self, nonce: u64) -> Self {
        Self(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

/// Internal representation of pure routing intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route<A: Address> {
    Global(A),
    Child(A::Nonce),
}

impl<A: Address> Route<A> {
    #[must_use]
    pub(crate) const fn global(address: A) -> Self {
        Self::Global(address)
    }

    #[must_use]
    pub(crate) const fn child(nonce: A::Nonce) -> Self {
        Self::Child(nonce)
    }
}

/// Pure routing intent for one concrete destination protocol signature.
///
/// The destination protocol owner is part of the type even when two protocols
/// share the same address namespace and message type. A recipient proves the
/// static signature only; it does not prove that an executable
/// [`Behavior`](crate::Behavior)
/// has been installed at the route. The value contains no mailbox, endpoint,
/// registry entry, or other interpreter-owned capability.
pub struct Recipient<B: Protocol> {
    route: Route<B::Addr>,
    protocol: PhantomData<fn() -> B>,
}

impl<B: Protocol> Copy for Recipient<B> {}

impl<B: Protocol> Clone for Recipient<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Protocol> Recipient<B> {
    #[must_use]
    pub fn global(address: B::Addr) -> Self {
        Self::from_route(Route::global(address))
    }

    #[must_use]
    pub fn child(nonce: <B::Addr as Address>::Nonce) -> Self {
        Self::from_route(Route::child(nonce))
    }

    /// Resolve this intent in the address namespace of the sending actor.
    ///
    /// Global recipients ignore `parent`; child recipients derive their
    /// address from it. The route representation remains private so runtimes
    /// cannot couple endpoint tables to Behaviorpass internals.
    #[must_use]
    pub fn resolve(self, parent: B::Addr) -> B::Addr {
        match self.route {
            Route::Global(address) => address,
            Route::Child(nonce) => parent.birth(nonce),
        }
    }

    #[doc(hidden)]
    pub fn is_child(self, expected: <B::Addr as Address>::Nonce) -> bool {
        matches!(self.route, Route::Child(nonce) if nonce == expected)
    }

    const fn from_route(route: Route<B::Addr>) -> Self {
        Self {
            route,
            protocol: PhantomData,
        }
    }
}

impl<B: Protocol> PartialEq for Recipient<B> {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl<B: Protocol> Eq for Recipient<B> {}

impl<B: Protocol> core::fmt::Debug for Recipient<B>
where
    B::Addr: core::fmt::Debug,
    <B::Addr as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.route.fmt(f)
    }
}

/// One pure communication addressed to a concrete protocol signature.
///
/// Protocol identity is not inferred from the payload. Consequently, two
/// behaviors accepting the same address and message types still have distinct
/// delivery types.
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, Delivery, MailAddr, Never, NoBirths, Protocol, Recipient, User};
///
/// struct Queue;
/// struct Worker;
/// macro_rules! inert {
///     ($actor:ty) => {
///         impl Protocol for $actor {
///             type Addr = MailAddr;
///             type Msg = u8;
///         }
///         impl Behavior for $actor {
///             type Event = User<MailAddr, u8>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///             fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///             fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> {
///                 Ok(Actions::cont())
///             }
///         }
///     };
/// }
/// inert!(Queue);
/// inert!(Worker);
///
/// let worker = Recipient::<Worker>::global(MailAddr(1));
/// let _: Delivery<Queue> = Delivery::new(worker, 7);
/// ```
///
/// A destination also fixes its message and address namespaces:
///
/// ```compile_fail
/// use behavior::{Actions, Address, Behavior, Delivery, MailAddr, Never, NoBirths, Protocol, Recipient, User};
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct OtherAddr(u64);
/// impl Address for OtherAddr {
///     type Nonce = u64;
///     fn birth(self, nonce: u64) -> Self { Self(self.0 ^ nonce) }
/// }
/// struct Worker;
/// impl Protocol for Worker {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// impl Behavior for Worker {
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// let _ = Recipient::<Worker>::global(OtherAddr(1));
/// ```
///
/// ```compile_fail
/// # use behavior::{Actions, Behavior, Delivery, MailAddr, Never, NoBirths, Protocol, Recipient, User};
/// # struct Worker;
/// # impl Protocol for Worker {
/// #     type Addr = MailAddr;
/// #     type Msg = u8;
/// # }
/// # impl Behavior for Worker {
/// #     type Event = User<MailAddr, u8>;
/// #     type Sends = Vec<Never>;
/// #     type Ph = Never;
/// #     type Error = Never;
/// #     type Birth = NoBirths;
/// #     fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// #     fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// let worker = Recipient::<Worker>::global(MailAddr(1));
/// let _ = Delivery::<Worker>::new(worker, "wrong payload");
/// ```
pub struct Delivery<B: Protocol> {
    pub to: Recipient<B>,
    pub message: B::Msg,
}

impl<B: Protocol> Delivery<B> {
    #[must_use]
    pub fn new(to: Recipient<B>, message: B::Msg) -> Self {
        Self { to, message }
    }
}

impl<B> Clone for Delivery<B>
where
    B: Protocol,
    B::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.to, self.message.clone())
    }
}

impl<B> PartialEq for Delivery<B>
where
    B: Protocol,
    B::Msg: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.message == other.message
    }
}

impl<B> Eq for Delivery<B>
where
    B: Protocol,
    B::Msg: Eq,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actions, Never, NoBirths, User};

    struct Inbox;

    struct SignatureOnly;

    impl crate::Protocol for SignatureOnly {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl behavior::Protocol for Inbox {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Inbox {
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    #[test]
    fn mail_address_conversion_preserves_nonzero_value() {
        assert_eq!(u64::from(MailAddr(41)), 41);
    }

    #[test]
    fn routing_requires_only_the_static_protocol_signature() {
        let recipient = Recipient::<SignatureOnly>::global(MailAddr(7));
        let delivery = Delivery::new(recipient, 11);

        assert_eq!(delivery.to.resolve(MailAddr(99)), MailAddr(7));
        assert_eq!(delivery.message, 11);
    }

    #[test]
    fn recipient_value_contract_distinguishes_routes() {
        let global = Recipient::<Inbox>::global(MailAddr(7));
        let same_global = Recipient::<Inbox>::global(MailAddr(7));
        let other_global = Recipient::<Inbox>::global(MailAddr(8));
        let child = Recipient::<Inbox>::child(3);
        let other_child = Recipient::<Inbox>::child(4);

        assert_eq!(global, same_global);
        assert_ne!(global, other_global);
        assert_ne!(global, child);
        assert_ne!(child, other_child);
        assert!(child.is_child(3));
        assert!(!child.is_child(4));
        assert!(!global.is_child(3));
        assert_eq!(format!("{global:?}"), "Global(MailAddr(7))");
        assert_eq!(format!("{child:?}"), "Child(3)");
    }

    #[test]
    fn delivery_equality_requires_both_destination_and_message() {
        let value = Delivery::<Inbox>::new(Recipient::global(MailAddr(1)), 5);
        let same = Delivery::<Inbox>::new(Recipient::global(MailAddr(1)), 5);
        let other_destination = Delivery::<Inbox>::new(Recipient::global(MailAddr(2)), 5);
        let other_message = Delivery::<Inbox>::new(Recipient::global(MailAddr(1)), 6);
        let both_different = Delivery::<Inbox>::new(Recipient::global(MailAddr(2)), 6);

        assert!(value == same);
        assert!(value != other_destination);
        assert!(value != other_message);
        assert!(value != both_different);
    }
}
