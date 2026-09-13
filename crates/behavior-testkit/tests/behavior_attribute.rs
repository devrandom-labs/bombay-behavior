use behavior::{Actions, Behavior, Delivery, MailAddr, Never, NoBirths, Recipient, SendEffects};
use behavior_testkit::InitializeTest;

struct Printer(u64);

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = {
        replies: Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
    },
)]
impl Printer {
    fn receive(&mut self, from: MailAddr, message: u64) -> behavior::BehaviorActed<Self> {
        self.0 += message;
        let mut sends = PrinterSends::empty();
        sends.send::<_, PrinterSendsReplies>(Delivery::new(Recipient::global(from), self.0));
        Ok(Actions::send(sends))
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
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            self.total,
        )]))
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
        Ok(Actions::send(vec![Delivery::new(
            Recipient::global(from),
            message,
        )]))
    }
}

#[test]
fn omitted_initialization_is_the_explicit_empty_transition() {
    let initialized = Printer(0).initialize().unwrap();
    let actions = initialized.actions;

    assert!(actions.sends.replies.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
}

#[test]
fn capability_defaults_cover_the_infallible_no_birth_subset() {
    fn assert_protocol<B>(_: &B)
    where
        B: Behavior<Error = Never, Birth = NoBirths>,
        B::Protocol: behavior::Protocol<Addr = MailAddr, Msg = u64>,
    {
    }

    let printer = Printer(1);
    assert_protocol(&printer);
    let initialized = printer.initialize().unwrap();
    assert!(initialized.actions.sends.replies.is_empty());
    let mut printer = initialized.behavior;
    let actions = printer.receive(MailAddr(7), 4).unwrap();
    assert_eq!(actions.sends.replies[0].message, 5);
    assert_eq!(actions.sends.replies[0].to, Recipient::global(MailAddr(7)));
}

#[test]
fn behavior_trait_provides_the_same_empty_initialization_transition() {
    let initialized = Manual.initialize().unwrap();
    let actions = initialized.actions;

    assert!(actions.sends.is_empty());
    assert!(actions.creates.is_empty());
    assert!(matches!(actions.become_, behavior::Step::Continue));
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
fn attribute_preserves_impl_generics_and_where_clause() {
    let generic = Generic::<u16> { last: None };
    let initialized = generic.initialize().unwrap();
    let mut generic = initialized.behavior;
    let actions = generic.receive(MailAddr(3), 11).unwrap();
    assert_eq!(generic.last, Some(11));
    assert_eq!(actions.sends[0].message, 11);
}
