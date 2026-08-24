//! Standalone fixed-fleet supervision over the shared ownership fold.

#![allow(
    private_interfaces,
    reason = "public inferred aliases use a closed policy trait over crate-private fleet ownership"
)]

use super::SupervisionEvent;
use super::adapter::{ChildTopology, ProxyWithParent, RestartConfiguration, SupervisorSends};
use super::domain::{
    FixedFleetOwnership, FleetError, OwnershipError, OwnershipFold, WorkerCommandRejection,
};
use behavior::{Actions, Address, Behavior, Births, Never, Step, User};

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
pub enum SupervisorError<A: Address> {
    #[error(transparent)]
    Fleet(#[from] FleetError<A::Nonce>),
    #[error("worker factory rejected configured fleet index {index}")]
    FactoryIndex { index: usize },
    #[error("owned proxy shutdown was rejected")]
    ChildShutdownRejected(crate::ChildShutdownRejected<A::Nonce>),
    #[error("a creation result does not belong to a pending stable-child creation")]
    UnexpectedCreation(crate::CreationResolved<A>),
    #[error("a stable-child stop does not belong to the current ownership state")]
    UnexpectedChildStopped(crate::ChildStopped<A>),
    #[error("a worker stop does not belong to the current worker incarnation")]
    UnexpectedWorkerStopped(crate::WorkerStopped<A>),
    #[error("a worker creation result does not belong to a pending worker creation")]
    UnexpectedWorkerCreation(crate::WorkerCreationResolved<A::Nonce>),
    #[error("a child-shutdown rejection does not belong to an outstanding shutdown request")]
    UnexpectedChildShutdownRejection(crate::ChildShutdownRejected<A::Nonce>),
    #[error("stable-child creation provenance did not match the pending request")]
    CreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::CreationResolved<A>,
    },
    #[error("worker-incarnation creation provenance did not match the pending request")]
    WorkerCreationProvenanceMismatch {
        expected: crate::CreationKind<A::Nonce>,
        observed: crate::WorkerCreationResolved<A::Nonce>,
    },
}

impl<A: Address> From<OwnershipError<A>> for SupervisorError<A> {
    fn from(error: OwnershipError<A>) -> Self {
        match error {
            OwnershipError::Fleet(error) => Self::Fleet(error),
            OwnershipError::FactoryIndex { index } => Self::FactoryIndex { index },
            OwnershipError::ChildShutdownRejected(event) => Self::ChildShutdownRejected(event),
            OwnershipError::UnexpectedCreation(event) => Self::UnexpectedCreation(event),
            OwnershipError::UnexpectedChildStopped(event) => Self::UnexpectedChildStopped(event),
            OwnershipError::UnexpectedWorkerStopped(event) => Self::UnexpectedWorkerStopped(event),
            OwnershipError::UnexpectedWorkerCreation(event) => {
                Self::UnexpectedWorkerCreation(event)
            }
            OwnershipError::UnexpectedChildShutdownRejection(event) => {
                Self::UnexpectedChildShutdownRejection(event)
            }
            OwnershipError::CreationProvenanceMismatch { expected, observed } => {
                Self::CreationProvenanceMismatch { expected, observed }
            }
            OwnershipError::WorkerCreationProvenanceMismatch { expected, observed } => {
                Self::WorkerCreationProvenanceMismatch { expected, observed }
            }
        }
    }
}

/// Why one selected stable worker cannot currently accept a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerUnavailable {
    /// The selector named no configured stable worker.
    Unknown,
    /// The stable proxy or its first worker has not committed yet.
    Starting,
    /// The stable proxy is waiting for a replacement worker to commit.
    Restarting,
    /// Supervision policy permanently retired the stable worker.
    Retired,
    /// Coordinated shutdown has closed command admission.
    ShuttingDown,
}

impl<N> From<WorkerCommandRejection<N>> for (N, WorkerUnavailable) {
    fn from(rejection: WorkerCommandRejection<N>) -> Self {
        match rejection {
            WorkerCommandRejection::UnknownWorker(worker) => (worker, WorkerUnavailable::Unknown),
            WorkerCommandRejection::Starting(worker) => (worker, WorkerUnavailable::Starting),
            WorkerCommandRejection::Restarting(worker) => (worker, WorkerUnavailable::Restarting),
            WorkerCommandRejection::Retired(worker) => (worker, WorkerUnavailable::Retired),
            WorkerCommandRejection::ShuttingDown(worker) => {
                (worker, WorkerUnavailable::ShuttingDown)
            }
        }
    }
}

