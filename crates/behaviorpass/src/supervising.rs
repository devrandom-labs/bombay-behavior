//! `Supervising` — a `Behavior` that decides child restarts. It reacts to
//! [`Envelope::ChildStopped`] and, within budget, EMITS a tagged
//! [`Create::Restart`] (slot + replacement behavior) rather than
//! rebuilding a child fold in place. Actual spawning — initial and restart —
//! is the future driver's job; this crate makes only the restart DECISION.
//! The keep-address semantics (2026-08-04 design): a restart reuses the
//! child's surviving mailbox — the driver swaps the behavior slot, it never
//! re-addresses, so handles that escaped in messages stay valid.

use std::alloc::Layout;
use std::ptr::NonNull;

use crate::verdict::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Acted, Actions, Behavior, Create, Envelope};

/// A `Behavior` that supervises children: the OUTER fold deciding restarts.
/// One-for-one, budget-bounded — model grade. A restart is a `create` the
/// driver interprets, not an in-place rebuild.
///
/// The liveness table is a LAZY bitset: the constructor state (every slot
/// alive) is `liveness: None` — zero heap — and a `Layout::array::<u64>`
/// buffer (bit *k* set ⇒ slot *k* alive) materializes only on the first
/// death. The buffer is a THIN pointer: its length, `div_ceil(n_children,
/// 64)` words, is derivable from `n_children`, so the fat pointer's length
/// word is elided. Every op is O(1): one bounds check plus a single word
/// read/write.
pub struct Supervising<B: Behavior, C: Behavior<Ph = Never>> {
    inner: B,
    liveness: Option<NonNull<u64>>,
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

    /// The number of `u64` words the liveness table occupies.
    fn word_count(&self) -> usize {
        (self.n_children as usize).div_ceil(64)
    }

    /// The materialized table as a slice. Call only when `liveness` is `Some`.
    fn table(&self) -> &[u64] {
        // SAFETY: the pointer was stored by `mark_dead` from a live
        // `Layout::array::<u64>(word_count())` allocation of all-ones words;
        // it stays 8-aligned and valid for that many words until `Drop` frees
        // it, which happens after every reference to it is dead.
        let ptr = self.liveness.expect("table() before materialization").as_ptr();
        unsafe { std::slice::from_raw_parts(ptr, self.word_count()) }
    }

    /// The materialized table as a mutable slice. Call only when `liveness`
    /// is `Some`.
    fn table_mut(&mut self) -> &mut [u64] {
        // SAFETY: as `table`, and the exclusive `&mut self` borrow guarantees
        // no other reference to the buffer is live.
        let ptr = self.liveness.expect("table_mut() before materialization").as_ptr();
        unsafe { std::slice::from_raw_parts_mut(ptr, self.word_count()) }
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
        if self.liveness.is_none() {
            return true;
        }
        (self.table()[idx / 64] & (1 << (idx % 64))) != 0
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
    /// one unit, re-marks the slot live, and yields ONE restart; every other
    /// case (normal stop, exhausted budget, out-of-range) marks it dead and
    /// yields no create.
    fn on_child_stopped(&mut self, idx: usize, abnormal: bool) -> Vec<Create<C>> {
        if idx >= self.n_children as usize {
            return Vec::new();
        }
        if abnormal && self.restarts_left > 0 {
            self.restarts_left -= 1;
            // Re-mark live: a no-op while the table is unmaterialized (all
            // alive), a bit set once the bitset exists.
            if self.liveness.is_some() {
                self.table_mut()[idx / 64] |= 1 << (idx % 64);
            }
            vec![Create::Restart { slot: idx, child: (self.build)(idx) }]
        } else {
            self.mark_dead(idx);
            Vec::new()
        }
    }

    /// Marks slot `idx` dead, materializing the all-alive table on first death.
    fn mark_dead(&mut self, idx: usize) {
        let words = self.word_count();
        let ptr = self.liveness.get_or_insert_with(|| {
            let raw = Box::into_raw(vec![u64::MAX; words].into_boxed_slice()); // *mut [u64]
            // SAFETY: `Box::into_raw` returns a non-null pointer to a live
            // `Layout::array::<u64>(words)` allocation; `Drop` frees it with
            // the same layout.
            NonNull::new(raw.cast::<u64>()).expect("Box::into_raw never returns null")
        });
        // SAFETY: `ptr` is the freshly materialized (or existing) table,
        // valid for `words` 8-aligned u64s, exclusively borrowed here.
        let table = unsafe { std::slice::from_raw_parts_mut(ptr.as_ptr(), words) };
        table[idx / 64] &= !(1 << (idx % 64));
    }

    /// Remap a supervisor-inner reaction (which creates nothing —
    /// `B::Offspring = Never`) into the supervisor's create-menu `C`: its creates
    /// list is provably empty, so it re-emerges as `Vec::new()`; sends and
    /// `become_` pass through unchanged.
    fn forward(inner: Actions<B::Ph, B::Outbound, Never>) -> Actions<B::Ph, B::Outbound, C> {
        Actions { sends: inner.sends, creates: Vec::new(), become_: inner.become_ }
    }
}

impl<B: Behavior, C: Behavior<Ph = Never>> Drop for Supervising<B, C> {
    fn drop(&mut self) {
        if let Some(ptr) = self.liveness {
            let words = self.word_count();
            if words > 0 {
                // SAFETY: `ptr` came from `Box::into_raw` on a
                // `vec![u64::MAX; words]` boxed slice — an allocation of
                // exactly `Layout::array::<u64>(words)` — and is freed here
                // exactly once, after every reference to it is dead.
                let layout = Layout::array::<u64>(words)
                    .expect("words ≤ u32::MAX/64 ⇒ the layout cannot overflow");
                unsafe { std::alloc::dealloc(ptr.as_ptr().cast(), layout) };
            }
        }
    }
}

// SAFETY: `liveness` is a uniquely-owned heap buffer (the same ownership a
// `Box<[u64]>` would carry) — moving the supervisor to another thread moves
// the ownership with it, and `Drop` frees it on whichever thread drops it.
// The remaining fields — `inner: B`, a `fn` pointer, two `u32`s — are `Send`
// under the bounds below (`fn` pointers are `Send` unconditionally).
unsafe impl<B: Behavior + Send, C: Behavior<Ph = Never> + Send> Send for Supervising<B, C> {}

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
    use crate::behavior::{Actions, Behavior, Create, Envelope};
    use crate::Base;
    use crate::verdict::Never;

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
        let [Create::Restart { slot, .. }] = actions.creates.as_slice() else {
            panic!("the restart emits exactly one tagged restart for the driver");
        };
        assert_eq!(*slot, 0, "the restart names the dead slot");
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
