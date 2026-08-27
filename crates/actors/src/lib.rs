//! Reusable, pure actors and behavior compositions built on [`mod@behavior`].
//!
//! The public taxonomy follows actor-system semantics: [`composition`],
//! [`lifecycle`], [`supervision`], [`routing`], [`discovery`], [`time`],
//! [`persistence`], [`workflow`], and [`operations`]. Every template is a
//! deterministic fold from one concrete event sum to explicit typed
//! [`Actions`]. It performs no scheduling, delivery, allocation, observation,
//! persistence, or I/O itself. Those effects remain inputs and named output
//! lanes for Bombay's one universal Driver and statically selected Environment
//! interpreters.
//!
//! Public [`Protocol`] identity remains orthogonal to those internal event and
//! effect algebras. Concrete actor templates declare their protocol; transparent
//! wrappers preserve `B::Protocol` and do not become new recipient identities.
//! [`BirthProtocols`] projects each template's own protocol and
//! complete transitive staged-birth protocols into one closed structural
//! product. Delivery-only external protocols do not enter that product.
//! [`LogicalHostRequirements`] separately derives every intentional logical
//! destination from the concrete sends algebra and every transitive birth.
//! The projection preserves repeated occurrences and excludes exact,
//! creator-local, and interpreter-owned lanes; it is static proof metadata,
//! not a host registry or runtime lookup.
//!
//! Catalogue values and wrappers are constructed directly through their public
//! owning-type constructors. [`Activate`] consumes any concrete definition
//! into its one initialized [`Active`] state. Rust can infer wrapper stacks at
//! construction and spawn call sites; applications do not need to name types
//! such as `Deadline<Stash<Machine<...>>>`.
//! Correctness-sensitive cross-family orders use ordinary typed nesting and
//! actor communication. The repository's composition guide records their
//! construction, error, initialization, and effect-preservation laws.
//!
//! The top-level Bombay package is the ordinary application façade. Direct use
//! of this component crate is intended for interpreter implementation,
//! component tests, and advanced framework extension.

pub use behavior::*;
mod activation;
pub mod composition;
pub mod discovery;
pub mod lifecycle;
mod machine;
pub mod operations;
pub mod persistence;
mod pool;
mod protocol;
mod requirements;
pub mod routing;
mod shutdown;
mod stash;
pub mod supervision;
mod termination;
#[path = "timing/mod.rs"]
pub mod time;
mod watch;
pub mod workflow;

