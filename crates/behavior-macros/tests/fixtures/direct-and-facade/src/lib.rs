use core_behavior::{Actions, BehaviorActed, Delivery, MailAddr, MessageProtocol, Recipient};
use actors::atomic::Assignment;

struct Direct;

#[core_behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = {
        notices: Vec<Delivery<MessageProtocol<MailAddr, u8>>>,
    },
)]
impl Direct {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        let recipient = Recipient::global(MailAddr(1));
        Ok(Actions::cont().send_notices(Delivery::new(recipient, 1)))
    }
}

fn facade_is_also_present() -> bombay::behavior::MailAddr {
    bombay::behavior::MailAddr(0)
}

struct DirectWorker;

#[actors::atomic::pool_worker(addr = MailAddr, result = u16)]
impl DirectWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        Ok(Actions::cont().with_send(assignment.complete(7)))
    }
}
