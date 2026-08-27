//! Protocol-indexed actor recipients and deliveries.

use core::marker::PhantomData;

use crate::{Behavior, MessageProtocol, Protocol};

/// A pure logical actor-address namespace.
///
/// An address names a transport or resolution domain. It does not allocate an
/// actor and it does not prove that an actor is installed. Fresh allocation is
/// interpreter-owned; the creator-local [`Address::Nonce`] is correlation
/// evidence only and is deliberately not convertible into an address here.
pub trait Address: Copy + Eq {
    type Nonce: Copy + Eq;
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
}

/// Runtime-owned exact endpoint family for one logical address namespace.
///
/// A runtime implements this trait on its own address newtype, selecting one
/// statically projected endpoint representation for each concrete protocol.
/// The projection may reuse a representation; [`EstablishedRecipient<P>`]
/// still preserves `P` and prevents cross-protocol substitution. Ordinary
/// protocols continue to declare only their canonical [`Protocol::Addr`] and
/// [`Protocol::Msg`]; they never author endpoint keys or endpoint associated
/// types. An endpoint is cloneable acquaintance evidence, but it is not
/// intrinsically `Send`: thread transfer is required only by the concrete
/// asynchronous interpretation boundary that performs it.
///
/// A local endpoint remains valid, but cannot enter the sendable
/// [`crate::InterpretSends`] path:
///
/// ```compile_fail
/// use behavior::{
///     Address, EndpointAddress, EstablishedDelivery, EstablishedRecipient,
///     Here, InterpretEstablishedDelivery, InterpretSends, Protocol,
///     SendInterpreter, User,
/// };
/// use core::marker::PhantomData;
/// use std::rc::Rc;
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct LocalAddr(u8);
/// impl Address for LocalAddr { type Nonce = u8; }
/// struct LocalEndpoint<P>(Rc<()>, PhantomData<fn() -> P>);
/// impl<P> Clone for LocalEndpoint<P> {
///     fn clone(&self) -> Self { Self(self.0.clone(), PhantomData) }
/// }
/// impl EndpointAddress for LocalAddr {
///     type Established<P> = LocalEndpoint<P> where P: Protocol<Addr = Self>;
/// }
/// struct LocalProtocol;
/// impl Protocol for LocalProtocol {
///     type Addr = LocalAddr;
///     type Msg = Rc<()>;
/// }
/// struct Runtime;
/// impl SendInterpreter for Runtime { type Error = (); }
/// impl InterpretEstablishedDelivery<LocalProtocol> for Runtime {
///     fn interpret_established_delivery(
///         &mut self,
///         endpoint: LocalEndpoint<LocalProtocol>,
///         message: Rc<()>,
///     ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send {
///         drop(endpoint);
///         drop(message);
///         async { Ok(()) }
///     }
/// }
/// fn require_async<T>()
/// where
///     T: InterpretSends<Runtime, User<LocalAddr, Rc<()>>, Here>,
/// {}
/// let endpoint = LocalEndpoint(Rc::new(()), PhantomData);
/// let recipient = EstablishedRecipient::<LocalProtocol>::issued(endpoint);
/// let _delivery = EstablishedDelivery::new(recipient, Rc::new(()));
/// require_async::<Vec<EstablishedDelivery<LocalProtocol>>>();
/// ```
pub trait EndpointAddress: Address + Sized {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

/// Pure logical destination for one concrete protocol signature.
///
/// The destination protocol owner is part of the type even when two protocols
/// share the same address namespace and message type. This value proves only
/// the static signature at a logical address; it does not prove that an exact
/// executable incarnation has been installed there. Addressed recipients are
/// retained for genuine transport and name-resolution boundaries.
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

    /// Return the logical address, independent of any sending actor.
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

/// Inert capability for one exact installed incarnation of protocol `P`.
///
/// The endpoint type is selected by `P::Addr`, so `P` remains the only
/// protocol identity and ordinary domain types carry no endpoint parameter.
/// The endpoint has no direct accessor or send method. It crosses only an
/// explicit interpretation boundary. That boundary is public and therefore a
/// deliberate power-user authority boundary, not exclusive runtime authority.
///
/// ```compile_fail
/// use behavior::{Address, EndpointAddress, EstablishedRecipient, Protocol};
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl Address for RuntimeAddr { type Nonce = u64; }
/// struct Worker;
/// impl Protocol for Worker { type Addr = RuntimeAddr; type Msg = (); }
/// #[derive(Clone)]
/// struct Endpoint;
/// impl EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: Protocol<Addr = Self>;
/// }
/// let recipient = EstablishedRecipient::<Worker>::issued(Endpoint);
/// let _endpoint = recipient.endpoint();
/// ```
pub struct EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub(crate) endpoint: <P::Addr as EndpointAddress>::Established<P>,
}

