#![no_main]
//! Stateful delayed recovery through one actual stopped search service.

mod fixed_supervisor;
mod fixed_supervisor_recovery;
mod stable_proxy;

use core::ops::ControlFlow;
use std::time::Duration;

use behavior::atomic::{
    CapabilityResult, DiagnosticAction, FixedDiagnostic, FixedSupervisorEvent, ImmediateActivation,
    Recovery, RestartLimit, RestartRelease, Strategy, UnavailablePhase, WorkerSource,
    WorkerSubmission,
};
use behavior::{
    ItemSettlement, Never, NoSends, ScheduleAfterRejection, SendSettlements, SettledItem, Step,
    TimerElapsed, TimerGeneration, TimerId, TimerScheduled,
};
use fixed_supervisor::Role;
use fixed_supervisor_recovery::{search_capability, search_recovery};
use libfuzzer_sys::fuzz_target;
use stable_proxy::Worker;

struct DelayedWorkshop;

impl WorkerSource<Role, Worker, ImmediateActivation> for DelayedWorkshop {
    type WorkerRejection = Never;
    type SourceRejection = Never;
}

#[derive(Clone, Copy)]
enum DelayInput {
    ScheduleAccepted,
    TimerElapsed,
    OtherScheduleAccepted,
    OtherTimerElapsed,
    ScheduleRejected,
}

impl DelayInput {
    const fn from_byte(byte: u8) -> Self {
        match byte % 5 {
            0 => Self::ScheduleAccepted,
            1 => Self::TimerElapsed,
            2 => Self::OtherScheduleAccepted,
            3 => Self::OtherTimerElapsed,
            _ => Self::ScheduleRejected,
        }
    }
}

#[derive(Clone, Copy)]
enum ExpectedService {
    ScheduleReceiptRequired,
    TimerRequired,
    ReplacementIssued,
    SupervisorStopped,
}

impl ExpectedService {
    const fn observe(self, input: DelayInput) -> Self {
        match (self, input) {
            (Self::ScheduleReceiptRequired, DelayInput::ScheduleAccepted) => Self::TimerRequired,
            (Self::TimerRequired, DelayInput::TimerElapsed) => Self::ReplacementIssued,
            (Self::SupervisorStopped, _) => Self::SupervisorStopped,
            (Self::ScheduleReceiptRequired, DelayInput::TimerElapsed)
            | (Self::ScheduleReceiptRequired, DelayInput::OtherScheduleAccepted)
            | (Self::ScheduleReceiptRequired, DelayInput::OtherTimerElapsed)
            | (Self::ScheduleReceiptRequired, DelayInput::ScheduleRejected)
            | (Self::TimerRequired, DelayInput::ScheduleAccepted)
            | (Self::TimerRequired, DelayInput::OtherScheduleAccepted)
            | (Self::TimerRequired, DelayInput::OtherTimerElapsed)
            | (Self::TimerRequired, DelayInput::ScheduleRejected)
            | (Self::ReplacementIssued, _) => Self::SupervisorStopped,
        }
    }
}

