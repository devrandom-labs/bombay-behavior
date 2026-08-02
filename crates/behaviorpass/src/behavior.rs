//! The async object and its driver — the ASYNC projection of ADR-0030's
//! `Behavior` (the sync twin is `behaviorpass-reference`).

use core::future::Future;

use bombay::capability::{Never, Step};
use fastpass::{Consumer, Received};

use crate::Exit;

/// The one async object: state in `&mut self`, one total `step` over the event
/// alphabet. A source-adding layer extends `Event` as a sum; the step routes
/// its own events and forwards the rest. `step` returns an explicit
/// `impl Future + Send` (not `async fn`) so the `Send` bound is nameable at
/// the driver's `spawn` boundary.
pub trait Behavior {
    /// The event alphabet this behavior folds over.
    type Event;
    /// The become-menu still exposed upward (`Never` once fully erased).
    type Ph;
    /// The controlled-crash type.
    type Error;
    /// One fold step: typed become — continue, switch behavior, or stop.
    fn step(
        &mut self,
        ev: Self::Event,
    ) -> impl Future<Output = Result<Step<Self::Ph, Exit>, Self::Error>> + Send;
}

/// Drive a fully-erased behavior over its fastpass mailbox until it stops or
/// the mailbox drains. The user lane carries `Behavior::Event`; the control
/// lane carries framework/control signals (`C`), routed by later layers —
/// for a plain actor no control signal is ever sent.
///
/// `Stop(exit)` ends the fold immediately; `Goto` is unconstructible at
/// `Ph = Never`; `Err` short-circuits unchanged; a drained mailbox is
/// collection, not success.
///
/// # Errors
/// Returns the behavior's `Error` the first time a step is a controlled crash.
pub async fn run<B, C>(
    mut b: B,
    mut mailbox: Consumer<C, B::Event>,
) -> Result<Exit, B::Error>
where
    B: Behavior<Ph = Never>,
{
    while let Some(recv) = mailbox.recv().await {
        match recv {
            Received::User(ev) => match b.step(ev).await? {
                Step::Continue => {}
                Step::Goto(never) => match never {},
                Step::Stop(exit) => return Ok(exit),
            },
            // The control lane becomes load-bearing with the Watching /
            // Supervising layers (Task 2); a plain actor sends none.
            Received::Control(_signal) => {}
        }
    }
    Ok(Exit::Collected)
}

#[cfg(test)]
mod tests {
    use super::{Behavior, run};
    use crate::Exit;
    use bombay::capability::{Never, Step};
    use fastpass::{Config, channel};

    /// Sums user messages; stops normally once the running total reaches 10.
    struct Counter(u32);

    impl Behavior for Counter {
        type Event = u32;
        type Ph = Never;
        type Error = &'static str;
        async fn step(&mut self, ev: u32) -> Result<Step<Never, Exit>, &'static str> {
            self.0 += ev;
            if self.0 >= 10 {
                Ok(Step::Stop(Exit::Normal))
            } else {
                Ok(Step::Continue)
            }
        }
    }

    #[tokio::test]
    async fn driver_folds_the_user_lane_until_a_stop_verdict() {
        let (_ctl, usr, rx) = channel::<Never, u32>(Config::new(8));
        let handle = tokio::spawn(run(Counter(0), rx));

        usr.send(3).await.expect("mailbox open");
        usr.send(4).await.expect("mailbox open");
        usr.send(5).await.expect("mailbox open"); // total 12 ⇒ Stop(Normal)

        let out = handle.await.expect("driver task joins");
        assert_eq!(out, Ok(Exit::Normal), "the Stop verdict's exit rides out");
    }

    #[tokio::test]
    async fn driver_reports_collected_when_the_mailbox_drains() {
        let (ctl, usr, rx) = channel::<Never, u32>(Config::new(8));
        let handle = tokio::spawn(run(Counter(0), rx));

        usr.send(1).await.expect("mailbox open");
        // Collection = EVERY sender gone (both lanes): only then does `recv`
        // yield `None`. Dropping just the user lane leaves the control lane
        // open and the actor still reachable — not collected.
        drop(usr);
        drop(ctl);

        let out = handle.await.expect("driver task joins");
        assert_eq!(out, Ok(Exit::Collected), "a fully-closed mailbox is collection");
    }
}
