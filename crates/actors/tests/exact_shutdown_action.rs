//! Total settlement contract for exact orderly worker shutdown.

use core::future::{Future, ready};
use core::marker::PhantomData;

use behavior::{
    ActionItem, Actions, Address, Behavior, BehaviorActed, EndpointAddress, EstablishedActor, Here,
    Ingress, InterpretInstalledActor, InterpretItem, InterpretSends, Interpretation,
    InterpretationProgress, InterpreterRequests, ItemSettlement, Never, NoBirths, Protocol,
    SettledItem, User,
};
use behavior_actors::{
    InterpretEstablishedShutdown, ShutdownEstablished, ShutdownId, ShutdownRejection,
    ShutdownRequested, StopOnShutdown,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

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

struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddr;
    type Msg = ();
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[derive(Clone, Copy)]
enum ShutdownAdmission {
    Accept,
    Reject(ShutdownRejection),
}

struct ShutdownRuntime {
    admission: ShutdownAdmission,
    calls: Vec<(ShutdownId, Endpoint, u64)>,
}

impl InterpretEstablishedShutdown<StopOnShutdown<Worker>, Here> for ShutdownRuntime {
    fn shutdown(
        &mut self,
        id: ShutdownId,
        installed: Installed<StopOnShutdown<Worker>>,
        _: Ingress<ShutdownRequested, Here>,
    ) -> Result<(), ShutdownRejection> {
        self.calls.push((id, installed.endpoint, installed.control));
        match self.admission {
            ShutdownAdmission::Accept => Ok(()),
            ShutdownAdmission::Reject(reason) => Err(reason),
        }
    }
}

impl InterpretInstalledActor<StopOnShutdown<Worker>> for ShutdownRuntime {
    type Output = (Endpoint, u64);

    fn interpret_actor(&mut self, installed: Installed<StopOnShutdown<Worker>>) -> Self::Output {
        (installed.endpoint, installed.control)
    }
}

impl<RootEvent> InterpretItem<ShutdownEstablished<StopOnShutdown<Worker>, Here>, RootEvent, Here>
    for ShutdownRuntime
{
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<ShutdownEstablished<StopOnShutdown<Worker>, Here>>,
        received: &'a mut Option<
            <ShutdownEstablished<StopOnShutdown<Worker>, Here> as ActionItem>::Reply,
        >,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        ShutdownEstablished<StopOnShutdown<Worker>, Here>: 'a,
    {
        if received.is_some() {
            return ready(());
        }
        let Some(item) = input.take() else {
            return ready(());
        };
        *received = Some(item.settle(self));
        ready(())
    }
}

fn request(id: u64, endpoint: u64) -> ShutdownEstablished<StopOnShutdown<Worker>, Here> {
    ShutdownEstablished::new(
        ShutdownId(id),
        EstablishedActor::issued(Installed {
            endpoint: Endpoint(endpoint),
            control: endpoint + 100,
            behavior: PhantomData,
        }),
        Ingress::new(),
    )
}

fn exact_shutdown_item(_: &ShutdownEstablished<StopOnShutdown<Worker>, Here>)
where
    ShutdownEstablished<StopOnShutdown<Worker>, Here>:
        ActionItem<Accepted = ShutdownId, Rejection = ShutdownRejection, Prerequisite = Never>,
{
}

#[test]
fn exact_shutdown_acceptance_consumes_the_request_and_keeps_its_id() {
    let request = request(3, 41);
    exact_shutdown_item(&request);
    let mut runtime = ShutdownRuntime {
        admission: ShutdownAdmission::Accept,
        calls: Vec::new(),
    };

    match request.settle(&mut runtime) {
        ItemSettlement::Accepted(id) => assert_eq!(id, ShutdownId(3)),
        _ => panic!("accepted exact shutdown produced the wrong settlement"),
    }
    assert_eq!(runtime.calls, [(ShutdownId(3), Endpoint(41), 141)]);
}

#[test]
fn exact_shutdown_rejections_return_the_complete_request() {
    for (reason, control) in [
        (ShutdownRejection::AlreadyStopping, 983),
        (ShutdownRejection::AlreadyStopped, 984),
    ] {
        let mut runtime = ShutdownRuntime {
            admission: ShutdownAdmission::Reject(reason),
            calls: Vec::new(),
        };
        let original = ShutdownEstablished::new(
            ShutdownId(5),
            EstablishedActor::issued(Installed {
                endpoint: Endpoint(43),
                control,
                behavior: PhantomData,
            }),
            Ingress::new(),
        );
        match original.settle(&mut runtime) {
            ItemSettlement::Rejected {
                item,
                reason: returned,
            } => {
                assert_eq!(returned, reason);
                assert_eq!(item.id, ShutdownId(5));
                let installed = item.actor().interpret_actor(&mut runtime);
                assert_eq!(installed, (Endpoint(43), control));
            }
            _ => panic!("rejected exact shutdown lost its request"),
        }
    }
}

#[tokio::test]
async fn exact_shutdown_uses_the_generic_ordered_product_law() {
    let sends = InterpreterRequests::new(vec![request(7, 47), request(11, 53)]);
    let mut runtime = ShutdownRuntime {
        admission: ShutdownAdmission::Accept,
        calls: Vec::new(),
    };

    match {
        let mut progress = Some(InterpretationProgress::Original(sends));
        <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(
            &mut progress,
            &mut runtime,
        )
        .await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the actual shutdown product must retain its complete settlement");
        };
        settlement
    } {
        Interpretation::Complete(settlements) => {
            assert_eq!(settlements.len(), 2);
            match &settlements[0] {
                SettledItem::Attempted(ItemSettlement::Accepted(id)) => {
                    assert_eq!(*id, ShutdownId(7));
                }
                _ => panic!("first exact shutdown did not settle"),
            }
            match &settlements[1] {
                SettledItem::Attempted(ItemSettlement::Accepted(id)) => {
                    assert_eq!(*id, ShutdownId(11));
                }
                _ => panic!("second exact shutdown did not settle"),
            }
        }
        Interpretation::Corrupt(_) => panic!("exact shutdown traversal corrupted"),
    }
    assert_eq!(
        runtime.calls,
        [
            (ShutdownId(7), Endpoint(47), 147),
            (ShutdownId(11), Endpoint(53), 153)
        ]
    );
}
