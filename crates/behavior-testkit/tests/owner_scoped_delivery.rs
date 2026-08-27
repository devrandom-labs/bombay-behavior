//! Owner-scoped delivery capability composition.

use core::marker::PhantomData;

use behavior::{DeliveryRouteFor, Proxy};
use foundation::{
    Actions, Address, Behavior, BehaviorActed, ChildHead, ChildRoute, EndpointAddress,
    EstablishedRecipient, Never, NoBirths, Protocol, Recipient, User,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

struct Endpoint<P> {
    id: u64,
    protocol: PhantomData<fn() -> P>,
}

impl<P> Clone for Endpoint<P> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            protocol: PhantomData,
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint<P>
    where
        P: Protocol<Addr = Self>;
}

struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = u8;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn deliver<Owner, Route>(route: Route, message: <Route::Protocol as Protocol>::Msg) -> Route::Sends
where
    Owner: Behavior,
    Route: DeliveryRouteFor<Owner>,
{
    route.deliver_for(message)
}

#[test]
fn one_owner_scoped_contract_selects_logical_exact_and_proven_direct_child_effects() {
    type Owner = Proxy<Worker>;

    let logical = deliver::<Owner, _>(Recipient::<Worker>::global(RuntimeAddr(7)), 1);
    assert_eq!(logical.len(), 1);
    assert_eq!(logical[0].to.address(), RuntimeAddr(7));
    assert_eq!(logical[0].message, 1);

    let exact = deliver::<Owner, _>(
        EstablishedRecipient::<Worker>::issued(Endpoint {
            id: 8,
            protocol: PhantomData,
        }),
        2,
    );
    assert_eq!(exact.len(), 1);
    assert_eq!(exact[0].message, 2);

    let child = deliver::<Owner, _>(ChildRoute::<Worker, ChildHead>::new(9), 3);
    assert_eq!(child.len(), 1);
    assert_eq!(child[0].nonce, 9);
    assert_eq!(child[0].message, 3);
}
