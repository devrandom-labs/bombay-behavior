//! The async object, its event alphabet, and the driver — the ASYNC
//! projection of ADR-0030's `Behavior` (the sync twin is
//! `behaviorpass-reference`).

use core::future::Future;
use core::marker::PhantomData;

use crate::verdict::{Never, Step};
use fastpass::{Consumer, Received};
use tokio::time::Instant;

use crate::{Crash, Exit};

/// An abstract mail address (Agha): the fold names send recipients and
/// derives child addresses as a PURE function — `birth` is AMST's
/// `newadr()` discharged by derivation, not computation. The nonce TYPE is
/// the address type's own business (how it mixes into the derivation is
/// the impl's concern — no encoding trait).
pub trait Address: Copy + Eq {
    /// The creator-minted birth nonce namespace (`Copy + Eq` only — no
    /// `Hash` bound leaks into the pure layer).
    type Nonce: Copy + Eq;
    /// The address of the child born of this address at `nonce`.
    #[must_use]
    fn birth(self, nonce: Self::Nonce) -> Self;
}

/// The fixed framework event alphabet folded by every layer. A source-adding
/// layer handles its own variant and forwards the rest inward; a plain actor
/// treats every non-`User` variant as a no-op. This flat alphabet (rather than
/// nested per-layer sums) keeps the machinery uniform at fixed arity — the
/// open source set is ADR-0030's deferred door.
pub enum Envelope<A: Address, M> {
    /// A user-lane message with the driver-stamped sender address.
    User {
        /// The sender, stamped by the driver (ADR-0015: the sender is the
        /// authority) — reply-to with zero behavior-side machinery.
        from: A,
        /// The user-lane payload.
        msg: M,
    },
    /// The single-shot deadline arm fired.
    Deadline,
    /// A watched/linked peer stopped, with how it ended.
    LinkDied {
        /// The dead peer's address.
        peer: A,
        /// The death OUTCOME — classification is pure policy in the layer,
        /// never a driver-pre-digested flag.
        outcome: Result<Exit<A>, Crash>,
    },
    /// A supervised child fold ended, received at `at` (the budget-window
    /// stamp), with how it ended.
    ChildStopped {
        /// The child's birth nonce (slot = nonce — symmetric with
        /// [`Target::Child`]).
        nonce: A::Nonce,
        /// The death OUTCOME (see `LinkDied`).
        outcome: Result<Exit<A>, Crash>,
        /// The driver-minted receipt stamp for windowed budgets — the fold
        /// never reads a clock.
        at: Instant,
    },
}

/// A behavior's **become** (Agha 1986): the replacement behavior it designates
/// as it processes a message. `Continue` = become(same), `Goto(p)` =
/// become(other from the phase menu), `Stop(_)` = become(⊥). One leg of the
/// Agha actions — the [`Actions`] carries it alongside the sends and creates.
pub type Become<A, Ph = Never> = Step<Ph, Exit<A>>;

/// An abstract mail address (Agha): the fold names a send recipient by an
/// opaque token; the driver owns the token -> real ref map. Golf vocabulary:
/// the nonce mixes into the address by a toy deterministic pure function
/// (golf vocabulary, not crypto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailAddr(pub u64);

impl Address for MailAddr {
    type Nonce = u64;

    fn birth(self, nonce: u64) -> Self {
        MailAddr(self.0 ^ nonce.wrapping_mul(0x9E37_79B9_7F4A_7C15))
    }
}

/// A send target: a global mail address, or one of the sender's own
/// children by birth nonce (the driver resolves the nonce against its
/// child table — symmetric with `Envelope::ChildStopped`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<A: Address> {
    /// A global address (Agha mobility: addresses travel in messages).
    Global(A),
    /// A child of the sender, by the nonce its birth carried.
    Child(A::Nonce),
}

