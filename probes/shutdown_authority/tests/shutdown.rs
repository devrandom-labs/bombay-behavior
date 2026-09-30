use core::marker::PhantomData;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, EndpointAddress, EstablishedActor,
    EstablishedRecipient, EventLayer, Here, InjectEvent, InterpretInstalledActor, Never, NoBirths,
    Protocol, User,
};
use communication::{channel, Config, ControlClosed, ControlSender};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

struct InstalledControl<B: Behavior> {
    endpoint: Endpoint,
    control: ControlSender<B::Event>,
}

impl<B: Behavior> Clone for InstalledControl<B> {
    fn clone(&self) -> Self {
        Self {
            endpoint: self.endpoint,
            control: self.control.clone(),
        }
    }
}

impl EndpointAddress for RuntimeAddr {
    type Established<P> = Endpoint where P: Protocol<Addr = Self>;

    type Installed<B> = InstalledControl<B> where B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> Endpoint
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

#[derive(Debug, Eq, PartialEq)]
struct ShutdownRequested(u64);

struct DirectService;
struct LayeredService;

impl Behavior for DirectService {
    type Protocol = Service;
    type Event = EventLayer<ShutdownRequested, User<RuntimeAddr, ()>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::stop())
    }
}

impl Behavior for LayeredService {
    type Protocol = Service;
    type Event = EventLayer<ShutdownRequested, EventLayer<u8, User<RuntimeAddr, ()>>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::stop())
    }
}

struct ControlRuntime<Path> {
    seen: Vec<Endpoint>,
    path: PhantomData<fn() -> Path>,
}

impl<Path> ControlRuntime<Path> {
    fn new() -> Self {
        Self { seen: Vec::new(), path: PhantomData }
    }
}

impl<B, Path> InterpretInstalledActor<B> for ControlRuntime<Path>
where
    B: Behavior<Protocol: Protocol<Addr = RuntimeAddr>>,
    B::Event: InjectEvent<ShutdownRequested, Path>,
{
    type Output = Result<(), ControlClosed<B::Event>>;

    fn interpret_actor(&mut self, installed: InstalledControl<B>) -> Self::Output {
        self.seen.push(installed.endpoint);
        installed.control.send(B::Event::inject_at(ShutdownRequested(42)))
    }
}

#[tokio::test]
async fn exact_control_survives_transfer_and_stale_address_reuse() {
    let (old_control, _, old_consumer) = channel::<<DirectService as Behavior>::Event, ()>(Config::new(2));
    let old = EstablishedActor::<DirectService>::issued(InstalledControl {
        endpoint: Endpoint(7),
        control: old_control,
    });
    let (new_control, _, mut new_consumer) = channel::<<DirectService as Behavior>::Event, ()>(Config::new(2));
    let fresh = EstablishedActor::<DirectService>::issued(InstalledControl {
        endpoint: Endpoint(7),
        control: new_control,
    });
    let (other_control, _, mut other_consumer) = channel::<<LayeredService as Behavior>::Event, ()>(Config::new(2));
    let unrelated = EstablishedActor::<LayeredService>::issued(InstalledControl {
        endpoint: Endpoint(9),
        control: other_control,
    });

    assert_eq!(old.recipient(), EstablishedRecipient::issued(Endpoint(7)));
    assert_eq!(fresh.recipient(), EstablishedRecipient::issued(Endpoint(7)));
    assert_eq!(unrelated.recipient(), EstablishedRecipient::issued(Endpoint(9)));

    drop(old_consumer);
    let mut runtime = ControlRuntime::<Here>::new();
    let Err(ControlClosed(old_event)) = old.interpret_actor(&mut runtime) else {
        panic!("closed old control must return its exact event");
    };
    assert!(matches!(old_event, EventLayer::Owned(ShutdownRequested(42))));
    assert!(fresh.interpret_actor(&mut runtime).is_ok());
    assert!(unrelated.interpret_actor(&mut runtime).is_ok());
    assert_eq!(runtime.seen, [Endpoint(7), Endpoint(7), Endpoint(9)]);
    assert!(matches!(new_consumer.recv_control().await, Some(EventLayer::Owned(ShutdownRequested(42)))));
    assert!(matches!(other_consumer.recv_control().await, Some(EventLayer::Owned(ShutdownRequested(42)))));
}
