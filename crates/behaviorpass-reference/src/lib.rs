//! The frozen-reference algebra (ADR-0030): one object ([`Behavior`]), one
//! fold ([`run`]), and one model layer per bombay capability. This crate is
//! the executable spec of the Behavior algebra — the synchronous projection
//! of "an actor is an async fold of one step shape over merged sources"
//! (ADR-0028). Trace equality to this model defines correctness for the
//! concision loop; the loop may never edit this crate.
//!
//! The verdict vocabulary is bombay's own ([`Step`]/[`Never`]) so a trace
//! produced by the real capability machinery is compared to a model trace at
//! the SAME type — no adapter, no coercion.

use std::collections::VecDeque;

use bombay::capability::{Deferred, Disposition, Never, Step};

/// The model's exit vocabulary — the `R` parameter of [`Step`] (ADR-0029
/// used as designed). The testkit maps it onto `ActorStopReason` kinds when
/// it compares a model trace to a real-actor trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Clean self-stop (`Flow::Stop(Normal)`'s model image).
    Normal,
    /// Sources exhausted — the mailbox-closed / ref-count-collection image.
    Collected,
    /// A watch layer propagated a linked peer's death, carrying its id.
    LinkDied(u64),
}

/// The one object (sync projection): state in `&mut self`, one total `step`
/// over the event alphabet. A capability that adds a source extends `Event`
/// as a sum type; the step routes its own events and forwards the rest.
pub trait Behavior {
    /// The event alphabet this behavior folds over.
    type Event;
    /// The become-menu still exposed upward (`Never` once fully erased).
    type Ph;
    /// The controlled-crash type.
    type Error;
    /// One fold step: typed become — continue, switch behavior, or stop.
    ///
    /// # Errors
    /// Returns the behavior's `Error` when the step is a controlled crash.
    fn step(&mut self, ev: Self::Event) -> Result<Step<Self::Ph, Exit>, Self::Error>;
}

/// The essence-fold: drive a fully-erased behavior (`Ph = Never`) over a
/// trace. `Stop` ends the fold immediately; `Goto` is unconstructible at
/// `Never`; `Err` short-circuits unchanged; an exhausted trace is
/// collection, not success.
///
/// # Errors
/// Returns the behavior's `Error` verbatim the first time a step crashes.
pub fn run<B: Behavior<Ph = Never>>(
    b: &mut B,
    events: impl IntoIterator<Item = B::Event>,
) -> Result<Exit, B::Error> {
    for ev in events {
        match b.step(ev)? {
            Step::Continue => {}
            Step::Goto(never) => match never {},
            Step::Stop(exit) => return Ok(exit),
        }
    }
    Ok(Exit::Collected)
}

// ----------------------------------------------------------------- Base --

/// A message handler: folds one message into `&mut S`, returning a verdict on
/// the phase menu `P` (fn pointer, not a closure, so the model stays nameable).
pub type Handler<S, M, P, E> = fn(&mut S, M) -> Result<Step<P, Exit>, E>;

/// A layer reaction over the whole inner behavior `B` with error `E` (the
/// deadline/link reactions share this shape).
pub type Reaction<B, E> = fn(&mut B) -> Result<Step<Never, Exit>, E>;

/// A link-death reaction: the inner behavior plus the dead peer's id and
/// abnormal flag.
pub type LinkReaction<B, E> = fn(&mut B, u64, bool) -> Result<Step<Never, Exit>, E>;

/// The floor layer: a plain actor = state + handler. `P` is the become menu
/// the handler exposes upward (`Never` for a one-phase actor).
pub struct Base<S, M, P, E> {
    /// The user state the fold accumulates into.
    pub state: S,
    /// The handler folding each message into `&mut state`.
    pub handle: Handler<S, M, P, E>,
}

impl<S, M, P, E> Behavior for Base<S, M, P, E> {
    type Event = M;
    type Ph = P;
    type Error = E;
    fn step(&mut self, ev: M) -> Result<Step<P, Exit>, E> {
        (self.handle)(&mut self.state, ev)
    }
}

// ------------------------------------------------------------- Deadlined --

/// A time-armed source's alphabet extension: expiry or the inner event.
#[derive(Debug, PartialEq, Eq)]
pub enum Timed<E> {
    /// The single-shot deadline source fired.
    Deadline,
    /// A pass-through inner event.
    Event(E),
}

/// The deadline capability as a layer: adds the expiry event, routes it to
/// the reaction, forwards everything else.
pub struct Deadlined<B: Behavior> {
    /// The wrapped behavior.
    pub inner: B,
    /// The expiry reaction — reads/writes the inner behavior.
    pub on_deadline: Reaction<B, B::Error>,
}

