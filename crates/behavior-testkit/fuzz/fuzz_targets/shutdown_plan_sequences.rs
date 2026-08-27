#![no_main]

use std::time::Instant;

use behavior::{
    Actions, Activate, Behavior, ChildHead, ChildStopped, Exit, MailAddr, Never, NoBirths,
    ReportShutdownPlan, ShutdownCoordinator, ShutdownCoordinatorError, ShutdownCoordinatorEvent,
    ShutdownPlan, ShutdownRequested, ShutdownState, Step, StopOnShutdown, User,
};
use libfuzzer_sys::fuzz_target;

struct Probe;

impl behavior::Protocol for Probe {
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
        _: behavior::ActiveTurn,
        _: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn plan(bytes: &[u8]) -> ShutdownPlan<u64> {
    let nonces = bytes
        .iter()
        .copied()
        .take(8)
        .map(u64::from)
        .collect::<Vec<_>>();
    let unique = nonces.into_iter().fold(Vec::new(), |mut values, nonce| {
        if !values.contains(&nonce) {
            values.push(nonce);
        }
        values
    });
    let phases = unique.chunks(2).map(<[u64]>::to_vec);
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
    let shutdown_plan = plan(bytes);

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
                let report = ReportShutdownPlan::<ShutdownPlan<u64>>::new(shutdown_plan.clone());
                let event: ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<u64>> =
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
                let observed = ChildStopped::new(u64::from(byte), Ok(Exit::Normal), Instant::now());
                match subject.on_path(observed.clone()) {
                    Ok(actions) => {
                        assert!(actions.sends.owned.len() <= 2);
                        assert!(actions.sends.inner.is_empty());
                        assert!(actions.creates.is_empty());
                        assert!(matches!(actions.become_, Step::Continue | Step::Stop(_)));
                    }
                    Err(ShutdownCoordinatorError::UnexpectedChildStopped(returned)) => {
                        assert_eq!(returned, observed);
                    }
                    Err(other) => panic!("child fact produced the wrong rejection: {other:?}"),
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
