use std::time::Duration;

use behavior::{
    Actions, Behavior, Births, CreationKind, Delivery, Effect, InterruptionPolicy, JobId, MailAddr,
    Never, NoBirths, PoolAssignment, PoolMessage, Recipient, RestartPolicy, Step,
    WorkerCreationResolved, WorkerPool, WorkerPoolProtocol,
};

struct Printer(u64);

#[behavior::actor]
impl Printer {
    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> Effect<Delivery<behavior_testkit::TestRecipient<u64>>> {
        self.0 += message;
        Effect::send(Delivery::new(Recipient::global(from), self.0))
    }
}

struct Counter {
    total: u64,
}

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
    births = NoBirths,
    error = Never,
)]
impl Counter {
    fn init(
        &mut self,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        NoBirths,
        Never,
    > {
        self.total = 1;
        Ok(Actions::cont())
    }

    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        NoBirths,
        Never,
    > {
        self.total += message;
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), self.total)],
            Vec::new(),
            Step::Continue,
        ))
    }
}

struct Worker;

type PoolReply = behavior_testkit::TestRecipient<behavior::PoolResponse<u8, (), MailAddr>>;
#[behavior::behavior(
    addr = MailAddr,
    message = PoolAssignment<WorkerPoolProtocol<MailAddr, PoolReply, u8, ()>>,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Worker {
    fn receive(
        &mut self,
        _from: MailAddr,
        _assignment: PoolAssignment<WorkerPoolProtocol<MailAddr, PoolReply, u8, ()>>,
    ) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

struct Manual;

impl behavior::Protocol for Manual {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Manual {
    type Protocol = Self;
    type Event = behavior::User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[test]
fn omitted_initialization_is_the_explicit_empty_transition() {
    let initialized = Worker.initialize().unwrap();
    let actions = initialized.actions;

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

#[test]
fn actor_attribute_infers_the_honest_infallible_no_birth_subset() {
    fn assert_protocol<B>(_: &B)
    where
        B: Behavior<Error = Never, Birth = NoBirths>,
        B::Protocol: behavior::Protocol<Addr = MailAddr, Msg = u64>,
    {
    }

    let printer = Printer(1);
    assert_protocol(&printer);
    let initialized = printer.initialize().unwrap();
    assert!(initialized.actions.sends.is_empty());
    let mut printer = initialized.behavior;
    let actions = printer.receive(MailAddr(7), 4).unwrap();
    assert_eq!(actions.sends[0].message, 5);
    assert_eq!(actions.sends[0].to, Recipient::global(MailAddr(7)));
}

#[test]
fn behavior_trait_provides_the_same_empty_initialization_transition() {
    let initialized = Manual.initialize().unwrap();
    let actions = initialized.actions;

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, Step::Continue));
}

struct Generic<T> {
    last: Option<T>,
}

#[behavior::behavior(
    addr = MailAddr,
    message = T,
    sends = Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
    births = NoBirths,
    error = Never,
)]
impl<T> Generic<T>
where
    T: Clone,
{
    fn init(
        &mut self,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
        NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }

    fn receive(
        &mut self,
        from: MailAddr,
        message: T,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
        NoBirths,
        Never,
    > {
        self.last = Some(message.clone());
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), message)],
            Vec::new(),
            Step::Continue,
        ))
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

#[test]
fn attribute_preserves_normal_methods_and_exact_actions() {
    let counter = Counter { total: 0 };
    let initialized = counter.initialize().unwrap();
    let mut counter = initialized.behavior;
    let actions = counter.receive(MailAddr(7), 4).unwrap();
    assert_eq!(counter.total, 5);
    assert_eq!(actions.sends[0].message, 5);
    assert_eq!(actions.sends[0].to, Recipient::global(MailAddr(7)));
}

#[test]
fn generated_behavior_is_nominal_in_pool_and_supervision_positions() {
    let pool = WorkerPool::new(
        behavior::ChildTopology::indexed(nonce, 1, |_| Some(Worker)),
        behavior::PoolConfiguration::new(
            0,
            InterruptionPolicy::Fail,
            RestartPolicy::Permanent,
            1,
            Duration::from_secs(1),
        ),
        Recipient::global(MailAddr(9)),
    )
    .unwrap();
    let initialized = pool.initialize().unwrap();
    let initial = initialized.actions;
    let mut pool = initialized.behavior;
    assert_eq!(initial.creates.len(), 1);
    pool.on(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap();
    let actions = pool
        .receive(
            MailAddr(1),
            PoolMessage::Submit {
                job: JobId(0),
                payload: 9,
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.behavior.assignments.len(), 1);
}

#[test]
fn attribute_preserves_impl_generics_and_where_clause() {
    let generic = Generic::<u16> { last: None };
    let initialized = generic.initialize().unwrap();
    let mut generic = initialized.behavior;
    let actions = generic.receive(MailAddr(3), 11).unwrap();
    assert_eq!(generic.last, Some(11));
    assert_eq!(actions.sends[0].message, 11);
}

#[allow(dead_code)]
type NameableBirth = Births<Counter>;
use behavior_testkit::InitializeTest;
