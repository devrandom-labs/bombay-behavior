use behavior::{Acted, Actions, Base, Delivery, MailAddr, Never, NoBirths, State};

struct Counter(u64);

impl State for Counter {
    type Addr = MailAddr;
    type Msg = u64;

    fn handle(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

fn main() {
    let _behavior = Base::new(Counter(0));
}
