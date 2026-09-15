use behavior::{Actions, Never};
use behavior_actors::Activate as _;
use behavior_actors::atomic::{
    ActivationPolicy, ActorDrainPolicy, Assignment, BacklogCapacity, DiagnosticDisposition,
    ImmediateActivation, Interruption, OrderedRoles, PoolFailureReaction, PoolRecovery,
    WorkerSubmission, fifo, pool_worker,
};

use super::RuntimeAddr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchRole {
    Primary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchJob(u8);

#[derive(Debug, Eq, PartialEq)]
struct SearchResult(u16);

struct SearchWorker;

#[pool_worker(addr = RuntimeAddr, result = SearchResult)]
impl SearchWorker {
    fn process(&mut self, job: &SearchJob) -> SearchResult {
        SearchResult(u16::from(job.0) + 100)
    }

    fn transition(&mut self, assignment: Assignment<SearchJob>) -> WorkerActed<Self> {
        let result = self.process(assignment.payload());
        Ok(Actions::cont().with_send(assignment.complete(result)))
    }
}

fn prepare_worker(
    _: &SearchRole,
) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}

#[test]
fn canonical_fifo_worker_and_constructor_need_no_structural_types() {
    let roles = OrderedRoles::new(SearchRole::Primary, core::iter::empty())
        .unwrap_or_else(|_| panic!("one role is a valid roster"));
    let pool = fifo(
        prepare_worker,
        roles,
        ActivationPolicy::new(1).unwrap_or_else(|_| panic!("one activation is valid")),
        PoolRecovery::temporary(PoolFailureReaction::RetireRole),
        BacklogCapacity::new(8),
        Interruption::Retry,
        ActorDrainPolicy::WaitForActorGraph,
        DiagnosticDisposition::terminate(),
    )
    .unwrap_or_else(|_| panic!("the infallible worker declaration constructs a pool"));

    let initialized = pool
        .initialize()
        .unwrap_or_else(|_| panic!("FIFO initialization is pure"));
    assert_eq!(initialized.actions.creates.len(), 1);
}

fn accepts_timed_pool_worker<W>(_: W)
where
    W: behavior::Behavior + behavior::BehaviorBase,
    W::Protocol: behavior::Protocol<Addr = RuntimeAddr, Msg = Assignment<SearchJob>>,
    W::Sends: behavior_actors::atomic::CompletesAssignments<WorkerResult = SearchResult>
        + behavior::SendSettlements,
{
}

#[test]
fn actual_timer_wrappers_preserve_completion_in_both_orders() {
    accepts_timed_pool_worker(behavior_actors::ReceiveTimeout::new(
        behavior_actors::Deadline::new(SearchWorker, behavior_actors::TimerId(10), None, |_| {
            behavior::Step::Stop(behavior::Stopped)
        }),
        behavior_actors::TimerId(11),
        std::time::Duration::from_secs(5),
        |_| Actions::stop(),
    ));
    accepts_timed_pool_worker(behavior_actors::Deadline::new(
        behavior_actors::ReceiveTimeout::new(
            SearchWorker,
            behavior_actors::TimerId(12),
            std::time::Duration::from_secs(5),
            |_| Actions::stop(),
        ),
        behavior_actors::TimerId(13),
        None,
        |_| behavior::Step::Stop(behavior::Stopped),
    ));
}
