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
    let _behavior = Counter(0);
}
