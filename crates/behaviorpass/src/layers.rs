//! The five async capability layers (ADR-0030) — async twins of
//! `behaviorpass-reference`. Each is a `C<B>: Behavior where B: Behavior`;
//! the loop golfs their code-only LOC while the frozen oracle holds them
//! trace-equal to the sync reference.

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Behavior, Handler, Wire};

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
                match (self.on_deadline)(&mut self.inner)? {
                    Step::Continue => Ok(Step::Continue),
                    Step::Goto(never) => match never {},
                    Step::Stop(exit) => Ok(Step::Stop(exit)),
                }
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

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{Base, Deadlined};
    use crate::Exit;
    use crate::behavior::{Behavior, Wire};
    use bombay::capability::{Never, Step};
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
}
