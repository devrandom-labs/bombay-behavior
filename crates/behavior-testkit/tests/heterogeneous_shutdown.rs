//! Independent model checks for arbitrary closed heterogeneous shutdown plans.

use std::time::Instant;

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, ChildHead, ChildStopped, CreationId,
    CreationSequence, Exit, HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan,
    InstallShutdownPlan, MailAddr, Never, NoBirths, NoShutdownTargets, ShutdownChoice,
    ShutdownCoordinator, ShutdownCoordinatorError, ShutdownPlan, ShutdownPlanError,
    ShutdownRequested, ShutdownState, Step, StopOnShutdown, User, shutdown_target,
};
use proptest::prelude::*;

struct Inert<const KIND: u8>;

impl<const KIND: u8> behavior::Protocol for Inert<KIND> {
    type Addr = MailAddr;
    type Msg = ();
}

impl<const KIND: u8> Behavior for Inert<KIND> {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct ShutdownTopology;

#[behavior::behavior(
    addr = MailAddr,
    message = Never,
    births = {
        zero: StopOnShutdown<Inert<0>>,
        one: StopOnShutdown<Inert<1>>,
        two: StopOnShutdown<Inert<2>>,
        three: StopOnShutdown<Inert<3>>,
        four: StopOnShutdown<Inert<4>>,
    },
)]
impl ShutdownTopology {
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

type RootTargets = ShutdownChoice<
    StopOnShutdown<Inert<4>>,
    ShutdownChoice<
        StopOnShutdown<Inert<3>>,
        ShutdownChoice<
            StopOnShutdown<Inert<2>>,
            ShutdownChoice<
                StopOnShutdown<Inert<1>>,
                ShutdownChoice<StopOnShutdown<Inert<0>>, NoShutdownTargets<MailAddr>>,
            >,
        >,
    >,
>;

fn child_ids(count: usize) -> Vec<CreationId> {
    let mut sequence = CreationSequence::new();
    (0..count)
        .map(|_| sequence.issue().expect("fixture creation ID exists"))
        .collect()
}

fn target(kind: u8, child: CreationId) -> RootTargets {
    match kind % 5 {
        0 => {
            shutdown_target::<ShutdownTopology, _, RootTargets>(ShutdownTopologyChild::Zero, child)
        }
        1 => shutdown_target::<ShutdownTopology, _, RootTargets>(ShutdownTopologyChild::One, child),
        2 => shutdown_target::<ShutdownTopology, _, RootTargets>(ShutdownTopologyChild::Two, child),
        3 => {
            shutdown_target::<ShutdownTopology, _, RootTargets>(ShutdownTopologyChild::Three, child)
        }
        _ => {
            shutdown_target::<ShutdownTopology, _, RootTargets>(ShutdownTopologyChild::Four, child)
        }
    }
}

fn stopped(child: CreationId) -> ChildStopped<MailAddr> {
    ChildStopped::new(child, Ok(Exit::Normal), Instant::now())
}

fn selected_child(selection: &RootTargets) -> CreationId {
    match selection {
        ShutdownChoice::Child { creation, .. } => *creation,
        ShutdownChoice::Other(selection) => match selection {
            ShutdownChoice::Child { creation, .. } => *creation,
            ShutdownChoice::Other(selection) => match selection {
                ShutdownChoice::Child { creation, .. } => *creation,
                ShutdownChoice::Other(selection) => match selection {
                    ShutdownChoice::Child { creation, .. } => *creation,
                    ShutdownChoice::Other(selection) => match selection {
                        ShutdownChoice::Child { creation, .. } => *creation,
                        ShutdownChoice::Other(_) => {
                            unreachable!("NoShutdownTargets has no inhabitant")
                        }
                    },
                },
            },
        },
    }
}

#[test]
fn five_unrelated_protocols_share_one_phase_machine() {
    let children = child_ids(5);
    let plan = HeterogeneousShutdownPlan::new([
        vec![
            target(3, children[3]),
            target(0, children[0]),
            target(4, children[4]),
        ],
        vec![target(2, children[2]), target(1, children[1])],
    ])
    .unwrap();
    let mut active = HeterogeneousShutdownCoordinator::<ShutdownTopology, RootTargets>::new(
        ShutdownTopology,
        plan,
    )
    .initialize()
    .unwrap()
    .behavior;

    let started = active.on_path(ShutdownRequested).unwrap();
    assert_eq!(
        started
            .sends
            .owned
            .as_slice()
            .iter()
            .map(selected_child)
            .collect::<Vec<_>>(),
        [children[3], children[0], children[4]]
    );
    assert!(matches!(started.sends.inner, behavior::NoSends));
    assert!(started.creates.is_empty());
    assert!(matches!(started.become_, Step::Continue));
    for child in [children[0], children[4]] {
        let retained = active.on_path(stopped(child)).unwrap();
        assert!(retained.sends.owned.as_slice().is_empty());
        assert!(matches!(retained.sends.inner, behavior::NoSends));
        assert!(retained.creates.is_empty());
        assert!(matches!(retained.become_, Step::Continue));
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping { phase: 0, .. }
        ));
    }
    let next_phase = active.on_path(stopped(children[3])).unwrap();
    assert_eq!(
        next_phase
            .sends
            .owned
            .as_slice()
            .iter()
            .map(selected_child)
            .collect::<Vec<_>>(),
        [children[2], children[1]]
    );
    assert!(matches!(next_phase.sends.inner, behavior::NoSends));
    assert!(next_phase.creates.is_empty());
    assert!(matches!(next_phase.become_, Step::Continue));
    assert!(matches!(
        active.state(),
        ShutdownState::Stopping { phase: 1, .. }
    ));
    let retained = active.on_path(stopped(children[1])).unwrap();
    assert!(retained.sends.owned.as_slice().is_empty());
    assert!(matches!(retained.sends.inner, behavior::NoSends));
    assert!(retained.creates.is_empty());
    assert!(matches!(retained.become_, Step::Continue));
    let completed = active.on_path(stopped(children[2])).unwrap();
    assert!(completed.sends.owned.as_slice().is_empty());
    assert!(matches!(completed.sends.inner, behavior::NoSends));
    assert!(completed.creates.is_empty());
    assert!(matches!(completed.become_, Step::Stop(_)));
    assert!(matches!(active.state(), ShutdownState::Completed));
}

