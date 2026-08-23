#![no_main]

use std::time::Instant;

use behavior::{
    Actions, Activate, Behavior, ChildHead, ChildStopped, Exit, Here, MailAddr, Never, NoBirths,
    ShutdownCoordinator, ShutdownCoordinatorEvent, ShutdownPlan, ShutdownPlanIngress,
    ShutdownRequested, ShutdownState, StopOnShutdown, User,
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
    let mut subject = ShutdownCoordinator::<Probe, StopOnShutdown<Probe>, ChildHead>::awaiting_plan(
        Probe,
    )
    .initialize()
    .unwrap()
    .behavior;
    let shutdown_plan = plan(bytes);

    for byte in bytes.iter().copied().take(512) {
        match byte % 4 {
            0 => {
                subject.on_path(ShutdownRequested).unwrap();
            }
            1 => {
                let report = ShutdownPlanIngress::<ShutdownPlan<u64>, Here>::new()
                    .report(shutdown_plan.clone());
                let event: ShutdownCoordinatorEvent<User<MailAddr, ()>, ShutdownPlan<u64>> =
                    report.into_event();
                let _ = subject.transition(event);
            }
            _ => {
                subject
                    .on_path(ChildStopped::new(
                        u64::from(byte),
                        Ok(Exit::Normal),
                        Instant::now(),
                    ))
                    .unwrap();
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
            assert!(awaiting
                .iter()
                .all(|nonce| plan.phases()[*phase].contains(nonce)));
        }
    }
});
