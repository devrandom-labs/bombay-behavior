//! Dependency-ordered workflow activation as a pure fold.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    User,
};
use thiserror::Error;

/// Bombay-owned immutable dependency-graph product.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowDefinition<K> {
    /// Steps in deterministic definition order.
    pub steps: Vec<K>,
    /// Directed prerequisite edges `(prerequisite, dependent)`.
    pub dependencies: Vec<(K, K)>,
}

/// Construction rejection for a malformed workflow graph.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkflowConfigError<K> {
    /// At least one step is required.
    #[error("workflow requires at least one step")]
    Empty,
    /// Step identity occurs more than once.
    #[error("workflow step identity is duplicated")]
    DuplicateStep { step: K },
    /// One edge names an undefined prerequisite.
    #[error("workflow dependency names an undefined prerequisite")]
    UnknownPrerequisite { step: K },
    /// One edge names an undefined dependent.
    #[error("workflow dependency names an undefined dependent")]
    UnknownDependent { step: K },
    /// One edge depends upon itself.
    #[error("workflow step cannot depend upon itself")]
    SelfDependency { step: K },
    /// The dependency relation contains a cycle.
    #[error("workflow dependencies contain a cycle")]
    Cycle,
}

/// Complete lifecycle of one workflow step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowStepState {
    /// At least one prerequisite has not completed.
    Blocked,
    /// Activation was emitted and completion is pending.
    Active,
    /// Successful completion was committed.
    Completed,
}

/// Complete workflow phase sum.
pub enum WorkflowState<K, Reply: behavior::Protocol> {
    /// Validated definition has not started.
    Ready,
    /// At least one step remains incomplete.
    Running {
        /// Per-step states in definition order.
        steps: Vec<(K, WorkflowStepState)>,
        /// Recipient for every activation and terminal fact.
        reply_to: Recipient<Reply>,
    },
    /// Every step completed successfully.
    Succeeded,
    /// One active step failed; no further activations are possible.
    Failed { step: K },
    /// Explicit cancellation terminated the run.
    Cancelled,
}

/// Rejected workflow input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRejection<K> {
    /// Start was requested after the only run had started or terminated.
    AlreadyStarted,
    /// Completion names no defined step.
    UnknownStep { step: K },
    /// A blocked step cannot complete or fail.
    Blocked { step: K },
    /// The step already completed.
    AlreadyCompleted { step: K },
    /// The workflow is terminal.
    Terminal { step: Option<K> },
}

/// Activation and terminal facts emitted by [`Workflow`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowOutcome<K> {
    /// A run began and these root steps became active in definition order.
    Started { activated: Vec<K> },
    /// One completion committed and may have activated dependents.
    Advanced { completed: K, activated: Vec<K> },
    /// The final completion committed.
    Succeeded { completed: K },
    /// One active step failed and terminated the run.
    Failed { step: K },
    /// Explicit cancellation terminated the run.
    Cancelled,
    /// Input was rejected without mutation.
    Rejected(WorkflowRejection<K>),
}

/// Closed workflow protocol.
pub enum WorkflowMessage<K, Reply: behavior::Protocol> {
    /// Begin the single workflow run.
    Start { reply_to: Recipient<Reply> },
    /// Report successful completion of an active step.
    Complete { step: K },
    /// Report failure of an active step.
    Fail { step: K },
    /// Cancel a ready or running workflow.
    Cancel { reply_to: Recipient<Reply> },
}

/// Deterministic dependency-ordered workflow coordinator.
///
/// Construction validates a finite acyclic graph. `Start` activates every
/// root in definition order. Completion activates a blocked step exactly once
/// when all of its prerequisites are complete. Unknown, blocked, duplicate,
/// and terminal input is explicitly rejected without mutation. Failure and
/// cancellation are terminal and prevent later activation. Initialization is
/// empty and the actor itself remains available to report terminal rejection.
/// Graph validation, one-run retention, activation ordering, and terminal
/// policy are Bombay choices. The actor model requires only that each event be
/// processed as one transition; participant execution and durable saga state
/// belong to the Driver/Mnesis boundaries. No transition panics.
pub struct Workflow<
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
> {
    definition: WorkflowDefinition<K>,
    state: WorkflowState<K, Reply>,
    marker: core::marker::PhantomData<fn() -> A>,
}

impl<A, K, Reply> Workflow<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
{
    /// Validate and retain one dependency graph.
    ///
    /// # Errors
    ///
    /// Returns a concrete [`WorkflowConfigError`] for empty, duplicate,
    /// unknown, self-dependent, or cyclic definitions.
    pub fn new(definition: WorkflowDefinition<K>) -> Result<Self, WorkflowConfigError<K>> {
        validate(&definition)?;
        Ok(Self {
            definition,
            state: WorkflowState::Ready,
            marker: core::marker::PhantomData,
        })
    }

    /// Borrow the validated definition.
    #[must_use]
    pub const fn definition(&self) -> &WorkflowDefinition<K> {
        &self.definition
    }

    /// Borrow the complete workflow phase.
    #[must_use]
    pub const fn state(&self) -> &WorkflowState<K, Reply> {
        &self.state
    }

