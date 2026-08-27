//! Standalone fixed-fleet ownership over the shared supervision fold.

use super::adapter::{ChildTopology, RestartConfiguration, SupervisorSends};
use super::domain::{FixedFleetOwnership, OwnershipFold};
use super::protocol::SupervisionEvent;
use super::{SuperviseError, SupervisionFailure};
use crate::protocol::ReplacementRequested;
use behavior::{
    Actions, Address, Behavior, BehaviorLayer, Births, ChildInputIngress, MessageProtocol, Never,
    Step, User,
};

type Addr<C> = crate::BehaviorAddr<C>;
type Stable<C, L> = <L as BehaviorLayer<C>>::Output;
type SupervisorActions<C, L> =
    Actions<Addr<C>, Never, SupervisorSends<Addr<C>, C, Stable<C, L>>, Births<Stable<C, L>>>;

fn retain_failure<A: Address>(_: &SupervisionFailure<A>) -> crate::Become {
    Step::Continue
}

/// Standalone owner of a fixed, homogeneous stable-child topology.
///
/// The configured worker factory returns the already-composed domain behavior.
/// `layer` supplies the outer stable-incarnation law and is reused for every
/// initial stable child. Replacement inputs remain owned by those stable
/// children; this actor only selects restart eligibility, budget, topology,
/// and terminal fleet drain through the shared fixed-fleet ownership fold.
///
/// The public message is uninhabited. Domain capability belongs to the stable
/// children, not to this ownership actor. This is a derived Bombay topology
/// construction rather than an actor-model primitive.
pub struct Supervisor<C, L>
where
    C: Behavior<Ph = Never>,
    Addr<C>: Address,
    <Addr<C> as Address>::Nonce: From<u64>,
    L: BehaviorLayer<C>,
    Stable<C, L>: Behavior<Ph = Never, Protocol = C::Protocol>,
    <Stable<C, L> as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    ownership: FixedFleetOwnership<Addr<C>, C, Stable<C, L>>,
    layer: L,
    on_failure: fn(&SupervisionFailure<Addr<C>>) -> crate::Become,
}

impl<C, L> Supervisor<C, L>
where
    C: Behavior<Ph = Never>,
    Addr<C>: Address,
    <Addr<C> as Address>::Nonce: Copy + Eq + From<u64>,
    L: BehaviorLayer<C>,
    Stable<C, L>: Behavior<Ph = Never, Protocol = C::Protocol>,
    <Stable<C, L> as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    /// Construct one fixed topology from composed workers and one reusable
    /// stable-incarnation layer.
    ///
    /// # Errors
    /// Returns the first duplicate or exhausted creator-local slot rejection.
    pub fn new(
        topology: ChildTopology<<Addr<C> as Address>::Nonce, C>,
        restart: RestartConfiguration,
        layer: L,
    ) -> Result<Self, super::FleetError<<Addr<C> as Address>::Nonce>> {
        Ok(Self {
            ownership: FixedFleetOwnership::new(topology, restart)?,
            layer,
            on_failure: retain_failure,
        })
    }

    /// Select the pure reaction to a typed ownership failure after the same
    /// action's reports and lifecycle effects have been retained.
    #[must_use]
    pub fn with_failure_reaction(
        mut self,
        reaction: fn(&SupervisionFailure<Addr<C>>) -> crate::Become,
    ) -> Self {
        self.on_failure = reaction;
        self
    }

    #[must_use]
    pub fn child_count(&self) -> usize {
        self.ownership.child_count()
    }

    #[must_use]
    pub fn restarts_in_window(&self) -> usize {
        self.ownership.restarts_in_window()
    }

    #[must_use]
    pub fn pending_restarts(&self) -> usize {
        self.ownership.pending_restarts()
    }

    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.ownership.is_shutting_down()
    }

    /// Report whether the interpreter has established the stable proxy.
    ///
    /// # Errors
    /// Returns the unknown creator-local nonce unchanged.
    pub fn is_established(
        &self,
        nonce: <Addr<C> as Address>::Nonce,
    ) -> Result<bool, super::FleetError<<Addr<C> as Address>::Nonce>> {
        self.ownership.is_established(nonce)
    }

    /// Report whether the configured slot remains eligible for replacement.
    pub fn is_restartable(
        &self,
        nonce: <Addr<C> as Address>::Nonce,
    ) -> Result<bool, super::FleetError<<Addr<C> as Address>::Nonce>> {
        self.ownership.is_restartable(nonce)
    }

    fn finish(&self, mut fold: OwnershipFold<Addr<C>, C, Stable<C, L>>) -> SupervisorActions<C, L> {
        if fold
            .failure
            .as_ref()
            .is_some_and(|failure| matches!((self.on_failure)(failure), Step::Stop(_)))
        {
            fold.actions.become_ = Step::Stop(crate::Stopped);
        }
        fold.actions
    }
}

