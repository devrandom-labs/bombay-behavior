//! Independent sequence model for conserving one selected terminal outcome.

use std::time::Instant;

use behavior::atomic::RestartReleaseFailure;
use behavior::{
    Actions, Activate as _, AllocationRejection, Behavior, BehaviorActed, ChildStopped, Crash,
    CreationRejection, CreationSequence, EventLayer, Exit, MailAddr, Never, NoBirths,
    PropagateTermination, ReportTerminalOutcome, RestartDenial, Step, SupervisionFailureReason,
    TerminalDisposition, TerminalOutcome, TerminalPropagationState, TerminationPropagationError,
    User, propagate_abnormal, propagate_all,
};
use proptest::prelude::any;
use proptest::{prop_assert, prop_assert_eq, proptest};

struct Domain;

impl foundation::Protocol for Domain {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Domain {
    type Protocol = Self;
    type Event = User<MailAddr, u8>;
    type Sends = Vec<u8>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::send(vec![event.message]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExpectedState {
    Listening,
    Discharged,
    Published,
}

#[derive(Clone, Copy, Debug)]
enum Policy {
    EveryOutcome,
    AbnormalOnly,
}

impl Policy {
    const fn from_tag(tag: u8) -> Self {
        match tag % 2 {
            0 => Self::EveryOutcome,
            _ => Self::AbnormalOnly,
        }
    }

    fn disposition(self, outcome: &TerminalOutcome<MailAddr>) -> TerminalDisposition {
        match self {
            Self::EveryOutcome => TerminalDisposition::Propagate,
            Self::AbnormalOnly => propagate_abnormal(outcome),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Input {
    OtherChild(TerminalOutcome<MailAddr>),
    SelectedChild(TerminalOutcome<MailAddr>),
    Message { sender: MailAddr, payload: u8 },
}

impl Input {
    fn from_parts(kind: u8, outcome: TerminalOutcome<MailAddr>, detail: u64) -> Self {
        match kind % 3 {
            0 => Self::OtherChild(outcome),
            1 => Self::SelectedChild(outcome),
            _ => Self::Message {
                sender: MailAddr(detail),
                payload: kind.wrapping_add(detail as u8),
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Prediction {
    ReturnStop(ExpectedState),
    ContinueSilently(ExpectedState),
    Reply(ExpectedState, u8),
    Publish(TerminalOutcome<MailAddr>),
}

fn predict(current: ExpectedState, policy: Policy, input: Input) -> Prediction {
    match (current, input) {
        (_, Input::Message { payload, .. }) => Prediction::Reply(current, payload),
        (ExpectedState::Listening, Input::SelectedChild(outcome)) => {
            match policy.disposition(&outcome) {
                TerminalDisposition::Discharge => {
                    Prediction::ContinueSilently(ExpectedState::Discharged)
                }
                TerminalDisposition::Propagate => Prediction::Publish(outcome),
            }
        }
        (state, Input::OtherChild(_) | Input::SelectedChild(_)) => Prediction::ReturnStop(state),
    }
}

fn terminal_outcome(tag: u8, detail: u64) -> TerminalOutcome<MailAddr> {
    match tag % 22 {
        0 => Ok(Exit::Normal),
        1 => Ok(Exit::Collected),
        2 => Ok(Exit::LinkDied(MailAddr(detail))),
        3 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildStopped,
        )),
        4 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                restarts_in_window: detail as usize,
                replacements_requested: detail.wrapping_add(1) as usize,
                maximum_restarts: detail as u32,
            }),
        )),
        5 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::ReleaseRejected(
                RestartReleaseFailure::DurationOverflow,
            )),
        )),
        6 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::AttemptSequenceExhausted),
        )),
        7 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::TimerGenerationExhausted),
        )),
        8 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::RestartDenied(RestartDenial::TimerIdentityExhausted),
        )),
        9 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(CreationRejection::Allocation(
                AllocationRejection::Exhausted,
            )),
        )),
        10 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(CreationRejection::Allocation(
                AllocationRejection::AddressAlreadyClaimed,
            )),
        )),
        11 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(
                CreationRejection::InitializationFailed,
            ),
        )),
        12 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(
                CreationRejection::EnvironmentFailed,
            ),
        )),
        13 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(
                AllocationRejection::Exhausted,
            )),
        )),
        14 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(CreationRejection::Allocation(
                AllocationRejection::AddressAlreadyClaimed,
            )),
        )),
        15 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(
                CreationRejection::InitializationFailed,
            ),
        )),
        16 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(CreationRejection::EnvironmentFailed),
        )),
        17 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerFactoryRejected,
        )),
        18 => Err(Crash::Failed),
        19 => Err(Crash::EnvironmentFailed),
        20 => Err(Crash::Panicked),
        _ => Err(Crash::Cancelled),
    }
}

