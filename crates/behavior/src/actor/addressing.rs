//! Protocol-indexed actor recipients and deliveries.

use core::marker::PhantomData;

use crate::{ActionItem, Behavior, MessageProtocol, Never, Protocol};

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

/// Runtime-owned exact message endpoint family for one logical address namespace.
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
/// ```compile_fail,E0277
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct LocalAddr(u8);
/// impl behavior::Address for LocalAddr { type Nonce = u8; }
/// struct LocalEndpoint<P>(std::rc::Rc<()>, core::marker::PhantomData<fn() -> P>);
/// impl<P> Clone for LocalEndpoint<P> {
///     fn clone(&self) -> Self { Self(self.0.clone(), core::marker::PhantomData) }
/// }
/// impl behavior::RecipientAddress for LocalAddr {
///     type Established<P> = LocalEndpoint<P> where P: behavior::Protocol<Addr = Self>;
/// }
/// struct LocalProtocol;
/// impl behavior::Protocol for LocalProtocol {
///     type Addr = LocalAddr;
///     type Msg = std::rc::Rc<()>;
/// }
/// struct Runtime;
/// impl<RootEvent, Path> behavior::InterpretItem<behavior::EstablishedDelivery<LocalProtocol>, RootEvent, Path>
///     for Runtime
/// {
///     fn interpret_item(
///         &mut self,
///         delivery: behavior::EstablishedDelivery<LocalProtocol>,
///     ) -> impl core::future::Future<Output = behavior::ItemSettlement<
///         behavior::EstablishedDelivery<LocalProtocol>, (), behavior::Never, behavior::Never,
///     >> + Send {
///         async move {
///             drop(delivery);
///             behavior::ItemSettlement::Accepted(())
///         }
///     }
/// }
/// fn require_async<T>()
/// where
///     T: behavior::InterpretSends<Runtime, behavior::User<LocalAddr, std::rc::Rc<()>>, behavior::Here>,
/// {}
/// let endpoint = LocalEndpoint(std::rc::Rc::new(()), core::marker::PhantomData);
/// let recipient = behavior::EstablishedRecipient::<LocalProtocol>::issued(endpoint);
/// let _delivery = behavior::EstablishedDelivery::new(recipient, std::rc::Rc::new(()));
/// require_async::<Vec<behavior::EstablishedDelivery<LocalProtocol>>>();
/// ```
pub trait RecipientAddress: Address + Sized {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;
}

/// Runtime-owned installed actor family for an address namespace.
///
/// The runtime must bind one protocol endpoint and its matching concrete
/// lifecycle authority to the same incarnation before issuing this value.
/// A namespace used only for protocol messaging implements
/// [`RecipientAddress`] without this stronger port.
pub trait EndpointAddress: Address + Sized {
    type Established<P>: Clone
    where
        P: Protocol<Addr = Self>;

    type Installed<B>: Clone
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    /// Project the message-only endpoint from one installed actor.
    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;
}

impl<A: EndpointAddress> RecipientAddress for A {
    type Established<P>
        = <A as EndpointAddress>::Established<P>
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

/// Inert capability for one exact protocol endpoint.
///
/// The endpoint type is selected by `P::Addr`, so `P` remains the only
/// protocol identity and ordinary domain types carry no endpoint parameter.
/// The endpoint has no direct accessor or send method. It crosses only an
/// explicit interpretation boundary. That boundary is public and therefore a
/// deliberate power-user authority boundary, not exclusive runtime authority.
///
/// ```compile_fail,E0599
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl behavior::Address for RuntimeAddr { type Nonce = u64; }
/// struct Worker;
/// impl behavior::Protocol for Worker { type Addr = RuntimeAddr; type Msg = (); }
/// #[derive(Clone)]
/// struct Endpoint;
/// impl behavior::RecipientAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
/// }
/// let recipient = behavior::EstablishedRecipient::<Worker>::issued(Endpoint);
/// let _endpoint = recipient.endpoint();
/// ```
pub struct EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    pub(crate) endpoint: <P::Addr as RecipientAddress>::Established<P>,
}

