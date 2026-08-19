//! Standalone fixed-fleet supervision over the shared ownership fold.

use super::SupervisionEvent;
use super::adapter::{ChildTopology, ProxyWithParent, RestartConfiguration, SupervisorSends};
use super::domain::{FixedFleetOwnership, FleetError, OwnershipError, OwnershipFold};
use behavior::{Address, Behavior, Births, Never, Step, User};

/// Nominal protocol of a standalone fixed supervisor.
pub struct SupervisorProtocol<A: Address>(core::marker::PhantomData<fn() -> A>);

impl<A: Address> behavior::Protocol for SupervisorProtocol<A> {
    type Addr = A;
    type Msg = Never;
}

/// Complete lifecycle input sum of a standalone fixed supervisor.
pub type SupervisorEvent<A> = SupervisionEvent<User<A, Never>>;

/// Reaction after the owned topology can no longer be preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyFailurePolicy {
    /// Publish the typed failure, retire the affected slot, and continue.
    Retire,
    /// Publish the typed failure and terminate after the same action's sends.
    Stop,
}

/// Controlled rejection from standalone fixed-fleet ownership.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SupervisorError<N> {
    #[error(transparent)]
    Fleet(#[from] FleetError<N>),
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected {
        nonce: N,
        reason: crate::ChildShutdownRejection,
    },
}

impl<N> From<OwnershipError<N>> for SupervisorError<N> {
    fn from(error: OwnershipError<N>) -> Self {
        match error {
            OwnershipError::Fleet(error) => Self::Fleet(error),
            OwnershipError::FactoryIndex { index } => Self::FactoryIndex { index },
            OwnershipError::ChildShutdownRejected { nonce, reason } => {
                Self::ChildShutdownRejected { nonce, reason }
            }
        }
    }
}

/// Standalone owner of a fixed stable-proxy fleet.
///
/// Its public message is uninhabited; lifecycle facts enter through the
/// concrete [`SupervisorEvent`] sum. Application messages therefore
/// cannot be smuggled into this ownership-only template:
///
/// ```compile_fail
/// use behavior_actors::{Activate as _, ChildTopology, Supervisor, MailAddr,
///     Never, RestartConfiguration, RestartPolicy, Strategy};
/// use std::time::Duration;
///
/// # struct Child;
/// # impl behavior_actors::Protocol for Child { type Addr = MailAddr; type Msg = (); }
/// # impl behavior_actors::Behavior for Child {
/// #   type Protocol = Self; type Event = behavior_actors::User<MailAddr, ()>;
/// #   type Sends = Vec<Never>; type Ph = Never; type Error = Never;
/// #   type Birth = behavior_actors::NoBirths;
/// #   fn transition(&mut self, _: behavior_actors::ActiveTurn, _: Self::Event)
/// #     -> behavior_actors::BehaviorActed<Self> { Ok(behavior_actors::Actions::cont()) }
/// # }
/// # fn child(_: usize) -> Option<Child> { Some(Child) }
/// let mut active = Supervisor::<MailAddr, Child>::new(
///     ChildTopology::new([1], child),
///     RestartConfiguration::new(Strategy::OneForOne, RestartPolicy::Permanent,
///         1, Duration::from_secs(1)),
/// ).unwrap().initialize().unwrap().behavior;
/// active.receive(MailAddr(9), ());
/// ```
pub struct SupervisorWithParent<A, C, ParentPath = behavior::Here>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    ownership: FixedFleetOwnership<A, C, ParentPath>,
    failure: TopologyFailurePolicy,
}

/// A standalone fixed supervisor with direct proxy-report ingress.
pub type Supervisor<A, C> = SupervisorWithParent<A, C, behavior::Here>;

impl<A, C> Supervisor<A, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Construct a standalone fixed supervisor without an inner behavior.
    ///
    /// # Errors
    /// Returns the first typed topology rejection.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
    ) -> Result<Self, FleetError<A::Nonce>> {
        Self::with_parent(topology, restart, crate::ProxyParentIngress::new())
    }
}

