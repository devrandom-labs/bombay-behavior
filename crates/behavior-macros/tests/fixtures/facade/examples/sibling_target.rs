use bombay::behavior::{Actions, BehaviorActed, MailAddr};

#[allow(dead_code)]
struct FacadeExample;

#[bombay::behavior::behavior(addr = MailAddr, message = u8)]
impl FacadeExample {
    #[allow(dead_code)]
    fn receive(&mut self, _: MailAddr, _: u8) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

fn main() {}