/// One create-effect, self-describing (2026-08-04 design): a **birth** is a
/// fresh actor at `Address::birth(self, nonce)` (creator-minted, framework
/// freshness-validated: the driver spawns at the derived address); a
/// **restart** is a supervisor's restart decision for the child born at
/// `nonce` — the address and mailbox SURVIVE, only the behavior is swapped
/// (keep-address restart; address mobility makes re-pointing escaped handles
/// impossible, so a restart is never a birth). The golf records the decision;
/// the live driver interprets: `Birth` spawns, `Restart` rides the child's
/// control lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Create<A: Address, New> {
    /// A fresh actor at `Address::birth(self, nonce)` (creator-minted,
    /// framework freshness-validated): the driver spawns at the derived
    /// address.
    Birth {
        /// The creator-minted birth nonce (the ONLY birth shape — the tree
        /// is total: every parent is a supervisor namespace).
        nonce: A::Nonce,
        /// The child spec the driver spawns.
        child: New,
    },
    /// A restart decision for the child born at `nonce`: the address and
    /// mailbox SURVIVE, only the behavior is swapped (keep-address).
    Restart {
        /// The birth nonce of the child to restart (slots ARE nonces).
        nonce: A::Nonce,
        /// The replacement behavior for the slot's surviving mailbox.
        child: New,
    },
}

/// The Agha actions: everything a behavior emits from ONE event — the messages
/// it SENT, the actors it CREATED, and the replacement behavior it BECAME. This
/// is the full triple [`Behavior::step`] returns; send and create are
/// first-class trace data, not swallowed side effects. `A` comes FIRST — it
/// is the namespace the other three parameters live in.
pub struct Actions<A: Address, Ph, Out, New> {
    /// Messages sent this turn, each addressed to a [`Target`] — a global
    /// address or an own child by birth nonce.
    pub sends: Vec<(Target<A>, Out)>,
    /// Create-effects this turn (the driver interprets each): births and
    /// restarts, self-describing via [`Create`].
    pub creates: Vec<Create<A, New>>,
    /// The replacement behavior.
    pub become_: Become<A, Ph>,
}

impl<A: Address, Ph, Out, New> Actions<A, Ph, Out, New> {
    /// Actions with no sends and no creates — just a `become`.
    #[must_use]
    pub fn just(become_: Become<A, Ph>) -> Self {
        Self { sends: Vec::new(), creates: Vec::new(), become_ }
    }

    /// The pure `Continue` actions (no effects).
    #[must_use]
    pub fn cont() -> Self {
        Self::just(Step::Continue)
    }

    /// The pure `Stop(exit)` actions (no effects).
    #[must_use]
    pub fn stop(exit: Exit<A>) -> Self {
        Self::just(Step::Stop(exit))
    }

    /// The pure `Goto(phase)` actions (no effects).
    #[must_use]
    pub fn goto(phase: Ph) -> Self {
        Self::just(Step::Goto(phase))
    }
}

/// The static fleet a behavior declares at construction (see
/// [`Behavior::fleet`]). A struct (not a tuple) to stay under the
/// `type_complexity` bar, like [`Acted`]; parameterized over the address
/// and offspring types directly (not the behavior) so a wrapper's forward
/// of `inner.fleet()` typechecks without reconstruction.
#[derive(Debug, Clone, Copy)]
pub struct Fleet<A: Address, New> {
    /// The number of static children.
    pub n: usize,
    /// The static fleet's birth-nonce minter. The driver mints child
    /// ADDRESSES from these same nonces (`Address::birth(self_addr,
    /// nonces(i))`), so the driver's child table and the behavior's
    /// liveness table agree by construction — slot = nonce.
    pub nonces: fn(usize) -> A::Nonce,
    /// The child constructor for a fleet index (also the restart builder —
    /// a static child's table position IS its fleet index).
    pub build: fn(usize) -> New,
}

/// The outcome of one fold: the full Agha [`Actions`] on the phase menu `Ph`
/// with outbound menu `Out` and create-spec `New`, or a controlled crash `E`.
/// Named so the [`Behavior::step`] future's `Output` stays legible (and under
/// the `type_complexity` bar).
pub type Acted<A, Ph, Out, New, E> = Result<Actions<A, Ph, Out, New>, E>;