impl<P> EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    /// Issue a capability from an exact endpoint established or imported by
    /// an interpreter.
    ///
    /// This constructor performs no allocation or validation. It proves only
    /// protocol messaging; it never establishes a concrete behavior or
    /// lifecycle authority.
    #[must_use]
    pub const fn issued(endpoint: <P::Addr as RecipientAddress>::Established<P>) -> Self {
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
    P::Addr: RecipientAddress,
{
    type Output;

    fn interpret_established(
        &mut self,
        endpoint: <P::Addr as RecipientAddress>::Established<P>,
    ) -> Self::Output;
}

impl<P> Clone for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    fn clone(&self) -> Self {
        Self::issued(self.endpoint.clone())
    }
}

impl<P> core::fmt::Debug for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: core::fmt::Debug,
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
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.endpoint == other.endpoint
    }
}

impl<P> Eq for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Eq,
{
}

/// Inert capability for one exact installed concrete behavior.
///
/// An established recipient proves the public protocol endpoint. This value
/// additionally preserves which concrete behavior was installed, allowing
/// lifecycle effects to require static evidence about that behavior's event
/// algebra. It remains inert: no direct send, shutdown, endpoint accessor, or
/// other ambient effect is exposed.
///
/// A protocol recipient cannot issue installed-actor authority:
///
/// ```compile_fail,E0271
/// #[derive(Clone, Copy, Eq, PartialEq)]
/// struct RuntimeAddr;
/// impl behavior::Address for RuntimeAddr { type Nonce = u8; }
/// #[derive(Clone)]
/// struct Endpoint;
/// struct Installed<B: behavior::Behavior>(Endpoint, std::sync::mpsc::Sender<B::Event>);
/// impl<B: behavior::Behavior> Clone for Installed<B> {
///     fn clone(&self) -> Self { Self(self.0.clone(), self.1.clone()) }
/// }
/// impl behavior::EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
///     type Installed<B> = Installed<B>
///         where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>>;
///     fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
///     where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>> {
///         installed.0.clone()
///     }
/// }
/// struct Worker;
/// impl behavior::Protocol for Worker { type Addr = RuntimeAddr; type Msg = (); }
/// impl behavior::Behavior for Worker {
///     type Protocol = Self;
///     type Event = behavior::User<RuntimeAddr, ()>;
///     type Sends = behavior::NoSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// }
/// let recipient = behavior::EstablishedRecipient::<Worker>::issued(Endpoint);
/// let _: behavior::EstablishedActor<Worker> = behavior::EstablishedActor::issued(recipient);
/// ```
///
/// Two behaviors sharing one protocol retain distinct installed authority:
///
/// ```compile_fail,E0308
/// #[derive(Clone, Copy, Eq, PartialEq)]
/// struct RuntimeAddr;
/// impl behavior::Address for RuntimeAddr { type Nonce = u8; }
/// #[derive(Clone)]
/// struct Endpoint;
/// struct Installed<B: behavior::Behavior>(Endpoint, std::sync::mpsc::Sender<B::Event>);
/// impl<B: behavior::Behavior> Clone for Installed<B> {
///     fn clone(&self) -> Self { Self(self.0.clone(), self.1.clone()) }
/// }
/// impl behavior::EndpointAddress for RuntimeAddr {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
///     type Installed<B> = Installed<B>
///         where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>>;
///     fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
///     where B: behavior::Behavior<Protocol: behavior::Protocol<Addr = Self>> {
///         installed.0.clone()
///     }
/// }
/// struct Shared;
/// impl behavior::Protocol for Shared { type Addr = RuntimeAddr; type Msg = (); }
/// struct First;
/// struct Second;
/// impl behavior::Behavior for First {
///     type Protocol = Shared;
///     type Event = behavior::User<RuntimeAddr, ()>;
///     type Sends = behavior::NoSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// }
/// impl behavior::Behavior for Second {
///     type Protocol = Shared;
///     type Event = behavior::EventLayer<u8, behavior::User<RuntimeAddr, ()>>;
///     type Sends = behavior::NoSends;
///     type Ph = behavior::Never;
///     type Error = behavior::Never;
///     type Birth = behavior::NoBirths;
///     fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event)
///         -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// }
/// let (control, _inbox) = std::sync::mpsc::channel::<<First as behavior::Behavior>::Event>();
/// let first = behavior::EstablishedActor::<First>::issued(Installed(Endpoint, control));
/// let _: behavior::EstablishedActor<Second> = first;
/// ```
pub struct EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    installed: <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>,
}

