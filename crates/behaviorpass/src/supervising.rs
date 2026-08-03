//! `Supervising` — a `Behavior` that decides child restarts. It reacts to
//! [`Envelope::ChildStopped`] and, within budget, EMITS a create-spec (an
//! `Offspring` the driver spawns) rather than rebuilding a child fold in place.
//! Actual spawning — initial and restart — is the future driver's job; this
//! crate makes only the restart DECISION and emits the create.

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Acted, Actions, Behavior, Envelope};

/// A `Behavior` that supervises children: the OUTER fold deciding restarts.
/// One-for-one, budget-bounded — model grade. A restart is a `create` the
/// driver interprets, not an in-place rebuild.
///
/// The liveness table is a LAZY bitset: the constructor state (every slot
/// alive) is `liveness: None` — zero heap — and a `Box<[u64]>` (bit *k* set ⇒
/// slot *k* alive) materializes only on the first death. Every op is O(1):
/// one bounds check plus a single word read/write.
pub struct Supervising<B: Behavior, C: Behavior<Ph = Never>> {
    inner: B,
    liveness: Option<Box<[u64]>>,
    n_children: u32,
    build: fn(usize) -> C,
    restarts_left: u32,
}

impl<B: Behavior, C: Behavior<Ph = Never>> Supervising<B, C> {
    /// Builds a supervisor with `n_children` live slots and a restart budget.
    /// The supervisor no longer instantiates children — it only tracks liveness
    /// and emits create-specs. All slots start alive, so nothing is allocated.
    /// A fleet beyond `u32::MAX` slots is a programming error (the old `Vec`
    /// table could not hold one either) and panics here.
    pub fn new(inner: B, n_children: usize, build: fn(usize) -> C, restarts_left: u32) -> Self {
        let n_children = u32::try_from(n_children)
            .expect("a Supervising fleet of more than u32::MAX children is a programming error");
        Self { inner, liveness: None, n_children, build, restarts_left }
    }

    /// Whether slot `idx` is still alive. Panics out of range, like indexing
    /// the old table did.
    #[must_use]
    pub fn is_alive(&self, idx: usize) -> bool {
        assert!(
            idx < self.n_children as usize,
            "child slot {idx} out of range ({})",
            self.n_children
        );
        match &self.liveness {
            None => true,
            Some(words) => (words[idx / 64] & (1 << (idx % 64))) != 0,
        }
    }

    /// The number of supervised slots.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.n_children as usize
    }

    /// Remaining restart budget (test observability).
    #[must_use]
    pub fn restarts_left(&self) -> u32 {
        self.restarts_left
    }

    /// The restart decision for slot `idx`: an abnormal stop within budget spends
    /// one unit, re-marks the slot live, and yields ONE create-spec; every other
    /// case (normal stop, exhausted budget, out-of-range) marks it dead and
    /// yields no create.
    fn on_child_stopped(&mut self, idx: usize, abnormal: bool) -> Vec<C> {
        if idx >= self.n_children as usize {
            return Vec::new();
        }
        if abnormal && self.restarts_left > 0 {
            self.restarts_left -= 1;
            // Re-mark live: a no-op while the table is unmaterialized (all
            // alive), a bit set once the bitset exists.
            if let Some(words) = &mut self.liveness {
                words[idx / 64] |= 1 << (idx % 64);
            }
            vec![(self.build)(idx)]
        } else {
            self.mark_dead(idx);
            Vec::new()
        }
    }

    /// Marks slot `idx` dead, materializing the all-alive table on first death.
    fn mark_dead(&mut self, idx: usize) {
        let words = self.liveness.get_or_insert_with(|| {
            vec![u64::MAX; (self.n_children.div_ceil(64)) as usize].into_boxed_slice()
        });
        words[idx / 64] &= !(1 << (idx % 64));
    }

    /// Remap a supervisor-inner reaction (which creates nothing —
    /// `B::Offspring = Never`) into the supervisor's create-menu `C`: its creates
    /// list is provably empty, so it re-emerges as `Vec::new()`; sends and
    /// `become_` pass through unchanged.
    fn forward(inner: Actions<B::Ph, B::Outbound, Never>) -> Actions<B::Ph, B::Outbound, C> {
        Actions { sends: inner.sends, creates: Vec::new(), become_: inner.become_ }
    }
}

impl<B, C> Behavior for Supervising<B, C>
where
    B: Behavior<Offspring = Never> + Send,
    B::Msg: Send,
    C: Behavior<Ph = Never> + Send,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = C;
    async fn step(
        &mut self,
        ev: Envelope<B::Msg>,
    ) -> Acted<B::Ph, B::Outbound, C, B::Error> {
        match ev {
            Envelope::ChildStopped { idx, abnormal } => {
                let creates = self.on_child_stopped(idx, abnormal);
                Ok(Actions { sends: Vec::new(), creates, become_: Step::Continue })
            }
            other => Ok(Self::forward(self.inner.step(other).await?)),
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::Supervising;
    use crate::behavior::{Actions, Behavior, Envelope};
    use crate::Base;
    use bombay::capability::Never;

    type Kid = Base<u32, u32, Never, &'static str>;

    fn kid() -> Kid {
        Base::new(0_u32, |count: &mut u32, n: u32| {
            *count += n;
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        })
    }

    fn supervisor(budget: u32) -> Supervising<Base<(), u64, Never, &'static str>, Kid> {
        let inner = Base::new((), |(): &mut (), _: u64| {
            Ok::<Actions<Never, Never, Never>, &'static str>(Actions::cont())
        });
        Supervising::new(inner, 1, |_| kid(), budget)
    }

    #[tokio::test]
    async fn supervising_restarts_an_abnormal_child_within_budget() {
        let mut sup = supervisor(1);
        let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.unwrap();
        assert_eq!(actions.creates.len(), 1, "the restart emits one create-spec for the driver");
        assert!(sup.is_alive(0), "the abnormal child's slot is marked live again");
        assert_eq!(sup.restarts_left(), 0, "the restart spent one budget unit");

        let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: true }).await.unwrap();
        assert_eq!(actions.creates.len(), 0, "no budget ⇒ no create emitted");
        assert!(!sup.is_alive(0), "no budget ⇒ give up");
    }

    #[tokio::test]
    async fn supervising_never_restarts_a_normal_child_stop() {
        let mut sup = supervisor(5);
        let actions = sup.step(Envelope::ChildStopped { idx: 0, abnormal: false }).await.unwrap();
        assert_eq!(actions.creates.len(), 0, "a normal stop emits no create");
        assert!(!sup.is_alive(0), "a normal stop is final under every policy");
        assert_eq!(sup.restarts_left(), 5, "no budget spent on a normal stop");
    }
}
