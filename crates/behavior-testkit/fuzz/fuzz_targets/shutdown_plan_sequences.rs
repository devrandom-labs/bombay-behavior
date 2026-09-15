#![no_main]

use std::array;
use std::collections::HashSet;
use std::time::Instant;

use behavior_actors::{
    Activate, ChildStopped, Exit, ReportShutdownPlan, ShutdownCoordinator,
    ShutdownCoordinatorError, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownRequested,
    ShutdownState, StopOnShutdown,
};

use behavior_core::{
    Actions, Behavior, ChildHead, CreationId, CreationSequence, MailAddr, Never, NoBirths, Step,
    User,
};
use libfuzzer_sys::fuzz_target;

struct Probe;

impl behavior_core::Protocol for Probe {
    type Addr = MailAddr;
    type Msg = ();
}

impl Behavior for Probe {
    type Protocol = Self;
    type Event = User<MailAddr, ()>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_core::ActiveTurn,
        _: Self::Event,
    ) -> behavior_core::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn child_ids() -> [CreationId; 256] {
    let mut sequence = CreationSequence::new();
    array::from_fn(|_| {
        sequence
            .issue()
            .unwrap_or_else(|| panic!("the fixture issues only 256 child IDs"))
    })
}

fn plan(bytes: &[u8], children: &[CreationId; 256]) -> ShutdownPlan<CreationId> {
    let mut seen = HashSet::with_capacity(8);
    let unique = bytes
        .iter()
        .copied()
        .take(8)
        .map(|byte| children[usize::from(byte)])
        .filter(|child| seen.insert(*child))
        .collect::<Vec<_>>();
    let phases = unique.chunks(2).map(<[CreationId]>::to_vec);
    ShutdownPlan::new(phases).unwrap()
}

fuzz_target!(|bytes: &[u8]| {
    let initialized =
        ShutdownCoordinator::<Probe, StopOnShutdown<Probe>, ChildHead>::awaiting_plan(Probe)
            .initialize()
            .unwrap();
    assert!(initialized.actions.sends.owned.is_empty());
    assert!(initialized.actions.sends.inner.is_empty());
    assert!(initialized.actions.creates.is_empty());
    assert!(matches!(initialized.actions.become_, Step::Continue));
    let mut subject = initialized.behavior;
    let children = child_ids();
    let shutdown_plan = plan(bytes, &children);

    for byte in bytes.iter().copied().take(512) {
        match byte % 4 {
            0 => {
                let actions = subject.on_path(ShutdownRequested).unwrap();
                assert!(actions.sends.owned.len() <= 2);
                assert!(actions.sends.inner.is_empty());
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, Step::Continue | Step::Stop(_)));
            }
            1 => {
                let report =
                    ReportShutdownPlan::<ShutdownPlan<CreationId>>::new(shutdown_plan.clone());
                let event: ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<CreationId>> =
                    report.into_event();
                match subject.transition(event) {
                    Ok(actions) => {
                        assert!(actions.sends.owned.len() <= 2);
                        assert!(actions.sends.inner.is_empty());
                        assert!(actions.creates.is_empty());
                        assert!(matches!(actions.become_, Step::Continue | Step::Stop(_)));
                    }
                    Err(ShutdownCoordinatorError::PlanAlreadyInstalled(returned)) => {
                        assert_eq!(returned, shutdown_plan);
                    }
                    Err(other) => panic!("plan event produced the wrong rejection: {other:?}"),
                }
            }
            _ => {
                let selected = children[usize::from(byte)];
                let observed = ChildStopped::new(selected, Ok(Exit::Normal), Instant::now());
                assert_eq!(observed.child, selected);
                match subject.on_path(observed) {
                    Ok(actions) => {
                        assert!(actions.sends.owned.len() <= 2);
                        assert!(actions.sends.inner.is_empty());
                        assert!(actions.creates.is_empty());
                        assert!(matches!(actions.become_, Step::Continue | Step::Stop(_)));
                    }
                    Err(ShutdownCoordinatorError::UnexpectedChildStopped(returned)) => {
                        assert_eq!(returned, observed);
                    }
                    Err(other) => panic!("child report produced the wrong rejection: {other:?}"),
                }
            }
        }

        if let ShutdownState::Stopping {
            plan,
            phase,
            awaiting,
        } = subject.state()
        {
            assert!(*phase < plan.phases().len());
            assert!(!awaiting.is_empty());
            assert!(
                awaiting
                    .iter()
                    .all(|nonce| plan.phases()[*phase].contains(nonce))
            );
        }
    }
});
