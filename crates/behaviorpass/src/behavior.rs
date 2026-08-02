//! The async object, its event alphabet, and the driver — the ASYNC
//! projection of ADR-0030's `Behavior` (the sync twin is
//! `behaviorpass-reference`).

use core::future::Future;

use bombay::capability::{Never, Step};
use fastpass::{Consumer, Received};
use tokio::time::Instant;

use crate::Exit;

/// The fixed framework event alphabet folded by every layer. A source-adding
/// layer handles its own variant and forwards the rest inward; a plain actor
/// treats every non-`User` variant as a no-op. This flat alphabet (rather than
/// nested per-layer sums) keeps the machinery uniform at fixed arity — the
/// open source set is ADR-0030's deferred door.
pub enum Wire<M> {
    /// A user-lane message.
    User(M),
    /// The single-shot deadline arm fired.
    Deadline,
    /// A watched/linked peer stopped.
    LinkDied {
        /// The dead peer's id.
        peer: u64,
        /// Whether the stop was abnormal (the propagation trigger).
        abnormal: bool,
    },
    /// A supervised child fold ended.
    ChildStopped {
        /// Index into the child table.
        idx: usize,
        /// Whether the child's stop was abnormal (restart-eligible).
        abnormal: bool,
    },
}

/// A synchronous message handler: folds one message into `&mut S`, returning a
/// verdict on the phase menu `P` (fn pointer, not a closure, so a generated
/// actor stays nameable).
pub type Handler<S, M, P, E> = fn(&mut S, M) -> Result<Step<P, Exit>, E>;

/// The one async object: state in `&mut self`, one total `step` over the
/// [`Wire`] alphabet, plus the `next_deadline` query the driver arms its timer
/// from. `step` returns an explicit `impl Future + Send` (not `async fn`) so
/// the `Send` bound is nameable at the driver's `spawn` boundary.
pub trait Behavior {
    /// The user-message type this behavior folds.
    type Msg;
    /// The become-menu still exposed upward (`Never` once fully erased).
    type Ph;
    /// The controlled-crash type.
    type Error;

    /// One fold step over the framework alphabet: typed become — continue,
    /// switch behavior, or stop.
    fn step(
        &mut self,
        ev: Wire<Self::Msg>,
    ) -> impl Future<Output = Result<Step<Self::Ph, Exit>, Self::Error>> + Send;

    /// The next instant this behavior needs waking, as a pure function of
    /// current state (`None` = no deadline). The deadline SOURCE is a query,
    /// not an event (quinn `poll_timeout` shape); its FIRING is `Wire::Deadline`.
    /// Default: no deadline (a plain actor arms nothing).
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
}

/// Re-type an erased reaction verdict into any phase menu: `Goto` cannot exist
/// at `Never`, so only `Continue`/`Stop` ride out of a framework reaction. This
/// is the phase-lift every source capability applies to its reaction's result.
pub fn lift<Ph, E>(v: Step<Never, E>) -> Step<Ph, E> {
    match v {
        Step::Continue => Step::Continue,
        Step::Goto(never) => match never {},
        Step::Stop(e) => Step::Stop(e),
    }
}

/// Drive a fully-erased behavior over its fastpass mailbox until it stops or
/// the mailbox drains. The user lane becomes `Wire::User`; the control lane is
/// routed by the Watching / Supervising layers (Task 2 continued). The deadline
/// and link arms join the `select!` with the layers that own those sources.
///
/// `Stop(exit)` ends the fold immediately; `Goto` is unconstructible at
/// `Ph = Never`; `Err` short-circuits; a drained mailbox is collection.
///
/// # Errors
/// Returns the behavior's `Error` the first time a step is a controlled crash.
pub async fn run<B, C>(mut b: B, mut mailbox: Consumer<C, B::Msg>) -> Result<Exit, B::Error>
where
    B: Behavior<Ph = Never>,
{
    while let Some(recv) = mailbox.recv().await {
        let ev = match recv {
            Received::User(m) => Wire::User(m),
            // The control lane becomes load-bearing with Watching / Supervising.
            Received::Control(_signal) => continue,
        };
        match b.step(ev).await? {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(exit) => return Ok(exit),
        }
    }
    Ok(Exit::Collected)
}

#[cfg(test)]
mod tests {
    use super::{Behavior, Wire, run};
    use crate::Exit;
    use bombay::capability::{Never, Step};
    use fastpass::{Config, channel};

    /// Sums user messages; stops normally once the running total reaches 10.
    struct Counter(u32);

    impl Behavior for Counter {
        type Msg = u32;
        type Ph = Never;
        type Error = &'static str;
        async fn step(&mut self, ev: Wire<u32>) -> Result<Step<Never, Exit>, &'static str> {
            if let Wire::User(n) = ev {
                self.0 += n;
            }
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