proptest! {
    #[test]
    fn validation_matches_an_independent_global_child_model(
        entries in prop::collection::vec((0_u8..5, 0_usize..16), 1..24)
    ) {
        let children = child_ids(16);
        let mut seen = Vec::new();
        let duplicate = entries.iter().any(|(_, child)| {
            if seen.contains(child) { true } else { seen.push(*child); false }
        });
        let plan = HeterogeneousShutdownPlan::new([
            entries.into_iter().map(|(kind, child)| target(kind, children[child])).collect()
        ]);
        prop_assert_eq!(
            plan.is_err(),
            duplicate,
            "validation must use one child namespace across every protocol alternative"
        );
        if let Err(error) = plan {
            prop_assert!(matches!(error, ShutdownPlanError::DuplicateChild(_)));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LatePlanModel {
    Waiting,
    WaitingAfterShutdown,
    Ready,
    Stopping {
        phase: usize,
        awaiting: Vec<CreationId>,
    },
    Completed,
}

fn assert_late_plan_state(
    actual: &ShutdownState<ShutdownPlan<CreationId>, CreationId>,
    expected: &LatePlanModel,
) {
    match (actual, expected) {
        (ShutdownState::AwaitingPlan, LatePlanModel::Waiting)
        | (ShutdownState::AwaitingPlanAfterShutdown, LatePlanModel::WaitingAfterShutdown)
        | (ShutdownState::Ready { .. }, LatePlanModel::Ready)
        | (ShutdownState::Completed, LatePlanModel::Completed) => {}
        (
            ShutdownState::Stopping {
                phase, awaiting, ..
            },
            LatePlanModel::Stopping {
                phase: expected_phase,
                awaiting: expected_awaiting,
            },
        ) => {
            assert_eq!(phase, expected_phase);
            assert_eq!(awaiting, expected_awaiting);
        }
        (actual, expected) => panic!("state mismatch: actual {actual:?}, model {expected:?}"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 512,
        max_shrink_iters: 100_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn late_plan_coordinator_matches_an_independent_phase_model(
        operations in prop::collection::vec(any::<u8>(), 0..128)
    ) {
        type Subject = ShutdownCoordinator<
            ShutdownTopology,
            StopOnShutdown<Inert<0>>,
            ChildHead,
        >;
        let mut actual = Subject::awaiting_plan(ShutdownTopology)
            .initialize()
            .unwrap()
            .behavior;
        let children = child_ids(3);
        let phases = [
            vec![children[0], children[1]],
            vec![children[2]],
        ];
        let mut model = LatePlanModel::Waiting;

        for operation in operations {
            match operation % 5 {
                0 => {
                    let actions = actual.on_path(ShutdownRequested).unwrap();
                    match model {
                        LatePlanModel::Waiting => {
                            model = LatePlanModel::WaitingAfterShutdown;
                            prop_assert!(actions.sends.owned.is_empty());
                        }
                        LatePlanModel::Ready => {
                            model = LatePlanModel::Stopping {
                                phase: 0,
                                awaiting: phases[0].clone(),
                            };
                            prop_assert_eq!(
                                actions
                                    .sends
                                    .owned
                                    .iter()
                                    .map(|request| request.child)
                                    .collect::<Vec<_>>(),
                                phases[0].clone()
                            );
                        }
                        LatePlanModel::WaitingAfterShutdown
                        | LatePlanModel::Stopping { .. }
                        | LatePlanModel::Completed => {
                            prop_assert!(actions.sends.owned.is_empty());
                        }
                    }
                }
                1 => {
                    let proposed = ShutdownPlan::new(phases.clone()).unwrap();
                    let result = actual.on_path(InstallShutdownPlan::new(proposed.clone()));
                    match model {
                        LatePlanModel::Waiting => {
                            prop_assert!(result.unwrap().sends.owned.is_empty());
                            model = LatePlanModel::Ready;
                        }
                        LatePlanModel::WaitingAfterShutdown => {
                            let actions = result.unwrap();
                            prop_assert_eq!(
                                actions
                                    .sends
                                    .owned
                                    .iter()
                                    .map(|request| request.child)
                                    .collect::<Vec<_>>(),
                                phases[0].clone()
                            );
                            model = LatePlanModel::Stopping {
                                phase: 0,
                                awaiting: phases[0].clone(),
                            };
                        }
                        LatePlanModel::Ready
                        | LatePlanModel::Stopping { .. }
                        | LatePlanModel::Completed => match result {
                            Err(error) => prop_assert_eq!(
                                error,
                                ShutdownCoordinatorError::PlanAlreadyInstalled(proposed)
                            ),
                            Ok(_) => prop_assert!(false, "a shutdown plan may only be installed once"),
                        },
                    }
                }
                _ => {
                    let child = children[usize::from((operation % 5) - 2)];
                    let stopped_child = stopped(child);
                    let accepted = matches!(
                        &model,
                        LatePlanModel::Stopping { awaiting, .. } if awaiting.contains(&child)
                    );
                    let result = actual.on_path(stopped_child);
                    if !accepted {
                        match result {
                            Err(ShutdownCoordinatorError::UnexpectedChildStopped(returned)) => {
                                prop_assert_eq!(returned, stopped_child);
                            }
                            Err(error) => prop_assert!(false, "wrong rejection: {error:?}"),
                            Ok(_) => prop_assert!(false, "unexpected child stop was accepted"),
                        }
                        assert_late_plan_state(actual.state(), &model);
                        continue;
                    }
                    let actions = result.unwrap();
                    if let LatePlanModel::Stopping { phase, awaiting } = &mut model {
                        let position = awaiting
                            .iter()
                            .position(|candidate| *candidate == child)
                            .unwrap();
                        awaiting.remove(position);
                        if awaiting.is_empty() {
                            let next = *phase + 1;
                            if next == phases.len() {
                                model = LatePlanModel::Completed;
                                prop_assert!(matches!(actions.become_, Step::Stop(_)));
                            } else {
                                *phase = next;
                                *awaiting = phases[next].clone();
                                prop_assert_eq!(
                                    actions
                                        .sends
                                        .owned
                                        .iter()
                                        .map(|request| request.child)
                                        .collect::<Vec<_>>(),
                                    phases[next].clone()
                                );
                            }
                        } else {
                            prop_assert!(actions.sends.owned.is_empty());
                        }
                    }
                }
            }
            assert_late_plan_state(actual.state(), &model);
        }
    }
}
