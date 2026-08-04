//! `Supervising` — a `Behavior` that decides the FULL strategy space of child
//! restarts as PURE decisions (the driver executes): one-for-one /
//! one-for-all / rest-for-one × permanent / transient / temporary, under a
//! windowed restart budget. It reacts to [`Envelope::ChildStopped`] and,
//! within budget, EMITS tagged [`Create::Restart`]s (nonce + replacement
//! behavior) rather than rebuilding a child fold in place. Actual spawning —
//! initial and restart — is the future driver's job; this crate makes only
//! the restart DECISION. The keep-address semantics (2026-08-04 design): a
//! restart reuses the child's surviving mailbox — the driver swaps the
//! behavior slot, it never re-addresses, so handles that escaped in messages
//! stay valid.

use std::time::Duration;

use crate::verdict::{Never, Step};
use tokio::time::Instant;

use crate::behavior::{Acted, Actions, Address, Behavior, Create, Envelope, Fleet};
use crate::{Crash, Exit};

/// Which children a triggered restart event restarts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Restart only the dead child.
    OneForOne,
    /// Restart every live child when one triggers.
    OneForAll,
    /// Restart the dead child and every child born AFTER it (birth
    /// SEQUENCE order, not nonce order).
    RestForOne,
}

/// When a child's stop is restart-eligible (the dead child's OWN policy gates
/// strategy evaluation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Restart on any stop, normal or abnormal.
    Permanent,
    /// Restart only on abnormal outcome.
    Transient,
    /// Never restart; a stop only marks the slot dead.
    Temporary,
}

/// One child-table entry: liveness plus the birth-sequence number
/// (`rest-for-one` compares ORDER OF BIRTH, immune to arbitrary nonce
/// values).
struct SlotRec {
    alive: bool,
    seq: u64,
}

/// A `Behavior` that supervises children: the OUTER fold deciding restarts.
/// A restart is a `create` the driver interprets, not an in-place rebuild.
///
/// The child table is one uniform `Vec<(A::Nonce, SlotRec)>` — nonce-keyed
/// assoc scans (fleets are small; nonces are arbitrary creator-minted values,
/// so no dense indexing exists). Static slots are inserted at construction
/// (sequence `0..n`); dynamic births (the inner's `Create::Birth`) append at
/// the next sequence, so the table's physical order IS birth order.
pub struct Supervising<B: Behavior, C: Behavior<Ph = Never, Addr = B::Addr>> {
    inner: B,
    slots: Vec<(<B::Addr as Address>::Nonce, SlotRec)>,
    n_static: usize,
    nonces: fn(usize) -> <B::Addr as Address>::Nonce,
    next_seq: u64,
    build: fn(usize) -> C,
    strategy: Strategy,
    policy: RestartPolicy,
    max_restarts: u32,
    window: Duration,
    restarts: Vec<Instant>,
}