impl<B> EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    /// Issue an exact actor capability after successful installation.
    ///
    /// This power-user boundary accepts one runtime-owned installed value.
    /// Only successful fresh installation and binding commit may issue it.
    /// It performs no allocation or validation itself.
    #[must_use]
    pub const fn issued(
        installed: <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>,
    ) -> Self {
        Self { installed }
    }

    /// Project the exact public-protocol recipient for this incarnation.
    #[must_use]
    pub fn recipient(&self) -> EstablishedRecipient<B::Protocol> {
        EstablishedRecipient::issued(
            <<B::Protocol as Protocol>::Addr as EndpointAddress>::recipient(&self.installed),
        )
    }

    /// Consume the concrete-actor proof and retain its exact protocol
    /// recipient.
    #[must_use]
    pub fn into_recipient(self) -> EstablishedRecipient<B::Protocol> {
        EstablishedRecipient::issued(
            <<B::Protocol as Protocol>::Addr as EndpointAddress>::recipient(&self.installed),
        )
    }

    /// Transfer the complete installed value with its exact `B` index.
    pub fn interpret_actor<I>(self, interpreter: &mut I) -> I::Output
    where
        I: InterpretInstalledActor<B>,
    {
        interpreter.interpret_actor(self.installed)
    }
}

/// Public power-user transfer boundary for one exact installed actor.
pub trait InterpretInstalledActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    type Output;

    fn interpret_actor(
        &mut self,
        installed: <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>,
    ) -> Self::Output;
}

impl<B> Clone for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
{
    fn clone(&self) -> Self {
        Self::issued(self.installed.clone())
    }
}

impl<B> core::fmt::Debug for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>: core::fmt::Debug,
{
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("EstablishedActor")
            .field(&self.installed)
            .finish()
    }
}

impl<B> PartialEq for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.installed == other.installed
    }
}

impl<B> Eq for EstablishedActor<B>
where
    B: Behavior,
    <B::Protocol as Protocol>::Addr: EndpointAddress,
    <<B::Protocol as Protocol>::Addr as EndpointAddress>::Installed<B>: Eq,
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
/// ```compile_fail,E0308
/// struct Queue;
/// struct Worker;
/// impl behavior::Protocol for Queue {
///     type Addr = behavior::MailAddr;
///     type Msg = u8;
/// }
/// impl behavior::Protocol for Worker {
///     type Addr = behavior::MailAddr;
///     type Msg = u8;
/// }
///
/// let worker = behavior::Recipient::<Worker>::global(behavior::MailAddr(1));
/// let _: behavior::Delivery<Queue> = behavior::Delivery::new(worker, 7);
/// ```
///
/// A destination also fixes its message and address namespaces:
///
/// ```compile_fail,E0308
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct OtherAddr(u64);
/// impl behavior::Address for OtherAddr {
///     type Nonce = u64;
/// }
/// struct Worker;
/// impl behavior::Protocol for Worker {
///     type Addr = behavior::MailAddr;
///     type Msg = u8;
/// }
/// let _ = behavior::Recipient::<Worker>::global(OtherAddr(1));
/// ```
///
/// ```compile_fail,E0308
/// # struct Worker;
/// # impl behavior::Protocol for Worker {
/// #     type Addr = behavior::MailAddr;
/// #     type Msg = u8;
/// # }
/// let worker = behavior::Recipient::<Worker>::global(behavior::MailAddr(1));
/// let _ = behavior::Delivery::<Worker>::new(worker, "wrong payload");
/// ```
/// Exact reason one logical delivery was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicalDeliveryReason {
    UnknownAddress,
    ClosedRecipient,
}