/// Controlled failure of application-facing fixed supervision.
///
/// Command admission is all-or-nothing. An available command produces exactly
/// one send through its selected stable route. Every unavailable outcome
/// rejects the transition without changing supervisor state and returns the
/// complete original [`User`] value. Commands are never dropped, stashed,
/// redirected, or partially reconstructed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SupervisedWorkersError<A: Address, M> {
    /// The underlying ownership or lifecycle fold rejected the transition.
    #[error("supervised ownership fold rejected the transition")]
    Supervision(#[source] SupervisorError<A>),
    /// The selected stable worker could not accept this exact command.
    #[error("the selected stable worker could not accept the command")]
    CommandNotAccepted {
        worker: A::Nonce,
        reason: WorkerUnavailable,
        command: User<A, M>,
    },
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
type FixedSupervisorActions<A, C, ParentPath> =
    Actions<A, Never, SupervisorSends<A, C, ParentPath>, Births<ProxyWithParent<C, ParentPath>>>;

/// Static application-command policy for one fixed supervisor.
#[doc(hidden)]
pub trait FixedSupervisorCommands<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    type Message;
    type Protocol: crate::Protocol<Addr = A, Msg = Self::Message>;
    type Error;

    fn command(
        &self,
        ownership: &FixedFleetOwnership<A, C, ParentPath>,
        command: User<A, Self::Message>,
    ) -> Result<FixedSupervisorActions<A, C, ParentPath>, Self::Error>;

    fn supervision_error(error: SupervisorError<A>) -> Self::Error;
}

/// Compiler representation of an ownership-only fixed supervisor.
#[doc(hidden)]
pub struct OwnershipOnly;

impl<A, C, ParentPath> FixedSupervisorCommands<A, C, ParentPath> for OwnershipOnly
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    type Message = Never;
    type Protocol = SupervisorProtocol<A>;
    type Error = SupervisorError<A>;

    fn command(
        &self,
        _: &FixedFleetOwnership<A, C, ParentPath>,
        command: User<A, Never>,
    ) -> Result<FixedSupervisorActions<A, C, ParentPath>, SupervisorError<A>> {
        match command.message {}
    }

    fn supervision_error(error: SupervisorError<A>) -> Self::Error {
        error
    }
}

/// Compiler representation of an explicit application worker selector.
#[doc(hidden)]
pub struct SelectWorker<Select>(Select);

impl<A, C, Select, ParentPath> FixedSupervisorCommands<A, C, ParentPath> for SelectWorker<Select>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Select: Fn(&<C::Protocol as crate::Protocol>::Msg) -> A::Nonce,
{
    type Message = <C::Protocol as crate::Protocol>::Msg;
    type Protocol = C::Protocol;
    type Error = SupervisedWorkersError<A, Self::Message>;

    fn command(
        &self,
        ownership: &FixedFleetOwnership<A, C, ParentPath>,
        command: User<A, Self::Message>,
    ) -> Result<FixedSupervisorActions<A, C, ParentPath>, Self::Error> {
        let worker = (self.0)(&command.message);
        match ownership.command(worker, command) {
            Ok(actions) => Ok(actions),
            Err((rejection, command)) => {
                let (worker, reason) = rejection.into();
                Err(SupervisedWorkersError::CommandNotAccepted {
                    worker,
                    reason,
                    command,
                })
            }
        }
    }

    fn supervision_error(error: SupervisorError<A>) -> Self::Error {
        SupervisedWorkersError::Supervision(error)
    }
}

/// One fixed stable-worker ownership actor parameterized only by its public
/// application-command policy.
#[doc(hidden)]
pub struct FixedSupervisor<A, C, Commands, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Commands: FixedSupervisorCommands<A, C, ParentPath>,
{
    ownership: FixedFleetOwnership<A, C, ParentPath>,
    commands: Commands,
    failure: TopologyFailurePolicy,
}

/// A standalone fixed supervisor whose proxy reports return directly.
pub type SupervisorWithParent<A, C, ParentPath> = FixedSupervisor<A, C, OwnershipOnly, ParentPath>;

pub type Supervisor<A, C> = SupervisorWithParent<A, C, behavior::Here>;

