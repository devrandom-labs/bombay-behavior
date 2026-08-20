//! Independent model checks for arbitrary closed heterogeneous shutdown plans.

use std::time::Instant;

use behavior::{
    Actions, Activate as _, Behavior, BehaviorActed, ChildRoute, ChildStopped, Exit,
    HeterogeneousShutdownCoordinator, HeterogeneousShutdownPlan, MailAddr, Never, NoBirths,
    NoShutdownTargets, ShutdownChoice, ShutdownPlanError, ShutdownRequested, ShutdownState, Step,
    StopOnShutdown, User, shutdown_target,
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

fn target(kind: u8, nonce: u64) -> RootTargets {
    match kind % 5 {
        0 => shutdown_target::<ShutdownTopology, _, RootTargets>(
            ShutdownTopologyChild::Zero,
            ChildRoute::new(nonce),
        ),
        1 => shutdown_target::<ShutdownTopology, _, RootTargets>(
            ShutdownTopologyChild::One,
            ChildRoute::new(nonce),
        ),
        2 => shutdown_target::<ShutdownTopology, _, RootTargets>(
            ShutdownTopologyChild::Two,
            ChildRoute::new(nonce),
        ),
        3 => shutdown_target::<ShutdownTopology, _, RootTargets>(
            ShutdownTopologyChild::Three,
            ChildRoute::new(nonce),
        ),
        _ => shutdown_target::<ShutdownTopology, _, RootTargets>(
            ShutdownTopologyChild::Four,
            ChildRoute::new(nonce),
        ),
    }
}

fn stopped(nonce: u64) -> ChildStopped<MailAddr> {
    ChildStopped::new(nonce, Ok(Exit::Normal), Instant::now())
}

#[test]
fn five_unrelated_protocols_share_one_phase_machine() {
    let plan = HeterogeneousShutdownPlan::new([
        vec![target(3, 30), target(0, 10), target(4, 40)],
        vec![target(2, 20), target(1, 11)],
    ])
    .unwrap();
    let mut active = HeterogeneousShutdownCoordinator::<ShutdownTopology, RootTargets>::new(
        ShutdownTopology,
        plan,
    )
    .initialize()
    .unwrap()
    .behavior;

    active.on_path(ShutdownRequested).unwrap();
    for nonce in [10, 40] {
        active.on_path(stopped(nonce)).unwrap();
        assert!(matches!(
            active.state(),
            ShutdownState::Stopping { phase: 0, .. }
        ));
    }
    active.on_path(stopped(30)).unwrap();
    assert!(matches!(
        active.state(),
        ShutdownState::Stopping { phase: 1, .. }
    ));
    active.on_path(stopped(11)).unwrap();
    let completed = active.on_path(stopped(20)).unwrap();
    assert!(matches!(completed.become_, Step::Stop(_)));
    assert_eq!(active.state(), &ShutdownState::Completed);
}

proptest! {
    #[test]
    fn validation_matches_an_independent_global_nonce_model(
        entries in prop::collection::vec((0_u8..5, 0_u64..16), 1..24)
    ) {
        let mut seen = Vec::new();
        let duplicate = entries.iter().any(|(_, nonce)| {
            if seen.contains(nonce) { true } else { seen.push(*nonce); false }
        });
        let plan = HeterogeneousShutdownPlan::new([
            entries.into_iter().map(|(kind, nonce)| target(kind, nonce)).collect()
        ]);
        prop_assert_eq!(
            plan.is_err(),
            duplicate,
            "validation must use one nonce namespace across every protocol alternative"
        );
        if let Err(error) = plan {
            prop_assert!(matches!(error, ShutdownPlanError::DuplicateChild(_)));
        }
    }
}