impl<C, L> crate::BehaviorBase for Supervisor<C, L>
where
    C: Behavior<Ph = Never>,
    Addr<C>: Address,
    <Addr<C> as Address>::Nonce: Copy + Eq + From<u64>,
    L: BehaviorLayer<C>,
    Stable<C, L>: Behavior<Ph = Never, Protocol = C::Protocol>,
    <Stable<C, L> as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<C, L> Behavior for Supervisor<C, L>
where
    C: Behavior<Ph = Never>,
    Addr<C>: Address,
    <Addr<C> as Address>::Nonce: Copy + Eq + From<u64>,
    L: BehaviorLayer<C>,
    Stable<C, L>: Behavior<Ph = Never, Protocol = C::Protocol>,
    <Stable<C, L> as Behavior>::Event: ChildInputIngress<C, ReplacementRequested<C>>,
{
    type Protocol = MessageProtocol<Addr<C>, Never>;
    type Event = SupervisionEvent<User<Addr<C>, Never>>;
    type Sends = SupervisorSends<Addr<C>, C, Stable<C, L>>;
    type Ph = Never;
    type Error = SuperviseError<Never, Addr<C>>;
    type Birth = Births<Stable<C, L>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.ownership
            .initialize(&self.layer)
            .map_err(super::adapter::map_ownership_error)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        let fold = match event {
            SupervisionEvent::Behavior(user) => match user.message {},
            SupervisionEvent::WorkerStopped(event) => self.ownership.worker_stopped(event),
            SupervisionEvent::ChildStopped(event) => self.ownership.child_stopped(event),
            SupervisionEvent::CreationResolved(event) => self.ownership.creation_resolved(event),
            SupervisionEvent::WorkerCreationResolved(event) => {
                self.ownership.worker_creation_resolved(event)
            }
            SupervisionEvent::TimerElapsed(event) => self.ownership.timer_elapsed(event),
            SupervisionEvent::ShutdownRequested(_) => Ok(self.ownership.shutdown()),
            SupervisionEvent::ChildShutdownRejected(event) => {
                self.ownership.child_shutdown_rejected(event)
            }
        }
        .map_err(super::adapter::map_ownership_error)?;
        Ok(self.finish(fold))
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{
        Activate as _, Crash, Proxy, ReceiveTimeout, RestartPolicy, Strategy, TimerId,
        WorkerStopped,
    };
    use behavior::{Actions, BehaviorActed, MailAddr, NoBirths};

    struct Worker;

    impl behavior::Protocol for Worker {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Worker {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn on_idle(_: &mut Worker) -> Actions<MailAddr, Never, Vec<Never>, NoBirths> {
        Actions::cont()
    }

    fn restart() -> RestartConfiguration {
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            2,
            Duration::from_secs(30),
            crate::RestartTiming::Immediate,
        )
    }

    #[test]
    fn inferred_topology_composes_worker_layer_then_stable_layer_without_output_aliases() {
        let topology = ChildTopology::new([7], |_| {
            Some(Worker.layer(|worker| {
                ReceiveTimeout::new(worker, TimerId(4), Duration::from_secs(5), on_idle)
            }))
        });
        let initialized = Supervisor::new(topology, restart(), Proxy::new)
            .unwrap()
            .initialize()
            .unwrap();

        assert_eq!(initialized.actions.creates.len(), 1);
        assert_eq!(initialized.actions.creates[0].nonce, 7);
        assert_eq!(initialized.actions.sends.child_observations.len(), 1);
        assert_eq!(initialized.actions.sends.creation_observations.len(), 1);

        let mut supervisor = initialized.behavior;
        let proxy = initialized
            .actions
            .creates
            .into_iter()
            .next()
            .unwrap()
            .child;
        let proxy_initialized = proxy.initialize().unwrap();
        assert_eq!(proxy_initialized.actions.creates.len(), 1);
        let timed_worker = proxy_initialized
            .actions
            .creates
            .into_iter()
            .next()
            .unwrap()
            .child;
        let timed_initialized = timed_worker.initialize().unwrap();
        assert_eq!(timed_initialized.actions.sends.owned.len(), 1);

        let replacement = supervisor
            .on_path(WorkerStopped::new(7, 0, Err(Crash::Failed), Instant::now()))
            .unwrap();
        assert_eq!(replacement.sends.replacement_inputs.len(), 1);
        assert!(replacement.creates.is_empty());
    }
}
