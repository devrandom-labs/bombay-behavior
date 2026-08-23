//! Independent sequence model for conservation of one selected terminal fact.

use std::time::Instant;

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, ChildStopped, Crash, CreationRejection,
    EventLayer, Exit, MailAddr, Never, NoBirths, PropagateTermination, ReportTerminalOutcome,
    RestartDenial, Step, SupervisionFailureReason, TerminalOutcome, TerminalPropagationState, User,
    propagate_abnormal, propagate_all,
};
use proptest::prelude::*;

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
enum OraclePhase {
    Waiting,
    Quiet,
    Published,
}

#[derive(Clone, Copy, Debug)]
enum Rule {
    Every,
    Abnormal,
}

impl Rule {
    fn publishes(self, outcome: TerminalOutcome<MailAddr>) -> bool {
        match self {
            Self::Every => true,
            Self::Abnormal => !matches!(outcome, Ok(Exit::Normal | Exit::Collected)),
        }
    }
}

fn outcome(tag: u8, detail: u64) -> TerminalOutcome<MailAddr> {
    match tag % 15 {
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
            SupervisionFailureReason::StableChildCreationRejected(
                CreationRejection::NonceAlreadyBound,
            ),
        )),
        6 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(
                CreationRejection::InitializationFailed,
            ),
        )),
        7 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::StableChildCreationRejected(
                CreationRejection::EnvironmentFailed,
            ),
        )),
        8 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(CreationRejection::NonceAlreadyBound),
        )),
        9 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(
                CreationRejection::InitializationFailed,
            ),
        )),
        10 => Ok(Exit::SupervisionFailed(
            SupervisionFailureReason::WorkerCreationRejected(CreationRejection::EnvironmentFailed),
        )),
        11 => Err(Crash::Failed),
        12 => Err(Crash::EnvironmentFailed),
        13 => Err(Crash::Panicked),
        _ => Err(Crash::Cancelled),
    }
}

proptest! {
    #[test]
    fn arbitrary_fact_sequences_match_an_independent_single_consumption_model(
        every in any::<bool>(),
        operations in prop::collection::vec((0_u8..3, any::<u8>(), any::<u64>()), 0..96),
    ) {
        let rule = if every { Rule::Every } else { Rule::Abnormal };
        let policy = if every { propagate_all } else { propagate_abnormal };
        let mut subject = PropagateTermination::new(
            Domain,
            behavior::ChildTermination::<MailAddr, behavior::ChildHead>::new(7),
            policy,
        )
        .initialize()
        .unwrap()
        .behavior;
        let mut oracle = OraclePhase::Waiting;

        for (kind, tag, detail) in operations {
            let terminal = outcome(tag, detail);
            let actions = match kind {
                0 => subject.transition(EventLayer::Owned(ChildStopped::new(
                    8,
                    terminal,
                    Instant::now(),
                ))).unwrap(),
                1 => subject.transition(EventLayer::Owned(ChildStopped::new(
                    7,
                    terminal,
                    Instant::now(),
                ))).unwrap(),
                _ => subject.transition(EventLayer::Inner(User::new(
                    MailAddr(detail),
                    tag,
                ))).unwrap(),
            };

            let expected_report = kind == 1
                && oracle == OraclePhase::Waiting
                && rule.publishes(terminal);
            let expected_stop = expected_report;
            let expected_domain_send = if kind == 2 { vec![tag] } else { Vec::new() };

            prop_assert_eq!(&actions.sends.inner, &expected_domain_send);
            prop_assert_eq!(actions.sends.owned.reports.as_slice(), if expected_report {
                &[ReportTerminalOutcome::new(terminal)][..]
            } else {
                &[]
            });
            prop_assert_eq!(matches!(actions.become_, Step::Stop(_)), expected_stop);

            if kind == 1 && oracle == OraclePhase::Waiting {
                oracle = if expected_report {
                    OraclePhase::Published
                } else {
                    OraclePhase::Quiet
                };
            }
            let actual = match subject.state() {
                TerminalPropagationState::Observing => OraclePhase::Waiting,
                TerminalPropagationState::Discharged => OraclePhase::Quiet,
                TerminalPropagationState::Propagated => OraclePhase::Published,
            };
            prop_assert_eq!(actual, oracle);
        }
    }
}