impl<B: Behavior> Behavior for Deadlined<B> {
    type Event = Timed<B::Event>;
    type Ph = B::Ph;
    type Error = B::Error;
    fn step(&mut self, ev: Self::Event) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Timed::Event(inner_ev) => self.inner.step(inner_ev),
            Timed::Deadline => Ok(match (self.on_deadline)(&mut self.inner)? {
                Step::Continue => Step::Continue,
                Step::Goto(never) => match never {},
                Step::Stop(exit) => Step::Stop(exit),
            }),
        }
    }
}

// -------------------------------------------------------------- Watching --

/// The link source's alphabet extension: a death notice or the inner event.
#[derive(Debug, PartialEq, Eq)]
pub enum Linked<E> {
    /// A watched/linked peer stopped.
    LinkDied {
        /// The dead peer's model id.
        peer: u64,
        /// Whether the stop was abnormal (the OTP propagation trigger).
        abnormal: bool,
    },
    /// A pass-through inner event.
    Event(E),
}

/// The watch capability as a layer: adds the death-notice event and routes
/// it to the policy.
pub struct Watching<B: Behavior> {
    /// The wrapped behavior.
    pub inner: B,
    /// The death reaction (the model image of `WatchPolicy`).
    pub on_link_died: LinkReaction<B, B::Error>,
}

/// The default policy's model image: propagate an abnormal linked death,
/// absorb everything else (`OtpPropagation`).
///
/// # Errors
/// Never — the propagation decision is pure; the signature matches the seat.
pub fn otp_propagation<B: Behavior>(
    _: &mut B,
    peer: u64,
    abnormal: bool,
) -> Result<Step<Never, Exit>, B::Error> {
    if abnormal {
        Ok(Step::Stop(Exit::LinkDied(peer)))
    } else {
        Ok(Step::Continue)
    }
}

impl<B: Behavior> Behavior for Watching<B> {
    type Event = Linked<B::Event>;
    type Ph = B::Ph;
    type Error = B::Error;
    fn step(&mut self, ev: Self::Event) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Linked::Event(inner_ev) => self.inner.step(inner_ev),
            Linked::LinkDied { peer, abnormal } => {
                Ok(match (self.on_link_died)(&mut self.inner, peer, abnormal)? {
                    Step::Continue => Step::Continue,
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Step::Stop(exit),
                })
            }
        }
    }
}

// -------------------------------------------------------------- Stashing --

/// The stash routing verdict (the model image of user-driven stashing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    /// Hold the event for a later release.
    Stash,
    /// Deliver now.
    Deliver,
    /// Deliver now, then replay the whole held batch in this same step.
    Release,
}

/// The stash capability as a layer: two queues (held vs the draining batch —
/// the shape the real `Stashing` arrived at) and an in-step drain that
/// preserves batch atomicity by construction (no outer event interleaves).
pub struct Stashing<B: Behavior> {
    /// The wrapped behavior.
    pub inner: B,
    route: fn(&B::Event) -> StashRoute,
    held: VecDeque<B::Event>,
}

impl<B: Behavior<Ph = Never>> Stashing<B> {
    /// Builds an empty-stash layer over `inner` with the given routing.
    pub fn new(inner: B, route: fn(&B::Event) -> StashRoute) -> Self {
        Self { inner, route, held: VecDeque::new() }
    }

