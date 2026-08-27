#![no_main]

use behavior::{
    Acted, Actions, Activate, ChildStopped, CreationKind, CreationResolved, Delivery, Exit,
    MailAddr, Never, Proxy, ProxyEvent, ReplacementRequested, Step, User, UserEvent,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

struct Worker;

#[behavior::behavior(addr = MailAddr, message = u8, sends = Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>, births = behavior::NoBirths, error = Never)]
impl Worker {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<
        MailAddr,
        Never,
        Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>,
        behavior::NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }
}

fn worker(_seed: usize) -> Worker {
    Worker
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let proxy = Proxy::new(worker(0));
        let initialized = (proxy).initialize().unwrap();
        let initial = initialized.actions;
        let mut proxy = initialized.behavior;
        assert_eq!(initial.creates.len(), 1);
        assert_eq!(initial.creates[0].nonce, 0);
        assert_eq!(initial.creates[0].kind, CreationKind::Birth);
        assert!(initial.sends.deliveries.is_empty());
        assert!(initial.sends.unavailable_reports.is_empty());
        assert_eq!(initial.sends.child_observations.len(), 1);
        assert_eq!(initial.sends.creation_observations.len(), 1);
        assert!(initial.sends.stopped_reports.is_empty());
        assert!(initial.sends.creation_reports.is_empty());
        assert!(initial.sends.shutdowns.is_empty());
        assert!(matches!(initial.become_, Step::Continue));
        let created = proxy
            .transition(ProxyEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(MailAddr(999)),
            }))
            .unwrap();
        assert!(created.sends.deliveries.is_empty());
        assert!(created.sends.unavailable_reports.is_empty());
        assert!(created.sends.child_observations.is_empty());
        assert!(created.sends.creation_observations.is_empty());
        assert!(created.sends.stopped_reports.is_empty());
        assert_eq!(created.sends.creation_reports.len(), 1);
        assert!(created.sends.shutdowns.is_empty());
        assert!(created.creates.is_empty());
        assert!(matches!(created.become_, Step::Continue));
        let mut generation = 0_u64;

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(MailAddr(0), byte)))
                    .unwrap();
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.deliveries.len(), 1);
                assert_eq!(actions.sends.deliveries[0].nonce, generation);
                assert_eq!(actions.sends.deliveries[0].message, byte);
                assert!(actions.sends.unavailable_reports.is_empty());
                assert!(actions.sends.child_observations.is_empty());
                assert!(actions.sends.creation_observations.is_empty());
                assert!(actions.sends.stopped_reports.is_empty());
                assert!(actions.sends.creation_reports.is_empty());
                assert!(actions.sends.shutdowns.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
            } else {
                generation = generation.checked_add(1).unwrap();
                let actions = proxy
                    .transition(ProxyEvent::WorkerRequested(ReplacementRequested::new(
                        worker(index),
                    )))
                    .unwrap();
                assert!(actions.sends.deliveries.is_empty());
                assert!(actions.sends.unavailable_reports.is_empty());
                assert!(actions.sends.child_observations.is_empty());
                assert!(actions.sends.creation_observations.is_empty());
                assert!(actions.sends.stopped_reports.is_empty());
                assert!(actions.sends.creation_reports.is_empty());
                assert_eq!(actions.sends.shutdowns.len(), 1);
                assert_eq!(actions.sends.shutdowns.as_slice()[0].nonce, generation - 1);
                assert!(actions.creates.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
                let actions = proxy
                    .transition(ProxyEvent::ChildStopped(ChildStopped {
                        nonce: generation - 1,
                        outcome: Ok(Exit::Normal),
                        at: std::time::Instant::now(),
                    }))
                    .unwrap();
                assert_eq!(actions.creates.len(), 1);
                assert_eq!(actions.creates[0].nonce, generation);
                assert_eq!(
                    actions.creates[0].kind,
                    CreationKind::ReplacementIncarnation {
                        replaces: generation - 1,
                    }
                );
                assert!(actions.sends.deliveries.is_empty());
                assert!(actions.sends.unavailable_reports.is_empty());
                assert_eq!(actions.sends.child_observations.len(), 1);
                assert_eq!(actions.sends.creation_observations.len(), 1);
                assert_eq!(actions.sends.stopped_reports.len(), 1);
                assert!(actions.sends.creation_reports.is_empty());
                assert!(actions.sends.shutdowns.is_empty());
                assert!(matches!(actions.become_, Step::Continue));
                let created = proxy
                    .transition(ProxyEvent::CreationResolved(CreationResolved {
                        nonce: generation,
                        kind: CreationKind::ReplacementIncarnation {
                            replaces: generation - 1,
                        },
                        result: Ok(MailAddr(999)),
                    }))
                    .unwrap();
                assert!(created.sends.deliveries.is_empty());
                assert!(created.sends.unavailable_reports.is_empty());
                assert!(created.sends.child_observations.is_empty());
                assert!(created.sends.creation_observations.is_empty());
                assert!(created.sends.stopped_reports.is_empty());
                assert_eq!(created.sends.creation_reports.len(), 1);
                assert!(created.sends.shutdowns.is_empty());
                assert!(created.creates.is_empty());
                assert!(matches!(created.become_, Step::Continue));
            }
        }
    });
});