fn expected_state(state: TerminalPropagationState) -> ExpectedState {
    match state {
        TerminalPropagationState::Observing => ExpectedState::Listening,
        TerminalPropagationState::Discharged => ExpectedState::Discharged,
        TerminalPropagationState::Propagated => ExpectedState::Published,
    }
}

proptest! {
    #[test]
    fn arbitrary_termination_sequences_match_an_independent_model(
        policy_tag in any::<u8>(),
        inputs in proptest::collection::vec((any::<u8>(), any::<u8>(), any::<u64>()), 0..96),
    ) {
        let policy = Policy::from_tag(policy_tag);
        let policy_function = match policy {
            Policy::EveryOutcome => propagate_all,
            Policy::AbnormalOnly => propagate_abnormal,
        };
        let mut creation_ids = CreationSequence::new();
        let selected_child = creation_ids.issue().expect("the selected child ID exists");
        let other_child = creation_ids.issue().expect("the other child ID exists");
        let mut subject = PropagateTermination::new(
            Domain,
            behavior::ChildTermination::<Domain, behavior::ChildHead>::new(selected_child),
            policy_function,
        )
        .initialize()
        .unwrap()
        .behavior;
        let mut model = ExpectedState::Listening;

        for (kind, outcome_tag, detail) in inputs {
            let input = Input::from_parts(kind, terminal_outcome(outcome_tag, detail), detail);
            let prediction = predict(model, policy, input);

            match (input, prediction) {
                (Input::Message { sender, payload }, Prediction::Reply(next, expected)) => {
                    let actions = subject
                        .transition(EventLayer::Inner(User::new(sender, payload)))
                        .unwrap();
                    prop_assert_eq!(actions.sends.inner, [expected]);
                    prop_assert!(actions.sends.owned.reports.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    model = next;
                }
                (Input::SelectedChild(outcome), Prediction::Publish(expected)) => {
                    let actions = subject
                        .transition(EventLayer::Owned(ChildStopped::new(
                            selected_child,
                            outcome,
                            Instant::now(),
                        )))
                        .unwrap();
                    prop_assert_eq!(outcome, expected);
                    prop_assert_eq!(
                        actions.sends.owned.reports.as_slice(),
                        [ReportTerminalOutcome::new(expected)]
                    );
                    prop_assert!(actions.sends.inner.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Stop(_)));
                    model = ExpectedState::Published;
                }
                (Input::SelectedChild(outcome), Prediction::ContinueSilently(next)) => {
                    let actions = subject
                        .transition(EventLayer::Owned(ChildStopped::new(
                            selected_child,
                            outcome,
                            Instant::now(),
                        )))
                        .unwrap();
                    prop_assert!(actions.sends.owned.reports.is_empty());
                    prop_assert!(actions.sends.inner.is_empty());
                    prop_assert!(matches!(actions.become_, Step::Continue));
                    model = next;
                }
                (Input::OtherChild(outcome), Prediction::ReturnStop(expected)) => {
                    let stopped = ChildStopped::new(other_child, outcome, Instant::now());
                    let returned = matches!(
                        subject.transition(EventLayer::Owned(stopped.clone())),
                        Err(TerminationPropagationError::UnexpectedReport {
                            state,
                            report: returned,
                        }) if expected_state(state) == expected && returned == stopped
                    );
                    prop_assert!(returned);
                }
                (Input::SelectedChild(outcome), Prediction::ReturnStop(expected)) => {
                    let stopped = ChildStopped::new(selected_child, outcome, Instant::now());
                    let returned = matches!(
                        subject.transition(EventLayer::Owned(stopped.clone())),
                        Err(TerminationPropagationError::UnexpectedReport {
                            state,
                            report: returned,
                        }) if expected_state(state) == expected && returned == stopped
                    );
                    prop_assert!(returned);
                }
                _ => prop_assert!(false, "model produced an incompatible prediction"),
            }

            prop_assert_eq!(expected_state(subject.state()), model);
        }
    }
}
