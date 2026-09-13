//! Current StableProxy fuzz setup and independent exact worker-return model.

use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::time::Instant;

use behavior::atomic::{
    BeginActivation, ImmediateActivation, ProxyControl, ProxyOutcome, ProxyPhase, StableProxy,
    WorkerActivation, WorkerInitializationOutcome,
};
use behavior::{
    Actions, Activate as _, Active, ActiveTurn, Address, Behavior, BehaviorActed,
    ChildCreationOutcome, ChildStopped, CreateChild, CreationId, CreationSettlement,
    CreationsSettled, EndpointAddress, EstablishedCreation, EstablishedRecipient, ItemSettlement,
    Never, NoBirths, Protocol, SettledItem, StopOnShutdown, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeAddress;

impl Address for RuntimeAddress {
    type Nonce = u8;
}

#[derive(Clone, Copy)]
pub(crate) struct WorkerEndpoint;

impl EndpointAddress for RuntimeAddress {
    type Established<P>
        = WorkerEndpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Worker(u8);

impl Worker {
    pub(crate) const fn new(value: u8) -> Self {
        Self(value)
    }
}

impl Protocol for Worker {
    type Addr = RuntimeAddress;
    type Msg = ();
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<RuntimeAddress, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn created_worker(
    creation: CreateChild<RuntimeAddress, StopOnShutdown<Worker>>,
) -> CreationsSettled<RuntimeAddress, StopOnShutdown<Worker>> {
    let (worker, _actor, kind) = creation.into_parts();
    CreationsSettled::new(CreationSettlement::Settled(
        [SettledItem::Attempted(ItemSettlement::Accepted(
            ChildCreationOutcome::Established {
                established: EstablishedCreation::installed(
                    worker,
                    kind,
                    EstablishedRecipient::issued(WorkerEndpoint),
                ),
            },
        ))]
        .into_iter()
        .collect(),
    ))
}

fn completed_activation(
    activation: BeginActivation<Worker, ImmediateActivation>,
) -> WorkerActivation<Worker, ImmediateActivation> {
    let mut future = core::pin::pin!(activation.activate());
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("immediate activation must not remain pending"),
    }
}

pub(crate) fn drive_ready_proxy(
    proxy: StableProxy<Worker, ImmediateActivation>,
    control: ProxyControl<Worker, ImmediateActivation>,
) -> (
    Active<StableProxy<Worker, ImmediateActivation>>,
    CreationId,
    ProxyOutcome<Worker, ImmediateActivation>,
) {
    let initialized = proxy.initialize().expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let (worker, outcome) = start_ready_worker(&mut proxy, control);
    (proxy, worker, outcome)
}

pub(crate) fn start_ready_worker(
    proxy: &mut Active<StableProxy<Worker, ImmediateActivation>>,
    control: ProxyControl<Worker, ImmediateActivation>,
) -> (CreationId, ProxyOutcome<Worker, ImmediateActivation>) {
    let starting = proxy.on(control).expect("worker start emits one creation");
    let creation = starting
        .creates
        .into_iter()
        .next()
        .expect("initial start owns one worker creation");
    let worker = creation.id();
    let initializing = proxy
        .on(created_worker(creation))
        .expect("worker creation commits");
    let initialization = initializing
        .sends
        .worker_initializations
        .into_requests()
        .pop()
        .expect("one worker initialization is requested");
    let activating = proxy
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForActivation))
        .expect("worker initialization authorizes activation");
    let activation = activating
        .sends
        .worker_activations
        .into_requests()
        .pop()
        .expect("one worker activation is requested");
    let started = proxy
        .on(activation.started())
        .expect("worker activation starts");
    assert!(started.sends.owner_outcomes.is_empty());
    let outcome = proxy
        .on(completed_activation(activation))
        .expect("worker activation opens the proxy")
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("ready proxy reports one initial outcome")
        .into_inner();
    assert_eq!(proxy.phase(), ProxyPhase::Ready);
    (worker, outcome)
}

pub(crate) fn worker_stopped(worker: CreationId) -> ChildStopped<RuntimeAddress> {
    ChildStopped::new(worker, Ok(behavior::Exit::Normal), Instant::now())
}
