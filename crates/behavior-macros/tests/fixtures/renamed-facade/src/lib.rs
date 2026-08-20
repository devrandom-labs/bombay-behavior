use runtime::behavior::{Actions, BehaviorActed, MailAddr};

struct First;
struct Second;

#[runtime::behavior::behavior(
    addr = MailAddr,
    message = u8,
    sends = {
        values: Vec<u8>,
    },
)]
impl First {
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        Ok(Actions::cont().send_values(1))
    }
}

#[runtime::behavior::behavior(
    addr = MailAddr,
    message = u16,
    births = {
        first: First,
    },
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
