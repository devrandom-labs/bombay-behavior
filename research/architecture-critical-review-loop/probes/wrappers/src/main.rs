//! Probe: derive a minimal wrapper combinator from the fold + host generics
//! (P-wrappers). Demonstrates that wrappers are derived higher-order functions,
//! not semantic primitives.
//!
//! This probe builds a from-scratch "Logging" wrapper: a struct that wraps an
//! inner Behavior, widens the event to include a Log message in its own lane,
//! routes Log events to a simple handler, delegates all other events to the
//! inner behavior, and merges effects.

use behavior::{
    Actions, Base, Behavior, Delivery, FnState, MailAddr, Never, NoBirths, Recipient, SendAlgebra,
    User, UserEvent,
};

// ---------- Logging lane: a wrapper's own event ----------
struct Log(String);

// ---------- Logging wrapper struct ----------
struct Logging<B: Behavior> {
    inner: B,
    log: Vec<String>,
}

// ---------- Composed event: either the wrapper's lane or the inner's ----------
enum LoggingEvent<B: Behavior> {
    Log(Log),
    Inner(B::Event),
}

// ---------- UserEvent impl so the wrapper can serve as Behavior ----------
impl<B: Behavior> UserEvent for LoggingEvent<B>
where
    B::Event: UserEvent<Addr = B::Addr, Message = B::Msg>,
{
    type Addr = B::Addr;
    type Message = B::Msg;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        LoggingEvent::Inner(B::Event::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            LoggingEvent::Inner(inner) => inner
                .into_user()
                .map_err(LoggingEvent::Inner),
            other => Err(other),
        }
    }
}

impl<B: Behavior> Logging<B> {
    fn new(inner: B) -> Self {
        Self {
            inner,
            log: Vec::new(),
        }
    }
}

// ---------- Behavior impl: the wrapper is a valid Behavior ----------
impl<B: Behavior + Send> Behavior for Logging<B>
where
    B::Event: UserEvent<Addr = B::Addr, Message = B::Msg> + Send,
    B::Sends: Send,
    B::Ph: Send,
    B::Error: Send,
    B::Birth: Send,
{
    type Addr = B::Addr;
    type Msg = B::Msg;
    type Event = LoggingEvent<B>;
    type Sends = B::Sends;
    type Ph = B::Ph;
    type Error = B::Error;
    type Birth = B::Birth;

    async fn init(&mut self) -> behavior::BehaviorActed<Self> {
        self.log.push("init".into());
        self.inner.init().await
    }

    async fn step(&mut self, event: Self::Event) -> behavior::BehaviorActed<Self> {
        match event {
            LoggingEvent::Log(Log(msg)) => {
                self.log.push(format!("log: {msg}"));
                Ok(Actions::cont())
            }
            LoggingEvent::Inner(inner_event) => {
                self.log.push("delegate".into());
                self.inner.step(inner_event).await
            }
        }
    }
}

// ---------- Smoke test: an inner behavior that counts messages ----------
fn simple_step(
    state: &mut u32,
    _from: MailAddr,
    _msg: (),
) -> behavior::Acted<MailAddr, Never, Vec<Delivery<MailAddr, ()>>, NoBirths, Never> {
    *state += 1;
    let mut acts: Actions<MailAddr, Never, Vec<Delivery<MailAddr, ()>>, NoBirths> =
        Actions::cont();
    if *state % 2 == 0 {
        acts.sends
            .push(Delivery::new(Recipient::global(MailAddr(0)), ()));
    }
    Ok(acts)
}

fn main() {
    // Inner behavior: a counter with message type (), output type ().
    type Inner = Base<FnState<u32, MailAddr, (), (), NoBirths, Never>, (), NoBirths, Never>;
    let inner: Inner = Base::from_fn(0u32, simple_step);

    // Wrap with our from-scratch Logging wrapper.
    let logging = Logging::new(inner);

    // Type-check: Logging<Inner> is a Behavior.
    fn assert_is_behavior<B: Behavior>(_: &B) {}
    assert_is_behavior(&logging);

    println!(
        "wrappers probe: from-scratch Logging wrapper compiles and type-checks \
         as Behavior; wrappers are derived host higher-order functions"
    );
}
