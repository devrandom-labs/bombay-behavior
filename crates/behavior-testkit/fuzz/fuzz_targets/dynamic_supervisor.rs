//! Inert typed runtime values shared by DynamicSupervisor fuzz targets.

#[path = "installed_control.rs"]
pub(super) mod installed_control;

use behavior_actors::atomic::{ImmediateActivation, ProxyInputResult, ProxyOperation, StableProxy};
use behavior_core::{
    ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, CreateChild, CreationId,
    CreationSettlement, CreationsSettled, EndpointAddress, EstablishedActor, ItemSettlement, Never,
    NoBirths, NoSends, Protocol, SettledItem, User,
};

use crate::proxy_control::admit_proxy_operation;

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

    type Installed<B>
        =
        installed_control::InstalledControl<B, <Self as EndpointAddress>::Established<B::Protocol>>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(
        installed: &Self::Installed<B>,
    ) -> <Self as EndpointAddress>::Established<B::Protocol>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        installed.endpoint().clone()
    }
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
        ChildCreationOutcome::Established(behavior_core::CommittedChild::new(
            creation,
            kind,
            behavior_core::EstablishedActor::issued(installed_control::InstalledControl::new(
                WorkerEndpoint,
            )),
        )),
    ));
    (
        creation,
        CreationsSettled::new(CreationSettlement::Settled(
            [settlement].into_iter().collect(),
        )),
    )
}

pub(super) fn accepted_proxy_input<Source>(
    operation: ProxyOperation<Source, Worker, ImmediateActivation>,
) -> ProxyInputResult<Source, Worker, ImmediateActivation> {
    let (_, _, receipt) = admit_proxy_operation(
        operation,
        EstablishedActor::issued(installed_control::InstalledControl::new(WorkerEndpoint)),
    );
    SettledItem::Attempted(ItemSettlement::Accepted(receipt))
}
