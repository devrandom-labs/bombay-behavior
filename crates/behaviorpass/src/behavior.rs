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
pub enum Envelope<M> {
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

/// A behavior's **become** (Agha 1986): the replacement behavior it designates
/// as it processes a message. `Continue` = become(same), `Goto(p)` =
/// become(other from the phase menu), `Stop(_)` = become(⊥). One leg of the
/// Agha actions — the [`Actions`] carries it alongside the sends and creates.
/// (Alias of bombay's `Step` with our [`Exit`].)
pub type Become<Ph = Never> = Step<Ph, Exit>;

/// An abstract mail address (Agha): the fold names a send recipient by an
/// opaque token; the driver owns the token -> real ref map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailAddr(pub u64);

/// One create-effect, self-describing (2026-08-04 design): a **birth** is a
/// fresh actor at a fresh address (the driver mints and spawns); a
/// **restart** is a supervisor's restart decision for a child slot —
/// the address and mailbox SURVIVE, only the behavior is swapped (keep-address
/// restart; address mobility makes re-pointing escaped handles impossible, so
/// a restart is never a birth). The golf records the decision; the live driver
/// interprets: `Birth` spawns, `Restart` rides the child's control lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Create<New> {
    /// A fresh actor: the driver spawns the spec at a new address.
    Birth(New),
    /// A restart decision for child `slot`: fresh behavior, surviving address.
    Restart {
        /// Index into the supervisor's child table.
        slot: usize,
        /// The replacement behavior for the slot's surviving mailbox.
        child: New,
    },
}

/// The Agha actions: everything a behavior emits from ONE event — the messages
/// it SENT, the actors it CREATED, and the replacement behavior it BECAME. This
/// is the full triple [`Behavior::step`] returns; send and create are
/// first-class trace data, not swallowed side effects.
pub struct Actions<Ph, Out, New> {
    /// Messages sent this turn, each addressed by an opaque [`MailAddr`] token.
    pub sends: Vec<(MailAddr, Out)>,
    /// Create-effects this turn (the driver interprets each): births and
    /// restarts, self-describing via [`Create`].
    pub creates: Vec<Create<New>>,
    /// The replacement behavior.
    pub become_: Become<Ph>,
}

impl<Ph, Out, New> Actions<Ph, Out, New> {
    /// Actions with no sends and no creates — just a `become`.
    #[must_use]
    pub fn just(become_: Become<Ph>) -> Self {
        Self { sends: Vec::new(), creates: Vec::new(), become_ }
    }

    /// The pure `Continue` actions (no effects).
    #[must_use]
    pub fn cont() -> Self {
        Self::just(Step::Continue)
    }

    /// The pure `Stop(exit)` actions (no effects).
    #[must_use]
    pub fn stop(exit: Exit) -> Self {
        Self::just(Step::Stop(exit))
    }

    /// The pure `Goto(phase)` actions (no effects).
    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }
}

/// The outcome of one fold: the full Agha [`Actions`] on the phase menu `Ph`
/// with outbound menu `Out` and create-spec `New`, or a controlled crash `E`.
/// Named so the [`Behavior::step`] future's `Output` stays legible (and under
/// the `type_complexity` bar).
pub type Acted<Ph, Out, New, E> = Result<Actions<Ph, Out, New>, E>;

/// A synchronous message handler: folds one message into `&mut S`, returning
/// the full Agha [`Actions`] on the phase menu `P` with outbound menu `O` and
/// create-spec `N` (fn pointer, not a closure, so a generated actor stays
/// nameable).
pub type Handler<S, M, P, E, O, N> = fn(&mut S, M) -> Acted<P, O, N, E>;

/// The one async object: state in `&mut self`, one total `step` over the
/// [`Envelope`] alphabet, plus the `next_deadline` query the driver arms its timer
/// from. `step` returns an explicit `impl Future + Send` (not `async fn`) so
/// the `Send` bound is nameable at the driver's `spawn` boundary.
pub trait Behavior {
    /// The user-message type this behavior folds.
    type Msg;
    /// The become-menu still exposed upward (`Never` once fully erased).
    type Ph;
    /// The controlled-crash type.
    type Error;
    /// The declared outbound-message menu (`Never` = sends nothing).
    type Outbound;
    /// The declared create-spec (`Never` = creates nothing).
    type Offspring;

