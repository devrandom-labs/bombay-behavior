//! Protocol-indexed actor recipients and deliveries.

use core::marker::PhantomData;

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

/// Pure routing intent for one concrete destination behavior protocol.
///
/// The destination behavior is part of the type even when two protocols share
/// the same address namespace and message type. The value contains no mailbox,
/// endpoint, registry entry, or other interpreter-owned capability.
pub struct Recipient<B: Behavior> {
    route: Route<B::Addr>,
    protocol: PhantomData<fn() -> B>,
}

impl<B: Behavior> Copy for Recipient<B> {}

impl<B: Behavior> Clone for Recipient<B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Behavior> Recipient<B> {
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

    pub(crate) fn is_child(self, expected: <B::Addr as Address>::Nonce) -> bool {
        matches!(self.route, Route::Child(nonce) if nonce == expected)
    }

    const fn from_route(route: Route<B::Addr>) -> Self {
        Self {
            route,
            protocol: PhantomData,
        }
    }
}

impl<B: Behavior> PartialEq for Recipient<B> {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl<B: Behavior> Eq for Recipient<B> {}

impl<B: Behavior> core::fmt::Debug for Recipient<B>
where
    B::Addr: core::fmt::Debug,
    <B::Addr as Address>::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.route.fmt(f)
    }
}

/// One pure communication addressed to a concrete behavior protocol.
///
/// Protocol identity is not inferred from the payload. Consequently, two
/// behaviors accepting the same address and message types still have distinct
/// delivery types.
///
/// ```compile_fail
/// use behavior::{Actions, Behavior, Delivery, MailAddr, Never, NoBirths, Recipient, User};
///
/// struct Queue;
/// struct Worker;
/// macro_rules! inert {
///     ($actor:ty) => {
///         impl Behavior for $actor {
///             type Addr = MailAddr;
///             type Msg = u8;
///             type Event = User<MailAddr, u8>;
///             type Sends = Vec<Never>;
///             type Ph = Never;
///             type Error = Never;
///             type Birth = NoBirths;
///             fn init(&mut self) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///             fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> {
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
/// use behavior::{Actions, Address, Behavior, Delivery, MailAddr, Never, NoBirths, Recipient, User};
/// #[derive(Clone, Copy, PartialEq, Eq)]
/// struct OtherAddr(u64);
/// impl Address for OtherAddr {
///     type Nonce = u64;
///     fn birth(self, nonce: u64) -> Self { Self(self.0 ^ nonce) }
/// }
/// struct Worker;
/// impl Behavior for Worker {
///     type Addr = MailAddr;
///     type Msg = u8;
///     type Event = User<MailAddr, u8>;
///     type Sends = Vec<Never>;
///     type Ph = Never;
///     type Error = Never;
///     type Birth = NoBirths;
///     fn init(&mut self) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
///     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// }
/// let _ = Recipient::<Worker>::global(OtherAddr(1));
/// ```
///
/// ```compile_fail
/// # use behavior::{Actions, Behavior, Delivery, MailAddr, Never, NoBirths, Recipient, User};
/// # struct Worker;
/// # impl Behavior for Worker {
/// #     type Addr = MailAddr;
/// #     type Msg = u8;
/// #     type Event = User<MailAddr, u8>;
/// #     type Sends = Vec<Never>;
/// #     type Ph = Never;
/// #     type Error = Never;
/// #     type Birth = NoBirths;
/// #     fn init(&mut self) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// #     fn transition(&mut self, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(Actions::cont()) }
/// # }
/// let worker = Recipient::<Worker>::global(MailAddr(1));
/// let _ = Delivery::<Worker>::new(worker, "wrong payload");
/// ```
pub struct Delivery<B: Behavior> {
    pub to: Recipient<B>,
    pub message: B::Msg,
}

impl<B: Behavior> Delivery<B> {
    #[must_use]
    pub fn new(to: Recipient<B>, message: B::Msg) -> Self {
        Self { to, message }
    }
}

impl<B> Clone for Delivery<B>
where
    B: Behavior,
    B::Msg: Clone,
{
    fn clone(&self) -> Self {
        Self::new(self.to, self.message.clone())
    }
}

impl<B> PartialEq for Delivery<B>
where
    B: Behavior,
    B::Msg: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.to == other.to && self.message == other.message
    }
}

impl<B> Eq for Delivery<B>
where
    B: Behavior,
    B::Msg: Eq,
{
}
