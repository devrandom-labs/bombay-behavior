//! Typed actor addresses, routes, recipients, and deliveries.

use core::marker::PhantomData;

/// A pure actor-address namespace.
pub trait Address: Copy + Eq {
    type Nonce: Copy + Eq;

    #[must_use]
    fn birth(self, nonce: Self::Nonce) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MailAddr(pub u64);

impl Address for MailAddr {
    type Nonce = u64;

    fn birth(self, nonce: u64) -> Self {
        Self(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

/// An address expression for ordinary actor delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route<A: Address> {
    Global(A),
    Child(A::Nonce),
}

/// A recipient statically coupled to the message it accepts.
pub struct Recipient<A: Address, M> {
    route: Route<A>,
    message: PhantomData<fn(M)>,
}

impl<A: Address, M> Copy for Recipient<A, M> {}

impl<A: Address, M> Clone for Recipient<A, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Address, M> Recipient<A, M> {
    #[must_use]
    pub fn global(address: A) -> Self {
        Self::from_route(Route::Global(address))
    }

    #[must_use]
    pub fn child(nonce: A::Nonce) -> Self {
        Self::from_route(Route::Child(nonce))
    }

    #[must_use]
    pub fn route(self) -> Route<A> {
        self.route
    }

    const fn from_route(route: Route<A>) -> Self {
        Self {
            route,
            message: PhantomData,
        }
    }
}

impl<A: Address, M> PartialEq for Recipient<A, M> {
    fn eq(&self, other: &Self) -> bool {
        self.route == other.route
    }
}

impl<A: Address, M> Eq for Recipient<A, M> {}

impl<A: Address + core::fmt::Debug, M> core::fmt::Debug for Recipient<A, M>
where
    A::Nonce: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.route.fmt(f)
    }
}

/// One statically typed send operation.
#[derive(Clone, PartialEq, Eq)]
pub struct Delivery<A: Address, M> {
    pub to: Recipient<A, M>,
    pub message: M,
}

impl<A: Address, M> Delivery<A, M> {
    #[must_use]
    pub fn new(to: Recipient<A, M>, message: M) -> Self {
        Self { to, message }
    }
}