    /// Fold one event and return the actor's [`Actions`] — the messages it sent,
    /// the actors it created, and its replacement behavior (Agha): keep the
    /// same, switch to another phase, or stop.
    fn step(
        &mut self,
        ev: Envelope<Self::Msg>,
    ) -> impl Future<Output = Acted<Self::Ph, Self::Outbound, Self::Offspring, Self::Error>> + Send;

    /// The next instant this behavior needs waking, as a pure function of
    /// current state (`None` = no deadline). The deadline SOURCE is a query,
    /// not an event (quinn `poll_timeout` shape); its FIRING is `Envelope::Deadline`.
    /// Default: no deadline (a plain actor arms nothing).
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
}

/// Lift a become-only reaction verdict into a full [`Actions`] with empty effect
/// lists: `Goto` cannot exist at `Never`, so only `Continue`/`Stop` ride out of
/// a framework reaction, and a reaction sends and creates nothing. This is the
/// phase-lift every source capability applies to its reaction's result.
pub fn lift<Ph, Out, New>(v: Step<Never, Exit>) -> Actions<Ph, Out, New> {
    Actions::just(match v {
        Step::Continue => Step::Continue,
        Step::Goto(never) => match never {},
        Step::Stop(e) => Step::Stop(e),
    })
}

/// The floor: a plain actor = state + a synchronous [`Handler`]. Framework
/// events (deadline / link-death / child-stop) are no-ops — a plain actor owns
/// no source. The `Never` defaults on `O`/`N` mean a floor provably sends and
/// creates nothing. Every capability wraps a `Behavior`; `Base` is the
/// innermost one.
pub struct Base<S, M, P, E, O = Never, N = Never> {
    state: S,
    handle: Handler<S, M, P, E, O, N>,
}

impl<S, M, P, E, O, N> Base<S, M, P, E, O, N> {
    /// Builds a floor over `state` with `handle`.
    pub fn new(state: S, handle: Handler<S, M, P, E, O, N>) -> Self {
        Self { state, handle }
    }

    /// The accumulated state (test observability).
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, M, P, E, O, N> Behavior for Base<S, M, P, E, O, N>
where
    S: Send,
    M: Send,
    P: Send,
    E: Send,
    O: Send,
    N: Send,
{
    type Msg = M;
    type Ph = P;
    type Error = E;
    type Outbound = O;
    type Offspring = N;
    async fn step(&mut self, ev: Envelope<M>) -> Acted<P, O, N, E> {
        match ev {
            Envelope::User(m) => (self.handle)(&mut self.state, m),
            // A plain actor owns no framework source — become(same), no effects.
            Envelope::Deadline | Envelope::LinkDied { .. } | Envelope::ChildStopped { .. } => {
                Ok(Actions::cont())
            }
        }
    }
}

/// The driver's interpretation of a fold: everything the behavior emitted over
/// its whole life (the accumulated sends and creates) plus its final [`Exit`].
/// The recording peer returns this instead of only the exit — send and create
/// are observable trace, not swallowed effects.
pub struct Transcript<Out, New> {
    /// Every message the behavior sent, in emission order.
    pub sends: Vec<(MailAddr, Out)>,
    /// Every create-effect the behavior emitted, in emission order.
    pub creates: Vec<Create<New>>,
    /// The exit that ended the fold.
    pub exit: Exit,
}

/// Drive a fully-erased behavior over its fastpass mailbox until it stops or the
/// mailbox drains, RECORDING the triple: each step's sends and creates are
/// accumulated and returned in the [`Transcript`]. The user lane becomes
/// `Envelope::User`; the control lane is routed by the Watching / Supervising
/// layers (Task 2 continued).
///
/// `Stop(exit)` ends the fold immediately; `Goto` is unconstructible at
/// `Ph = Never`; `Err` short-circuits; a drained mailbox is collection.
///
/// # Errors
/// Returns the behavior's `Error` the first time a step is a controlled crash.
pub async fn run<B, C>(
    mut b: B,
    mut mailbox: Consumer<C, B::Msg>,
) -> Result<Transcript<B::Outbound, B::Offspring>, B::Error>
where
    B: Behavior<Ph = Never>,
{
    let mut sends = Vec::new();
    let mut creates = Vec::new();
    while let Some(recv) = mailbox.recv().await {
        let ev = match recv {
            Received::User(m) => Envelope::User(m),
            // The control lane becomes load-bearing with Watching / Supervising.
            Received::Control(_signal) => continue,
        };
        let actions = b.step(ev).await?;
        sends.extend(actions.sends);
        creates.extend(actions.creates);
        match actions.become_ {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(exit) => return Ok(Transcript { sends, creates, exit }),
        }
    }
    Ok(Transcript { sends, creates, exit: Exit::Collected })
}

#[cfg(test)]
mod tests {
    use super::{Actions, Base, Behavior, Envelope, MailAddr, Step, run};
    use crate::Exit;
    use bombay::capability::Never;
    use fastpass::{Config, channel};

