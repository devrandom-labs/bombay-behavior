use core_behavior::{Actions, BehaviorActed, MailAddr, Never, NoBirths};

struct Direct;

#[core_behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Direct {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[core_behavior::births]
enum Children {
    Direct(Direct),
}

fn facade_is_also_present() -> bombay::behavior::MailAddr {
    bombay::behavior::MailAddr(0)
}