impl<P> EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    /// Issue a capability from an endpoint already established by an
    /// interpreter.
    ///
    /// This constructor performs no allocation or validation. Callers at this
    /// power-user boundary must issue it only after successful fresh
    /// installation and commit.
    #[must_use]
    pub const fn issued(endpoint: <P::Addr as EndpointAddress>::Established<P>) -> Self {
        Self { endpoint }
    }

    /// Transfer the endpoint through an explicit interpretation boundary.
    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablished<P>,
    {
        interpreter.interpret_established(self.endpoint)
    }
}

/// Public power-user transfer boundary for one exact endpoint.
pub trait InterpretEstablished<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    type Output;

    fn interpret_established(
        &mut self,
        endpoint: <P::Addr as EndpointAddress>::Established<P>,
    ) -> Self::Output;
}

impl<P> Clone for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        Self::issued(self.endpoint.clone())
    }
}

impl<P> core::fmt::Debug for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("EstablishedRecipient")
            .field(&self.endpoint)
            .finish()
    }
}

impl<P> PartialEq for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
    }
}

impl<P> Eq for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Eq,
{
}

/// Inert capability for one exact installed concrete behavior.
///
/// An established recipient proves the public protocol endpoint. This value
/// additionally preserves which concrete behavior was installed, allowing
/// lifecycle effects to require static evidence about that behavior's event
/// algebra. It remains inert: no direct send, shutdown, endpoint accessor, or
/// other ambient effect is exposed.
pub struct EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    recipient: EstablishedRecipient<B::Protocol>,
    behavior: PhantomData<fn() -> B>,
}

impl<B> EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    /// Issue an exact actor capability after successful installation.
    ///
    /// This is the same public power-user boundary as
    /// [`EstablishedRecipient::issued`]. It performs no allocation,
    /// initialization, installation, or validation.
    #[must_use]
    pub const fn issued(
        endpoint: <<B::Protocol as Protocol>::Addr as EndpointAddress>::Established<B::Protocol>,
    ) -> Self {
        Self {
            recipient: EstablishedRecipient::issued(endpoint),
            behavior: PhantomData,
        }
    }

    pub(crate) const fn from_recipient(recipient: EstablishedRecipient<B::Protocol>) -> Self {
        Self {
            recipient,
            behavior: PhantomData,
        }
    }

    /// Project the exact public-protocol recipient for this incarnation.
    #[must_use]
    pub fn recipient(&self) -> EstablishedRecipient<B::Protocol> {
        self.recipient.clone()
    }

    /// Consume the concrete-actor proof and retain its exact protocol
    /// recipient.
    #[must_use]
    pub fn into_recipient(self) -> EstablishedRecipient<B::Protocol> {
        self.recipient
    }

    /// Transfer the endpoint through the explicit interpretation boundary
    /// while preserving `B` in the caller's request type.
    pub fn interpret<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretEstablished<B::Protocol>,
    {
        self.recipient.interpret(interpreter)
    }
}

impl<B> Clone for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        Self {
            recipient: self.recipient.clone(),
            behavior: PhantomData,
        }
    }
}

impl<B> core::fmt::Debug for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Established<B::Protocol>:
        core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("EstablishedActor")
            .field(&self.recipient)
            .finish()
    }
}

impl<B> PartialEq for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Established<B::Protocol>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.recipient == other.recipient
    }
}

impl<B> Eq for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Established<B::Protocol>: Eq,
{
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
    pub to: Recipient<P>,
    pub message: P::Msg,
}

