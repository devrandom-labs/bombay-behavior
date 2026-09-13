use std::time::Instant;

use behavior::{
    Actions, Activate, Behavior, BehaviorActed, Delivery, MailAddr, Never, NoBirths, Recipient,
    StashRoute, Step, TimerId, User,
};

struct Sink;

impl behavior::Protocol for Sink {
    type Addr = MailAddr;
    type Msg = u64;
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
fn inferred_stack_crosses_one_generic_adapter_layer() {
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
