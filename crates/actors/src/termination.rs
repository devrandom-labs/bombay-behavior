//! Typed lifecycle outcomes used by reusable actor compositions.

use behavior::{InterpretationProgress, ItemSettlement, finish_item, prepare_item};

use behavior::Address;

/// The authoritative terminal fact for one exact actor incarnation.
///
/// Successful lifecycle classification and execution failure are disjoint
/// outcomes.  Keeping the complete sum intact prevents compositions from
/// reconstructing provenance from a stop verdict, address reuse, or an
/// adjacent diagnostic.
pub type TerminalOutcome<A> = Result<Exit<A>, Crash>;

/// A successfully observed actor termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit<A: Address> {
    /// The behavior explicitly designated termination.
    Normal,
    /// The Bombay runtime collected an actor after its sources were exhausted.
    Collected,
    /// A watch composition propagated a linked peer's death.
    LinkDied(A),
    /// A supervisor could no longer preserve its child topology.
    SupervisionFailed(SupervisionFailureReason),
}

/// Why a supervisor could no longer preserve its child topology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionFailureReason {
    RestartDenied(RestartDenial),
    StableChildStopped,
    StableChildCreationRejected(behavior::CreationRejection),
    WorkerFactoryRejected,
    WorkerCreationRejected(behavior::CreationRejection),
}

/// Why an otherwise eligible replacement set was denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartDenial {
    BudgetExceeded {
        restarts_in_window: usize,
        replacements_requested: usize,
        maximum_restarts: u32,
    },
    /// The configured release policy rejected the next delay.
    ReleaseRejected(crate::RestartReleaseFailure),
    /// The per-trigger restart-attempt sequence could not advance.
    AttemptSequenceExhausted,
    /// The per-trigger timer generation could not advance.
    TimerGenerationExhausted,
    /// No fresh local restart-timer identity remained.
    TimerIdentityExhausted,
}

/// Why execution terminated without a behavior-requested stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crash {
    Failed,
    EnvironmentFailed,
    /// Live actor execution selected a capability task failure as its stop cause.
    /// Full failure and recoverable actor-state custody belongs to the runtime.
    CapabilityFailed,
    Panicked,
    Cancelled,
}

/// Ask the interpreter to publish one exact terminal outcome for the
/// emitting incarnation before interpreting the same action's terminal
/// verdict.
///
/// This is a Bombay lifecycle-publication policy, not an actor-model
/// primitive.  It is an explicit effect so a pure composition can propagate
/// an authoritative child or peer fact without placing lifecycle provenance
/// in [`behavior::Step`] or using an ambient runtime side channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportTerminalOutcome<A: Address> {
    pub outcome: TerminalOutcome<A>,
}

impl<A: Address> ReportTerminalOutcome<A> {
    #[must_use]
    pub const fn new(outcome: TerminalOutcome<A>) -> Self {
        Self { outcome }
    }
}

impl<A: Address> behavior::InterpreterRequest for ReportTerminalOutcome<A> {
    type ReturnToEmitter = behavior::NoReturnToEmitter;
    type LogicalProtocols = behavior::NoBirthProtocols;
}

impl<A> behavior::ActionItem for ReportTerminalOutcome<A>
where
    A: Address + Send,
{
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;

    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }

    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        match custody {
            (input @ Some(_), received @ None) => Some((input, received)),
            _ => None,
        }
    }

    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = ();
    type Rejection = behavior::Never;
    type Prerequisite = behavior::Never;
}
