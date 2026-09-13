//! Total settlement contract for exact orderly worker shutdown.

use core::future::Future;

use behavior_actors::{
    ActionItem, Actions, Address, Behavior, BehaviorActed, EndpointAddress, EstablishedActor, Here,
    Ingress, InterpretEstablishedShutdown, InterpretItem, InterpretSends, Interpretation,
    InterpreterRequests, ItemSettlement, Never, NoBirths, Protocol, SettledItem,
    ShutdownEstablished, ShutdownId, ShutdownRejection, ShutdownRequested, StopOnShutdown, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
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

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        _: Self::Event,
    ) -> BehaviorActed<Self> {
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
    calls: Vec<(ShutdownId, Endpoint)>,
}

impl InterpretEstablishedShutdown<StopOnShutdown<Worker>, Here> for ShutdownRuntime {
    fn shutdown(
        &mut self,
        id: ShutdownId,
        endpoint: Endpoint,
        _: Ingress<ShutdownRequested, Here>,
    ) -> Result<(), ShutdownRejection> {
        self.calls.push((id, endpoint));
        match self.admission {
            ShutdownAdmission::Accept => Ok(()),
            ShutdownAdmission::Reject(reason) => Err(reason),
        }
    }
}

impl<RootEvent> InterpretItem<ShutdownEstablished<StopOnShutdown<Worker>, Here>, RootEvent, Here>
    for ShutdownRuntime
{
    fn interpret_item(
        &mut self,
        item: ShutdownEstablished<StopOnShutdown<Worker>, Here>,
    ) -> impl Future<
        Output = ItemSettlement<
            ShutdownEstablished<StopOnShutdown<Worker>, Here>,
            ShutdownId,
            ShutdownRejection,
            Never,
        >,
    > + Send {
        core::future::ready(item.settle(self))
    }
}

fn request(id: u64, endpoint: u64) -> ShutdownEstablished<StopOnShutdown<Worker>, Here> {
    ShutdownEstablished::new(
        ShutdownId(id),
        EstablishedActor::issued(Endpoint(endpoint)),
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
    assert_eq!(runtime.calls, [(ShutdownId(3), Endpoint(41))]);
}

#[test]
fn exact_shutdown_rejections_return_the_complete_request() {
    for reason in [
        ShutdownRejection::AlreadyStopping,
        ShutdownRejection::AlreadyStopped,
    ] {
        let mut runtime = ShutdownRuntime {
            admission: ShutdownAdmission::Reject(reason),
            calls: Vec::new(),
        };
        match request(5, 43).settle(&mut runtime) {
            ItemSettlement::Rejected {
                item,
                reason: returned,
            } => {
                assert_eq!(returned, reason);
                assert_eq!(item.id, ShutdownId(5));
                assert_eq!(item.actor().recipient(), request(8, 43).actor().recipient());
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

    match <_ as InterpretSends<_, User<RuntimeAddr, ()>, Here>>::interpret(sends, &mut runtime)
        .await
    {
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
            (ShutdownId(7), Endpoint(47)),
            (ShutdownId(11), Endpoint(53))
        ]
    );
}
