#![no_main]

use behavior::{
    Acted, Actions, Base, Behavior, Delivery, MailAddr, Never, Proxy, ProxyCommand, Route, State,
    User, UserEvent,
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
        let mut generation = 0_u64;

        for (index, byte) in bytes.iter().copied().enumerate() {
            if byte & 1 == 0 {
                let actions = proxy
                    .step(User::user(MailAddr(0), ProxyCommand::Forward(byte)))
                    .await
                    .unwrap();
                assert!(actions.creates.is_empty());
                assert_eq!(actions.sends.len(), 1);
                assert_eq!(actions.sends[0].to.route(), Route::Child(generation));
                assert_eq!(actions.sends[0].message, byte);
            } else {
                generation = generation.checked_add(1).unwrap();
                let actions = proxy
                    .step(User::user(
                        MailAddr(0),
                        ProxyCommand::Replace(worker(index)),
                    ))
                    .await
                    .unwrap();
                assert!(actions.sends.is_empty());
                assert_eq!(actions.creates.len(), 1);
                assert_eq!(actions.creates[0].nonce, generation);
            }
        }
    });
});