fn exercise(inputs: &[u8]) {
    let delay = Duration::from_secs(3);
    let recovering = search_recovery(Recovery::permanent(
        DelayedWorkshop,
        Strategy::OneForOne,
        RestartLimit::new(3, Duration::from_secs(60)),
        RestartRelease::constant(delay).expect("the delay is positive"),
    ));
    let mut supervisor = recovering.supervisor;
    let proxy_actor = recovering.proxy;
    let proxy_id = recovering.proxy_id;
    drop(recovering.previous);
    let preparation = match recovering
        .preparation
        .accept(WorkerSubmission::immediate(Worker::new(1)))
    {
        ControlFlow::Break(preparation) => preparation,
        ControlFlow::Continue(_) => panic!("one role needs one worker submission"),
    };
    let scheduled = supervisor
        .transition(FixedSupervisorEvent::WorkerPreparationSettled(
            SettledItem::Attempted(ItemSettlement::Accepted(preparation)),
        ))
        .unwrap_or_else(|_| panic!("prepared delayed recovery emits one schedule"));
    assert!(scheduled.creates.is_empty());
    let observations = scheduled.sends.proxy_observations.into_requests();
    assert!(observations.is_empty());
    let preparations = scheduled
        .sends
        .worker_preparations
        .unattempted()
        .into_inputs();
    assert!(preparations.is_empty());
    let operations = scheduled.sends.proxy_operations.unattempted().into_inputs();
    assert!(operations.is_empty());
    let schedule = match scheduled
        .sends
        .restart_schedules
        .unattempted()
        .into_inputs()
        .pop()
        .expect("one delayed recovery schedule is emitted")
    {
        SettledItem::Unattempted(schedule) => schedule,
        SettledItem::Attempted(_) => panic!("the schedule has not been interpreted"),
    };
    assert_eq!(schedule.after, delay);
    let NoSends = scheduled.sends.lifecycle;
    assert!(scheduled.sends.status_replies.into_deliveries().is_empty());
    let capabilities = scheduled.sends.capability_replies.into_deliveries();
    assert!(capabilities.is_empty());
    assert!(scheduled.sends.diagnostics.is_empty());
    assert!(matches!(scheduled.become_, Step::Continue));

    let other_id = TimerId(schedule.id.0 + 1);
    let other_generation = TimerGeneration(schedule.generation.0 + 1);
    let mut expected_service = ExpectedService::ScheduleReceiptRequired;

    for byte in inputs {
        match expected_service {
            ExpectedService::SupervisorStopped => break,
            ExpectedService::ScheduleReceiptRequired
            | ExpectedService::TimerRequired
            | ExpectedService::ReplacementIssued => {}
        }
        let service_before = expected_service;
        let input = DelayInput::from_byte(*byte);
        let action = match input {
            DelayInput::ScheduleAccepted => {
                supervisor.transition(FixedSupervisorEvent::RestartScheduleSettled(
                    SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                        id: schedule.id,
                        generation: schedule.generation,
                    })),
                ))
            }
            DelayInput::TimerElapsed => {
                supervisor.transition(FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(
                    schedule.id,
                    schedule.generation,
                )))
            }
            DelayInput::OtherScheduleAccepted => {
                supervisor.transition(FixedSupervisorEvent::RestartScheduleSettled(
                    SettledItem::Attempted(ItemSettlement::Accepted(TimerScheduled {
                        id: other_id,
                        generation: other_generation,
                    })),
                ))
            }
            DelayInput::OtherTimerElapsed => supervisor.transition(
                FixedSupervisorEvent::RestartElapsed(TimerElapsed::new(other_id, other_generation)),
            ),
            DelayInput::ScheduleRejected => {
                supervisor.transition(FixedSupervisorEvent::RestartScheduleSettled(
                    SettledItem::Attempted(ItemSettlement::Rejected {
                        item: schedule,
                        reason: ScheduleAfterRejection::DeadlineOverflow,
                    }),
                ))
            }
        }
        .unwrap_or_else(|_| panic!("every modeled timer input is total"));
        expected_service = expected_service.observe(input);

        assert!(action.creates.is_empty());
        assert!(action.sends.proxy_observations.into_requests().is_empty());
        let preparations = action.sends.worker_preparations.unattempted().into_inputs();
        assert!(preparations.is_empty());
        let schedules = action.sends.restart_schedules.unattempted().into_inputs();
        assert!(schedules.is_empty());
        let NoSends = action.sends.lifecycle;
        assert!(action.sends.status_replies.into_deliveries().is_empty());
        assert!(action.sends.capability_replies.into_deliveries().is_empty());
        let mut replacements = action.sends.proxy_operations.unattempted().into_inputs();
        let mut diagnostics = action.sends.diagnostics.into_requests();

        match expected_service {
            ExpectedService::TimerRequired => {
                assert!(replacements.is_empty());
                assert!(diagnostics.is_empty());
                assert!(matches!(action.become_, Step::Continue));
                assert!(matches!(
                    search_capability(&mut supervisor),
                    CapabilityResult::Unavailable {
                        phase: UnavailablePhase::Recovering,
                        ..
                    }
                ));
            }
            ExpectedService::ReplacementIssued => {
                let replacement = match replacements
                    .pop()
                    .expect("the exact timer releases one replacement")
                {
                    SettledItem::Unattempted(replacement) => replacement,
                    SettledItem::Attempted(_) => {
                        panic!("the replacement has not been interpreted")
                    }
                };
                assert_eq!(replacement.creation(), proxy_id);
                assert!(replacements.is_empty());
                assert!(diagnostics.is_empty());
                assert!(matches!(action.become_, Step::Continue));
            }
            ExpectedService::SupervisorStopped => {
                assert!(replacements.is_empty());
                assert!(matches!(action.become_, Step::Stop(_)));
                let diagnostic = diagnostics
                    .pop()
                    .expect("one exact timer diagnostic enters terminal custody");
                assert!(diagnostics.is_empty());
                let DiagnosticAction::Terminal { diagnostic } = diagnostic;
                match diagnostic {
                    FixedDiagnostic::RestartScheduleFailed(failure) => {
                        assert!(matches!(
                            service_before,
                            ExpectedService::ScheduleReceiptRequired
                        ));
                        assert!(matches!(input, DelayInput::ScheduleRejected));
                        assert_eq!(failure.role(), &Role::Search);
                        assert_eq!(failure.request(), schedule);
                        assert_eq!(failure.reason(), ScheduleAfterRejection::DeadlineOverflow);
                    }
                    FixedDiagnostic::UnexpectedInput { input: returned } => match (input, returned)
                    {
                        (
                            DelayInput::ScheduleAccepted,
                            FixedSupervisorEvent::RestartScheduleSettled(SettledItem::Attempted(
                                ItemSettlement::Accepted(receipt),
                            )),
                        ) => assert_eq!(
                            receipt,
                            TimerScheduled {
                                id: schedule.id,
                                generation: schedule.generation,
                            }
                        ),
                        (
                            DelayInput::OtherScheduleAccepted,
                            FixedSupervisorEvent::RestartScheduleSettled(SettledItem::Attempted(
                                ItemSettlement::Accepted(receipt),
                            )),
                        ) => assert_eq!(
                            receipt,
                            TimerScheduled {
                                id: other_id,
                                generation: other_generation,
                            }
                        ),
                        (
                            DelayInput::TimerElapsed,
                            FixedSupervisorEvent::RestartElapsed(elapsed),
                        ) => {
                            assert_eq!(elapsed, TimerElapsed::new(schedule.id, schedule.generation))
                        }
                        (
                            DelayInput::OtherTimerElapsed,
                            FixedSupervisorEvent::RestartElapsed(elapsed),
                        ) => assert_eq!(elapsed, TimerElapsed::new(other_id, other_generation)),
                        (
                            DelayInput::ScheduleRejected,
                            FixedSupervisorEvent::RestartScheduleSettled(SettledItem::Attempted(
                                ItemSettlement::Rejected { item, reason },
                            )),
                        ) => {
                            assert_eq!(item, schedule);
                            assert_eq!(reason, ScheduleAfterRejection::DeadlineOverflow);
                        }
                        _ => panic!("the diagnostic must retain the complete timer input"),
                    },
                    FixedDiagnostic::ProxyOutcomeFailed(_)
                    | FixedDiagnostic::ProxyInputRejected(_)
                    | FixedDiagnostic::WorkerPreparationFailed(_)
                    | FixedDiagnostic::RecoveryDenied(_)
                    | FixedDiagnostic::WorkerUnavailable(_) => {
                        panic!("timer input cannot become another diagnostic")
                    }
                }
            }
            ExpectedService::ScheduleReceiptRequired => {
                panic!("every generated input advances or stops delayed recovery")
            }
        }
    }

    drop(proxy_actor);
}

fuzz_target!(|inputs: &[u8]| {
    exercise(inputs);
});
