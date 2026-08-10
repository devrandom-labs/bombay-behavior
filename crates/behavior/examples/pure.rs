use behavior::{Acted, Actions, Delivery, Handler, MailAddr, Never, NoBirths, Pure};

struct Counter(u64);

impl Handler for Counter {
    type Addr = MailAddr;
    type Msg = u64;

    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> Acted<MailAddr, Never, Vec<Delivery<MailAddr, Never>>, NoBirths, Never> {
        self.0 += message;
        Ok(Actions::cont())
    }
}

fn main() {
    let _behavior = Pure::new(Counter(0));
}
