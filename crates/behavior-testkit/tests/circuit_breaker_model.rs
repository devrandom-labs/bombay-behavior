use std::num::NonZeroU32;
use std::time::Duration;

use behavior_actors::{
    Activate as _, BreakerAttempt, BreakerCompletion, BreakerError, BreakerMessage, BreakerOutcome,
    BreakerPhase, BreakerRejection, CircuitBreaker, ClosedPhase, ProbePhase, ScheduleAfter,
    TimerElapsed, TimerGeneration, TimerId,
};
use behavior_core::{MailAddr, Protocol, Recipient, Step};
use proptest::prelude::*;

struct Reply;

impl Protocol for Reply {
    type Addr = MailAddr;
    type Msg = BreakerOutcome;
}

#[derive(Clone, Copy, Debug)]
enum CompletionKind {
    Success,
    Failure,
}

#[derive(Clone, Copy, Debug)]
enum Instruction {
    Admit(u8),
    CompleteCurrent(CompletionKind),
    CompleteForeign(CompletionKind),
    ElapsedCurrent,
    ElapsedStale,
    ElapsedForeign,
}

#[derive(Clone, Copy, Debug)]
enum CircuitStage {
    Free {
        failures: u32,
    },
    Running {
        failures: u32,
        attempt: BreakerAttempt,
        owner: MailAddr,
    },
    Cooling {
        generation: TimerGeneration,
    },
    TrialAvailable {
        generation: TimerGeneration,
    },
    TrialRunning {
        generation: TimerGeneration,
        attempt: BreakerAttempt,
        owner: MailAddr,
    },
    Exhausted,
}

#[derive(Clone, Copy, Debug)]
enum CircuitInput {
    Admit(MailAddr),
    Complete(BreakerCompletion),
    Elapsed(TimerElapsed),
}

#[derive(Debug, PartialEq, Eq)]
struct CircuitEffects {
    replies: Vec<(MailAddr, BreakerOutcome)>,
    schedules: Vec<ScheduleAfter>,
}

impl CircuitEffects {
    fn empty() -> Self {
        Self {
            replies: Vec::new(),
            schedules: Vec::new(),
        }
    }

    fn reply(owner: MailAddr, outcome: BreakerOutcome) -> Self {
        Self {
            replies: vec![(owner, outcome)],
            schedules: Vec::new(),
        }
    }
}

struct CircuitOracle {
    stage: CircuitStage,
    next_attempt: u64,
}

impl CircuitOracle {
    fn new() -> Self {
        Self {
            stage: CircuitStage::Free { failures: 0 },
            next_attempt: 0,
        }
    }

    fn instruction(&self, instruction: Instruction) -> CircuitInput {
        match instruction {
            Instruction::Admit(owner) => CircuitInput::Admit(MailAddr(u64::from(owner))),
            Instruction::CompleteCurrent(kind) => {
                let attempt = match self.stage {
                    CircuitStage::Running { attempt, .. }
                    | CircuitStage::TrialRunning { attempt, .. } => attempt,
                    _ => BreakerAttempt(999),
                };
                CircuitInput::Complete(match kind {
                    CompletionKind::Success => BreakerCompletion::Succeeded { attempt },
                    CompletionKind::Failure => BreakerCompletion::Failed { attempt },
                })
            }
            Instruction::CompleteForeign(kind) => {
                let attempt = BreakerAttempt(999);
                CircuitInput::Complete(match kind {
                    CompletionKind::Success => BreakerCompletion::Succeeded { attempt },
                    CompletionKind::Failure => BreakerCompletion::Failed { attempt },
                })
            }
            Instruction::ElapsedCurrent => {
                let generation = match self.stage {
                    CircuitStage::Cooling { generation } => generation,
                    _ => TimerGeneration(999),
                };
                CircuitInput::Elapsed(TimerElapsed::new(TimerId(8), generation))
            }
            Instruction::ElapsedStale => {
                let generation = match self.stage {
                    CircuitStage::Cooling { generation } => TimerGeneration(generation.0 + 1),
                    _ => TimerGeneration(999),
                };
                CircuitInput::Elapsed(TimerElapsed::new(TimerId(8), generation))
            }
            Instruction::ElapsedForeign => {
                CircuitInput::Elapsed(TimerElapsed::new(TimerId(9), TimerGeneration(999)))
            }
        }
    }

