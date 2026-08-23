//! Typed protocol adaptation through an ordinary actor hop.

use behavior::{Actions, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Recipient, User};

use super::DeliveryRoute;

/// A pure actor that maps one input protocol into one destination protocol.
///
/// For every accepted `Input`, the adapter invokes its function pointer exactly
/// once. If that invocation returns normally, the transition emits exactly one
/// effect selected by [`DeliveryRoute`] and continues with the same behavior.
/// A logical recipient produces `Delivery`, an established recipient produces
/// `EstablishedDelivery`, and a child route produces occurrence-aware
/// `ChildDelivery`. Initialization emits no effects; the adapter cannot
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
pub struct MessageAdapterWithRoute<Input, Destination, Route>
where
    Destination: behavior::Protocol,
    Route: DeliveryRoute<Destination>,
{
    destination: Route,
    adapt: fn(Input) -> Destination::Msg,
}

/// An adapter whose destination is a logical protocol name.
pub type MessageAdapter<Input, Destination> =
    MessageAdapterWithRoute<Input, Destination, Recipient<Destination>>;

impl<Input, Destination, Route> MessageAdapterWithRoute<Input, Destination, Route>
where
    Destination: behavior::Protocol,
    Route: DeliveryRoute<Destination>,
{
    /// Construct an adapter for one concrete destination protocol.
    #[must_use]
    pub const fn new(destination: Route, adapt: fn(Input) -> Destination::Msg) -> Self {
        Self { destination, adapt }
    }

    /// Return the destination routing intent.
    #[must_use]
    pub const fn destination(&self) -> &Route {
        &self.destination
    }
}

impl<Input, Destination, Route> BehaviorBase for MessageAdapterWithRoute<Input, Destination, Route>
where
    Destination: behavior::Protocol,
    Route: DeliveryRoute<Destination>,
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<Input, Destination, Route> behavior::Protocol
    for MessageAdapterWithRoute<Input, Destination, Route>
where
    Destination: behavior::Protocol,
    Route: DeliveryRoute<Destination>,
{
    type Addr = Destination::Addr;
    type Msg = Input;
}

impl<Input, Destination, Route> Behavior for MessageAdapterWithRoute<Input, Destination, Route>
where
    Destination: behavior::Protocol,
    Route: DeliveryRoute<Destination> + Clone,
{
    type Protocol = Self;
    type Event = User<Destination::Addr, Input>;
    type Sends = Vec<Route::Effect>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let message = (self.adapt)(event.message);
        Ok(Actions::send(vec![
            self.destination.clone().deliver(message),
        ]))
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
        type Protocol = Self;
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
        assert_eq!(*initialized.behavior.destination(), destination);
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