/// The floor coalgebra: a state type with its transition, bound in one
/// type. Every plain actor is one of these — the state IS the behavior's
/// state, `handle` is the fold over user messages. The effect menus
/// (`Out`/`Child`/`Err`) are generic with `Never` defaults so a pure actor
/// declares exactly two types; a sender names its menu
/// (`impl State<Msg> for Worker`), a creator its offspring.
pub trait State<Out = Never, Child = Never, Err = Never> {
    /// The mail-address namespace this behavior names recipients and
    /// children with.
    type Addr: Address;
    /// The user-message menu it folds.
    type Msg;
    /// Fold one user message with its driver-stamped sender: the messages
    /// sent, the actors created, the replacement verdict (Agha's triple),
    /// or a controlled crash.
    ///
    /// # Errors
    /// Returns the impl's declared controlled-crash type `Err` — the fold
    /// never panics on data.
    fn handle(&mut self, from: Self::Addr, msg: Self::Msg) -> Acted<Self::Addr, Never, Out, Child, Err>;
}

/// The one async object: state in `&mut self`, one total `step` over the
/// [`Envelope`] alphabet, plus the `next_deadline` query the driver arms its timer
/// from. `step` returns an explicit `impl Future + Send` (not `async fn`) so
/// the `Send` bound is nameable at the driver's `spawn` boundary.
pub trait Behavior {
    /// The address type this behavior names recipients and children with.
    type Addr: Address;
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
    #[allow(
        clippy::type_complexity,
        reason = "the five seats are the Behavior assoc types themselves — further factoring hides the fold's own signature"
    )]
    fn step(
        &mut self,
        ev: Envelope<Self::Addr, Self::Msg>,
    ) -> impl Future<Output = Acted<Self::Addr, Self::Ph, Self::Outbound, Self::Offspring, Self::Error>> + Send;

    /// The next instant this behavior needs waking, as a pure function of
    /// current state (`None` = no deadline). The deadline SOURCE is a query,
    /// not an event (quinn `poll_timeout` shape); its FIRING is `Envelope::Deadline`.
    /// Default: no deadline (a plain actor arms nothing).
    fn next_deadline(&self) -> Option<Instant> {
        None
    }

    /// The static child fleet this behavior declares at construction, as a
    /// pure function of state (`None` = no fleet — a plain actor). The
    /// driver's fleet birth happens at process START only; restarts do NOT
    /// re-spawn (the child table survives the slot swap). The fleet carries
    /// its birth-nonce minter: the driver derives child addresses from the
    /// same nonces the behavior's table records (slot = nonce).
    fn fleet(&self) -> Option<Fleet<Self::Addr, Self::Offspring>> {
        None
    }
}

/// Lift a become-only reaction verdict into a full [`Actions`] with empty effect
/// lists: `Goto` cannot exist at `Never`, so only `Continue`/`Stop` ride out of
/// a framework reaction, and a reaction sends and creates nothing. This is the
/// phase-lift every source capability applies to its reaction's result.
pub fn lift<A: Address, Ph, Out, New>(v: Step<Never, Exit<A>>) -> Actions<A, Ph, Out, New> {
    Actions::just(match v {
        Step::Continue => Step::Continue,
        Step::Goto(never) => match never {},
        Step::Stop(e) => Step::Stop(e),
    })
}

/// The floor: a plain actor = one [`State`]. Framework events (deadline /
/// link-death / child-stop) are no-ops — a plain actor owns no source. The
/// `Never` defaults on the effect menus mean a floor provably sends and
/// creates nothing. Every capability wraps a `Behavior`; `Base` is the
/// innermost one.
pub struct Base<S: State<O, N, E>, O = Never, N = Never, E = Never> {
    state: S,
    /// The effect menus live on `State`'s generics, not in the state
    /// value — the marker carries them for the struct (variance-safe, no
    /// ownership).
    fx: PhantomData<fn(O, N, E)>,
}

