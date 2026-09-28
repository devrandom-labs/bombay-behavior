//! An interpreter witness for staged creation and initialization custody.

use behavior_core::{
    Actions, ActiveTurn, Address, AllocationRejection, Behavior, BehaviorActed,
    ChildCreationOutcome, ChildHead, CreateChild, CreationRejection, CreationSequence,
    EndpointAddress, EstablishChild, EstablishedCreation, EstablishedRecipient, InitializationTurn,
    ItemSettlement, Never, NoBirths, Protocol, RoutedCreation, Step, User,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr;

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint;

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildInitialization {
    Continue,
    Stop,
    Reject,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChildError {
    Rejected,
}

struct Child {
    initialization: ChildInitialization,
}

impl Protocol for Child {
    type Addr = RuntimeAddr;
    type Msg = Never;
}

impl Behavior for Child {
    type Protocol = Self;
    type Event = User<RuntimeAddr, Never>;
    type Sends = Vec<u8>;
    type Ph = Never;
    type Error = ChildError;
    type Birth = NoBirths;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        match self.initialization {
            ChildInitialization::Continue => Ok(Actions::cont().with_send(7)),
            ChildInitialization::Stop => Ok(Actions::stop().with_send(9)),
            ChildInitialization::Reject => Err(ChildError::Rejected),
        }
    }

    fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event.message {}
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostPlan {
    Commit,
    RejectAllocation,
    RejectInstallation,
    RejectEffects,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostTrace {
    Reserve,
    Initialize,
    Install,
    Commit,
    Settle(u8),
    EffectsRejected,
    Ready,
    Drain,
}

struct Host {
    plan: HostPlan,
    trace: Vec<HostTrace>,
    retained: Vec<Child>,
}

type ChildBirthSettlement = ItemSettlement<
    RoutedCreation<RuntimeAddr, Child>,
    ChildCreationOutcome<Child, ChildHead>,
    CreationRejection,
    Never,
>;

impl Host {
    fn new(plan: HostPlan) -> Self {
        Self {
            plan,
            trace: Vec::new(),
            retained: Vec::new(),
        }
    }
}

impl EstablishChild<ChildHead, Child> for Host {
    async fn establish_child(
        &mut self,
        mut creation: RoutedCreation<RuntimeAddr, Child>,
    ) -> ChildBirthSettlement {
        self.trace.push(HostTrace::Reserve);
        if self.plan == HostPlan::RejectAllocation {
            return ItemSettlement::Rejected {
                item: creation,
                reason: CreationRejection::Allocation(AllocationRejection::Exhausted),
            };
        }

        self.trace.push(HostTrace::Initialize);
        let initialization = match behavior_core::initialize(creation.child_mut()) {
            Ok(actions) => actions,
            Err(error) => {
                return ItemSettlement::Accepted(ChildCreationOutcome::InitializationRejected {
                    creation,
                    error,
                });
            }
        };

        self.trace.push(HostTrace::Install);
        if self.plan == HostPlan::RejectInstallation {
            return ItemSettlement::Accepted(ChildCreationOutcome::HostRejected {
                creation,
                initialization,
                reason: CreationRejection::EnvironmentFailed,
            });
        }

        self.trace.push(HostTrace::Commit);
        let (request, _) = creation.into_parts();
        let (id, child, kind) = request.into_parts();
        let Actions {
            sends,
            creates,
            become_,
        } = initialization;
        assert!(creates.is_empty());
        for send in sends {
            self.trace.push(HostTrace::Settle(send));
        }
        match (self.plan, become_) {
            (HostPlan::RejectEffects, _) => {
                self.trace.push(HostTrace::EffectsRejected);
                self.trace.push(HostTrace::Drain);
            }
            (_, Step::Continue) => self.trace.push(HostTrace::Ready),
            (_, Step::Stop(_)) => self.trace.push(HostTrace::Drain),
            (_, Step::Goto(never)) => match never {},
        }
        self.retained.push(child);
        ItemSettlement::Accepted(ChildCreationOutcome::Established {
            established: EstablishedCreation::installed(
                id,
                kind,
                EstablishedRecipient::issued(Endpoint),
            ),
        })
    }
}

async fn attempt(
    host_plan: HostPlan,
    child_plan: ChildInitialization,
) -> (Host, ChildBirthSettlement) {
    let id = CreationSequence::new()
        .issue()
        .expect("the first creation correlation is available");
    let creation = RoutedCreation::new(
        CreateChild::birth(
            id,
            Child {
                initialization: child_plan,
            },
        ),
        1,
    );
    let mut host = Host::new(host_plan);
    let settlement = host.establish_child(creation).await;
    (host, settlement)
}

#[tokio::test]
async fn successful_creation_commits_before_effects_and_opens_ingress_last() {
    let (host, settlement) = attempt(HostPlan::Commit, ChildInitialization::Continue).await;
    assert!(matches!(
        settlement,
        ItemSettlement::Accepted(ChildCreationOutcome::Established { .. })
    ));
    assert_eq!(
        host.trace,
        [
            HostTrace::Reserve,
            HostTrace::Initialize,
            HostTrace::Install,
            HostTrace::Commit,
            HostTrace::Settle(7),
            HostTrace::Ready,
        ]
    );
    assert_eq!(host.retained.len(), 1);
}

#[tokio::test]
async fn allocation_rejection_preserves_the_uninitialized_creation() {
    let (host, settlement) =
        attempt(HostPlan::RejectAllocation, ChildInitialization::Continue).await;
    let ItemSettlement::Rejected { item, reason } = settlement else {
        panic!("fresh allocation must reject the staged request");
    };
    assert_eq!(
        reason,
        CreationRejection::Allocation(AllocationRejection::Exhausted)
    );
    let (creation, route) = item.into_parts();
    assert_eq!(route, 1);
    assert_eq!(
        creation.child().initialization,
        ChildInitialization::Continue
    );
    assert_eq!(host.trace, [HostTrace::Reserve]);
    assert!(host.retained.is_empty());
}

#[tokio::test]
async fn initialization_rejection_preserves_the_current_child_and_exact_error() {
    let (host, settlement) = attempt(HostPlan::Commit, ChildInitialization::Reject).await;
    let ItemSettlement::Accepted(ChildCreationOutcome::InitializationRejected { creation, error }) =
        settlement
    else {
        panic!("pure initialization failure is a typed creation outcome");
    };
    assert_eq!(error, ChildError::Rejected);
    assert_eq!(
        creation.into_parts().0.child().initialization,
        ChildInitialization::Reject
    );
    assert_eq!(host.trace, [HostTrace::Reserve, HostTrace::Initialize]);
    assert!(host.retained.is_empty());
}

#[tokio::test]
async fn installation_rejection_preserves_uninterpreted_initialization_actions() {
    let (host, settlement) =
        attempt(HostPlan::RejectInstallation, ChildInitialization::Continue).await;
    let ItemSettlement::Accepted(ChildCreationOutcome::HostRejected {
        creation,
        initialization,
        reason,
    }) = settlement
    else {
        panic!("host refusal retains the staged child and its actions");
    };
    assert_eq!(reason, CreationRejection::EnvironmentFailed);
    assert_eq!(
        creation.into_parts().0.child().initialization,
        ChildInitialization::Continue
    );
    assert_eq!(initialization.sends, [7]);
    assert_eq!(initialization.become_, Step::Continue);
    assert_eq!(
        host.trace,
        [
            HostTrace::Reserve,
            HostTrace::Initialize,
            HostTrace::Install
        ]
    );
    assert!(host.retained.is_empty());
}

#[tokio::test]
async fn stopped_initialization_settles_final_effects_without_opening_ingress() {
    let (host, settlement) = attempt(HostPlan::Commit, ChildInitialization::Stop).await;
    assert!(matches!(
        settlement,
        ItemSettlement::Accepted(ChildCreationOutcome::Established { .. })
    ));
    assert_eq!(
        host.trace,
        [
            HostTrace::Reserve,
            HostTrace::Initialize,
            HostTrace::Install,
            HostTrace::Commit,
            HostTrace::Settle(9),
            HostTrace::Drain,
        ]
    );
    assert_eq!(host.retained.len(), 1);
}

#[tokio::test]
async fn post_commit_effect_failure_drains_without_recasting_the_birth() {
    let (host, settlement) = attempt(HostPlan::RejectEffects, ChildInitialization::Continue).await;
    assert!(matches!(
        settlement,
        ItemSettlement::Accepted(ChildCreationOutcome::Established { .. })
    ));
    assert_eq!(
        host.trace,
        [
            HostTrace::Reserve,
            HostTrace::Initialize,
            HostTrace::Install,
            HostTrace::Commit,
            HostTrace::Settle(7),
            HostTrace::EffectsRejected,
            HostTrace::Drain,
        ]
    );
    assert_eq!(host.retained.len(), 1);
}
