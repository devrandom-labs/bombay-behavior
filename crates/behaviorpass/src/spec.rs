//! `Spec` — the intent builder: a spawnable behavior specification.
//!
//! One typestate builder over the capability stack (design: actorpass
//! docs, surface talks). The user tunes an actor with intent-named
//! transitions — they never name a layer; the builder assembles the
//! canonical stack behind it. Every stopped chain is complete (required
//! parameters ride in the transition; optional tuning has documented
//! defaults), and a `Spec<B>` IS a `Behavior` by delegation, so it is
//! spawnable (or drivable) at any point. actorpass adds the runtime
//! terminal (`system.spawn(spec) -> Handle`); this crate owns only the
//! composition.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{Acted, Address, Behavior, Envelope, Fleet};
use crate::stashing::{StashRoute, Stashing};
use crate::supervising::{RestartPolicy, Strategy, Supervising};
use crate::verdict::Never;
use crate::watching::{LinkReaction, Watching};
use crate::{Base, State};

/// The default restart strategy (OTP's one-for-one): restart only the
/// dead child.
const DEFAULT_STRATEGY: Strategy = Strategy::OneForOne;

/// The default per-child policy: restart only on an abnormal outcome.
const DEFAULT_POLICY: RestartPolicy = RestartPolicy::Transient;

/// The default restart budget — OTP's default intensity: at most one
/// restart inside any five-second window (meltdown protection ON by
/// default; tune with [`Spec::budget`]).
const DEFAULT_BUDGET: (u32, Duration) = (1, Duration::from_secs(5));

/// The identity fleet minter: slot = nonce = fleet index.
fn identity_nonce<N: From<u64>>(i: usize) -> N {
    N::from(u64::try_from(i).expect("a fleet index always fits u64"))
}

/// The intent builder: a composed, spawnable behavior specification.
/// `Spec<B>` delegates `Behavior` to the composed stack, so a spec is
/// drivable at every stage of the chain.
pub struct Spec<B>(B);

impl<S: State<O, N, E>, O, N, E> Spec<Base<S, O, N, E>> {
    /// The floor intent: a plain actor from its [`State`].
    #[must_use]
    pub fn new(state: S) -> Self {
        Self(Base::new(state))
    }
}

impl<B: Behavior> Spec<B> {
    /// Wrap an already-composed behavior (the raw-layers escape hatch
    /// meets the builder).
    #[must_use]
    pub fn from_behavior(b: B) -> Self {
        Self(b)
    }

    /// The composed behavior (the spawnable payload).
    #[must_use]
    pub fn build(self) -> B {
        self.0
    }

    /// The composed behavior, by reference.
    #[must_use]
    pub fn behavior(&self) -> &B {
        &self.0
    }

    /// Intent: react to a watched peer's death (materializes the link
    /// layer). `stop_on_abnormal_death` is the shipped default policy;
    /// any non-capturing closure of the same shape coerces.
    #[must_use]
    pub fn watch(self, on_link_died: LinkReaction<B>) -> Spec<Watching<B>> {
        Spec(Watching::new(self.0, on_link_died))
    }

    /// Intent: hold messages this actor isn't ready for (materializes
    /// the buffer layer); `route` decides per message — `Stash`,
    /// `Deliver`, or `Release` (deliver, then replay the held batch).
    /// The buffer erases the phase menu (`Ph = Never`).
    #[must_use]
    pub fn stash(self, route: fn(&B::Msg) -> StashRoute) -> Spec<Stashing<B>>
    where
        B: Behavior<Ph = Never>,
    {
        Spec(Stashing::new(self.0, route))
    }

    /// Intent: a child fleet `(n, build)` — built, and rebuilt on every
    /// restart, by `build` (the constructor IS the restart builder, so
    /// it takes the fleet index, not a captured value). The tuple is
    /// exactly what [`workers!`] yields, so mixed fleets inline:
    /// `.children(workers![(4, worker_a), (2, worker_b)])`. Slots are
    /// the identity nonces (slot = nonce = index); use
    /// [`Spec::children_with_nonces`] for an explicit minter. The inner
    /// behavior's offspring menu must be the child type (the layer's
    /// bookkeeping law — declare `Child` in its [`State`] impl).
    ///
    /// [`workers!`]: crate::workers
    #[must_use]
    pub fn children<C>(self, fleet: (usize, fn(usize) -> C)) -> Spec<Supervising<B, C>>
    where
        B: Behavior<Offspring = C>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
        <B::Addr as Address>::Nonce: From<u64>,
    {
        self.children_with_nonces(identity_nonce, fleet.0, fleet.1)
    }

    /// [`Spec::children`] with an explicit fleet nonce minter.
    #[must_use]
    pub fn children_with_nonces<C>(
        self,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        n: usize,
        build: fn(usize) -> C,
    ) -> Spec<Supervising<B, C>>
    where
        B: Behavior<Offspring = C>,
        C: Behavior<Ph = Never, Addr = B::Addr>,
    {
        Spec(Supervising::new(
            self.0,
            nonces,
            n,
            build,
            DEFAULT_STRATEGY,
            DEFAULT_POLICY,
            DEFAULT_BUDGET.0,
            DEFAULT_BUDGET.1,
        ))
    }
}