impl<S: State<O, N, E>, O, N, E> Base<S, O, N, E> {
    /// Builds a floor over `state`.
    pub fn new(state: S) -> Self {
        Self { state, fx: PhantomData }
    }

    /// The accumulated state (test observability).
    pub fn state(&self) -> &S {
        &self.state
    }
}

/// A plain actor's transition as a fn pointer: state, sender, message →
/// the Agha triple on the erased menu (or a controlled crash). Named for
/// the `type_complexity` bar, like [`Acted`].
pub type Transition<S, A, M, O, N, E> = fn(&mut S, A, M) -> Acted<A, Never, O, N, E>;

/// A fn-pointer-backed [`State`], for tests and trivial actors: the state
/// value and the transition, paired. The field's fn-pointer type carries
/// the whole signature, so the type is always nameable (a generated actor
/// stays nameable); non-capturing closures coerce at the call site. Real
/// actors write a named [`State`] impl.
pub struct FnState<S, A: Address, M, O = Never, N = Never, E = Never> {
    /// The state value the transition folds over.
    pub state: S,
    /// The transition.
    pub handle: Transition<S, A, M, O, N, E>,
}

impl<S, A: Address, M, O, N, E> State<O, N, E> for FnState<S, A, M, O, N, E> {
    type Addr = A;
    type Msg = M;
    fn handle(&mut self, from: A, msg: M) -> Acted<A, Never, O, N, E> {
        (self.handle)(&mut self.state, from, msg)
    }
}

impl<S, A: Address, M, O, N, E> Base<FnState<S, A, M, O, N, E>, O, N, E> {
    /// Builds a floor from a state value and a transition (fn pointer, or a
    /// non-capturing closure, which coerces).
    pub fn from_fn(state: S, handle: Transition<S, A, M, O, N, E>) -> Self {
        Self { state: FnState { state, handle }, fx: PhantomData }
    }
}

