use bombay::behavior::{Actions, BehaviorActed, MailAddr, Never, NoBirths};

struct First;
struct Second;

#[bombay::behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl First {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[bombay::behavior::behavior(
    addr = MailAddr,
    message = u16,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Second {
    fn receive(&mut self, _: MailAddr, _: u16) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

#[bombay::behavior::births]
enum Children {
    First(First),
    Second(Second),
}
