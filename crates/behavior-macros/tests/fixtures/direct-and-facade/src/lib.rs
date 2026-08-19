use core_behavior::{Actions, BehaviorActed, MailAddr};

struct Direct;

#[core_behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = {
        values: Vec<u8>,
    },
)]
impl Direct {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        Ok(Actions::send(DirectSends { values: vec![1] }))
    }
}

fn facade_is_also_present() -> bombay::behavior::MailAddr {
    bombay::behavior::MailAddr(0)
}
