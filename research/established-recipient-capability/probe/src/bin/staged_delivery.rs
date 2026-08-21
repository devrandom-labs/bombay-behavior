use established_recipient_probe::{EstablishedDelivery, Parent, PrimaryRole, StagedChild};

fn main() {
    let staged = StagedChild::<Parent, PrimaryRole>::new(7);
    let _ = EstablishedDelivery::new(staged, 1);
}