impl<S, O, N, E> Behavior for Base<S, O, N, E>
where
    S: State<O, N, E> + Send,
    S::Addr: Send,
    <S::Addr as Address>::Nonce: Send,
    S::Msg: Send,
    O: Send,
    N: Send,
    E: Send,
{
    type Addr = S::Addr;
    type Msg = S::Msg;
    type Ph = Never;
    type Error = E;
    type Outbound = O;
    type Offspring = N;
    async fn step(&mut self, ev: Envelope<S::Addr, S::Msg>) -> Acted<S::Addr, Never, O, N, E> {
        match ev {
            Envelope::User { from, msg } => self.state.handle(from, msg),
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
pub struct Transcript<A: Address, Out, New> {
    /// Every message the behavior sent, in emission order.
    pub sends: Vec<(Target<A>, Out)>,
    /// Every create-effect the behavior emitted, in emission order.
    pub creates: Vec<Create<A, New>>,
    /// The exit that ended the fold.
    pub exit: Exit<A>,
}

/// Drive a fully-erased behavior over its fastpass mailbox until it stops or the
/// mailbox drains, RECORDING the triple: each step's sends and creates are
/// accumulated and returned in the [`Transcript`]. The user lane becomes
/// `Envelope::User` stamped with `from`; the control lane is routed by the
/// Watching / Supervising layers (Task 2 continued).
///
/// `Stop(exit)` ends the fold immediately; `Goto` is unconstructible at
/// `Ph = Never`; `Err` short-circuits; a drained mailbox is collection.
///
/// # Errors
/// Returns the behavior's `Error` the first time a step is a controlled crash.
pub async fn run<B, C>(
    mut b: B,
    mut mailbox: Consumer<C, B::Msg>,
    from: B::Addr,
) -> Result<Transcript<B::Addr, B::Outbound, B::Offspring>, B::Error>
where
    B: Behavior<Ph = Never>,
{
    let mut sends = Vec::new();
    let mut creates = Vec::new();
    while let Some(recv) = mailbox.recv().await {
        let ev = match recv {
            Received::User(m) => Envelope::User { from, msg: m },
            // The control lane becomes load-bearing with Watching / Supervising;
            // the user-lane-closed leg is drain-stop observability for the live
            // driver (fork C) — the golf driver waits for full collection.
            Received::Control(_) | Received::UserLaneClosed => continue,
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
    use super::{Actions, Base, Behavior, Envelope, FnState, MailAddr, Step, Target, run};
    use crate::Exit;
    use crate::verdict::Never;
    use fastpass::{Config, channel};

    type Rec = Base<FnState<Vec<u64>, MailAddr, u64, Never, Never, &'static str>, Never, Never, &'static str>;

    #[tokio::test]
    async fn base_folds_user_messages_and_ignores_framework_events() {
        let mut b: Rec = Base::from_fn(Vec::<u64>::new(), |seen: &mut Vec<u64>, _from: MailAddr, id: u64| {
            if id == 0 {
                return Ok(Actions::stop(Exit::Normal));
            }
            seen.push(id);
            Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
        });

        assert!(matches!(b.step(Envelope::Deadline).await.unwrap().become_, Step::Continue));
        assert!(matches!(
            b.step(Envelope::User { from: MailAddr(1), msg: 7 }).await.unwrap().become_,
            Step::Continue
        ));
        assert!(matches!(
            b.step(Envelope::User { from: MailAddr(1), msg: 0 }).await.unwrap().become_,
            Step::Stop(Exit::Normal)
        ));
        assert_eq!(b.state().state, vec![7], "only the delivered user message folded");
    }

    #[tokio::test]
    async fn base_has_no_deadline() {
        type Plain = Base<FnState<(), MailAddr, (), Never, Never, Never>, Never, Never, Never>;
        let b: Plain =
            Base::from_fn((), |(): &mut (), _from: MailAddr, (): ()| Ok::<Actions<MailAddr, Never, Never, Never>, Never>(Actions::cont()));
        assert!(b.next_deadline().is_none(), "a plain actor arms no deadline");
    }

    /// Sums user messages; stops normally once the running total reaches 10.
    struct Counter(u32);

    impl Behavior for Counter {
        type Addr = MailAddr;
        type Msg = u32;
        type Ph = Never;
        type Error = &'static str;
        type Outbound = Never;
        type Offspring = Never;
        async fn step(
            &mut self,
            ev: Envelope<MailAddr, u32>,
        ) -> Result<Actions<MailAddr, Never, Never, Never>, &'static str> {
            if let Envelope::User { msg, .. } = ev {
                self.0 += msg;
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
        let handle = tokio::spawn(run(Counter(0), rx, MailAddr(0)));

        usr.send(3).await.expect("mailbox open");
        usr.send(4).await.expect("mailbox open");
        usr.send(5).await.expect("mailbox open"); // total 12 ⇒ Stop(Normal)

        let out = handle.await.expect("driver task joins");
        assert_eq!(out.unwrap().exit, Exit::Normal, "the Stop verdict's exit rides out");
    }

    #[tokio::test]
    async fn driver_reports_collected_when_the_mailbox_drains() {
        let (ctl, usr, rx) = channel::<Never, u32>(Config::new(8));
        let handle = tokio::spawn(run(Counter(0), rx, MailAddr(0)));

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
        type Sender = Base<FnState<St, MailAddr, u64, u64, Never, &'static str>, u64, Never, &'static str>;
        let b: Sender =
            Base::from_fn(St, |_: &mut St, _from: MailAddr, _m: u64| {
                Ok(Actions { sends: vec![(Target::Global(MailAddr(7)), 99)], creates: vec![], become_: Step::Stop(Exit::Normal) })
            });
        let (_ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
        let handle = tokio::spawn(run(b, rx, MailAddr(0)));
        usr.send(1).await.expect("mailbox open");

        let transcript = handle.await.expect("driver task joins").expect("no crash");
        assert_eq!(
            transcript.sends,
            vec![(Target::Global(MailAddr(7)), 99)],
            "the peer receives what the actor sent"
        );
        assert_eq!(transcript.exit, Exit::Normal);
    }
}
