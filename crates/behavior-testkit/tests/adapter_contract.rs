use std::time::{Duration, Instant};

use behavior::{
    Actions, Activate, Behavior, BehaviorActed, BehaviorBase, Births, ChildChoice, ChildTopology,
    Create, Delivery, EventIngress, Here, InterpreterRequests, JobId, MailAddr, Never, NoBirths,
    ObserveChild, PoolAssignment, PoolConfiguration, PoolMessage, PoolResponse, Proxy, Recipient,
    RestartConfiguration, RestartPolicy, StashRoute, Step, Strategy, Supervise,
    SupervisionLifecycle, SupervisorSends, TimerId, User, UserEvent, WorkerPool,
    WorkerPoolProtocol,
};

struct Sink;

impl behavior::Protocol for Sink {
    type Addr = MailAddr;
    type Msg = u64;
}

impl Behavior for Sink {
    type Protocol = Self;
    type Event = User<MailAddr, u64>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Domain;

impl behavior::Protocol for Domain {
    type Addr = MailAddr;
    type Msg = u64;
}

impl Behavior for Domain {
    type Protocol = Self;
    type Event = User<MailAddr, u64>;
    type Sends = Vec<Delivery<Sink>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(event.from),
            event.message,
        )]))
    }
}

impl behavior::BehaviorBase for Domain {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "StashRoute requires one uniform reaction signature for every message type"
)]
fn deliver(_: &u64) -> StashRoute {
    StashRoute::Deliver
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "DeadlineReaction requires the behavior's exact controlled-failure result"
)]
fn deadline(_: &mut behavior::Stash<Domain>) -> behavior::Become {
    Step::Continue
}

fn accepts_closed_behavior<B>(behavior: B) -> B
where
    B: Behavior<Ph = Never>,
{
    behavior
}

#[test]
fn inferred_stack_crosses_one_generic_adapter_boundary() {
    let inferred = accepts_closed_behavior(behavior::Deadline::new(
        behavior::Stash::new(Domain, deliver),
        TimerId(4),
        Some(Instant::now()),
        deadline,
    ));
    let initialized = inferred.initialize().unwrap();

    let Actions {
        sends:
            behavior::SendLayer {
                owned: schedules,
                inner: behavior,
            },
        creates,
        become_,
    } = initialized.actions;
    assert!(behavior.is_empty());
    assert!(creates.is_empty());
    assert!(matches!(become_, Step::Continue));
    let [schedule] = schedules.as_slice() else {
        panic!("one named absolute schedule lane")
    };
    assert_eq!(schedule.id, TimerId(4));
}

struct Child;

impl behavior::Protocol for Child {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Child {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Parent;

enum ParentEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, ()>),
}

impl UserEvent for ParentEvent {
    type Addr = MailAddr;
    type Message = ();

    fn user(from: MailAddr, message: ()) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, ()>, Self> {
        match self {
            Self::User(user) => Ok(user),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for ParentEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl behavior::Protocol for Parent {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Parent {
    type Protocol = Self;
    type Event = ParentEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::create(vec![Create::birth(9, Child)]))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ParentEvent::Lifecycle(_lifecycle) => {}
            ParentEvent::User(_user) => {}
        }
        Ok(Actions::cont())
    }
}

impl BehaviorBase for Parent {
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "ChildTopology factories represent index rejection with absence"
)]
fn child(_: usize) -> Option<Child> {
    Some(Child)
}

#[test]
fn supervision_preserves_inner_births_without_claiming_their_lifecycle() {
    let supervisor = Supervise::new(
        Parent,
        ChildTopology::new([1, 2], child),
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            4,
            Duration::from_secs(1),
            behavior::RestartTiming::Immediate,
        ),
        Proxy::new,
    )
    .unwrap();
    let initialized = supervisor.initialize().unwrap();
    let Actions {
        sends:
            behavior::SendLayer {
                owned:
                    SupervisorSends {
                        child_observations,
                        creation_observations,
                        schedules,
                        replacement_inputs,
                        failure_reports,
                        shutdowns,
                    },
                inner: behavior,
            },
        creates,
        become_,
    } = initialized.actions;

    let created: Vec<_> = creates.iter().map(|creation| creation.nonce).collect();
    assert_eq!(created, [9, 1, 2]);
    assert!(matches!(&creates[0].child, ChildChoice::Head(_)));
    assert!(matches!(&creates[1].child, ChildChoice::Tail(_)));
    assert!(matches!(&creates[2].child, ChildChoice::Tail(_)));
    let observed: Vec<_> = child_observations
        .iter()
        .map(|ObserveChild { nonce, .. }| *nonce)
        .collect();
    assert_eq!(observed, [1, 2]);
    let creation_observed: Vec<_> = creation_observations
        .iter()
        .map(|behavior::ObserveCreation { nonce, .. }| *nonce)
        .collect();
    assert_eq!(creation_observed, [1, 2]);
    assert!(behavior.is_empty());
    assert!(schedules.is_empty());
    assert!(replacement_inputs.is_empty());
    assert!(failure_reports.is_empty());
    assert!(shutdowns.is_empty());
    assert!(matches!(become_, Step::Continue));
}

struct Reply;

impl behavior::Protocol for Reply {
    type Addr = MailAddr;
    type Msg = PoolResponse<u8, (), MailAddr>;
}

impl Behavior for Reply {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Worker;
impl behavior::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = PoolAssignment<u8>;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, behavior::BehaviorMessage<Self>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[test]
fn pool_configuration_separates_topology_from_runtime_neutral_policy() {
    fn owned_pool<B>(behavior: B) -> B
    where
        B: Behavior<Protocol = WorkerPoolProtocol<MailAddr, u8, (), Recipient<Reply>>>,
    {
        behavior
    }

    let pool = owned_pool(
        WorkerPool::new(
            ChildTopology::new([3, 7], |_| Some(Worker)),
            PoolConfiguration::new(
                8,
                behavior::InterruptionPolicy::Retry,
                RestartPolicy::Transient,
                2,
                Duration::from_secs(10),
                behavior::RestartTiming::Immediate,
            ),
            Proxy::new,
        )
        .unwrap(),
    );
    let mut initialized = pool.initialize().unwrap();
    assert_eq!(initialized.actions.creates.len(), 2);
    assert!(initialized.actions.sends.responses.is_empty());
    assert!(initialized.actions.sends.assignments.is_empty());
    assert_eq!(
        initialized
            .actions
            .sends
            .supervision
            .child_observations
            .as_slice()
            .len(),
        2
    );
    let _: InterpreterRequests<ObserveChild<MailAddr, behavior::ChildHead>> =
        initialized.actions.sends.supervision.child_observations;
    let _: InterpreterRequests<behavior::ObserveCreation<MailAddr, behavior::ChildHead>> =
        initialized.actions.sends.supervision.creation_observations;
    assert!(
        initialized
            .actions
            .sends
            .supervision
            .replacement_inputs
            .is_empty()
    );

    let command: PoolMessage<MailAddr, u8, (), Recipient<Reply>> = PoolMessage::Submit {
        job: JobId(1),
        payload: 9_u8,
        reply_to: Recipient::<Reply>::global(MailAddr(2)),
    };
    let submitted = initialized.behavior.receive(MailAddr(1), command).unwrap();
    assert_eq!(submitted.sends.responses.len(), 1);
}