    fn reply(
        reply_to: Recipient<Reply>,
        outcome: WorkflowOutcome<K>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        Actions::send(vec![Delivery::new(reply_to, outcome)])
    }

    fn start(
        &mut self,
        reply_to: Recipient<Reply>,
    ) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        if !matches!(self.state, WorkflowState::Ready) {
            return Self::reply(
                reply_to,
                WorkflowOutcome::Rejected(WorkflowRejection::AlreadyStarted),
            );
        }
        let mut steps: Vec<_> = self
            .definition
            .steps
            .iter()
            .cloned()
            .map(|step| (step, WorkflowStepState::Blocked))
            .collect();
        let mut activated = Vec::new();
        for (step, state) in &mut steps {
            if !self
                .definition
                .dependencies
                .iter()
                .any(|(_, dependent)| dependent == step)
            {
                *state = WorkflowStepState::Active;
                activated.push(step.clone());
            }
        }
        self.state = WorkflowState::Running { steps, reply_to };
        Self::reply(reply_to, WorkflowOutcome::Started { activated })
    }

    fn complete(&mut self, step: K) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        let WorkflowState::Running { steps, reply_to } = &mut self.state else {
            return Actions::cont();
        };
        let reply_to = *reply_to;
        let Some(index) = steps.iter().position(|(candidate, _)| candidate == &step) else {
            return Self::reply(
                reply_to,
                WorkflowOutcome::Rejected(WorkflowRejection::UnknownStep { step }),
            );
        };
        match steps[index].1 {
            WorkflowStepState::Blocked => {
                return Self::reply(
                    reply_to,
                    WorkflowOutcome::Rejected(WorkflowRejection::Blocked { step }),
                );
            }
            WorkflowStepState::Completed => {
                return Self::reply(
                    reply_to,
                    WorkflowOutcome::Rejected(WorkflowRejection::AlreadyCompleted { step }),
                );
            }
            WorkflowStepState::Active => steps[index].1 = WorkflowStepState::Completed,
        }
        if steps
            .iter()
            .all(|(_, state)| *state == WorkflowStepState::Completed)
        {
            self.state = WorkflowState::Succeeded;
            return Self::reply(reply_to, WorkflowOutcome::Succeeded { completed: step });
        }
        let completed: Vec<_> = steps
            .iter()
            .filter(|(_, state)| *state == WorkflowStepState::Completed)
            .map(|(key, _)| key.clone())
            .collect();
        let mut activated = Vec::new();
        for (candidate, state) in steps
            .iter_mut()
            .filter(|(_, state)| *state == WorkflowStepState::Blocked)
        {
            let ready = self
                .definition
                .dependencies
                .iter()
                .filter(|(_, dependent)| dependent == candidate)
                .all(|(required, _)| completed.contains(required));
            if ready {
                *state = WorkflowStepState::Active;
                activated.push(candidate.clone());
            }
        }
        Self::reply(
            reply_to,
            WorkflowOutcome::Advanced {
                completed: step,
                activated,
            },
        )
    }

    fn fail(&mut self, step: K) -> Actions<A, Never, Vec<Delivery<Reply>>, NoBirths> {
        let WorkflowState::Running { steps, reply_to } = &self.state else {
            return Actions::cont();
        };
        let reply_to = *reply_to;
        let Some((_, phase)) = steps.iter().find(|(candidate, _)| candidate == &step) else {
            return Self::reply(
                reply_to,
                WorkflowOutcome::Rejected(WorkflowRejection::UnknownStep { step }),
            );
        };
        match phase {
            WorkflowStepState::Active => {
                self.state = WorkflowState::Failed { step: step.clone() };
                Self::reply(reply_to, WorkflowOutcome::Failed { step })
            }
            WorkflowStepState::Blocked => Self::reply(
                reply_to,
                WorkflowOutcome::Rejected(WorkflowRejection::Blocked { step }),
            ),
            WorkflowStepState::Completed => Self::reply(
                reply_to,
                WorkflowOutcome::Rejected(WorkflowRejection::AlreadyCompleted { step }),
            ),
        }
    }
}

fn validate<K: Clone + Eq>(
    definition: &WorkflowDefinition<K>,
) -> Result<(), WorkflowConfigError<K>> {
    if definition.steps.is_empty() {
        return Err(WorkflowConfigError::Empty);
    }
    for (index, step) in definition.steps.iter().enumerate() {
        if definition.steps[..index].contains(step) {
            return Err(WorkflowConfigError::DuplicateStep { step: step.clone() });
        }
    }
    for (required, dependent) in &definition.dependencies {
        if !definition.steps.contains(required) {
            return Err(WorkflowConfigError::UnknownPrerequisite {
                step: required.clone(),
            });
        }
        if !definition.steps.contains(dependent) {
            return Err(WorkflowConfigError::UnknownDependent {
                step: dependent.clone(),
            });
        }
        if required == dependent {
            return Err(WorkflowConfigError::SelfDependency {
                step: required.clone(),
            });
        }
    }
    let mut reached = Vec::new();
    while reached.len() < definition.steps.len() {
        let before = reached.len();
        for step in &definition.steps {
            if !reached.contains(step)
                && definition
                    .dependencies
                    .iter()
                    .filter(|(_, dependent)| dependent == step)
                    .all(|(required, _)| reached.contains(required))
            {
                reached.push(step.clone());
            }
        }
        if reached.len() == before {
            return Err(WorkflowConfigError::Cycle);
        }
    }
    Ok(())
}

