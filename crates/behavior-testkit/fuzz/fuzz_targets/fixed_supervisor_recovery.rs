//! Actual stopped search-service setup shared by recovery sequence targets.

use core::convert::Infallible;

use behavior::atomic::{
    CapabilityResult, FixedCommand, FixedSupervisor, ImmediateActivation, PrepareWorkers,
    ProxyOutcome, Recovery, StableProxy, WorkerAttempt, WorkerSource,
};
use behavior::{
    Active, ChildReport, MessageProtocol, NoSends, Recipient, ReplyDelivery, SendSettlements,
    SettledItem, Step,
};

use crate::fixed_supervisor::{Role, ready_supervisor};
use crate::stable_proxy::{RuntimeAddress, Worker, worker_stopped};

pub(crate) struct SearchRecovery<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    pub(crate) supervisor:
        Active<FixedSupervisor<Role, Worker, ImmediateActivation, Source, Infallible, Infallible>>,
    pub(crate) proxy: Active<StableProxy<Worker, ImmediateActivation>>,
    pub(crate) proxy_id: behavior::CreationId,
    pub(crate) previous: WorkerAttempt,
    pub(crate) preparation: PrepareWorkers<Source, Role, Worker, ImmediateActivation>,
}

pub(crate) fn search_recovery<Source>(recovery: Recovery<Source>) -> SearchRecovery<Source>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    let ready = ready_supervisor(recovery);
    let mut supervisor = ready.supervisor;
    let mut proxy = ready.proxy;
    let proxy_id = ready.proxy_id;
    let previous = ready.worker;
    let stopped = proxy
        .on(worker_stopped(previous.creation()))
        .expect("the actual worker stop reaches its stable proxy");
    let stop_report = stopped
        .sends
        .owner_outcomes
        .into_requests()
        .pop()
        .expect("the stable proxy reports its exact worker stop")
        .into_inner();
    assert!(matches!(stop_report, ProxyOutcome::WorkerStopped { .. }));
    let preparing = supervisor
        .on(ChildReport::new(proxy_id, stop_report))
        .unwrap_or_else(|_| panic!("the exact eligible stop begins preparation"));
    let preparation = match preparing
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one-role recovery emits one preparation")
    {
        SettledItem::Unattempted(preparation) => preparation,
        SettledItem::Attempted(_) => panic!("the preparation has not been interpreted"),
    };

    SearchRecovery {
        supervisor,
        proxy,
        proxy_id,
        previous,
        preparation,
    }
}

pub(crate) fn search_capability<Source>(
    supervisor: &mut Active<
        FixedSupervisor<Role, Worker, ImmediateActivation, Source, Infallible, Infallible>,
    >,
) -> CapabilityResult<Role, Worker>
where
    Source: WorkerSource<Role, Worker, ImmediateActivation>,
{
    let queried = supervisor
        .receive(
            RuntimeAddress,
            FixedCommand::capability(
                Role::Search,
                Recipient::<MessageProtocol<RuntimeAddress, CapabilityResult<Role, Worker>>>::global(
                    RuntimeAddress,
                ),
            ),
        )
        .unwrap_or_else(|_| panic!("capability query observes the search service"));
    assert!(queried.creates.is_empty());
    assert!(queried.sends.proxy_observations.into_requests().is_empty());
    let preparations = queried
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs();
    assert!(preparations.is_empty());
    let operations = queried.sends.proxy_operations.unattempted().into_inputs();
    assert!(operations.is_empty());
    let schedules = queried.sends.restart_schedules.unattempted().into_inputs();
    assert!(schedules.is_empty());
    let NoSends = queried.sends.lifecycle;
    assert!(queried.sends.status_replies.into_deliveries().is_empty());
    assert!(queried.sends.diagnostics.is_empty());
    assert!(matches!(queried.become_, Step::Continue));
    let reply = queried
        .sends
        .capability_replies
        .into_deliveries()
        .pop()
        .expect("one capability reply is emitted");
    let ReplyDelivery::Logical(delivery) = reply else {
        panic!("the logical capability recipient remains logical")
    };
    delivery.message
}
