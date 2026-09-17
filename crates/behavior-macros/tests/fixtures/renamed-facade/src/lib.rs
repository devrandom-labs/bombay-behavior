use runtime::behavior::{
    Actions, BehaviorActed, Delivery, MailAddr, MessageProtocol, Recipient,
};
use runtime::atomic::{Assignment, pool_worker};

struct First;
struct Second;

#[runtime::behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = {
        notices: Vec<Delivery<MessageProtocol<MailAddr, u8>>>,
    },
)]
impl First {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        let recipient = Recipient::global(MailAddr(1));
        Ok(Actions::cont().send_notices(Delivery::new(recipient, 1)))
    }
}

#[runtime::behavior::behavior(
    addr = MailAddr,
    message = u16,
    births = {
        first: First,
    },
    creation_settlements = retain_for_retirement,
)]
impl Second {
    fn receive(&mut self, _: MailAddr, _: u16) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct Inferred;

#[runtime::behavior::behavior(addr = MailAddr, message = u32)]
impl Inferred {
    fn receive(&mut self, _: MailAddr, _: u32) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct RenamedFacadeWorker;

#[pool_worker(addr = MailAddr, result = u16)]
impl RenamedFacadeWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        Ok(Actions::cont().with_send(assignment.complete(8)))
    }
}