impl<A, K, Reply> BehaviorBase for Workflow<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, K, Reply> behavior::Protocol for Workflow<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
{
    type Addr = A;
    type Msg = WorkflowMessage<K, Reply>;
}

impl<A, K, Reply> behavior::KeyedProtocol for Workflow<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, K, Reply> Behavior for Workflow<A, K, Reply>
where
    A: Address,
    K: Clone + Eq,
    Reply: behavior::Protocol<Addr = A, Msg = WorkflowOutcome<K>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            WorkflowMessage::Start { reply_to } => self.start(reply_to),
            WorkflowMessage::Complete { step } => self.complete(step),
            WorkflowMessage::Fail { step } => self.fail(step),
            WorkflowMessage::Cancel { reply_to } => match self.state {
                WorkflowState::Ready | WorkflowState::Running { .. } => {
                    self.state = WorkflowState::Cancelled;
                    Self::reply(reply_to, WorkflowOutcome::Cancelled)
                }
                _ => Self::reply(
                    reply_to,
                    WorkflowOutcome::Rejected(WorkflowRejection::Terminal { step: None }),
                ),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    struct Reply;
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = WorkflowOutcome<&'static str>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = Workflow<MailAddr, &'static str, Reply>;
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(9))
    }
    fn diamond() -> WorkflowDefinition<&'static str> {
        WorkflowDefinition {
            steps: vec!["root", "left", "right", "join"],
            dependencies: vec![
                ("root", "left"),
                ("root", "right"),
                ("left", "join"),
                ("right", "join"),
            ],
        }
    }

    #[test]
    fn validates_empty_duplicate_unknown_and_cyclic_graphs() {
        assert!(matches!(
            Subject::new(WorkflowDefinition {
                steps: vec![],
                dependencies: vec![]
            }),
            Err(WorkflowConfigError::Empty)
        ));
        assert!(matches!(
            Subject::new(WorkflowDefinition {
                steps: vec!["a", "a"],
                dependencies: vec![]
            }),
            Err(WorkflowConfigError::DuplicateStep { .. })
        ));
        assert!(matches!(
            Subject::new(WorkflowDefinition {
                steps: vec!["a"],
                dependencies: vec![("missing", "a")]
            }),
            Err(WorkflowConfigError::UnknownPrerequisite { .. })
        ));
        assert!(matches!(
            Subject::new(WorkflowDefinition {
                steps: vec!["a", "b"],
                dependencies: vec![("a", "b"), ("b", "a")]
            }),
            Err(WorkflowConfigError::Cycle)
        ));
    }

    #[test]
    fn diamond_activates_each_step_once_after_all_prerequisites() {
        let mut subject = (Subject::new(diamond()).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        let started = subject
            .receive(MailAddr(0), WorkflowMessage::Start { reply_to: reply() })
            .unwrap();
        assert!(
            matches!(&started.sends[0].message, WorkflowOutcome::Started { activated } if activated == &["root"])
        );
        let root = subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "root" })
            .unwrap();
        assert!(
            matches!(&root.sends[0].message, WorkflowOutcome::Advanced { activated, .. } if activated == &["left", "right"])
        );
        let left = subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "left" })
            .unwrap();
        assert!(
            matches!(&left.sends[0].message, WorkflowOutcome::Advanced { activated, .. } if activated.is_empty())
        );
        let right = subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "right" })
            .unwrap();
        assert!(
            matches!(&right.sends[0].message, WorkflowOutcome::Advanced { activated, .. } if activated == &["join"])
        );
        let finished = subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "join" })
            .unwrap();
        assert!(matches!(
            finished.sends[0].message,
            WorkflowOutcome::Succeeded { completed: "join" }
        ));
    }

    #[test]
    fn blocked_failure_and_duplicate_completion_are_atomic() {
        let mut subject = (Subject::new(diamond()).unwrap())
            .initialize()
            .unwrap()
            .behavior;
        subject
            .receive(MailAddr(0), WorkflowMessage::Start { reply_to: reply() })
            .unwrap();
        let blocked = subject
            .receive(MailAddr(0), WorkflowMessage::Fail { step: "join" })
            .unwrap();
        assert!(matches!(
            blocked.sends[0].message,
            WorkflowOutcome::Rejected(WorkflowRejection::Blocked { step: "join" })
        ));
        subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "root" })
            .unwrap();
        let duplicate = subject
            .receive(MailAddr(0), WorkflowMessage::Complete { step: "root" })
            .unwrap();
        assert!(matches!(
            duplicate.sends[0].message,
            WorkflowOutcome::Rejected(WorkflowRejection::AlreadyCompleted { step: "root" })
        ));
    }
}
