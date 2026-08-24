#![no_main]

use behavior::{
    Acted, Actions, Activate, ChildStopped, CreationKind, CreationResolved, Delivery, Exit,
    MailAddr, Never, Proxy, ProxyCommand, ProxyEvent, Recipient, User, UserEvent,
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
        proxy
            .transition(ProxyEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(MailAddr(999)),
            }))
            .unwrap();
        let mut generation = 0_u64;

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(
                        MailAddr(0),
                        ProxyCommand::Forward {
                            command: byte,
                            unavailable_to: Recipient::global(MailAddr(0)),
                        },
                    )))
                    .unwrap();
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.deliveries.len(), 1);
                assert_eq!(actions.sends.deliveries[0].nonce, generation);
                assert_eq!(actions.sends.deliveries[0].message, byte);
            } else {
                generation = generation.checked_add(1).unwrap();
                let actions = proxy
                    .transition(ProxyEvent::Command(User::user(
                        MailAddr(0),
                        ProxyCommand::Replace(worker(index)),
                    )))
                    .unwrap();
                assert!(actions.sends.deliveries.is_empty());
                assert!(actions.creates.is_empty());
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
                proxy
                    .transition(ProxyEvent::CreationResolved(CreationResolved {
                        nonce: generation,
                        kind: CreationKind::ReplacementIncarnation {
                            replaces: generation - 1,
                        },
                        result: Ok(MailAddr(999)),
                    }))
                    .unwrap();
            }
        }
    });
});