    fn apply(&mut self, input: CircuitInput) -> Result<CircuitEffects, BreakerCompletion> {
        match input {
            CircuitInput::Admit(owner) => self.admit(owner),
            CircuitInput::Complete(completion) => self.complete(completion),
            CircuitInput::Elapsed(elapsed) => {
                if let CircuitStage::Cooling { generation } = self.stage
                    && elapsed.id == TimerId(8)
                    && elapsed.generation == generation
                {
                    self.stage = CircuitStage::TrialAvailable { generation };
                }
                Ok(CircuitEffects::empty())
            }
        }
    }

    fn admit(&mut self, owner: MailAddr) -> Result<CircuitEffects, BreakerCompletion> {
        let attempt = BreakerAttempt(self.next_attempt);
        let outcome = match self.stage {
            CircuitStage::Free { failures } => {
                self.next_attempt += 1;
                self.stage = CircuitStage::Running {
                    failures,
                    attempt,
                    owner,
                };
                BreakerOutcome::Admitted { attempt }
            }
            CircuitStage::TrialAvailable { generation } => {
                self.next_attempt += 1;
                self.stage = CircuitStage::TrialRunning {
                    generation,
                    attempt,
                    owner,
                };
                BreakerOutcome::ProbeAdmitted { attempt }
            }
            CircuitStage::Cooling { generation } => {
                BreakerOutcome::Rejected(BreakerRejection::Open { generation })
            }
            CircuitStage::Running { .. } | CircuitStage::TrialRunning { .. } => {
                BreakerOutcome::Rejected(BreakerRejection::Busy)
            }
            CircuitStage::Exhausted => BreakerOutcome::Rejected(BreakerRejection::Exhausted),
        };
        Ok(CircuitEffects::reply(owner, outcome))
    }

    fn complete(
        &mut self,
        completion: BreakerCompletion,
    ) -> Result<CircuitEffects, BreakerCompletion> {
        let reported = match completion {
            BreakerCompletion::Succeeded { attempt } | BreakerCompletion::Failed { attempt } => {
                attempt
            }
        };
        let (owner, failures, trial_generation) = match self.stage {
            CircuitStage::Running {
                owner,
                failures,
                attempt,
            } if attempt == reported => (owner, failures, None),
            CircuitStage::TrialRunning {
                owner,
                generation,
                attempt,
            } if attempt == reported => (owner, 0, Some(generation)),
            _ => return Err(completion),
        };
        match completion {
            BreakerCompletion::Succeeded { attempt } => {
                self.stage = CircuitStage::Free { failures: 0 };
                Ok(CircuitEffects::reply(
                    owner,
                    BreakerOutcome::Succeeded { attempt },
                ))
            }
            BreakerCompletion::Failed { attempt } => {
                let failures = failures + 1;
                if let None = trial_generation
                    && failures < 2
                {
                    self.stage = CircuitStage::Free { failures };
                    return Ok(CircuitEffects::reply(
                        owner,
                        BreakerOutcome::FailureRecorded {
                            attempt,
                            consecutive_failures: failures,
                        },
                    ));
                }
                let generation = match trial_generation {
                    None => TimerGeneration(0),
                    Some(TimerGeneration(previous)) => {
                        let Some(next) = previous.checked_add(1) else {
                            self.stage = CircuitStage::Exhausted;
                            return Ok(CircuitEffects::reply(
                                owner,
                                BreakerOutcome::Rejected(BreakerRejection::Exhausted),
                            ));
                        };
                        TimerGeneration(next)
                    }
                };
                self.stage = CircuitStage::Cooling { generation };
                Ok(CircuitEffects {
                    replies: vec![(
                        owner,
                        BreakerOutcome::Opened {
                            attempt,
                            generation,
                        },
                    )],
                    schedules: vec![ScheduleAfter::new(
                        TimerId(8),
                        generation,
                        Duration::from_secs(1),
                    )],
                })
            }
        }
    }
}

fn assert_stage(actual: &BreakerPhase<Recipient<Reply>>, expected: CircuitStage) {
    match (actual, expected) {
        (
            BreakerPhase::Closed(ClosedPhase::Idle {
                consecutive_failures,
            }),
            CircuitStage::Free { failures },
        ) => assert_eq!(*consecutive_failures, failures),
        (
            BreakerPhase::Closed(ClosedPhase::Awaiting {
                consecutive_failures,
                attempt,
                reply_to,
            }),
            CircuitStage::Running {
                failures,
                attempt: expected_attempt,
                owner,
            },
        ) => {
            assert_eq!(*consecutive_failures, failures);
            assert_eq!(*attempt, expected_attempt);
            assert_eq!(reply_to.address(), owner);
        }
        (
            BreakerPhase::Open { generation },
            CircuitStage::Cooling {
                generation: expected_generation,
            },
        ) => assert_eq!(*generation, expected_generation),
        (
            BreakerPhase::Probing {
                generation,
                phase: ProbePhase::Available,
            },
            CircuitStage::TrialAvailable {
                generation: expected_generation,
            },
        ) => assert_eq!(*generation, expected_generation),
        (
            BreakerPhase::Probing {
                generation,
                phase: ProbePhase::Awaiting { attempt, reply_to },
            },
            CircuitStage::TrialRunning {
                generation: expected_generation,
                attempt: expected_attempt,
                owner,
            },
        ) => {
            assert_eq!(*generation, expected_generation);
            assert_eq!(*attempt, expected_attempt);
            assert_eq!(reply_to.address(), owner);
        }
        (BreakerPhase::Exhausted, CircuitStage::Exhausted) => {}
        _ => panic!("circuit phase differs from its independent expected trace"),
    }
}

