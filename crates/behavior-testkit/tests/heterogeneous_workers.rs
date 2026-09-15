//! Application-defined worker variants sharing one actor protocol.

use behavior_core::{
    Acted, Actions, Behavior, BehaviorActed, Delivery, MailAddr, Never, Recipient, User,
};
use behavior_testkit::InitializeTest;

struct WorkerA;

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior_core::NoBirths, error = Never)]
impl WorkerA {
    fn receive(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior_core::NoBirths,
        Never,
    > {
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            1,
        )]))
    }
}

struct WorkerB;

#[behavior_core::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>, births = behavior_core::NoBirths, error = Never)]
impl WorkerB {
    fn receive(
        &mut self,
        from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u8>>>,
        behavior_core::NoBirths,
        Never,
    > {
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            2,
        )]))
    }
}

enum Worker {
    A(WorkerA),
    B(WorkerB),
}

impl behavior_core::Protocol for Worker {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Worker {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Delivery<behavior_testkit::TestRecipient<u8>>>;
    type Ph = Never;
    type Error = Never;
    type Birth = behavior_core::NoBirths;

    fn init(&mut self, turn: behavior_core::InitializationTurn) -> BehaviorActed<Self> {
        match self {
            Self::A(worker) => worker.init(turn),
            Self::B(worker) => worker.init(turn),
        }
    }

    fn transition(
        &mut self,
        turn: behavior_core::ActiveTurn,
        event: Self::Event,
    ) -> BehaviorActed<Self> {
        match self {
            Self::A(worker) => worker.transition(turn, event),
            Self::B(worker) => worker.transition(turn, event),
        }
    }
}

fn worker_at(position: usize) -> Option<Worker> {
    match position {
        0..2 => Some(Worker::A(WorkerA)),
        2 => Some(Worker::B(WorkerB)),
        _ => None,
    }
}

#[test]
fn declared_positions_select_their_worker_behavior() {
    for (position, expected) in [(0, 1), (1, 1), (2, 2)] {
        let mut worker = worker_at(position)
            .expect("the declared position has a worker")
            .initialize()
            .expect("worker initialization succeeds")
            .behavior;
        let actions = worker
            .transition(User::new(MailAddr(0), 7))
            .expect("worker processing succeeds");
        assert_eq!(actions.sends[0].message, expected);
    }
}

#[test]
fn an_undeclared_position_has_no_worker() {
    assert!(worker_at(3).is_none());
}
