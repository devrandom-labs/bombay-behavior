use behavior_actors::atomic::{Assignment, ImmediateActivation, WorkerSubmission, pool_worker};
use behavior_actors::{Actions, Address, EndpointAddress, Never, Protocol};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RuntimeAddr(pub(super) u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Endpoint(pub(super) u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum SearchRole {
    Primary,
    Replica,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Account(pub(super) u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SearchJob(pub(super) u8);

#[derive(Debug, Eq, PartialEq)]
pub(super) struct SearchResult(pub(super) u16);

pub(super) struct SearchWorker;

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

pub(super) fn prepare_worker(
    _: &SearchRole,
) -> Result<WorkerSubmission<SearchWorker, ImmediateActivation>, Never> {
    Ok(WorkerSubmission::immediate(SearchWorker))
}
