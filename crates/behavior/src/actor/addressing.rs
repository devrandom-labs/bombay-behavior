//! Protocol-indexed actor recipients and deliveries.

use core::marker::PhantomData;

use crate::{MessageProtocol, Protocol};

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

/// Pure established identity for one concrete destination protocol signature.
///
/// The destination protocol owner is part of the type even when two protocols
/// share the same address namespace and message type. A recipient proves the
/// static signature only; it does not prove that an executable
/// [`Behavior`](crate::Behavior)
/// has been installed at the address. The value contains no mailbox, endpoint,
/// registry entry, or other interpreter-owned capability.
///
/// Creator-local routes are deliberately a different type and cannot be
/// smuggled into APIs that retain established recipients:
///
/// ```compile_fail
/// use behavior::{ChildRecipient, Delivery, MailAddr, Protocol};
/// struct Destination;
/// impl Protocol for Destination {
///     type Addr = MailAddr;
///     type Msg = u8;
/// }
/// let local = ChildRecipient::<Destination>::new(7);
/// let _ = Delivery::new(local, 1);
/// ```
pub struct Recipient<P: Protocol> {
    address: P::Addr,
    protocol: PhantomData<fn() -> P>,
}

impl<P: Protocol> Copy for Recipient<P> {}

impl<P: Protocol> Clone for Recipient<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol> Recipient<P> {
    #[must_use]
    pub fn global(address: P::Addr) -> Self {
        Self::new(address)
    }

    /// Return the established address, independent of any sending actor.
    #[must_use]
    pub const fn address(self) -> P::Addr {
        self.address
    }

    #[doc(hidden)]
    const fn new(address: P::Addr) -> Self {
        Self {
            address,
            protocol: PhantomData,
        }
    }
}

impl<A: Address, M> From<A> for Recipient<MessageProtocol<A, M>> {
    fn from(address: A) -> Self {
        Self::new(address)
    }
}

/// A creator-local route to one staged or established direct child.
///
/// Unlike [`Recipient`], this value is not an actor identity and cannot be
/// transferred as a generally usable destination. It is resolved only by the
/// interpreter of the actor whose child namespace owns `nonce`.
pub struct ChildRecipient<P: Protocol> {
    nonce: <P::Addr as Address>::Nonce,
    protocol: PhantomData<fn() -> P>,
}

impl<P: Protocol> Copy for ChildRecipient<P> {}
impl<P: Protocol> Clone for ChildRecipient<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol> ChildRecipient<P> {
    #[must_use]
    pub const fn new(nonce: <P::Addr as Address>::Nonce) -> Self {
        Self {
            nonce,
            protocol: PhantomData,
        }
    }

    #[must_use]
    pub const fn nonce(self) -> <P::Addr as Address>::Nonce {
        self.nonce
    }
}

/// Exhaustive destination of one delivery.
pub enum DeliveryTarget<P: Protocol> {
    Established(Recipient<P>),
    LocalChild(ChildRecipient<P>),
}

impl<P: Protocol> Copy for DeliveryTarget<P> {}
impl<P: Protocol> Clone for DeliveryTarget<P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<P: Protocol> PartialEq for ChildRecipient<P> {
    fn eq(&self, other: &Self) -> bool {
        self.nonce == other.nonce
    }
}
impl<P: Protocol> Eq for ChildRecipient<P> {}

impl<P: Protocol> PartialEq for DeliveryTarget<P> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Established(left), Self::Established(right)) => left == right,
            (Self::LocalChild(left), Self::LocalChild(right)) => left == right,
            _ => false,
        }
    }
}
impl<P: Protocol> Eq for DeliveryTarget<P> {}

impl<P: Protocol> PartialEq<Recipient<P>> for DeliveryTarget<P> {
    fn eq(&self, other: &Recipient<P>) -> bool {
        matches!(self, Self::Established(recipient) if recipient == other)
    }
}

impl<P: Protocol> core::fmt::Debug for DeliveryTarget<P>
where
    P::Addr: core::fmt::Debug,
    <P::Addr as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Established(recipient) => f.debug_tuple("Established").field(recipient).finish(),
            Self::LocalChild(child) => f.debug_tuple("LocalChild").field(&child.nonce()).finish(),
        }
    }
}

impl<P: Protocol> DeliveryTarget<P> {
    #[must_use]
    pub fn resolve(self, emitter: P::Addr) -> P::Addr {
        match self {
            Self::Established(recipient) => recipient.address(),
            Self::LocalChild(child) => emitter.birth(child.nonce()),
        }
    }

    #[doc(hidden)]
    #[must_use]
    pub fn is_local_child(self, expected: <P::Addr as Address>::Nonce) -> bool {
        matches!(self, Self::LocalChild(child) if child.nonce() == expected)
    }
}

impl<P: Protocol> PartialEq for Recipient<P> {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

impl<P: Protocol> Eq for Recipient<P> {}

impl<P: Protocol> core::fmt::Debug for Recipient<P>
where
    P::Addr: core::fmt::Debug,
    <P::Addr as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.address.fmt(f)
    }
}

/// One pure communication addressed to a concrete protocol signature.
///
/// Protocol identity is not inferred from the payload. Consequently, two
/// protocols with the same address and message types still have distinct
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
/// #     type Protocol = Self;
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
pub struct Delivery<P: Protocol> {
    pub to: DeliveryTarget<P>,
    pub message: P::Msg,
}

impl<P: Protocol> Delivery<P> {
    #[must_use]
    pub fn new(to: Recipient<P>, message: P::Msg) -> Self {
        Self {
            to: DeliveryTarget::Established(to),
            message,
        }
    }

    #[must_use]
    pub fn local_child(to: ChildRecipient<P>, message: P::Msg) -> Self {
        Self {
            to: DeliveryTarget::LocalChild(to),
            message,
        }
    }
}

impl<P> Clone for Delivery<P>
where
    P: Protocol,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self {
            to: self.to,
            message: self.message.clone(),
        }
    }
}

impl<P> PartialEq for Delivery<P>
where
    P: Protocol,
    P::Msg: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.message == other.message
    }
}

impl<P> Eq for Delivery<P>
where
    P: Protocol,
    P::Msg: Eq,
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
        type Protocol = Self;
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
        assert_eq!(delivery.to.resolve(MailAddr(1234)), MailAddr(7));
        assert_eq!(delivery.message, 11);
    }

    #[test]
    fn recipient_value_contract_distinguishes_routes() {
        let global = Recipient::<Inbox>::global(MailAddr(7));
        let same_global = Recipient::<Inbox>::global(MailAddr(7));
        let other_global = Recipient::<Inbox>::global(MailAddr(8));
        let child = DeliveryTarget::<Inbox>::LocalChild(ChildRecipient::new(3));
        let same_child = DeliveryTarget::<Inbox>::LocalChild(ChildRecipient::new(3));
        let other_child = DeliveryTarget::<Inbox>::LocalChild(ChildRecipient::new(4));

        assert_eq!(global, same_global);
        assert_ne!(global, other_global);
        assert_ne!(child, global);
        assert_eq!(child, same_child);
        assert_ne!(child, other_child);
        assert!(child.is_local_child(3));
        assert!(!child.is_local_child(4));
        assert_eq!(global.address(), MailAddr(7));
        assert_eq!(format!("{global:?}"), "MailAddr(7)");
        assert_eq!(format!("{child:?}"), "LocalChild(3)");
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