    #[tokio::test]
    async fn base_folds_user_messages_and_ignores_framework_events() {
        let mut b = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            if id == 0 {
                return Ok(Actions::stop(Exit::Normal));
            }
            seen.push(id);
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        });

        assert!(matches!(b.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
        assert!(matches!(b.step(Envelope::User(7)).await.unwrap().become_, Step::Continue));
        assert!(matches!(
            b.step(Envelope::User(0)).await.unwrap().become_,
            Step::Stop(Exit::Normal)
        ));
        assert_eq!(b.state(), &vec![7], "only the delivered user message folded");
    }

    #[tokio::test]
    async fn base_has_no_deadline() {
        let b = Base::new((), |(): &mut (), (): ()| Ok::<Actions<Never, Never, Never>, Never>(Actions::cont()));
        assert!(b.next_deadline().is_none(), "a plain actor arms no deadline");
    }

    /// Sums user messages; stops normally once the running total reaches 10.
    struct Counter(u32);

    impl Behavior for Counter {
        type Msg = u32;
        type Ph = Never;
        type Error = &'static str;
        type Outbound = Never;
        type Offspring = Never;
        async fn step(
            &mut self,
            ev: Envelope<u32>,
        ) -> Result<Actions<Never, Never, Never>, &'static str> {
            if let Envelope::User(n) = ev {
                self.0 += n;
            }
            if self.0 >= 10 {
                Ok(Actions::stop(Exit::Normal))
            } else {
                Ok(Actions::cont())
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
        assert_eq!(out.unwrap().exit, Exit::Normal, "the Stop verdict's exit rides out");
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
        assert_eq!(out.unwrap().exit, Exit::Collected, "a fully-closed mailbox is collection");
    }

    /// The payoff: the driver is the RECORDING PEER. A behavior pushes a send
    /// into its actions; `run` accumulates it and hands it back — the test
    /// receives exactly what the actor sent, with no transport in between.
    #[tokio::test]
    async fn run_records_a_behaviors_sends_the_test_is_the_peer() {
        struct St;
        let b: Base<St, u64, Never, &'static str, u64, Never> =
            Base::new(St, |_: &mut St, _m: u64| {
                Ok(Actions { sends: vec![(MailAddr(7), 99)], creates: vec![], become_: Step::Stop(Exit::Normal) })
            });
        let (_ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
        let handle = tokio::spawn(run(b, rx));
        usr.send(1).await.expect("mailbox open");

        let transcript = handle.await.expect("driver task joins").expect("no crash");
        assert_eq!(transcript.sends, vec![(MailAddr(7), 99)], "the peer receives what the actor sent");
        assert_eq!(transcript.exit, Exit::Normal);
    }
}
