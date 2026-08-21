use behavior::{
    Actions, Behavior, Delivery, MailAddr, MessageProtocol, Never, NoBirths, Recipient, User,
};

struct Queue;
struct Worker;

macro_rules! inert {
    ($actor:ty) => {
        impl behavior::Protocol for $actor {
            type Addr = MailAddr;
            type Msg = u8;
        }

        impl Behavior for $actor {
            type Protocol = Self;
            type Event = User<MailAddr, u8>;
            type Sends = Vec<Never>;
            type Ph = Never;
            type Error = Never;
            type Birth = NoBirths;

            fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
                Ok(Actions::cont())
            }

            fn transition(
                &mut self,
                _: behavior::ActiveTurn,
                _: Self::Event,
            ) -> behavior::BehaviorActed<Self> {
                Ok(Actions::cont())
            }
        }
    };
}

inert!(Queue);
inert!(Worker);

fn queue_lane(_: Vec<Delivery<Queue>>) {}
fn worker_lane(_: Vec<Delivery<Worker>>) {}

#[test]
fn identical_address_and_message_types_keep_distinct_protocol_lanes() {
    let queues = vec![Delivery::new(Recipient::<Queue>::global(MailAddr(4)), 7)];
    let workers = vec![Delivery::new(Recipient::<Worker>::global(MailAddr(4)), 7)];

    queue_lane(queues);
    worker_lane(workers);
}

struct SelfSending;

impl behavior::Protocol for SelfSending {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for SelfSending {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Delivery<Self>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(MailAddr(9)), 11)],
            Vec::new(),
            behavior::Step::Continue,
        ))
    }
}

#[test]
fn a_behavior_can_name_its_own_protocol_as_a_destination() {
    let actions = SelfSending
        .initialize()
        .unwrap()
        .behavior
        .receive(MailAddr(1), 3)
        .expect("self-send transition succeeds");

    assert_eq!(actions.sends[0].message, 11);
}

#[test]
fn equal_payloads_cannot_conflate_adjacent_protocol_lanes() {
    struct DestinationSends {
        queues: Vec<Delivery<Queue>>,
        workers: Vec<Delivery<Worker>>,
    }

    let sends = DestinationSends {
        queues: vec![Delivery::new(Recipient::<Queue>::global(MailAddr(4)), 7)],
        workers: vec![Delivery::new(Recipient::<Worker>::global(MailAddr(4)), 7)],
    };

    assert_eq!(sends.queues.len(), 1);
    assert_eq!(sends.workers.len(), 1);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RemoteAddr(u64);

impl behavior::Address for RemoteAddr {
    type Nonce = u16;
}

struct Remote;

impl behavior::Protocol for Remote {
    type Addr = RemoteAddr;
    type Msg = u8;
}

impl Behavior for Remote {
    type Protocol = Self;
    type Event = User<RemoteAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct CrossNamespaceSender;

impl behavior::Protocol for CrossNamespaceSender {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for CrossNamespaceSender {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Delivery<Remote>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: behavior::InitializationTurn) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(
        &mut self,
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(RemoteAddr(31)), 5)],
            Vec::new(),
            behavior::Step::Continue,
        ))
    }
}

#[test]
fn destination_protocol_owns_the_address_namespace() {
    let actions = CrossNamespaceSender
        .initialize()
        .unwrap()
        .behavior
        .receive(MailAddr(1), ())
        .expect("cross-namespace send succeeds");

    assert_eq!(actions.sends[0].to.address(), RemoteAddr(31));
}

#[test]
fn protocol_markers_need_no_clone_or_equality_implementation() {
    let delivery = Delivery::<Queue>::new(Recipient::global(MailAddr(3)), 8);
    let cloned = delivery.clone();

    assert!(cloned == delivery);
}

#[test]
fn reusable_message_protocol_retains_one_established_identity_across_emitters() {
    type RootProtocol = MessageProtocol<MailAddr, u8>;

    let root = Recipient::<RootProtocol>::global(MailAddr(0));
    let first = Delivery::new(root, 1);
    let second = Delivery::new(root, 2);

    assert_eq!(first.to.address(), MailAddr(0));
    assert_eq!(second.to.address(), MailAddr(0));
}
use behavior_testkit::InitializeTest;
