//! `Supervising` — a `Behavior` that restarts child behaviors. It reacts to
//! [`Envelope::ChildStopped`] (restart within budget) and forwards the rest.

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Become, Behavior, Envelope};

/// One supervised child: an inner behavior and its liveness.
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

/// A `Behavior` that supervises children: the OUTER fold restarting inner
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
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Become<B::Ph>, B::Error> {
        match ev {
            Envelope::ChildStopped { idx, abnormal } => {
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
    use super::Supervising;
    use crate::behavior::{Behavior, Envelope};
    use crate::{Base, Exit};
    use bombay::capability::{Never, Step};

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
        let _ = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await;
        assert!(sup.children()[0].alive(), "the abnormal child is restarted");
        assert_eq!(sup.restarts_left(), 0, "the restart spent one budget unit");

        let _ = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await;
        assert!(!sup.children()[0].alive(), "no budget ⇒ give up");
    }

    #[tokio::test]
    async fn supervising_never_restarts_a_normal_child_stop() {
        let mut sup = supervisor(5);
        let _ = sup.step(Envelope::ChildStopped { idx: 0, abnormal: false }).await;
        assert!(!sup.children()[0].alive(), "a normal stop is final under every policy");
        assert_eq!(sup.restarts_left(), 5, "no budget spent on a normal stop");
    }
}
