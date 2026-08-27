//! Independent state-machine attacks for workflow coordination templates.

use behavior::{
    Actions, Activate as _, Barrier, BarrierError, BarrierGeneration, BarrierMembership,
    BarrierMessage, BarrierReleased, BarrierState, Behavior, BehaviorActed, Latch, LatchMessage,
    LatchState, MailAddr, MessageProtocol, Never, NoBirths, Recipient, User, Workflow,
    WorkflowDefinition, WorkflowError, WorkflowInput, WorkflowMessage, WorkflowOutcome,
    WorkflowRejection, WorkflowState, WorkflowStepState,
};
use proptest::collection::vec;
use proptest::prelude::*;

struct WorkflowReply;
impl behavior::Protocol for WorkflowReply {
    type Addr = MailAddr;
    type Msg = WorkflowOutcome<u8>;
}
impl Behavior for WorkflowReply {
    type Protocol = Self;
    type Event = User<MailAddr, WorkflowOutcome<u8>>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

type TestBarrier = Barrier<MailAddr, u8, Recipient<MessageProtocol<MailAddr, BarrierReleased>>>;
type TestLatch = Latch<MailAddr, Recipient<MessageProtocol<MailAddr, behavior::LatchReleased>>>;
type TestWorkflow = Workflow<MailAddr, u8, Recipient<WorkflowReply>>;

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 384,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn cyclic_barrier_matches_generation_and_arrival_order(
        operations in vec((0_u8..5, 0_u8..6, 0_u8..8), 0..220),
    ) {
        let mut actual = TestBarrier::new(BarrierMembership::new(vec![0, 1, 2]).unwrap())
            .initialize().unwrap().behavior;
        let mut generation = 0_u64;
        let mut arrivals: Vec<(u8, Recipient<MessageProtocol<MailAddr, BarrierReleased>>)> = Vec::new();

        for (participant, generation_seed, address) in operations {
            let observed = u64::from(generation_seed);
            let reply = Recipient::global(MailAddr(u64::from(address)));
            let before = arrivals.clone();
            let result = actual.receive(MailAddr(9), BarrierMessage {
                generation: BarrierGeneration(observed), participant, reply_to: reply,
            });
            if participant >= 3 {
                let exact = matches!(result, Err(BarrierError::UnknownParticipant { participant: returned, reply_to }) if returned == participant && reply_to == reply);
                prop_assert!(exact);
                prop_assert_eq!(&arrivals, &before);
            } else if observed < generation {
                let matched = matches!(result, Err(BarrierError::StaleGeneration { participant: returned, reply_to, .. }) if returned == participant && reply_to == reply);
                prop_assert!(matched);
                prop_assert_eq!(&arrivals, &before);
            } else if observed > generation {
                let matched = matches!(result, Err(BarrierError::FutureGeneration { participant: returned, reply_to, .. }) if returned == participant && reply_to == reply);
                prop_assert!(matched);
                prop_assert_eq!(&arrivals, &before);
            } else if arrivals.iter().any(|(arrived, _)| *arrived == participant) {
                let matched = matches!(result, Err(BarrierError::DuplicateArrival { participant: returned, reply_to, .. }) if returned == participant && reply_to == reply);
                prop_assert!(matched);
                prop_assert_eq!(&arrivals, &before);
            } else {
                arrivals.push((participant, reply));
                let actions = result.unwrap();
                if arrivals.len() == 3 {
                    let recipients = actions.sends.iter().map(|delivery| delivery.to).collect::<Vec<_>>();
                    prop_assert_eq!(recipients, arrivals.iter().map(|(_, route)| *route).collect::<Vec<_>>());
                    prop_assert!(actions.sends.iter().all(|delivery| delivery.message.generation == BarrierGeneration(generation)));
                    arrivals.clear();
                    generation += 1;
                } else {
                    prop_assert!(actions.sends.is_empty());
                }
            }
            match actual.state() {
                BarrierState::Gathering { generation: actual_generation, arrivals: actual_arrivals } => {
                    prop_assert_eq!(*actual_generation, BarrierGeneration(generation));
                    prop_assert_eq!(actual_arrivals.iter().map(|arrival| (arrival.participant, arrival.reply_to)).collect::<Vec<_>>(), arrivals.clone());
                }
                BarrierState::Exhausted { .. } => prop_assert!(false, "small generated histories cannot exhaust u64 generations"),
            }
        }
    }