fn exercise(instructions: Vec<Instruction>) {
    let initialized = CircuitBreaker::<MailAddr, Recipient<Reply>>::new(
        NonZeroU32::new(2).expect("positive threshold"),
        Duration::from_secs(1),
        TimerId(8),
    )
    .expect("positive reset delay")
    .initialize()
    .expect("empty initialization succeeds");
    assert!(initialized.actions.sends.replies.is_empty());
    assert!(initialized.actions.sends.schedules.is_empty());
    assert!(initialized.actions.creates.is_empty());
    assert_eq!(initialized.actions.become_, Step::Continue);
    let mut breaker = initialized.behavior;
    let mut oracle = CircuitOracle::new();

    for instruction in instructions {
        let input = oracle.instruction(instruction);
        let expected = oracle.apply(input);
        let actual = match input {
            CircuitInput::Admit(owner) => breaker.receive(
                MailAddr(0),
                BreakerMessage::Admit {
                    reply_to: Recipient::global(owner),
                },
            ),
            CircuitInput::Complete(BreakerCompletion::Succeeded { attempt }) => {
                breaker.receive(MailAddr(0), BreakerMessage::Succeeded { attempt })
            }
            CircuitInput::Complete(BreakerCompletion::Failed { attempt }) => {
                breaker.receive(MailAddr(0), BreakerMessage::Failed { attempt })
            }
            CircuitInput::Elapsed(elapsed) => breaker.on_path(elapsed),
        };
        match (expected, actual) {
            (Ok(expected), Ok(actions)) => {
                let replies = actions
                    .sends
                    .replies
                    .into_iter()
                    .map(|delivery| (delivery.to.address(), delivery.message))
                    .collect::<Vec<_>>();
                assert_eq!(replies, expected.replies);
                assert_eq!(actions.sends.schedules.as_slice(), expected.schedules);
                assert!(actions.creates.is_empty());
                assert_eq!(actions.become_, Step::Continue);
            }
            (Err(completion), Err(error)) => {
                assert_eq!(error, BreakerError::UnexpectedCompletion(completion));
            }
            _ => panic!("circuit command acceptance differs from the expected trace"),
        }
        assert_stage(breaker.phase(), oracle.stage);
    }
}

fn instructions() -> impl Strategy<Value = Vec<Instruction>> {
    let completion = prop_oneof![Just(CompletionKind::Success), Just(CompletionKind::Failure),];
    let instruction = prop_oneof![
        4 => (1_u8..6).prop_map(Instruction::Admit),
        3 => completion.clone().prop_map(Instruction::CompleteCurrent),
        1 => completion.prop_map(Instruction::CompleteForeign),
        2 => Just(Instruction::ElapsedCurrent),
        1 => Just(Instruction::ElapsedStale),
        1 => Just(Instruction::ElapsedForeign),
    ];
    proptest::collection::vec(instruction, 1..80)
}

proptest! {
    #[test]
    fn generated_circuit_commands_match_single_flight_and_reset_policy(trace in instructions()) {
        exercise(trace);
    }
}

#[test]
fn opening_and_reopening_use_distinct_timer_generations() {
    exercise(vec![
        Instruction::Admit(1),
        Instruction::CompleteCurrent(CompletionKind::Failure),
        Instruction::Admit(2),
        Instruction::CompleteCurrent(CompletionKind::Failure),
        Instruction::ElapsedStale,
        Instruction::ElapsedForeign,
        Instruction::ElapsedCurrent,
        Instruction::Admit(3),
        Instruction::CompleteForeign(CompletionKind::Success),
        Instruction::CompleteCurrent(CompletionKind::Failure),
        Instruction::ElapsedCurrent,
        Instruction::Admit(4),
        Instruction::CompleteCurrent(CompletionKind::Success),
    ]);
}