/// A fixed supervised worker set that accepts the workers' application
/// command directly.
///
/// `Select` is the application policy that chooses one configured stable
/// worker nonce for each command. Successful commands are sent through the
/// supervisor-owned proxy route. Proxy commands and routes are not part of
/// this actor's public protocol.
pub type SupervisedWorkersWithParent<A, C, Select, ParentPath> =
    FixedSupervisor<A, C, SelectWorker<Select>, ParentPath>;

/// Supervised application workers whose proxy reports return directly.
pub type SupervisedWorkers<A, C, Select> =
    SupervisedWorkersWithParent<A, C, Select, behavior::Here>;

impl<A, C, Select> SupervisedWorkers<A, C, Select>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Select: Fn(&<C::Protocol as crate::Protocol>::Msg) -> A::Nonce,
{
    /// Configure fixed workers and the application policy that selects one
    /// stable worker for every command.
    ///
    /// # Errors
    /// Returns the first typed topology rejection.
    pub fn new(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
        select: Select,
    ) -> Result<Self, FleetError<A::Nonce>> {
        Self::with_parent(topology, restart, select, crate::ProxyParentIngress::new())
    }
}

impl<A, C, Select, ParentPath> SupervisedWorkersWithParent<A, C, Select, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Select: Fn(&<C::Protocol as crate::Protocol>::Msg) -> A::Nonce,
{
    /// Configure fixed workers whose proxy reports target `parent`.
    ///
    /// # Errors
    /// Returns the first typed topology rejection.
    pub fn with_parent(
        topology: ChildTopology<A::Nonce, C>,
        restart: RestartConfiguration,
        select: Select,
        parent: crate::ProxyParentIngress<A, ParentPath>,
    ) -> Result<Self, FleetError<A::Nonce>> {
        Ok(Self {
            ownership: FixedFleetOwnership::new(topology, restart, parent)?,
            commands: SelectWorker(select),
            failure: TopologyFailurePolicy::Retire,
        })
    }
}

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
            commands: OwnershipOnly,
            failure: TopologyFailurePolicy::Retire,
        })
    }
}

impl<A, C, Commands, ParentPath> FixedSupervisor<A, C, Commands, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Commands: FixedSupervisorCommands<A, C, ParentPath>,
{
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
        if !fold.failures.is_empty() && self.failure == TopologyFailurePolicy::Stop {
            fold.actions.become_ = Step::Stop(crate::Stopped);
        }
        Ok(fold.actions)
    }
}

impl<A, C, Commands, ParentPath> crate::BehaviorBase for FixedSupervisor<A, C, Commands, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Commands: FixedSupervisorCommands<A, C, ParentPath>,
{
    type Base = Self;
    fn base(&self) -> &Self::Base {
        self
    }
}

