use core::marker::PhantomData;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, EndpointAddress, EstablishedActor,
    EstablishedRecipient, EventLayer, InterpretInstalledActor, Never, NoBirths, Protocol,
    RecipientAddress, User,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Endpoint(u64);

#[derive(Clone, Copy, PartialEq, Eq)]
struct DeliveryOnlyAddr;

impl Address for DeliveryOnlyAddr {
    type Nonce = u64;
}

impl RecipientAddress for DeliveryOnlyAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

struct DeliveryOnly;

impl Protocol for DeliveryOnly {
    type Addr = DeliveryOnlyAddr;
    type Msg = ();
}

struct Installed<B: Behavior> {
    endpoint: Endpoint,
    control: u64,
    behavior: PhantomData<fn() -> B>,
}

impl<B: Behavior> Clone for Installed<B> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint,
            control: self.control,
            behavior: PhantomData,
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
    type Installed<B>
        = Installed<B>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> Self::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint
    }
}

struct Service;

impl Protocol for Service {
    type Addr = RuntimeAddr;
    type Msg = ();
}

struct Plain;

impl Behavior for Plain {
    type Protocol = Service;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Controlled;

impl Behavior for Controlled {
    type Protocol = Service;
    type Event = EventLayer<u64, User<RuntimeAddr, ()>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Runtime;

impl InterpretInstalledActor<Plain> for Runtime {
    type Output = (Endpoint, u64);

    fn interpret_actor(&mut self, installed: Installed<Plain>) -> Self::Output {
        (installed.endpoint, installed.control)
    }
}

impl InterpretInstalledActor<Controlled> for Runtime {
    type Output = (Endpoint, u64);

    fn interpret_actor(&mut self, installed: Installed<Controlled>) -> Self::Output {
        (installed.endpoint, installed.control)
    }
}

#[test]
fn one_runtime_issued_value_retains_endpoint_and_exact_behavior_control() {
    let plain = EstablishedActor::<Plain>::issued(Installed {
        endpoint: Endpoint(4),
        control: 19,
        behavior: PhantomData,
    });
    let controlled = EstablishedActor::<Controlled>::issued(Installed {
        endpoint: Endpoint(4),
        control: 23,
        behavior: PhantomData,
    });

    assert_eq!(plain.recipient(), EstablishedRecipient::issued(Endpoint(4)));
    assert_eq!(
        controlled.recipient(),
        EstablishedRecipient::issued(Endpoint(4))
    );
    assert_eq!(plain.interpret_actor(&mut Runtime), (Endpoint(4), 19));
    assert_eq!(controlled.interpret_actor(&mut Runtime), (Endpoint(4), 23));
}

#[test]
fn protocol_only_destination_needs_no_installed_actor_family() {
    let recipient = EstablishedRecipient::<DeliveryOnly>::issued(Endpoint(8));
    assert_eq!(recipient, EstablishedRecipient::issued(Endpoint(8)));
}
