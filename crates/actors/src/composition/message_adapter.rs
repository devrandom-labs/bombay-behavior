//! Typed protocol adaptation through an ordinary actor hop.

use behavior::{
    Actions, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient, User,
};

/// A pure actor that maps one input protocol into one destination protocol.
///
/// For every accepted `Input`, the adapter invokes its function pointer exactly
/// once. If that invocation returns normally, the transition emits exactly one
/// [`Delivery<Destination>`] to the configured recipient and continues with
/// the same behavior. Initialization emits no effects; the adapter cannot
/// create actors, enter another phase, stop itself, or return a controlled
/// error.
///
/// This is a derived Bombay protocol composition, not an additional actor-model
/// primitive. Delivery is interpreted as an ordinary actor communication, so
/// the destination observes the adapter actor as the sender. The adapter has no
/// channel, runtime handle, task, I/O, or hidden effect.
///
/// The mapper is deliberately a function pointer rather than a closure or
/// erased callable. Consequently, the complete adapter type remains nameable
/// in birth products, child products, supervisors, and topology evidence. If
/// the mapper panics, unwinding follows the same runtime policy as a panic from
/// any other [`Behavior`] fold; no successful [`Actions`] value is returned.
pub struct MessageAdapter<Input, Destination>
where
    Destination: behavior::Protocol,
{
    destination: Recipient<Destination>,
    adapt: fn(Input) -> Destination::Msg,
}

impl<Input, Destination> MessageAdapter<Input, Destination>
where
    Destination: behavior::Protocol,
{
    /// Construct an adapter for one concrete destination protocol.
    #[must_use]
    pub const fn new(
        destination: Recipient<Destination>,
        adapt: fn(Input) -> Destination::Msg,
    ) -> Self {
        Self { destination, adapt }
    }

    /// Return the destination routing intent.
    #[must_use]
    pub const fn destination(&self) -> Recipient<Destination> {
        self.destination
    }
}

impl<Input, Destination> BehaviorBase for MessageAdapter<Input, Destination>
where
    Destination: behavior::Protocol,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<Input, Destination> behavior::Protocol for MessageAdapter<Input, Destination>
where
    Destination: behavior::Protocol,
{
    type Addr = Destination::Addr;
    type Msg = Input;
}

impl<Input, Destination> Behavior for MessageAdapter<Input, Destination>
where
    Destination: behavior::Protocol,
{
    type Event = User<Destination::Addr, Input>;
    type Sends = Vec<Delivery<Destination>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let message = (self.adapt)(event.message);
        Ok(Actions::send(vec![Delivery::new(
            self.destination,
            message,
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, MailAddr};
    use core::sync::atomic::{AtomicUsize, Ordering};

    struct Destination;

    impl behavior::Protocol for Destination {
        type Addr = MailAddr;
        type Msg = String;
    }

    impl Behavior for Destination {
        type Event = User<MailAddr, String>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    static INVOCATIONS: AtomicUsize = AtomicUsize::new(0);

    fn describe(value: u8) -> String {
        INVOCATIONS.fetch_add(1, Ordering::Relaxed);
        format!("value={value}")
    }

    #[test]
    fn initialization_is_empty() {
        let destination = Recipient::<Destination>::global(MailAddr(7));
        let initialized = MessageAdapter::new(destination, describe)
            .initialize()
            .unwrap();

        assert!(initialized.actions.sends.is_empty());
        assert!(initialized.actions.creates.is_empty());
        assert_eq!(initialized.actions.become_, behavior::Step::Continue);
        assert_eq!(initialized.behavior.destination(), destination);
    }

    #[test]
    fn each_input_maps_once_and_emits_one_delivery_then_continues() {
        INVOCATIONS.store(0, Ordering::Relaxed);
        let destination = Recipient::<Destination>::global(MailAddr(7));
        let mut adapter = MessageAdapter::new(destination, describe)
            .initialize()
            .unwrap()
            .behavior;

        for (input, expected) in [(3, "value=3"), (9, "value=9")] {
            let actions = adapter.receive(MailAddr(99), input).unwrap();
            assert_eq!(actions.sends.len(), 1);
            assert_eq!(actions.sends[0].to, destination);
            assert_eq!(actions.sends[0].message, expected);
            assert!(actions.creates.is_empty());
            assert_eq!(actions.become_, behavior::Step::Continue);
        }

        assert_eq!(INVOCATIONS.load(Ordering::Relaxed), 2);
    }
}