impl<A, C, Commands, ParentPath> Behavior for FixedSupervisor<A, C, Commands, ParentPath>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Commands: FixedSupervisorCommands<A, C, ParentPath>,
    SupervisorSends<A, C, ParentPath>:
        behavior::SendsFor<SupervisionEvent<User<A, Commands::Message>>>,
{
    type Protocol = Commands::Protocol;
    type Event = SupervisionEvent<User<A, Commands::Message>>;
    type Sends = SupervisorSends<A, C, ParentPath>;
    type Ph = Never;
    type Error = Commands::Error;
    type Birth = Births<ProxyWithParent<C, ParentPath>>;

    fn init(&mut self, _: crate::InitializationTurn) -> crate::BehaviorActed<Self> {
        self.ownership
            .initialize()
            .map_err(SupervisorError::from)
            .map_err(Commands::supervision_error)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> crate::BehaviorActed<Self> {
        match event {
            SupervisionEvent::Behavior(user) => self.commands.command(&self.ownership, user),
            SupervisionEvent::WorkerStopped(event) => {
                let fold = self
                    .ownership
                    .worker_stopped(event)
                    .map_err(SupervisorError::from)
                    .map_err(Commands::supervision_error)?;
                self.finish(fold)
            }
            SupervisionEvent::ChildStopped(event) => {
                let fold = self
                    .ownership
                    .child_stopped(event)
                    .map_err(SupervisorError::from)
                    .map_err(Commands::supervision_error)?;
                self.finish(fold)
            }
            SupervisionEvent::CreationResolved(event) => {
                let fold = self
                    .ownership
                    .creation_resolved(event)
                    .map_err(SupervisorError::from)
                    .map_err(Commands::supervision_error)?;
                self.finish(fold)
            }
            SupervisionEvent::WorkerCreationResolved(event) => {
                let fold = self
                    .ownership
                    .worker_creation_resolved(event)
                    .map_err(SupervisorError::from)
                    .map_err(Commands::supervision_error)?;
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
                    .map_err(SupervisorError::from)
                    .map_err(Commands::supervision_error)?;
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
        Activate as _, ChildShutdownRejected, ChildShutdownRejection, ChildStopped, Crash,
        CreationKind, CreationRejection, CreationResolved, Exit, RestartPolicy, ShutdownRequested,
        Strategy, SupervisionFailure, WorkerCreationResolved, WorkerStopped,
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

    #[test]
    fn worker_creation_rejection_is_an_exact_topology_failure() {
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart())
                .unwrap()
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;
        let rejected = active
            .on(WorkerCreationResolved::new(
                7,
                70,
                CreationKind::Birth,
                Err(CreationRejection::NonceAlreadyBound),
            ))
            .unwrap();

        assert_eq!(rejected.sends.failure_reports.len(), 1);
        assert_eq!(
            rejected.sends.failure_reports[0].failure,
            SupervisionFailure::WorkerCreationRejected {
                proxy: 7,
                worker: 70,
                kind: CreationKind::Birth,
                rejection: CreationRejection::NonceAlreadyBound,
            }
        );
        assert!(!active.is_alive(7).unwrap());
    }

    #[test]
    fn worker_creation_provenance_mismatch_is_typed_and_atomic() {
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart())
                .unwrap()
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;
        let observed = WorkerCreationResolved::new(
            7,
            70,
            CreationKind::ReplacementIncarnation { replaces: 69 },
            Ok(()),
        );
        let result = active.on(observed);
        let Err(error) = result else {
            panic!("mismatched worker provenance must be rejected");
        };

        assert_eq!(
            error,
            SupervisorError::WorkerCreationProvenanceMismatch {
                expected: CreationKind::Birth,
                observed,
            }
        );
        assert!(active.is_alive(7).unwrap());
    }

    #[test]
    fn foreign_and_duplicate_lifecycle_facts_are_returned_exactly_and_do_not_advance_state() {
        let initialized =
            Supervisor::<MailAddr, Child>::new(ChildTopology::new([7], child), restart())
                .unwrap()
                .initialize()
                .unwrap();
        let mut active = initialized.behavior;

        let foreign_creation = CreationResolved::birth(8, MailAddr(80));
        assert!(matches!(
            active.on(foreign_creation),
            Err(SupervisorError::UnexpectedCreation(returned)) if returned == foreign_creation
        ));
        active.on(CreationResolved::birth(7, MailAddr(70))).unwrap();
        let duplicate_creation = CreationResolved::birth(7, MailAddr(71));
        assert!(matches!(
            active.on(duplicate_creation),
            Err(SupervisorError::UnexpectedCreation(returned)) if returned == duplicate_creation
        ));

        let foreign_worker_creation =
            WorkerCreationResolved::new(8, 80, CreationKind::Birth, Ok(()));
        assert!(matches!(
            active.on(foreign_worker_creation),
            Err(SupervisorError::UnexpectedWorkerCreation(returned))
                if returned == foreign_worker_creation
        ));
        active
            .on(WorkerCreationResolved::new(
                7,
                70,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();

        let now = Instant::now();
        let stale_worker = WorkerStopped::new(7, 71, Ok(Exit::Normal), now);
        assert!(matches!(
            active.on(stale_worker.clone()),
            Err(SupervisorError::UnexpectedWorkerStopped(returned)) if returned == stale_worker
        ));
        let foreign_child = ChildStopped::new(8, Ok(Exit::Normal), now);
        assert!(matches!(
            active.on(foreign_child),
            Err(SupervisorError::UnexpectedChildStopped(returned)) if returned == foreign_child
        ));
        let foreign_rejection =
            ChildShutdownRejected::new(8, ChildShutdownRejection::NotEstablished);
        assert!(matches!(
            active.on(foreign_rejection),
            Err(SupervisorError::UnexpectedChildShutdownRejection(returned))
                if returned == foreign_rejection
        ));
        assert!(active.is_alive(7).unwrap());
    }
}