impl<B: Behavior<Offspring = C>, C: Behavior<Ph = Never, Addr = B::Addr>> Supervising<B, C> {
    /// Builds a supervisor: `nonces` mints the static fleet's birth nonces
    /// (the driver mints child ADDRESSES from the same indices, so driver
    /// table and behavior table agree by construction — slot = nonce);
    /// `build` constructs the replacement behavior for a restart (indexed by
    /// table position: the fleet index for a static child). The budget is
    /// windowed: at most `max_restarts` restarts inside any `window` span
    /// (`Duration::MAX` = the count-only case).
    ///
    /// # Panics
    /// Never in practice — the `u64` conversions of fleet indices cannot fail
    /// on any supported target.
    #[allow(
        clippy::too_many_arguments,
        reason = "the constructor IS the supervision spec (fleet, strategy, policy, budget) — grouping it into a builder would hide the card-1 vocabulary"
    )]
    pub fn new(
        inner: B,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        n_children: usize,
        build: fn(usize) -> C,
        strategy: Strategy,
        policy: RestartPolicy,
        max_restarts: u32,
        window: Duration,
    ) -> Self {
        let slots = (0..n_children)
            .map(|i| {
                let seq = u64::try_from(i).expect("a fleet index always fits u64");
                (nonces(i), SlotRec { alive: true, seq })
            })
            .collect();
        let next_seq = u64::try_from(n_children).expect("a fleet size always fits u64");
        Self {
            inner,
            slots,
            n_static: n_children,
            nonces,
            next_seq,
            build,
            strategy,
            policy,
            max_restarts,
            window,
            restarts: Vec::new(),
        }
    }

    /// The table position of `nonce`, if known (static or dynamically born —
    /// alive or dead, a known nonce is taken forever).
    fn position(&self, nonce: <B::Addr as Address>::Nonce) -> Option<usize> {
        self.slots.iter().position(|(n, _)| *n == nonce)
    }

    /// Whether the child born at `nonce` is still alive.
    ///
    /// # Panics
    /// On an unknown nonce (driver/behavior desync), like indexing out of
    /// range did.
    #[must_use]
    pub fn is_alive(&self, nonce: <B::Addr as Address>::Nonce) -> bool {
        let Some(pos) = self.position(nonce) else {
            panic!("is_alive names an unknown nonce — driver/behavior desync");
        };
        self.slots[pos].1.alive
    }

    /// The number of supervised slots (static fleet plus dynamic births).
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.slots.len()
    }

    /// Restart timestamps inside the current window, as pruned at the last
    /// evaluation (test observability).
    #[must_use]
    pub fn restarts_in_window(&self) -> usize {
        self.restarts.len()
    }

    /// The outcome classification (the bool's replacement): the NORMAL subset
    /// is `{Exit::Normal, Exit::Collected}`; ABNORMAL ≡ `Err(Crash)` or
    /// `Ok(_)` outside that subset. Pure policy, matched in the layer.
    fn is_normal(outcome: &Result<Exit<B::Addr>, Crash>) -> bool {
        matches!(outcome, Ok(Exit::Normal | Exit::Collected))
    }

    /// The restart decision for the child born at `nonce`: the dead child's
    /// own policy gates evaluation (`Temporary` never; `Transient` only on an
    /// abnormal outcome; `Permanent` always), the strategy picks the
    /// candidate set, and the windowed budget admits the whole set or
    /// nothing. Admitted: one `Create::Restart` per candidate — the dead
    /// child FIRST, the rest in birth-sequence order — one `at` pushed per
    /// restart, and the dead child re-marked live. Denied: the dead child is
    /// marked dead and nothing is emitted. An unknown nonce is a
    /// driver/behavior desync — a programmer bug — and panics.
    fn on_child_stopped(
        &mut self,
        nonce: <B::Addr as Address>::Nonce,
        outcome: &Result<Exit<B::Addr>, Crash>,
        at: Instant,
    ) -> Vec<Create<B::Addr, C>> {
        let Some(dead) = self.position(nonce) else {
            panic!("ChildStopped names an unknown nonce — driver/behavior desync");
        };
        let evaluate = match self.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => !Self::is_normal(outcome),
            RestartPolicy::Temporary => false,
        };
        if !evaluate {
            self.slots[dead].1.alive = false;
            return Vec::new();
        }
        // Window eviction: entries older than the window stop counting. The
        // count-only case (`Duration::MAX`) can never evict — `age <= MAX`
        // holds for every `Duration` — so the scan is skipped wholesale (and
        // the in-window count is always ≤ `max_restarts`, keeping the scan
        // bounded whenever it does run).
        let window = self.window;
        if window != Duration::MAX {
            self.restarts.retain(|&ts| at.checked_duration_since(ts).is_none_or(|age| age <= window));
        }
        // The candidate set by strategy — the dead child is still marked live
        // here, so every strategy's set contains it.
        let dead_seq = self.slots[dead].1.seq;
        let candidates: Vec<usize> = match self.strategy {
            Strategy::OneForOne => vec![dead],
            Strategy::OneForAll => {
                (0..self.slots.len()).filter(|&i| self.slots[i].1.alive).collect()
            }
            Strategy::RestForOne => {
                (0..self.slots.len())
                    .filter(|&i| self.slots[i].1.alive && self.slots[i].1.seq >= dead_seq)
                    .collect()
            }
        };
        // All-or-nothing per event: the whole set fits the windowed budget or
        // nothing restarts.
        if self.restarts.len() + candidates.len() > self.max_restarts as usize {
            self.slots[dead].1.alive = false;
            return Vec::new();
        }
        let mut creates = Vec::with_capacity(candidates.len());
        // The dead child first, then the rest in table order (= birth-sequence
        // order — slots are only ever appended at the next sequence).
        creates.push(self.restart_for(dead));
        for &i in candidates.iter().filter(|&&i| i != dead) {
            creates.push(self.restart_for(i));
        }
        self.restarts.resize(self.restarts.len() + candidates.len(), at);
        creates
    }

    /// The restart emission for the slot at position `i`: a fresh behavior
    /// from `build` (indexed by table position — the fleet index for a static
    /// child), re-marked live, naming the slot's nonce.
    fn restart_for(&mut self, i: usize) -> Create<B::Addr, C> {
        self.slots[i].1.alive = true;
        Create::Restart { nonce: self.slots[i].0, child: (self.build)(i) }
    }

    /// Remap the supervisor-inner reaction onto the supervisor's create-menu:
    /// sends and `become_` pass through unchanged; every inner `Create::Birth`
    /// is freshness-validated against the liveness table (every known nonce —
    /// static or dynamic, alive or dead — is taken; a collision is a
    /// programmer bug and panics), recorded at the next birth sequence, and
    /// emitted as-is. An inner-emitted `Create::Restart` is a programmer bug
    /// (restart decisions belong to this layer) and panics.
    fn forward(
        &mut self,
        inner: Actions<B::Addr, B::Ph, B::Outbound, C>,
    ) -> Actions<B::Addr, B::Ph, B::Outbound, C> {
        for create in &inner.creates {
            match create {
                Create::Birth { nonce, .. } => {
                    assert!(
                        self.position(*nonce).is_none(),
                        "a birth nonce collides with a known slot — creator-minted nonces must be fresh"
                    );
                    let seq = self.next_seq;
                    self.next_seq =
                        self.next_seq.checked_add(1).expect("birth sequence space exhausted");
                    self.slots.push((*nonce, SlotRec { alive: true, seq }));
                }
                Create::Restart { .. } => {
                    panic!("an inner behavior never emits Create::Restart — restart decisions belong to the Supervising layer");
                }
            }
        }
        inner
    }
}

