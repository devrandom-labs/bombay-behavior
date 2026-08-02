//! The five async capability layers (ADR-0030) — async twins of
//! `behaviorpass-reference`. Each is a `C<B>: Behavior where B: Behavior`;
//! the loop golfs their code-only LOC while the frozen oracle holds them
//! trace-equal to the sync reference.

use std::collections::VecDeque;

use bombay::capability::{Deferred, Disposition, Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Behavior, Handler, Wire, lift};

/// The floor layer: a plain actor = state + a synchronous handler. Framework
/// events (deadline / link-death / child-stop) are no-ops — a plain actor has
/// no layer that owns them.
pub struct Base<S, M, P, E> {
    state: S,
    handle: Handler<S, M, P, E>,
}

impl<S, M, P, E> Base<S, M, P, E> {
    /// Builds a floor over `state` with `handle`.
    pub fn new(state: S, handle: Handler<S, M, P, E>) -> Self {
        Self { state, handle }
    }

    /// The accumulated state (test observability).
    pub fn state(&self) -> &S {
        &self.state
    }
}

impl<S, M, P, E> Behavior for Base<S, M, P, E>
where
    S: Send,
    M: Send,
    P: Send,
    E: Send,
{
    type Msg = M;
    type Ph = P;
    type Error = E;
    async fn step(&mut self, ev: Wire<M>) -> Result<bombay::capability::Step<P, crate::Exit>, E> {
        match ev {
            Wire::User(m) => (self.handle)(&mut self.state, m),
            // A plain actor owns no framework source — every non-user event
            // is a no-op.
            Wire::Deadline | Wire::LinkDied { .. } | Wire::ChildStopped { .. } => {
                Ok(bombay::capability::Step::Continue)
            }
        }
    }
}

/// The reaction a deadline fire runs: mutates the inner behavior, returns a
/// verdict on the erased menu (`Never` — a deadline reaction cannot `Goto`).
pub type DeadlineReaction<B> = fn(&mut B) -> Result<Step<Never, Exit>, <B as Behavior>::Error>;

/// The deadline capability as a layer: adds the single-shot deadline source
/// (armed via [`Behavior::next_deadline`], fired as [`Wire::Deadline`]),
/// routes the fire to its reaction, and forwards every other event inward.
pub struct Deadlined<B: Behavior> {
    inner: B,
    due: Option<Instant>,
    on_deadline: DeadlineReaction<B>,
}

impl<B: Behavior> Deadlined<B> {
    /// Builds the layer with an initial deadline slot and its reaction.
    pub fn new(inner: B, due: Option<Instant>, on_deadline: DeadlineReaction<B>) -> Self {
        Self { inner, due, on_deadline }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }
}

