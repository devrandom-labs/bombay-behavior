//! Reusable, pure actors and behavior compositions built on [`behavior`].
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
//! Catalogue values and wrappers are constructed directly through their public
//! owning-type constructors. [`Activate`] consumes any concrete definition
//! into its one initialized [`Active`] state. Rust can infer wrapper stacks at
//! construction and spawn call sites; applications do not need to name types
//! such as `Deadline<Stash<Machine<...>>>`.
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
pub use discovery::{
    Presence, PresenceEntry, PresenceError, PresenceMessage, PresenceOutcome, PresencePhase,
    PresenceReply, PresenceReport, PresenceSends, PresenceVersion, PubSub, PubSubError,
    PubSubMessage, Registry, RegistryError, RegistryMessage, RegistryResult, Resolution, Resolver,
    ResolverConfigError, ResolverMessage, Topic, TopicError, TopicMembership, TopicMessage,
};
pub use lifecycle::{
    CleanupReaction, Guardian, LifecyclePublication, LifecyclePublisher, Reaper,
    ShutdownCoordinator, ShutdownCoordinatorError, ShutdownCoordinatorEvent,
    ShutdownCoordinatorSends, ShutdownPlan, ShutdownPlanError, ShutdownState, ShutdownTree,
    ShutdownTreeError, Task, TaskError, TaskMessage, TaskResult, TaskState, TerminationMonitor,
    TerminationObservation, TerminationReaction, TreeShutdown,
};
pub use machine::{Machine, Move};
pub use operations::{
    ComponentHealth, ComponentHealthState, Configuration, ConfigurationError, ConfigurationMessage,
    ConfigurationState, ConfigurationVersion, DependencyReadiness, Feature, FeatureSet,
    FeatureStatus, Features, FeaturesState, Health, HealthError, HealthMessage, HealthReport,
    HealthStatus, ObservationVersion, Readiness, ReadinessError, ReadinessEvidence,
    ReadinessMessage, ReadinessReport, ReadinessStatus,
};
pub use persistence::{Cache, CacheConfigError, CacheEntry, CacheMessage, CacheResult, CacheState};
pub use pool::{
    AffinitySelector, AssignmentId, InterruptionPolicy, JobId, KeyedPoolEvent, KeyedPoolMessage,
    KeyedWorkerPool, PoolActions, PoolAssignment, PoolBehaviorSends, PoolConfigError,
    PoolConfiguration, PoolError, PoolEvent, PoolInterruption, PoolMessage, PoolRejection,
    PoolResponse, PoolSends, WorkerPhase, WorkerPool, WorkerRetirement,
};
pub use protocol::{
    ChildShutdownRejected, ChildShutdownRejection, ChildStopped, CreationRejection,
    CreationResolved, ObserveChild, ObserveCreation, ObservePeer, PeerStopped,
    ReplacementResolution, ReportWorkerCreationResolved, ReportWorkerStopped, ScheduleAfter,
    ScheduleAt, ShutdownChild, ShutdownRequested, TimerElapsed, TimerGeneration, TimerId,
    UnwatchPeer, WorkerCreationResolved, WorkerStopped,
};
pub use routing::{
    AcknowledgementError, AcknowledgementMessage, AcknowledgementOutcome, AcknowledgementRecord,
    AcknowledgementState, Acknowledgements, BreakerAttempt, BreakerConfigError, BreakerMessage,
    BreakerOutcome, BreakerPhase, BreakerRejection, BreakerSends, Broadcast, Buffer,
    BufferConfigError, BufferMessage, BufferOutcome, BufferRejection, BufferSends, BufferState,
    Buffered, CircuitBreaker, ClosedPhase, ConsistentHash, CorrelationResult, CorrelationState,
    Correlator, CorrelatorError, CorrelatorMessage, Deduplicator, DeduplicatorConfigError,
    DeduplicatorMessage, DeduplicatorOutcome, DeduplicatorSends, DeduplicatorState,
    HashPolicyError, LeastLoaded, LeastLoadedError, Load, LoadEvidence, LoadObservation,
    LoadVersion, MemberToken, MemberTokenEvidence, MemberTokenObservation, MemberTokenVersion,
    OrderGate, OrderGateMessage, OrderGateOutcome, OrderGateSends, OrderGateState, OverflowPolicy,
    PriorityQueue, PriorityQueueConfigError, PriorityQueueMessage, PriorityQueueOutcome,
    PriorityQueueRejection, PriorityQueueSends, PriorityQueueState, ProbePhase, RateLimitRejection,
    RateLimiter, RateLimiterConfigError, RateLimiterMessage, RateLimiterOutcome, RateLimiterSends,
    RateLimiterState, RendezvousHash, RoundRobin, RouteKey, Router, RouterError, RouterMessage,
    RoutingStrategy, Sequence, Sequencer, SequencerMessage, SequencerOutcome, SequencerSends,
    SequencerState, TokenCount, WorkQueue, WorkQueueMessage, WorkQueueOutcome, WorkQueueRejection,
    WorkQueueSends, WorkQueueState,
};
pub use shutdown::{FinalizeOnShutdown, ShutdownProtocol, ShutdownReaction, StopOnShutdown};
pub use stash::{Stash, StashRoute, StashStatus};
pub use supervision::{
    Backoff, BackoffConfigError, BackoffError, BackoffSupervisor, BackoffSupervisorError,
    BackoffSupervisorSends, ChildTopology, DynamicChildPhase, DynamicProxy, DynamicSupervisor,
    DynamicSupervisorEvent, DynamicSupervisorMessage, DynamicSupervisorOutcome,
    DynamicSupervisorRejection, DynamicSupervisorSends, FleetError, IncarnationPhase, Proxy,
    ProxyCommand, ProxyError, ProxyEvent, ProxySends, ReportSupervisionFailure,
    RestartConfiguration, RestartPolicy, Strategy, SupervisionEvent, SupervisionFailure,
    SupervisionFailureReaction, Supervisor, SupervisorError, SupervisorSends, restart_all,
    restart_one, restart_rest, retire_on_supervision_failure, stop_on_supervision_failure,
};
pub use termination::{Crash, Exit, RestartDenial, SupervisionFailureReason};
pub use time::{
    Deadline, DeadlineEvent, DeadlineReaction, DeadlineSends, Lease, LeaseMessage, LeaseOutcome,
    LeaseRejection, LeaseSends, LeaseState, OneShot, OneShotEvent, OneShotReaction, OneShotSends,
    Periodic, PeriodicEvent, PeriodicReaction, PeriodicSends, ReceiveTimeout, ReceiveTimeoutEvent,
    ReceiveTimeoutReaction, ReceiveTimeoutSends, TimedEvent,
};
pub use watch::{Link, LinkReaction, Watch, WatchEvent, WatchSends, stop_on_abnormal_death};
pub use workflow::{
    Barrier, BarrierArrival, BarrierConfigError, BarrierError, BarrierGeneration, BarrierMessage,
    BarrierReleased, BarrierState, Latch, LatchMessage, LatchReleased, LatchState, Workflow,
    WorkflowConfigError, WorkflowDefinition, WorkflowMessage, WorkflowOutcome, WorkflowRejection,
    WorkflowState, WorkflowStepState,
};
