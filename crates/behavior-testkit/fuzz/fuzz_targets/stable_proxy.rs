//! Current StableProxy fuzz setup and independent exact worker-return model.

#[path = "installed_control.rs"]
pub(super) mod installed_control;

use core::future::Future;
use core::task::{Context, Poll, Waker};
use std::time::Instant;

use behavior_actors::proxy::{ProxyControl, ProxyOutcome, ProxyPhase, StableProxy};
use behavior_actors::worker::{
    ImmediateStartup, StartWorker, WorkerInitializationOutcome, WorkerStartup,
};
use behavior_actors::{Activate as _, Active, ChildStopped, StopOnShutdown};
use behavior_core::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, ChildCreationOutcome, CreateChild,
    CreationId, CreationSettlement, CreationsSettled, EndpointAddress, ItemSettlement, Never,
    NoBirths, Protocol, SettledItem, User,
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
            ChildCreationOutcome::Established(behavior_core::CommittedChild::new(
                worker,
                kind,
                behavior_core::EstablishedActor::issued(installed_control::InstalledControl::new(
                    WorkerEndpoint,
                )),
            )),
        ))]
        .into_iter()
        .collect(),
    ))
}

fn completed_activation(
    startup: StartWorker<Worker, ImmediateStartup>,
) -> WorkerStartup<Worker, ImmediateStartup> {
    let mut future = core::pin::pin!(startup.start());
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("immediate startup must not remain pending"),
    }
}

pub(crate) fn drive_ready_proxy(
    proxy: StableProxy<Worker, ImmediateStartup>,
    control: ProxyControl<Worker, ImmediateStartup>,
) -> (
    Active<StableProxy<Worker, ImmediateStartup>>,
    CreationId,
    ProxyOutcome<Worker, ImmediateStartup>,
) {
    let initialized = proxy.initialize().expect("proxy initialization is pure");
    let mut proxy = initialized.behavior;
    let (worker, outcome) = start_ready_worker(&mut proxy, control);
    (proxy, worker, outcome)
}

pub(crate) fn start_ready_worker(
    proxy: &mut Active<StableProxy<Worker, ImmediateStartup>>,
    control: ProxyControl<Worker, ImmediateStartup>,
) -> (CreationId, ProxyOutcome<Worker, ImmediateStartup>) {
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
        .on(initialization.resolve(WorkerInitializationOutcome::ReadyForStartup))
        .expect("worker initialization authorizes startup");
    let startup = activating
        .sends
        .worker_startups
        .into_requests()
        .pop()
        .expect("one worker startup is requested");
    let started = proxy.on(startup.started()).expect("worker startup starts");
    assert!(started.sends.owner_outcomes.is_empty());
    let outcome = proxy
        .on(completed_activation(startup))
        .expect("worker startup opens the proxy")
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
    ChildStopped::new(worker, Ok(behavior_actors::Exit::Normal), Instant::now())
}