/// Exact reason one established delivery was not accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactDeliveryReason {
    ClosedRecipient,
}

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

impl<P> ActionItem for Delivery<P>
where
    P: Protocol,
    P::Addr: Send,
    P::Msg: Send,
{
    type Accepted = ();
    type Rejection = LogicalDeliveryReason;
    type Prerequisite = Never;
}

/// One pure communication to an exact installed incarnation.
///
/// Unlike [`Delivery`], this effect carries the runtime-issued endpoint and
/// requires no address-to-endpoint resolution. Constructing it remains pure;
/// only an explicit [`crate::InterpretItem`] implementation for this exact
/// delivery type can
/// perform the communication.
///
/// Exact endpoints remain protocol-indexed even when two protocols share an
/// address namespace and message type:
///
/// ```compile_fail,E0308
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct RuntimeAddr(u64);
/// impl behavior::Address for RuntimeAddr { type Nonce = u64; }
/// struct Endpoint<P>(core::marker::PhantomData<fn() -> P>);
/// impl<P> Clone for Endpoint<P> {
///     fn clone(&self) -> Self { Self(core::marker::PhantomData) }
/// }
/// impl behavior::RecipientAddress for RuntimeAddr {
///     type Established<P> = Endpoint<P> where P: behavior::Protocol<Addr = Self>;
/// }
/// struct Queue;
/// struct Worker;
/// impl behavior::Protocol for Queue { type Addr = RuntimeAddr; type Msg = u8; }
/// impl behavior::Protocol for Worker { type Addr = RuntimeAddr; type Msg = u8; }
/// let worker = behavior::EstablishedRecipient::<Worker>::issued(Endpoint(core::marker::PhantomData));
/// let _: behavior::EstablishedDelivery<Queue> = behavior::EstablishedDelivery::new(worker, 7);
/// ```
pub struct EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    pub to: EstablishedRecipient<P>,
    pub message: P::Msg,
}

impl<P> EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
{
    #[must_use]
    pub const fn new(to: EstablishedRecipient<P>, message: P::Msg) -> Self {
        Self { to, message }
    }
}

impl<P> Clone for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    P::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.to.clone(), self.message.clone())
    }
}

impl<P> core::fmt::Debug for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
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
    P::Addr: RecipientAddress,
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
    P::Addr: RecipientAddress,
    EstablishedRecipient<P>: Eq,
    P::Msg: Eq,
{
}