    #[test]
    fn latch_releases_each_accepted_route_exactly_once(
        count in 0_usize..8,
        arrivals in vec(0_u8..16, 0..80),
    ) {
        let mut actual = TestLatch::new(count).initialize().unwrap().behavior;
        let mut waiting = Vec::new();
        let mut released = count == 0;

        for address in arrivals {
            let route = Recipient::global(MailAddr(u64::from(address)));
            let actions = actual.receive(MailAddr(9), LatchMessage::arrive(route)).unwrap();
            if released {
                prop_assert_eq!(actions.sends.iter().map(|delivery| delivery.to).collect::<Vec<_>>(), vec![route]);
            } else {
                waiting.push(route);
                if waiting.len() == count {
                    released = true;
                    prop_assert_eq!(actions.sends.iter().map(|delivery| delivery.to).collect::<Vec<_>>(), waiting.clone());
                    waiting.clear();
                } else {
                    prop_assert!(actions.sends.is_empty());
                }
            }
            match actual.state() {
                LatchState::Released => prop_assert!(released),
                LatchState::Counting { remaining, waiting: actual_waiting } => {
                    prop_assert!(!released);
                    prop_assert_eq!(*remaining, count - waiting.len());
                    prop_assert_eq!(actual_waiting, &waiting);
                }
            }
        }
    }

    #[test]
    fn workflow_matches_an_independent_dependency_run(
        operations in vec((0_u8..4, 0_u8..6), 0..180),
    ) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Phase { Ready, Running, Succeeded, Failed, Cancelled }
        let definition = WorkflowDefinition {
            steps: vec![0, 1, 2, 3],
            dependencies: vec![(0, 1), (0, 2), (1, 3), (2, 3)],
        };
        let mut actual = TestWorkflow::new(definition).unwrap().initialize().unwrap().behavior;
        let reply = Recipient::global(MailAddr(1));
        let mut phase = Phase::Ready;
        let mut steps = [WorkflowStepState::Blocked; 4];

