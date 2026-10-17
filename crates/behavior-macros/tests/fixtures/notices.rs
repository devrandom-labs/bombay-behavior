use behavior::{Own, SendEffects, SendInput};

#[derive(behavior_macros::SendProduct)]
struct Notices<Invoices, Receipts> {
    invoices: Invoices,
    receipts: Receipts,
}

impl<Invoice, Receipts> SendInput<Invoice, Own> for Notices<Vec<Invoice>, Receipts> {
    fn emit(&mut self, input: Invoice) {
        self.invoices.push(input);
    }
}

fn main() {
    let first = Box::new([7_u8, 11]);
    let second = Box::new([13_u8, 17]);
    let allocations = [first.as_ptr(), second.as_ptr()];
    let mut notices = Notices {
        invoices: vec![first],
        receipts: vec![19_u32, 23],
    };
    notices.send(second);
    assert_eq!(notices.invoices.len(), 2);
    for ((invoice, original), contents) in notices
        .invoices
        .iter()
        .zip(allocations)
        .zip([[7, 11], [13, 17]])
    {
        assert_eq!(invoice.as_ptr(), original);
        assert_eq!(**invoice, contents);
    }
    assert_eq!(notices.receipts, [19, 23]);
}
