use behavior_actors::atomic::{
    ActivationPlan, FixedDiagnostic, WorkerPreparationFailure, WorkerPreparationFailureReason,
    WorkerSource,
};
use behavior_actors::{
    ActiveTurn, Address, Behavior, BehaviorActed, EndpointAddress, InterpreterFault,
    MessageProtocol, Never, NoBirths, NoSends, Protocol, Recipient, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone)]
struct Endpoint;

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

struct SearchRole;

struct SearchWorker;

impl Behavior for SearchWorker {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

struct SearchPlan;

impl ActivationPlan for SearchPlan {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

struct SearchSource;
struct WorkerRefused;
struct SourceUnavailable;

impl WorkerSource<SearchRole, SearchWorker, SearchPlan> for SearchSource {
    type WorkerRejection = WorkerRefused;
    type SourceRejection = SourceUnavailable;
}

fn inspect_failure(
    failure: &WorkerPreparationFailure<
        SearchRole,
        SearchWorker,
        SearchPlan,
        WorkerRefused,
        SourceUnavailable,
    >,
) {
    match failure.role() {
        SearchRole => {}
    }
    for (role, submission) in failure.prepared() {
        let _: &SearchRole = role;
        let _ = submission;
    }
    for role in failure.remaining_roles() {
        let _: &SearchRole = role;
    }
    match failure.reason() {
        WorkerPreparationFailureReason::WorkerRejected { role, reason } => {
            let _: &SearchRole = role;
            let _: &WorkerRefused = reason;
        }
        WorkerPreparationFailureReason::SourceRejected(reason) => {
            let _: &SourceUnavailable = reason;
        }
        WorkerPreparationFailureReason::InterpreterFault(
            InterpreterFault::MissingCapability | InterpreterFault::CorruptTraversal,
        )
        | WorkerPreparationFailureReason::Unattempted => {}
    }
}

fn accepts_inspector<Inspector>(_: Inspector) {}

#[test]
fn diagnostic_route_names_only_domain_types() {
    accepts_inspector(inspect_failure);
    let route: Recipient<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchPlan, SearchSource>,
        >,
    > = Recipient::global(RuntimeAddr(8));
    assert_eq!(route.address(), RuntimeAddr(8));
}
