use behavior::{Acted, Actions, MailAddr, Never, NoBirths};

struct Counter(u64);

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Counter {
    #[allow(
        clippy::unnecessary_wraps,
        reason = "the behavior macro requires the declared typed error result"
    )]
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

fn main() {
    let mut behavior = Counter(0);
    let result = behavior.receive(MailAddr(1), 2);
    assert!(result.is_ok());
    assert_eq!(behavior.0, 2);
}