impl<B, C> Spec<Supervising<B, C>>
where
    B: Behavior<Offspring = C>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    /// Intent: the restart strategy when a child dies —
    /// `restart_one()` (default), `restart_all()`, `restart_rest()`.
    #[must_use]
    pub fn on_child_death(self, strategy: Strategy) -> Self {
        Spec(self.0.with_strategy(strategy))
    }

    /// Intent: when a child's stop is restart-eligible —
    /// `RestartPolicy::Transient` (default), `Permanent`, `Temporary`.
    #[must_use]
    pub fn policy(self, policy: RestartPolicy) -> Self {
        Spec(self.0.with_policy(policy))
    }

    /// Intent: the windowed restart budget — at most `max` restarts in
    /// any `window` span (default: OTP's intensity, one per five
    /// seconds).
    #[must_use]
    pub fn budget(self, max: u32, window: Duration) -> Self {
        Spec(self.0.with_budget(max, window))
    }
}

impl<B: Behavior + Send> Behavior for Spec<B>
where
    B::Addr: Send,
    <B::Addr as Address>::Nonce: Send,
    B::Msg: Send,
{
    type Addr = B::Addr;
    type Msg = B::Msg;
    type Ph = B::Ph;
    type Error = B::Error;
    type Outbound = B::Outbound;
    type Offspring = B::Offspring;

    async fn step(
        &mut self,
        ev: Envelope<B::Addr, B::Msg>,
    ) -> Acted<B::Addr, B::Ph, B::Outbound, B::Offspring, B::Error> {
        self.0.step(ev).await
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.0.next_deadline()
    }

    fn fleet(&self) -> Option<Fleet<B::Addr, B::Offspring>> {
        self.0.fleet()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::Spec;
    use crate::verdict::{Never, Step};
    use crate::{
        Acted, Actions, Behavior, Crash, Create, Envelope, Exit, MailAddr, RestartPolicy, State,
        restart_all, stop_on_abnormal_death,
    };

    struct Counter {
        n: u64,
    }

    impl State for Counter {
        type Addr = MailAddr;
        type Msg = u64;
        fn handle(&mut self, _from: MailAddr, m: u64) -> Acted<MailAddr, Never, Never, Never, Never> {
            self.n += m;
            Ok(Actions::cont())
        }
    }

    /// The supervisor's inner behavior: plain, but its offspring menu is
    /// the child type (the layer's bookkeeping law).
    struct Router;

    impl State<Never, crate::Base<Counter>, Never> for Router {
        type Addr = MailAddr;
        type Msg = u64;
        fn handle(
            &mut self,
            _from: MailAddr,
            _m: u64,
        ) -> Acted<MailAddr, Never, Never, crate::Base<Counter>, Never> {
            Ok(Actions::cont())
        }
    }

    fn counter(i: usize) -> crate::Base<Counter> {
        crate::Base::new(Counter { n: i as u64 })
    }

    struct Beta;

    impl State for Beta {
        type Addr = MailAddr;
        type Msg = u64;
        fn handle(&mut self, _from: MailAddr, _m: u64) -> Acted<MailAddr, Never, Never, Never, Never> {
            Ok(Actions::cont())
        }
    }

    fn beta(_i: usize) -> crate::Base<Beta> {
        crate::Base::new(Beta)
    }

    #[tokio::test]
    async fn spec_children_inlines_the_workers_macro() {
        let inner = crate::Base::from_fn((), |(): &mut (), _from: MailAddr, _: u64| {
            Ok::<Actions<MailAddr, Never, Never, _>, Never>(Actions::cont())
        });
        let mut spec = Spec::from_behavior(inner)
            .children(crate::workers![(1, crate::Base<Counter>, counter), (1, crate::Base<Beta>, beta)])
            .budget(2, Duration::MAX);

        let fleet = Behavior::fleet(&spec).expect("a supervisor declares its fleet");
        assert_eq!(fleet.n, 2, "the macro's mixed fleet is the supervisor's fleet");

        let actions = spec
            .step(Envelope::ChildStopped { nonce: 1, outcome: Err(Crash::Failed), at: Instant::now() })
            .await
            .expect("no crash");
        let [Create::Restart { nonce, .. }] = actions.creates.as_slice() else {
            panic!("an abnormal stop within budget emits exactly one restart");
        };
        assert_eq!(*nonce, 1, "the restart names the dead crew member's nonce");
    }

    #[tokio::test]
    async fn spec_floor_folds_and_delegates_behavior() {
        let mut spec = Spec::new(Counter { n: 0 });
        let actions = spec.step(Envelope::User { from: MailAddr(1), msg: 7 }).await.expect("no crash");
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(spec.behavior().state().n, 7, "a Spec IS the composed behavior");
    }

    #[tokio::test]
    async fn spec_watch_materializes_the_link_layer() {
        let mut spec = Spec::new(Counter { n: 0 }).watch(stop_on_abnormal_death);
        let actions = spec
            .step(Envelope::LinkDied { peer: MailAddr(9), outcome: Err(Crash::Failed) })
            .await
            .expect("no crash");
        assert!(
            matches!(actions.become_, Step::Stop(Exit::LinkDied(_))),
            "an abnormal peer death propagates through the intent"
        );
    }

    #[tokio::test]
    async fn spec_children_fleet_with_tuning_and_restart() {
        let mut spec = Spec::new(Router)
            .children((2, counter))
            .on_child_death(restart_all())
            .policy(RestartPolicy::Transient)
            .budget(5, Duration::from_secs(30));

        let fleet = Behavior::fleet(&spec).expect("a supervisor declares its fleet");
        assert_eq!(fleet.n, 2);

        let at = Instant::now();
        let actions = spec
            .step(Envelope::ChildStopped { nonce: 0, outcome: Err(Crash::Failed), at })
            .await
            .expect("no crash");
        let restarts = actions
            .creates
            .iter()
            .filter(|c| matches!(c, Create::Restart { .. }))
            .count();
        assert_eq!(restarts, 2, "restart_all restarts every live child");
    }
}