impl<B, C> Behavior for Supervising<B, C>
where
    B: Behavior<Offspring = C> + Send,
    B::Addr: Send,
    <B::Addr as Address>::Nonce: Send,
    B::Msg: Send,
    C: Behavior<Ph = Never, Addr = B::Addr> + Send,
{
    type Addr = B::Addr;
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = C;
    async fn step(
        &mut self,
        ev: Envelope<B::Addr, B::Msg>,
    ) -> Acted<B::Addr, B::Ph, B::Outbound, C, B::Error> {
        match ev {
            Envelope::ChildStopped { nonce, outcome, at } => {
                let creates = self.on_child_stopped(nonce, &outcome, at);
                Ok(Actions { sends: Vec::new(), creates, become_: Step::Continue })
            }
            other => {
                let acted = self.inner.step(other).await?;
                Ok(self.forward(acted))
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }

    fn fleet(&self) -> Option<Fleet<Self::Addr, Self::Offspring>> {
        Some(Fleet { n: self.n_static, nonces: self.nonces, build: self.build })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{RestartPolicy, Strategy, Supervising};
    use crate::behavior::{Actions, Behavior, Create, Envelope};
    use crate::{Base, Crash, Exit, FnState, MailAddr};
    use crate::verdict::Never;
    use tokio::time::Instant;

    type Kid = Base<FnState<u32, MailAddr, u32, Never, Never, &'static str>, Never, Never, &'static str>;
    // The inner's Offspring is the child menu C (the relaxed bound): it
    // creates nothing at runtime, but the TYPE agrees with the fleet.
    type Inner = Base<FnState<(), MailAddr, u64, Never, Kid, &'static str>, Never, Kid, &'static str>;

    fn kid() -> Kid {
        Base::from_fn(0_u32, |count: &mut u32, _from: MailAddr, n: u32| {
            *count += n;
            Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
        })
    }

    fn inner() -> Inner {
        Base::from_fn((), |(): &mut (), _from: MailAddr, _: u64| {
            Ok::<Actions<MailAddr, Never, Never, Kid>, &'static str>(Actions::cont())
        })
    }

    fn supervisor(budget: u32) -> Supervising<Inner, Kid> {
        Supervising::new(
            inner(),
            |i| i as u64,
            1,
            |_| kid(),
            Strategy::OneForOne,
            RestartPolicy::Transient,
            budget,
            Duration::MAX,
        )
    }

    fn stopped(nonce: u64, outcome: Result<Exit<MailAddr>, Crash>) -> Envelope<MailAddr, u64> {
        Envelope::ChildStopped { nonce, outcome, at: Instant::now() }
    }

    #[tokio::test]
    async fn supervising_restarts_an_abnormal_child_within_budget() {
        let mut sup = supervisor(1);
        let actions = sup.step(stopped(0, Err(Crash::Failed))).await.unwrap();
        let [Create::Restart { nonce, .. }] = actions.creates.as_slice() else {
            panic!("the restart emits exactly one tagged restart for the driver");
        };
        assert_eq!(*nonce, 0, "the restart names the dead slot's nonce");
        assert!(sup.is_alive(0), "the abnormal child's slot is marked live again");
        assert_eq!(sup.restarts_in_window(), 1, "the restart spent one budget unit");

        let actions = sup.step(stopped(0, Err(Crash::Panicked))).await.unwrap();
        assert_eq!(actions.creates.len(), 0, "no budget ⇒ no create emitted");
        assert!(!sup.is_alive(0), "no budget ⇒ give up");
    }

    #[tokio::test]
    async fn supervising_never_restarts_a_normal_child_stop() {
        let mut sup = supervisor(5);
        let actions = sup.step(stopped(0, Ok(Exit::Normal))).await.unwrap();
        assert_eq!(actions.creates.len(), 0, "a normal stop emits no create");
        assert!(!sup.is_alive(0), "a normal stop is final under transient");
        assert_eq!(sup.restarts_in_window(), 0, "no budget spent on a normal stop");
    }

    #[tokio::test]
    async fn fleet_reports_the_static_fleet() {
        let sup = supervisor(5);
        let Some(fleet) = sup.fleet() else {
            panic!("Supervising declares its static fleet");
        };
        assert_eq!(fleet.n, 1);
        assert_eq!((fleet.nonces)(0), 0, "the minter surfaces for driver-side address derivation");
        let _ = (fleet.build)(0);
    }
}
