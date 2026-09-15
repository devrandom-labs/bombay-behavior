//! Owner-scoped delivery capability composition.

use core::marker::PhantomData;

use behavior_actors::{DeliveryRouteFor, ReplyDelivery, ReplyRoute};
use behavior_core::{
    Actions, Address, Behavior, BehaviorActed, EndpointAddress, EstablishedRecipient, Never,
    NoBirths, Protocol, Recipient, User,
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

    fn transition(&mut self, _: behavior_core::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
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
fn owner_scoped_delivery_preserves_logical_exact_and_mixed_capabilities() {
    type Owner = Worker;

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

    let mixed_logical = deliver::<Owner, _>(
        ReplyRoute::<Worker>::logical(Recipient::global(RuntimeAddr(9))),
        3,
    );
    let [ReplyDelivery::Logical(delivery)] = mixed_logical.as_slice() else {
        panic!("the submitted logical capability remains logical")
    };
    assert_eq!(delivery.message, 3);

    let mixed_exact = deliver::<Owner, _>(
        ReplyRoute::<Worker>::established(EstablishedRecipient::issued(Endpoint {
            id: 10,
            protocol: PhantomData,
        })),
        4,
    );
    let [ReplyDelivery::Established(delivery)] = mixed_exact.as_slice() else {
        panic!("the submitted exact capability remains exact")
    };
    assert_eq!(delivery.message, 4);
}