impl<A, C, ParentPath> SupervisorWithParent<A, C, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Construct with an explicit structural proxy-report path.
    ///
    /// # Errors
    /// Returns the first typed topology rejection.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
        parent: crate::ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, FleetError<A::Nonce>> {
        Ok(Self {
            ownership: FixedFleetOwnership::new(topology, restart, parent)?,
            failure: TopologyFailurePolicy::Retire,
        })
    }

    /// Select the terminal reaction to an exhausted topology policy.
    #[must_use]
    pub const fn with_failure_policy(mut self, failure: TopologyFailurePolicy) -> Self {
        self.failure = failure;
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
    pub fn is_shutting_down(&self) -> bool {
        self.ownership.is_shutting_down()
    }
    pub fn is_alive(&self, nonce: A::Nonce) -> Result<bool, FleetError<A::Nonce>> {
        self.ownership.is_alive(nonce)
    }

    fn finish(&self, mut fold: OwnershipFold<A, C, ParentPath>) -> crate::BehaviorActed<Self> {
        if fold.failure.is_some() && self.failure == TopologyFailurePolicy::Stop {
            fold.actions.become_ = Step::Stop(crate::Stopped);
        }
        Ok(fold.actions)
    }
}

impl<A, C, ParentPath> crate::BehaviorBase for SupervisorWithParent<A, C, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    type Base = Self;
    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, C, ParentPath> Behavior for SupervisorWithParent<A, C, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    SupervisorSends<A, C, ParentPath>: behavior::SendsFor<SupervisorEvent<A>>,
{
    type Protocol = SupervisorProtocol<A>;
    type Event = SupervisorEvent<A>;
    type Sends = SupervisorSends<A, C, ParentPath>;
    type Ph = Never;
    type Error = SupervisorError<A::Nonce>;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.ownership.initialize().map_err(Into::into)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(user) => match user.message {},
            SupervisionEvent::WorkerStopped(event) => {
                let fold = self
                    .ownership
                    .worker_stopped(event)
                    .map_err(SupervisorError::from)?;
                self.finish(fold)
            }
            SupervisionEvent::ChildStopped(event) => {
                let fold = self
                    .ownership
                    .child_stopped(event)
                    .map_err(SupervisorError::from)?;
                self.finish(fold)
            }
            SupervisionEvent::CreationResolved(event) => {
                let fold = self.ownership.creation_resolved(event);
                self.finish(fold)
            }
            SupervisionEvent::WorkerCreationResolved(event) => {
                let fold = self.ownership.worker_creation_resolved(event);
                self.finish(fold)
            }
            SupervisionEvent::ShutdownRequested(_) => {
                let fold = self.ownership.shutdown();
                self.finish(fold)
            }
            SupervisionEvent::ChildShutdownRejected(event) => {
                let fold = self
                    .ownership
                    .child_shutdown_rejected(event)
                    .map_err(SupervisorError::from)?;
                self.finish(fold)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{
        Activate as _, Crash, CreationResolved, RestartPolicy, ShutdownRequested, Strategy,
        WorkerStopped,
    };
    use behavior::{Actions, MailAddr, NoBirths};

    struct Child;

    impl behavior::Protocol for Child {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }

    fn restart() -> RestartConfiguration {
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            3,
            Duration::from_secs(10),
        )
    }

    #[test]
    fn initialization_owns_creation_without_a_carrier_behavior() {
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart())
                .unwrap()
                .initialize()
                .unwrap();

        assert_eq!(initialized.actions.creates.len(), 1);
        assert_eq!(initialized.actions.creates[0].nonce, 7);
        assert_eq!(initialized.actions.sends.child_observations.len(), 1);
        assert_eq!(initialized.actions.sends.creation_observations.len(), 1);
    }

    #[test]
    fn uninhabited_public_message_does_not_hide_lifecycle_inputs() {
        fn accepts<T: behavior::InjectEvent<crate::ShutdownRequested, behavior::Here>>() {}
        accepts::<SupervisorEvent<MailAddr>>();
    }

    #[test]
    fn standalone_adapter_preserves_replacement_and_drain_effects() {
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart())
                .unwrap()
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;
        active.on(CreationResolved::birth(7, MailAddr(70))).unwrap();

        let replacement = active
            .on(WorkerStopped::new(
                7,
                70,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap();
        assert_eq!(replacement.sends.replacement_commands.len(), 1);
        assert!(replacement.creates.is_empty());

        let shutdown = active.on(ShutdownRequested).unwrap();
        assert_eq!(shutdown.sends.shutdowns.len(), 1);
        assert!(active.is_shutting_down());
    }

    #[test]
    fn standalone_failure_policy_stops_only_after_publishing_denial() {
        let restart = RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            0,
            Duration::from_secs(1),
        );
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart)
                .unwrap()
                .with_failure_policy(TopologyFailurePolicy::Stop)
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;
        let denied = active
            .on(WorkerStopped::new(
                7,
                70,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap();
        assert_eq!(denied.sends.failure_reports.len(), 1);
        assert!(matches!(denied.become_, Step::Stop(_)));
    }
}
