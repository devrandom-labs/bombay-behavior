//! Inert typed runtime values shared by DynamicSupervisor fuzz targets.

use behavior_actors::atomic::{
    ImmediateActivation, ProxyInputReceipt, ProxyInputResult, ProxyOperationId, StableProxy,
};
use behavior_core::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, CreateChild, CreationId,
    CreationSettlement, CreationsSettled, EndpointAddress, EstablishedActor, EstablishedCreation,
    EstablishedRecipient, ItemSettlement, Never, NoBirths, NoSends, Protocol, SettledItem, User,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct RuntimeAddress;

impl Address for RuntimeAddress {
    type Nonce = u8;
}

#[derive(Clone, Copy)]
pub(super) struct WorkerEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = WorkerEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Worker;

impl Protocol for Worker {
    type Addr = RuntimeAddress;
    type Msg = Never;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

pub(super) fn committed_proxy(
    created: CreateChild<RuntimeAddress, StableProxy<Worker, ImmediateActivation>>,
) -> (
    CreationId,
    CreationsSettled<RuntimeAddress, StableProxy<Worker, ImmediateActivation>>,
) {
    let (creation, _proxy, kind) = created.into_parts();
    let settlement = SettledItem::Attempted(ItemSettlement::Accepted(
        ChildCreationOutcome::Established {
            established: EstablishedCreation::installed(
                creation,
                kind,
                EstablishedRecipient::issued(WorkerEndpoint),
            ),
        },
    ));
    (
        creation,
        CreationsSettled::new(CreationSettlement::Settled(
            [settlement].into_iter().collect(),
        )),
    )
}

pub(super) fn accepted_proxy_input(
    creation: CreationId,
    operation: ProxyOperationId,
) -> ProxyInputResult<behavior_core::Here, Worker, ImmediateActivation> {
    SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
        creation,
        EstablishedActor::issued(WorkerEndpoint),
        operation,
    )))
}