        for (operation, step) in operations {
            let phase_before = phase;
            let before = steps;
            let result = match operation {
                0 => actual.receive(MailAddr(9), WorkflowMessage::Start { reply_to: reply }),
                1 => actual.receive(MailAddr(9), WorkflowMessage::Complete { step }),
                2 => actual.receive(MailAddr(9), WorkflowMessage::Fail { step }),
                _ => actual.receive(MailAddr(9), WorkflowMessage::Cancel { reply_to: reply }),
            };
            if phase == Phase::Ready && matches!(operation, 1 | 2) {
                let expected = if operation == 1 {
                    WorkflowInput::Complete { step }
                } else {
                    WorkflowInput::Fail { step }
                };
                prop_assert!(matches!(
                    result,
                    Err(WorkflowError::NotStarted(returned)) if returned == expected
                ));
                prop_assert!(matches!(actual.state(), WorkflowState::Ready));
                continue;
            }
            let actions = result.unwrap();

            match (phase, operation) {
                (Phase::Ready, 0) => {
                    phase = Phase::Running;
                    steps[0] = WorkflowStepState::Active;
                    prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Started { activated: vec![0] });
                }
                (Phase::Ready, 3) => {
                    phase = Phase::Cancelled;
                    prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Cancelled);
                }
                (Phase::Running, 0) => {
                    prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Rejected(WorkflowRejection::AlreadyStarted));
                }
                (Phase::Running, 1) if usize::from(step) >= steps.len() => {
                    let matched = matches!(actions.sends[0].message, WorkflowOutcome::Rejected(WorkflowRejection::UnknownStep { .. }));
                    prop_assert!(matched);
                }
                (Phase::Running, 1) => {
                    let index = usize::from(step);
                    match steps[index] {
                        WorkflowStepState::Blocked => {
                            let matched = matches!(actions.sends[0].message,
                                WorkflowOutcome::Rejected(WorkflowRejection::Blocked { .. }));
                            prop_assert!(matched);
                        }
                        WorkflowStepState::Completed => {
                            let matched = matches!(actions.sends[0].message,
                                WorkflowOutcome::Rejected(WorkflowRejection::AlreadyCompleted { .. }));
                            prop_assert!(matched);
                        }
                        WorkflowStepState::Active => {
                            steps[index] = WorkflowStepState::Completed;
                            if steps.iter().all(|state| *state == WorkflowStepState::Completed) {
                                phase = Phase::Succeeded;
                                prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Succeeded { completed: step });
                            } else {
                                let mut activated = Vec::new();
                                if steps[0] == WorkflowStepState::Completed {
                                    for candidate in [1_usize, 2] {
                                        if steps[candidate] == WorkflowStepState::Blocked {
                                            steps[candidate] = WorkflowStepState::Active;
                                            activated.push(u8::try_from(candidate).unwrap());
                                        }
                                    }
                                }
                                if steps[1] == WorkflowStepState::Completed
                                    && steps[2] == WorkflowStepState::Completed
                                    && steps[3] == WorkflowStepState::Blocked
                                {
                                    steps[3] = WorkflowStepState::Active;
                                    activated.push(3);
                                }
                                prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Advanced { completed: step, activated });
                            }
                        }
                    }
                }
                (Phase::Running, 2) if usize::from(step) >= steps.len() => {
                    let matched = matches!(actions.sends[0].message, WorkflowOutcome::Rejected(WorkflowRejection::UnknownStep { .. }));
                    prop_assert!(matched);
                }
                (Phase::Running, 2) => {
                    match steps[usize::from(step)] {
                        WorkflowStepState::Active => {
                            phase = Phase::Failed;
                            prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Failed { step });
                        }
                        WorkflowStepState::Blocked => {
                            let matched = matches!(actions.sends[0].message,
                                WorkflowOutcome::Rejected(WorkflowRejection::Blocked { .. }));
                            prop_assert!(matched);
                        }
                        WorkflowStepState::Completed => {
                            let matched = matches!(actions.sends[0].message,
                                WorkflowOutcome::Rejected(WorkflowRejection::AlreadyCompleted { .. }));
                            prop_assert!(matched);
                        }
                    }
                }
                (Phase::Running, 3) => {
                    phase = Phase::Cancelled;
                    prop_assert_eq!(&actions.sends[0].message, &WorkflowOutcome::Cancelled);
                }
                (_, 0) => prop_assert_eq!(&actions.sends[0].message,
                    &WorkflowOutcome::Rejected(WorkflowRejection::AlreadyStarted)),
                (_, 1 | 2) => prop_assert_eq!(&actions.sends[0].message,
                    &WorkflowOutcome::Rejected(WorkflowRejection::Terminal { step: Some(step) })),
                (_, 3) => prop_assert_eq!(&actions.sends[0].message,
                    &WorkflowOutcome::Rejected(WorkflowRejection::Terminal { step: None })),
                _ => unreachable!(),
            }

            if matches!(phase_before, Phase::Succeeded | Phase::Failed | Phase::Cancelled) {
                prop_assert_eq!(steps, before, "terminal operations cannot mutate step phases");
            }
            match (phase, actual.state()) {
                (Phase::Ready, WorkflowState::Ready)
                | (Phase::Succeeded, WorkflowState::Succeeded { .. })
                | (Phase::Failed, WorkflowState::Failed { .. })
                | (Phase::Cancelled, WorkflowState::Cancelled { .. }) => {}
                (Phase::Running, WorkflowState::Running { steps: actual_steps, .. }) => {
                    prop_assert_eq!(actual_steps.iter().map(|(_, state)| *state).collect::<Vec<_>>(), steps.to_vec());
                }
                _ => prop_assert!(false, "workflow phase diverged from independent model"),
            }
        }
    }
}
