use core::num::NonZeroUsize;
use std::time::Instant;

use behavior::{
    Address, Behavior, BehaviorActed, EndpointAddress, MessageProtocol, Never, NoBirths, NoSends,
    Protocol, User,
};
use behavior_actors::ChildStopped;
use behavior_actors::atomic::{
    FixedDiagnostic, RecoveryDenialReason, RecoveryDenied, RestartReleaseFailure,
};

#[derive(Clone, Copy, Eq, PartialEq)]
struct RuntimeAddr;

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
struct SearchPlan;
struct SearchSource;

impl Behavior for SearchWorker {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

impl behavior_actors::atomic::ActivationPlan for SearchPlan {
    type Ready = ();
    type Rejection = Never;

    fn activate(
        self,
    ) -> impl core::future::Future<Output = Result<Self::Ready, Self::Rejection>> + Send {
        core::future::ready(Ok(()))
    }
}

impl behavior_actors::atomic::WorkerSource<SearchRole, SearchWorker, SearchPlan> for SearchSource {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

fn inspect_stop(_: &ChildStopped<RuntimeAddr>) {}

fn inspect(denied: &RecoveryDenied<SearchRole, SearchWorker>) {
    match denied.role() {
        SearchRole => {}
    }
    inspect_stop(denied.stopped());
    match denied.reason() {
        RecoveryDenialReason::RestartLimitReached {
            active,
            requested,
            maximum,
        } => {
            let _: (&u32, &NonZeroUsize, &u32) = (active, requested, maximum);
        }
        RecoveryDenialReason::ClockRegressed { previous, observed } => {
            let _: (&Instant, &Instant) = (previous, observed);
        }
        RecoveryDenialReason::RecoveryCountExhausted { admitted } => {
            let _: &u32 = admitted;
        }
        RecoveryDenialReason::ReleaseCalculationFailed(RestartReleaseFailure::DurationOverflow) => {
        }
        RecoveryDenialReason::RecoveryTicketsExhausted => {}
        RecoveryDenialReason::RestartTimersExhausted => {}
    }
}

fn accepts_inspector(_: fn(&RecoveryDenied<SearchRole, SearchWorker>)) {}

#[test]
fn diagnostic_route_names_only_domain_types() {
    accepts_inspector(inspect);
    let _diagnostic_route: behavior::EstablishedRecipient<
        MessageProtocol<
            RuntimeAddr,
            FixedDiagnostic<SearchRole, SearchWorker, SearchPlan, SearchSource>,
        >,
    > = behavior::EstablishedRecipient::issued(Endpoint);
}