impl<P> ActionItem for EstablishedDelivery<P>
where
    P: Protocol,
    P::Addr: RecipientAddress,
    <P::Addr as RecipientAddress>::Established<P>: Send,
    P::Msg: Send,
{
    type Accepted = ();
    type Rejection = ExactDeliveryReason;
    type Prerequisite = Never;
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
        type Msg = u8;
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

    impl<P> core::fmt::Debug for LocalEndpoint<P> {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("LocalEndpoint")
        }
    }

    impl<P> PartialEq for LocalEndpoint<P> {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.token, &other.token)
        }
    }

    impl<P> Eq for LocalEndpoint<P> {}

    struct LocalInstalled<B: Behavior> {
        endpoint: LocalEndpoint<B::Protocol>,
        control: std::sync::mpsc::Sender<B::Event>,
        incarnation: Rc<()>,
    }

    impl<B: Behavior> Clone for LocalInstalled<B> {
        fn clone(&self) -> Self {
            Self {
                endpoint: self.endpoint.clone(),
                control: self.control.clone(),
                incarnation: self.incarnation.clone(),
            }
        }
    }

    impl<B: Behavior> core::fmt::Debug for LocalInstalled<B> {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("LocalInstalled")
        }
    }

    impl<B: Behavior> PartialEq for LocalInstalled<B> {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.incarnation, &other.incarnation)
        }
    }

    impl<B: Behavior> Eq for LocalInstalled<B> {}

    impl EndpointAddress for LocalAddr {
        type Established<P>
            = LocalEndpoint<P>
        where
            P: Protocol<Addr = Self>;

        type Installed<B>
            = LocalInstalled<B>
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>;

        fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
        where
            B: Behavior<Protocol: Protocol<Addr = Self>>,
        {
            installed.endpoint.clone()
        }
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

    struct LocalEndpointTransfer;

    impl InterpretEstablished<LocalProtocol> for LocalEndpointTransfer {
        type Output = LocalEndpoint<LocalProtocol>;

        fn interpret_established(
            &mut self,
            endpoint: LocalEndpoint<LocalProtocol>,
        ) -> Self::Output {
            endpoint
        }
    }

    struct LocalBehavior;

    impl Behavior for LocalBehavior {
        type Protocol = LocalProtocol;
        type Event = User<LocalAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
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
    fn established_capability_accepts_a_local_non_send_endpoint() {
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
    fn established_values_compare_complete_identity_and_expose_debug_shape() {
        let token = Rc::new(());
        let other_token = Rc::new(());
        let recipient = EstablishedRecipient::<LocalProtocol>::issued(LocalEndpoint {
            token: token.clone(),
            protocol: PhantomData,
        });
        let same_recipient = EstablishedRecipient::<LocalProtocol>::issued(LocalEndpoint {
            token,
            protocol: PhantomData,
        });
        let other_recipient = EstablishedRecipient::<LocalProtocol>::issued(LocalEndpoint {
            token: other_token,
            protocol: PhantomData,
        });

        assert_eq!(recipient, same_recipient);
        assert_ne!(recipient, other_recipient);
        assert_eq!(
            format!("{recipient:?}"),
            "EstablishedRecipient(LocalEndpoint)"
        );

        let (control, _consumer) = std::sync::mpsc::channel();
        let installed = LocalInstalled {
            endpoint: recipient.clone().interpret(&mut LocalEndpointTransfer),
            control,
            incarnation: Rc::new(()),
        };
        let actor = EstablishedActor::<LocalBehavior>::issued(installed.clone());
        let same_actor = EstablishedActor::<LocalBehavior>::issued(installed);
        let (other_control, _other_consumer) = std::sync::mpsc::channel();
        let other_actor = EstablishedActor::<LocalBehavior>::issued(LocalInstalled {
            endpoint: other_recipient
                .clone()
                .interpret(&mut LocalEndpointTransfer),
            control: other_control,
            incarnation: Rc::new(()),
        });
        assert_eq!(actor, same_actor);
        assert_ne!(actor, other_actor);
        assert_eq!(format!("{actor:?}"), "EstablishedActor(LocalInstalled)");

        let delivery = EstablishedDelivery::new(recipient, 7);
        let same_delivery = EstablishedDelivery::new(same_recipient.clone(), 7);
        let other_destination = EstablishedDelivery::new(other_recipient.clone(), 7);
        let other_message = EstablishedDelivery::new(same_recipient, 9);
        let both_different = EstablishedDelivery::new(other_recipient, 9);
        assert_eq!(delivery, same_delivery);
        assert_ne!(delivery, other_destination);
        assert_ne!(delivery, other_message);
        assert_ne!(delivery, both_different);
        assert_eq!(
            format!("{delivery:?}"),
            "EstablishedDelivery { to: EstablishedRecipient(LocalEndpoint), message: 7 }"
        );
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