impl<B> Behavior for Deadlined<B>
where
    B: Behavior + Send,
    B::Msg: Send,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    async fn step(&mut self, ev: Wire<B::Msg>) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Wire::Deadline => {
                self.due = None; // fires once per armed value
                Ok(lift((self.on_deadline)(&mut self.inner)?))
            }
            other => self.inner.step(other).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        // The min-fold law (ADR-0030): the earliest of this layer's slot and
        // any inner deadline arms the one timer.
        match (self.due, self.inner.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

// -------------------------------------------------------------- Watching --

/// The reaction a link-death runs: the inner behavior plus the dead peer's id
/// and abnormal flag, returning a verdict on the erased menu.
pub type LinkReaction<B> =
    fn(&mut B, u64, bool) -> Result<Step<Never, Exit>, <B as Behavior>::Error>;

/// The watch capability as a layer: adds the link-death source
/// ([`Wire::LinkDied`]), routes it to the policy, forwards the rest.
pub struct Watching<B: Behavior> {
    inner: B,
    on_link_died: LinkReaction<B>,
}

impl<B: Behavior> Watching<B> {
    /// Builds the layer over `inner` with a death policy.
    pub fn new(inner: B, on_link_died: LinkReaction<B>) -> Self {
        Self { inner, on_link_died }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }
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

impl<B> Behavior for Watching<B>
where
    B: Behavior + Send,
    B::Msg: Send,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    async fn step(&mut self, ev: Wire<B::Msg>) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Wire::LinkDied { peer, abnormal } => {
                Ok(lift((self.on_link_died)(&mut self.inner, peer, abnormal)?))
            }
            other => self.inner.step(other).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

// -------------------------------------------------------------- Stashing --

/// The stash routing verdict for a user message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StashRoute {
    /// Hold for a later release.
    Stash,
    /// Deliver now.
    Deliver,
    /// Deliver now, then replay the whole held batch in this same step.
    Release,
}

/// The stash capability as a layer: two queues (held vs the draining batch)
/// and an in-step drain preserving batch atomicity by construction — no outer
/// event interleaves a release.
pub struct Stashing<B: Behavior> {
    inner: B,
    route: fn(&B::Msg) -> StashRoute,
    held: VecDeque<B::Msg>,
}

impl<B: Behavior<Ph = Never>> Stashing<B> {
    /// Builds an empty-stash layer over `inner` with the given routing.
    pub fn new(inner: B, route: fn(&B::Msg) -> StashRoute) -> Self {
        Self { inner, route, held: VecDeque::new() }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// How many messages are currently held.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    async fn drain(&mut self) -> Result<Step<Never, Exit>, B::Error> {
        let mut batch: VecDeque<B::Msg> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            match (self.route)(&m) {
                StashRoute::Stash => self.held.push_back(m),
                StashRoute::Deliver | StashRoute::Release => {
                    if let Step::Stop(exit) = self.inner.step(Wire::User(m)).await? {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B> Behavior for Stashing<B>
where
    B: Behavior<Ph = Never> + Send,
    B::Msg: Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Wire<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        let Wire::User(m) = ev else {
            // Framework events are never stashed — forward inward.
            return self.inner.step(ev).await;
        };
        match (self.route)(&m) {
            StashRoute::Stash => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            StashRoute::Deliver => self.inner.step(Wire::User(m)).await,
            StashRoute::Release => {
                if let Step::Stop(exit) = self.inner.step(Wire::User(m)).await? {
                    return Ok(Step::Stop(exit));
                }
                self.drain().await
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

// ---------------------------------------------------------------- Phased --

/// The phase capability as a layer — both planes: the gate wraps user-message
/// admission and the deferral seat's replay extends behavior within the step.
/// Erases the inner become-menu (`Ph = Never` upward — the menu is consumed
/// here as phase transitions).
pub struct Phased<B: Behavior> {
    inner: B,
    phase: B::Ph,
    gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>,
    held: VecDeque<B::Msg>,
    batch: VecDeque<B::Msg>,
}

impl<B> Phased<B>
where
    B: Behavior,
    B::Ph: Copy + PartialEq,
{
    /// Builds the layer in `initial`, empty queues.
    pub fn new(inner: B, initial: B::Ph, gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>) -> Self {
        Self { inner, phase: initial, gate, held: VecDeque::new(), batch: VecDeque::new() }
    }

    /// The committed phase (test observability).
    pub fn phase(&self) -> B::Ph {
        self.phase
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// Run one event through the inner behavior, committing a `Goto` (D3:
    /// only inside the `Ok`, so an `Err` cannot half-switch) and releasing the
    /// held queue into the batch on a phase CHANGE.
    async fn run_inner(&mut self, ev: Wire<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        match self.inner.step(ev).await? {
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

    async fn deliver_and_drain(&mut self, ev: Wire<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        if let Step::Stop(exit) = self.run_inner(ev).await? {
            return Ok(Step::Stop(exit));
        }
        while let Some(m) = self.batch.pop_front() {
            match (self.gate)(self.phase, &m) {
                Disposition::Ignore => {}
                Disposition::Defer(Deferred) => self.held.push_back(m),
                Disposition::Deliver => {
                    if let Step::Stop(exit) = self.run_inner(Wire::User(m)).await? {
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
    B: Behavior + Send,
    B::Msg: Send,
    B::Ph: Copy + PartialEq + Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Wire<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        let Wire::User(m) = ev else {
            // Framework events are not gated; a reaction that gotos still
            // commits + releases via run_inner.
            return self.deliver_and_drain(ev).await;
        };
        match (self.gate)(self.phase, &m) {
            Disposition::Ignore => Ok(Step::Continue),
            Disposition::Defer(Deferred) => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            Disposition::Deliver => self.deliver_and_drain(Wire::User(m)).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

// ----------------------------------------------------------- Supervising --

/// One supervised child: an inner fold and its liveness.
pub struct Child<C> {
    behavior: C,
    alive: bool,
}

impl<C> Child<C> {
    /// The child's behavior (test observability).
    pub fn behavior(&self) -> &C {
        &self.behavior
    }

    /// Whether the child is still alive.
    pub fn alive(&self) -> bool {
        self.alive
    }
}

/// The supervision capability as a layer: the OUTER fold restarting inner
/// folds. One-for-one, budget-bounded — model grade.
pub struct Supervising<B: Behavior, C: Behavior<Ph = Never>> {
    inner: B,
    children: Vec<Child<C>>,
    build: fn(usize) -> C,
    restarts_left: u32,
}

impl<B: Behavior, C: Behavior<Ph = Never>> Supervising<B, C> {
    /// Builds a supervisor with an initial child table and restart budget.
    pub fn new(inner: B, children: Vec<C>, build: fn(usize) -> C, restarts_left: u32) -> Self {
        let children = children.into_iter().map(|c| Child { behavior: c, alive: true }).collect();
        Self { inner, children, build, restarts_left }
    }

    /// The child table (test observability).
    pub fn children(&self) -> &[Child<C>] {
        &self.children
    }

    /// Remaining restart budget (test observability).
    pub fn restarts_left(&self) -> u32 {
        self.restarts_left
    }

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

impl<B, C> Behavior for Supervising<B, C>
where
    B: Behavior + Send,
    B::Msg: Send,
    C: Behavior<Ph = Never> + Send,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    async fn step(&mut self, ev: Wire<B::Msg>) -> Result<Step<B::Ph, Exit>, B::Error> {
        match ev {
            Wire::ChildStopped { idx, abnormal } => {
                self.on_child_stopped(idx, abnormal);
                Ok(Step::Continue)
            }
            other => self.inner.step(other).await,
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{
        Base, Deadlined, Phased, StashRoute, Stashing, Supervising, Watching, otp_propagation,
    };
    use crate::Exit;
    use crate::behavior::{Behavior, Wire};
    use bombay::capability::{Deferred, Disposition, Never, Step};
    use tokio::time::Instant;

    #[tokio::test]
    async fn deadlined_routes_the_fire_forwards_the_rest_and_arms_once() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        });
        let due = Instant::now() + Duration::from_secs(5);
        let mut d = Deadlined::new(inner, Some(due), |_inner| Ok(Step::Stop(Exit::Normal)));

        assert_eq!(d.next_deadline(), Some(due), "the declared slot arms the timer");
        // A user event forwards to the inner behavior.
        assert!(matches!(d.step(Wire::User(7)).await, Ok(Step::Continue)));
        assert_eq!(d.inner().state(), &vec![7], "non-deadline events forward inward");
        // The deadline fire routes to the reaction.
        assert!(
            matches!(d.step(Wire::Deadline).await, Ok(Step::Stop(Exit::Normal))),
            "the reaction's verdict rides out",
        );
        assert_eq!(d.next_deadline(), None, "fires once — the slot clears after firing");
    }

    #[tokio::test]
    async fn base_folds_user_messages_and_ignores_framework_events() {
        let mut b = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            if id == 0 {
                return Ok(Step::Stop(Exit::Normal));
            }
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        });

        // A framework event is a no-op for a plain actor.
        assert!(matches!(b.step(Wire::Deadline).await, Ok(Step::Continue)));
        // User messages fold through the handler.
        assert!(matches!(b.step(Wire::User(7)).await, Ok(Step::Continue)));
        assert!(matches!(
            b.step(Wire::User(0)).await,
            Ok(Step::Stop(Exit::Normal))
        ));
        assert_eq!(b.state(), &vec![7], "only the delivered user message folded");
    }

    #[tokio::test]
    async fn base_has_no_deadline() {
        let b = Base::new((), |(): &mut (), (): ()| Ok::<_, Never>(Step::<Never, Exit>::Continue));
        assert!(b.next_deadline().is_none(), "a plain actor arms no deadline");
    }

    fn recorder() -> Base<Vec<u64>, u64, Never, &'static str> {
        Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, id: u64| {
            seen.push(id);
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

    #[tokio::test]
    async fn watching_propagates_abnormal_death_and_absorbs_normal() {
        let mut w = Watching::new(recorder(), otp_propagation);
        assert!(
            matches!(
                w.step(Wire::LinkDied { peer: 42, abnormal: true }).await,
                Ok(Step::Stop(Exit::LinkDied(42)))
            ),
            "an abnormal linked death propagates with the carried reason",
        );

        let mut w2 = Watching::new(recorder(), otp_propagation);
        assert!(matches!(
            w2.step(Wire::LinkDied { peer: 42, abnormal: false }).await,
            Ok(Step::Continue)
        ));
        assert!(matches!(w2.step(Wire::User(2)).await, Ok(Step::Continue)));
        assert_eq!(w2.inner().state(), &vec![2], "a normal death is absorbed; user forwards");
    }

    #[tokio::test]
    async fn stashing_holds_and_re_stashes_to_held_under_the_snapshot_bound() {
        let mut s = Stashing::new(recorder(), |&id| match id {
            0 => StashRoute::Release,
            n if n % 2 == 1 => StashRoute::Stash,
            _ => StashRoute::Deliver,
        });
        for id in [1_u64, 2, 3, 0, 4] {
            let _ = s.step(Wire::User(id)).await;
        }
        // 2 delivered; release(0) delivers 0 then drains [1,3] — both odd, so
        // they re-stash to `held` (never redelivered — the snapshot bound); 4
        // delivered. No livelock.
        assert_eq!(s.inner().state(), &vec![2, 0, 4]);
        assert_eq!(s.held(), 2, "re-stashed messages land in held, not the batch");
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
        Quit,
    }

    #[tokio::test]
    async fn phased_releases_the_deferred_batch_fifo_on_goto() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, msg: Msg| match msg {
            Msg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            Msg::Promote => Ok(Step::Goto(Ph::Ready)),
            Msg::Quit => Ok(Step::Stop(Exit::Normal)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |ph, msg| match (ph, msg) {
            (Ph::Loading, Msg::Work(_)) => Disposition::Defer(Deferred),
            _ => Disposition::Deliver,
        });
        for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
            let _ = p.step(Wire::User(m)).await;
        }
        assert_eq!(
            p.inner().state(),
            &vec![1, 2, 3],
            "the deferred batch replays FIFO inside the Promote step, ahead of 3",
        );
        assert_eq!(p.phase(), Ph::Ready);
    }

    #[tokio::test]
    async fn phased_never_commits_a_failed_handlers_goto() {
        let inner = Base::new((), |(): &mut (), msg: Msg| match msg {
            Msg::Work(_) => Err("bang"),
            _ => Ok::<Step<Ph, Exit>, &'static str>(Step::Goto(Ph::Ready)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |_, _| Disposition::Deliver);
        assert_eq!(p.step(Wire::User(Msg::Work(1))).await, Err("bang"));
        assert_eq!(p.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    }

    type Kid = Base<u32, u32, Never, &'static str>;

    fn kid() -> Kid {
        Base::new(0_u32, |count: &mut u32, n: u32| {
            *count += n;
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        })
    }

    fn supervisor(budget: u32) -> Supervising<Base<(), u64, Never, &'static str>, Kid> {
        let inner = Base::new((), |(): &mut (), _: u64| {
            Ok::<Step<Never, Exit>, &'static str>(Step::Continue)
        });
        Supervising::new(inner, vec![kid()], |_| kid(), budget)
    }

    #[tokio::test]
    async fn supervising_restarts_an_abnormal_child_within_budget() {
        let mut sup = supervisor(1);
        let _ = sup.step(Wire::ChildStopped { idx: 0, abnormal: true }).await;
        assert!(sup.children()[0].alive(), "the abnormal child is restarted");
        assert_eq!(sup.restarts_left(), 0, "the restart spent one budget unit");

        let _ = sup.step(Wire::ChildStopped { idx: 0, abnormal: true }).await;
        assert!(!sup.children()[0].alive(), "no budget ⇒ give up");
    }

    #[tokio::test]
    async fn supervising_never_restarts_a_normal_child_stop() {
        let mut sup = supervisor(5);
        let _ = sup.step(Wire::ChildStopped { idx: 0, abnormal: false }).await;
        assert!(!sup.children()[0].alive(), "a normal stop is final under every policy");
        assert_eq!(sup.restarts_left(), 5, "no budget spent on a normal stop");
    }
}