impl<P: Protocol> Delivery<P> {
    #[must_use]
    pub fn new(to: Recipient<P>, message: P::Msg) -> Self {
        Self { to, message }
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

/// One pure communication to an exact installed incarnation.
///
/// Unlike [`Delivery`], this effect carries the runtime-issued endpoint and
/// requires no address-to-endpoint resolution. Constructing it remains pure;
/// only an explicit [`crate::InterpretEstablishedDelivery`] implementation can
/// perform the communication.
///
/// Exact endpoints remain protocol-indexed even when two protocols share an
/// address namespace and message type:
///
/// ```compile_fail
/// use behavior::{
///     Address, EndpointAddress, EstablishedDelivery, EstablishedRecipient,
///     Protocol,
/// };
/// use core::marker::PhantomData;
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl Address for RuntimeAddr { type Nonce = u64; }
/// struct Endpoint<P>(PhantomData<fn() -> P>);
/// impl<P> Clone for Endpoint<P> {
///     fn clone(&self) -> Self { Self(PhantomData) }
/// }
/// impl EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint<P> where P: Protocol<Addr = Self>;
/// }
/// struct Queue;
/// struct Worker;
/// impl Protocol for Queue { type Addr = RuntimeAddr; type Msg = u8; }
/// impl Protocol for Worker { type Addr = RuntimeAddr; type Msg = u8; }
/// let worker = EstablishedRecipient::<Worker>::issued(Endpoint(PhantomData));
/// let _: EstablishedDelivery<Queue> = EstablishedDelivery::new(worker, 7);
/// ```
pub struct EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    pub to: EstablishedRecipient<P>,
    pub message: P::Msg,
}

impl<P> EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
{
    #[must_use]
    pub const fn new(to: EstablishedRecipient<P>, message: P::Msg) -> Self {
        Self { to, message }
    }
}

impl<P> Clone for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.to.clone(), self.message.clone())
    }
}

impl<P> core::fmt::Debug for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    EstablishedRecipient<P>: core::fmt::Debug,
    P::Msg: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("EstablishedDelivery")
            .field("to", &self.to)
            .field("message", &self.message)
            .finish()
    }
}

impl<P> PartialEq for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    EstablishedRecipient<P>: PartialEq,
    P::Msg: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.message == other.message
    }
}

impl<P> Eq for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    EstablishedRecipient<P>: Eq,
    P::Msg: Eq,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Actions, Never, NoBirths, User};
    use std::rc::Rc;

    struct Inbox;

    struct SignatureOnly;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct LocalAddr(u8);

    impl Address for LocalAddr {
        type Nonce = u8;
    }

    struct LocalProtocol;

    impl Protocol for LocalProtocol {
        type Addr = LocalAddr;
        type Msg = Rc<()>;
    }

    struct LocalEndpoint<P> {
        token: Rc<()>,
        protocol: PhantomData<fn() -> P>,
    }

    impl<P> Clone for LocalEndpoint<P> {
        fn clone(&self) -> Self {
            Self {
                token: self.token.clone(),
                protocol: PhantomData,
            }
        }
    }

    impl EndpointAddress for LocalAddr {
        type Established<P>
            = LocalEndpoint<P>
        where
            P: Protocol<Addr = Self>;
    }

    struct LocalTransfer;

    impl InterpretEstablished<LocalProtocol> for LocalTransfer {
        type Output = Rc<()>;

        fn interpret_established(
            &mut self,
            endpoint: LocalEndpoint<LocalProtocol>,
        ) -> Self::Output {
            endpoint.token
        }
    }

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

        assert_eq!(delivery.to.address(), MailAddr(7));
        assert_eq!(delivery.message, 11);
    }

    #[test]
    fn recipient_value_contract_distinguishes_logical_addresses() {
        let global = Recipient::<Inbox>::global(MailAddr(7));
        let same_global = Recipient::<Inbox>::global(MailAddr(7));
        let other_global = Recipient::<Inbox>::global(MailAddr(8));

        assert_eq!(global, same_global);
        assert_ne!(global, other_global);
        assert_eq!(global.address(), MailAddr(7));
        assert_eq!(format!("{global:?}"), "MailAddr(7)");
    }

    #[test]
    fn established_capability_can_remain_local_until_an_async_boundary_requires_send() {
        let token = Rc::new(());
        let recipient = EstablishedRecipient::<LocalProtocol>::issued(LocalEndpoint {
            token: token.clone(),
            protocol: PhantomData,
        });
        let retained = recipient.clone();
        let extracted = retained.interpret(&mut LocalTransfer);

        assert!(Rc::ptr_eq(&token, &extracted));
        drop(recipient);
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
