#![no_main]

use behavior::{
    Acted, Actions, Behavior, ChildStopped, CreationKind, CreationResolved, Delivery, Exit,
    Handler, MailAddr, Never, Proxy, ProxyCommand, ProxyEvent, Pure, User, UserEvent,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

struct Worker;

impl Handler<Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>> for Worker {
    type Addr = MailAddr;
    type Msg = u8;

    fn receive(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn worker(_seed: usize) -> Pure<Worker, Vec<Delivery<bombay_behavior_fuzz::TestRecipient<u8>>>> {
    Pure::new(Worker)
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let mut proxy = Proxy::new(worker(0));
        let initial = proxy.init().unwrap();
        assert_eq!(initial.creates.len(), 1);
        assert_eq!(initial.creates[0].nonce, 0);
        assert_eq!(initial.creates[0].kind, CreationKind::Birth);
        proxy
            .transition(ProxyEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(()),
            }))
            .unwrap();
        let mut generation = 0_u64;

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                let actions = proxy
                    .transition(ProxyEvent::Inner(User::user(
                        MailAddr(0),
                        ProxyCommand::Forward(byte),
                    )))
                    .unwrap();
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.deliveries.len(), 1);
                assert_eq!(
                    actions.sends.deliveries[0].to.resolve(MailAddr(17)),
                    behavior::Address::birth(MailAddr(17), generation)
                );
                assert_eq!(actions.sends.deliveries[0].message, byte);
            } else {
                generation = generation.checked_add(1).unwrap();
                let actions = proxy
                    .transition(ProxyEvent::Inner(User::user(
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
                        at: tokio::time::Instant::now(),
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
                        result: Ok(()),
                    }))
                    .unwrap();
            }
        }
    });
});
