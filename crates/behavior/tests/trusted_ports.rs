//! Public composition ports rely on the wrapper or runtime to enforce turn order.

use behavior::{
    Actions, ActiveTurn, Behavior, BehaviorActed, InitializationTurn, MailAddr, Never, NoBirths,
    Protocol, User, delegate_transition, initialize,
};

struct Counter {
    calls: u8,
}

impl Protocol for Counter {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Counter {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        self.calls += 1;
        Ok(Actions::cont())
    }

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        self.calls += 10;
        Ok(Actions::cont())
    }
}

#[test]
fn raw_composition_ports_leave_initialization_order_to_their_owner() {
    let mut counter = Counter { calls: 0 };
    let delegated = delegate_transition(&mut counter, User::new(MailAddr(1), ())).unwrap();
    let first = initialize(&mut counter).unwrap();
    let second = initialize(&mut counter).unwrap();
    for actions in [delegated, first, second] {
        assert!(actions.sends.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, behavior::Step::Continue));
    }
    assert_eq!(counter.calls, 12);
}