pub use activation::{Activate, Active, Initialized};
pub use composition::{
    DeliveryRoute, DeliveryRouteFor, MessageAdapter, MessageAdapterWithRoute, ReplyDeliveries,
    ReplyDelivery, ReplyRoute,
};
pub use discovery::{
    Presence, PresenceEntry, PresenceError, PresenceMessage, PresenceOutcome, PresencePhase,
    PresenceReply, PresenceReport, PresenceSends, PresenceVersion, PubSub, PubSubError,
    PubSubMessage, Registry, RegistryError, RegistryMessage, RegistryResult, Resolution, Resolver,
    ResolverConfigError, ResolverMessage, Topic, TopicError, TopicMembership, TopicMessage,
};
pub use lifecycle::{
    BeginShutdownPhases, ChildCreationExpectation, ChildShutdownPhases, ChildShutdownPlanError,
    ChildTermination, DeclareShutdownPhase, EstablishedTerminationMonitor,
    EstablishedTerminationReaction, EstablishedTerminationTarget, FinishShutdownPhases,
    HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan, HeterogeneousShutdownSends,
    InstallShutdownPlan, LogicalTerminationTarget, NoShutdownTargets, PeerTermination,
    PropagateTermination, ReportShutdownPlan, ShutdownChoice, ShutdownCoordinator,
    ShutdownCoordinatorError, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownPlanError,
    ShutdownState, ShutdownTargetAt, ShutdownTree, ShutdownTreeError, Task, TaskError, TaskMessage,
    TaskResult, TaskState, TerminalDisposition, TerminalPropagationPolicy,
    TerminalPropagationSends, TerminalPropagationState, TerminationMonitor,
    TerminationMonitorError, TerminationMonitorWith, TerminationObservation,
    TerminationObservationTarget, TerminationPropagationError, TerminationReaction,
    TerminationTarget, propagate_abnormal, propagate_all, shutdown_after_children, shutdown_target,
};
pub use machine::{Machine, MachineError, Move};
pub use operations::{
    ComponentHealth, ComponentHealthState, Configuration, ConfigurationError, ConfigurationMessage,
    ConfigurationState, ConfigurationVersion, DependencyReadiness, Feature, FeatureSet,
    FeatureStatus, Health, HealthError, HealthEvidence, HealthMessage, HealthReport, HealthStatus,
    ObservationVersion, Readiness, ReadinessError, ReadinessEvidence, ReadinessMessage,
    ReadinessReport, ReadinessStatus,
};
pub use persistence::{
    Cache, CacheConfigError, CacheConfiguration, CacheEntry, CacheMessage, CacheResult, CacheState,
};
pub use pool::{
    AffinitySelector, AssignmentId, CompletionRejection, InterruptionPolicy, JobId,
    KeyedPoolMessage, KeyedWorkerPool, KeyedWorkerPoolEvent, PoolAssignment, PoolCompletion,
    PoolConfigError, PoolConfiguration, PoolCustomer, PoolError, PoolFailure, PoolInterruption,
    PoolMessage, PoolRejection, PoolResponse, PoolSends, RebalanceRejection, WorkerPhase,
    WorkerPool, WorkerPoolEvent, WorkerRetirement,
};
pub use protocol::{
    CancelObservation, ChildShutdownRejected, ChildShutdownRejection, ChildStopped,
    CreationResolved, EstablishedChild, EstablishedObservation, EstablishedShutdownResolved,
    InterpretEstablishedObservation, InterpretEstablishedShutdown, KeyedWorkerPoolProtocol,
    ObservationId, ObservationOperation, ObservationRejection, ObserveChild, ObserveCreation,
    ObserveEstablished, ObserveEstablishedCreation, ObservePeer, PeerStopped, ReplacementRequested,
    ReplacementResolution, ReportProxyUnavailable, ReportWorkerCreationResolved,
    ReportWorkerStopped, ScheduleAfter, ScheduleAt, ShutdownChild, ShutdownEstablished, ShutdownId,
    ShutdownRejection, ShutdownRequested, TimerElapsed, TimerGeneration, TimerId, UnwatchPeer,
    WorkerCreationResolved, WorkerPoolProtocol, WorkerStopped, established_child,
};
pub use routing::{
    AcknowledgementError, AcknowledgementInput, AcknowledgementMessage, AcknowledgementOutcome,
    AcknowledgementRecord, AcknowledgementState, Acknowledgements, BreakerAttempt,
    BreakerCompletion, BreakerConfigError, BreakerError, BreakerMessage, BreakerOutcome,
    BreakerPhase, BreakerRejection, BreakerSends, Buffer, BufferConfigError, BufferConfiguration,
    BufferMessage, BufferOutcome, BufferRejection, BufferSends, BufferState, Buffered,
    CircuitBreaker, ClosedPhase, ConsistentHash, CorrelationResult, CorrelationState, Correlator,
    CorrelatorError, CorrelatorMessage, Deduplicator, DeduplicatorConfigError, DeduplicatorMessage,
    DeduplicatorOutcome, DeduplicatorState, DeliveryOutcomes, HashPolicyError, LeastLoaded,
    LeastLoadedError, Load, LoadEvidence, LoadObservation, LoadVersion, MemberToken,
    MemberTokenEvidence, MemberTokenObservation, MemberTokenVersion, OrderGate, OrderGateMessage,
    OrderGateOutcome, OrderGateState, OverflowPolicy, PriorityQueue, PriorityQueueConfigError,
    PriorityQueueMessage, PriorityQueueOutcome, PriorityQueueRejection, PriorityQueueState,
    ProbePhase, RateLimitRejection, RateLimiter, RateLimiterConfigError, RateLimiterMessage,
    RateLimiterOutcome, RateLimiterState, RendezvousHash, RoundRobin, RouteKey, Router,
    RouterError, RouterMessage, RoutingStrategy, Sequence, Sequencer, SequencerMessage,
    SequencerOutcome, SequencerState, TokenCount, WorkQueue, WorkQueueMessage, WorkQueueOutcome,
    WorkQueueRejection, WorkQueueSends, WorkQueueState,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownEvent, ShutdownReaction, StopOnShutdown};
pub use stash::{Stash, StashRoute, StashStatus, StaticallyInfallible};
pub use supervision::{
    Backoff, BackoffConfigError, BackoffError, ChildTopology, DynamicChildPhase, DynamicSupervisor,
    DynamicSupervisorError, DynamicSupervisorEvent, DynamicSupervisorMessage,
    DynamicSupervisorOutcome, DynamicSupervisorProtocol, DynamicSupervisorRejection,
    DynamicSupervisorSends, FleetError, IncarnationPhase, Proxy, ProxyError, ProxyEvent,
    ProxyLifecycleError, ProxySends, ProxyUnavailable, ReportSupervisionFailure,
    RestartConfiguration, RestartPolicy, RestartTiming, Strategy, Supervise, SuperviseError,
    SupervisionEvent, SupervisionFailure, SupervisionFailureReaction, SupervisionLifecycle,
    Supervisor, SupervisorSends, restart_all, restart_one, restart_rest,
    retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use termination::{
    Crash, Exit, ReportTerminalOutcome, RestartDenial, SupervisionFailureReason, TerminalOutcome,
};
pub use time::{
    Deadline, DeadlineEvent, DeadlineReaction, Lease, LeaseMessage, LeaseOutcome, LeaseRejection,
    LeaseRequest, LeaseSends, LeaseState, OneShot, OneShotEvent, OneShotReaction, Periodic,
    PeriodicEvent, PeriodicReaction, ReceiveTimeout, ReceiveTimeoutEvent, ReceiveTimeoutReaction,
    TimedEvent,
};
pub use watch::{LinkReaction, Watch, WatchEvent, stop_on_abnormal_death};
pub use workflow::{
    Barrier, BarrierArrival, BarrierConfigError, BarrierError, BarrierGeneration,
    BarrierMembership, BarrierMessage, BarrierReleased, BarrierState, Latch, LatchMessage,
    LatchReleased, LatchState, Workflow, WorkflowConfigError, WorkflowDefinition, WorkflowError,
    WorkflowInput, WorkflowMessage, WorkflowOutcome, WorkflowRejection, WorkflowState,
    WorkflowStepState,
};