    /// How many events are currently held (test observability).
    #[must_use]
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Drains a SNAPSHOT of the held queue through the route: re-stashed
    /// events return to `held`, never to the draining batch (the bound that
    /// terminates the drain); `Stop` abandons the rest of the batch.
    fn drain(&mut self) -> Result<Step<Never, Exit>, B::Error> {
        let mut batch: VecDeque<B::Event> = self.held.drain(..).collect();
        while let Some(ev) = batch.pop_front() {
            match (self.route)(&ev) {
                StashRoute::Stash => self.held.push_back(ev),
                StashRoute::Deliver | StashRoute::Release => {
                    if let Step::Stop(exit) = self.inner.step(ev)? {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B: Behavior<Ph = Never>> Behavior for Stashing<B> {
    type Event = B::Event;
    type Ph = Never;
    type Error = B::Error;
    fn step(&mut self, ev: Self::Event) -> Result<Step<Never, Exit>, B::Error> {
        match (self.route)(&ev) {
            StashRoute::Stash => {
                self.held.push_back(ev);
                Ok(Step::Continue)
            }
            StashRoute::Deliver => self.inner.step(ev),
            StashRoute::Release => {
                if let Step::Stop(exit) = self.inner.step(ev)? {
                    return Ok(Step::Stop(exit));
                }
                self.drain()
            }
        }
    }
}

// ---------------------------------------------------------------- Phased --

/// The phase capability as a layer — BOTH planes: the gate wraps the step
/// (message plane) and the deferral seat's replay extends behavior within
/// the step (event plane). Erases the inner become-menu (`Ph = Never`
/// upward — the menu is consumed here).
pub struct Phased<B: Behavior> {
    /// The wrapped behavior (its `Ph` is this layer's phase menu).
    pub inner: B,
    phase: B::Ph,
    gate: fn(B::Ph, &B::Event) -> Disposition<Deferred>,
    held: VecDeque<B::Event>,
    batch: VecDeque<B::Event>,
}

impl<B> Phased<B>
where
    B: Behavior,
    B::Ph: Copy + PartialEq,
{
    /// Builds the layer in `initial`, empty queues.
    pub fn new(
        inner: B,
        initial: B::Ph,
        gate: fn(B::Ph, &B::Event) -> Disposition<Deferred>,
    ) -> Self {
        Self { inner, phase: initial, gate, held: VecDeque::new(), batch: VecDeque::new() }
    }

    /// The committed phase (test observability).
    #[must_use]
    pub fn phase(&self) -> B::Ph {
        self.phase
    }

    /// One delivered inner step. A `Goto` verdict commits HERE — inside the
    /// `Ok`, so an `Err` cannot half-switch (D3 is structural) — and a phase
    /// CHANGE releases the held queue into the draining batch.
    fn deliver(&mut self, ev: B::Event) -> Result<Step<Never, Exit>, B::Error> {
        match self.inner.step(ev)? {
            Step::Continue => Ok(Step::Continue),
            Step::Stop(exit) => Ok(Step::Stop(exit)),
            Step::Goto(next) => {
                if next != self.phase {
                    self.phase = next;
                    self.batch.extend(self.held.drain(..));
                }
                Ok(Step::Continue)
            }
        }
    }

    /// The in-step replay drain: every replayed event RE-GATES in the current
    /// phase; a re-deferred event returns to `held`, never to the batch
    /// (snapshot bound); `Stop` abandons the rest.
    fn drain(&mut self) -> Result<Step<Never, Exit>, B::Error> {
        while let Some(ev) = self.batch.pop_front() {
            match (self.gate)(self.phase, &ev) {
                Disposition::Ignore => {}
                Disposition::Defer(Deferred) => self.held.push_back(ev),
                Disposition::Deliver => {
                    if let Step::Stop(exit) = self.deliver(ev)? {
                        self.batch.clear();
                        return Ok(Step::Stop(exit));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B> Behavior for Phased<B>
where
    B: Behavior,
    B::Ph: Copy + PartialEq,
{
    type Event = B::Event;
    type Ph = Never;
    type Error = B::Error;
    fn step(&mut self, ev: Self::Event) -> Result<Step<Never, Exit>, B::Error> {
        match (self.gate)(self.phase, &ev) {
            Disposition::Ignore => Ok(Step::Continue),
            Disposition::Defer(Deferred) => {
                self.held.push_back(ev);
                Ok(Step::Continue)
            }
            Disposition::Deliver => {
                if let Step::Stop(exit) = self.deliver(ev)? {
                    return Ok(Step::Stop(exit));
                }
                self.drain()
            }
        }
    }
}

// ----------------------------------------------------------- Supervising --

/// The supervision source's alphabet extension.
#[derive(Debug, PartialEq, Eq)]
pub enum Sup<E> {
    /// A child fold ended (the link-notice image on the supervisor side).
    ChildStopped {
        /// Index into the child table.
        idx: usize,
        /// Whether the child's stop was abnormal (restart-eligible).
        abnormal: bool,
    },
    /// A pass-through inner event.
    Event(E),
}

/// One supervised child: an inner fold and its liveness.
pub struct Child<C> {
    /// The child's behavior — a whole fold instance.
    pub behavior: C,
    /// False once stopped-for-good (normal stop or budget exhausted).
    pub alive: bool,
}

/// The supervision capability as a layer: the OUTER fold restarting inner
/// folds. One-for-one, budget-bounded — model grade.
pub struct Supervising<B: Behavior, C: Behavior<Ph = Never>> {
    /// The supervisor's own behavior.
    pub inner: B,
    /// The child table (each child an inner fold).
    pub children: Vec<Child<C>>,
    /// The restart factory: a restart is a FRESH fold, never resumed state.
    pub build: fn(usize) -> C,
    /// Remaining restart budget (the two-counter accounting, collapsed).
    pub restarts_left: u32,
}

impl<B: Behavior, C: Behavior<Ph = Never>> Supervising<B, C> {
    fn on_child_stopped(&mut self, idx: usize, abnormal: bool) {
        let Some(child) = self.children.get_mut(idx) else {
            return;
        };
        if abnormal && self.restarts_left > 0 {
            self.restarts_left -= 1;
            *child = Child { behavior: (self.build)(idx), alive: true };
        } else {
            child.alive = false;
        }
    }
}

impl<B: Behavior, C: Behavior<Ph = Never>> Behavior for Supervising<B, C> {
    type Event = Sup<B::Event>;
    type Ph = B::Ph;
    type Error = B::Error;
    fn step(&mut self, ev: Self::Event) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Sup::Event(inner_ev) => self.inner.step(inner_ev),
            Sup::ChildStopped { idx, abnormal } => {
                self.on_child_stopped(idx, abnormal);
                Ok(Step::Continue)
            }
        }
    }
}
