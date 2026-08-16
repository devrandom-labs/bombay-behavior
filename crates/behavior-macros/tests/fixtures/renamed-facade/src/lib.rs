use runtime::behavior::{Actions, BehaviorActed, MailAddr, Never, NoBirths};

struct First;
struct Second;

#[runtime::behavior::behavior(
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

#[runtime::behavior::behavior(
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

#[runtime::behavior::births]
enum Children {
    First(First),
    Second(Second),
}
