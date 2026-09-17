use bombay::behavior::{Actions, BehaviorActed, Delivery, MailAddr, MessageProtocol, Recipient};
use bombay::atomic::{Assignment, pool_worker};

struct First;
struct Second;

#[bombay::behavior::behavior(
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

#[bombay::behavior::behavior(
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

#[bombay::behavior::behavior(addr = MailAddr, message = u32)]
impl Inferred {
    fn receive(&mut self, _: MailAddr, _: u32) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

struct FacadeWorker;

#[pool_worker(addr = MailAddr, result = u16)]
impl FacadeWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        Ok(Actions::cont().with_send(assignment.complete(5)))
    }
}
