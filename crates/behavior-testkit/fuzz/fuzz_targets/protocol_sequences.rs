#![no_main]

use behavior::{
    Acted, Actions, Base, Behavior, ChildStopped, CreationKind, CreationResolved, Delivery, Exit,
    MailAddr, Never, Proxy, ProxyCommand, Route, State, SupervisionEvent, User, UserEvent,
};
use libfuzzer_sys::fuzz_target;
use tokio::runtime::Builder;

struct Worker;

impl State<u8> for Worker {
    type Addr = MailAddr;
    type Msg = u8;

    fn handle(
        &mut self,
        _from: MailAddr,
        _message: u8,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, u8>>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

fn worker(_seed: usize) -> Base<Worker, u8> {
    Base::new(Worker)
}

fuzz_target!(|bytes: &[u8]| {
    let runtime = Builder::new_current_thread().enable_time().build().unwrap();
    runtime.block_on(async {
        let mut proxy = Proxy::new(worker(0));
        let initial = proxy.init().await.unwrap();
        assert_eq!(initial.creates.len(), 1);
        assert_eq!(initial.creates[0].nonce, 0);
        assert_eq!(initial.creates[0].kind, CreationKind::Birth);
        proxy
            .step(SupervisionEvent::CreationResolved(CreationResolved {
                nonce: 0,
                kind: CreationKind::Birth,
                result: Ok(()),
            }))
            .await
            .unwrap();
        let mut generation = 0_u64;

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                let actions = proxy
                    .step(SupervisionEvent::Inner(User::user(
                        MailAddr(0),
                        ProxyCommand::Forward(byte),
                    )))
                    .await
                    .unwrap();
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.deliveries.len(), 1);
                assert_eq!(actions.sends.deliveries[0].to.route(), Route::Child(generation));
                assert_eq!(actions.sends.deliveries[0].message, byte);
            } else {
                generation = generation.checked_add(1).unwrap();
                let actions = proxy
                    .step(SupervisionEvent::Inner(User::user(
                        MailAddr(0),
                        ProxyCommand::Replace(worker(index)),
                    )))
                    .await
                    .unwrap();
                assert!(actions.sends.deliveries.is_empty());
                assert!(actions.creates.is_empty());
                let actions = proxy
                    .step(SupervisionEvent::ChildStopped(ChildStopped {
                        nonce: generation - 1,
                        outcome: Ok(Exit::Normal),
                        at: tokio::time::Instant::now(),
                    }))
                    .await
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
                    .step(SupervisionEvent::CreationResolved(CreationResolved {
                        nonce: generation,
                        kind: CreationKind::ReplacementIncarnation {
                            replaces: generation - 1,
                        },
                        result: Ok(()),
                    }))
                    .await
                    .unwrap();
            }
        }
    });
});
