//! The five async capability layers (ADR-0030) — async twins of
//! `behaviorpass-reference`. Each is a `C<B>: Behavior where B: Behavior`;
//! the loop golfs their code-only LOC while the frozen oracle holds them
//! trace-equal to the sync reference.

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

#[cfg(test)]
mod tests {
    use super::Base;
    use crate::Exit;
    use crate::behavior::{Behavior, Wire};
    use bombay::capability::{Never, Step};

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
