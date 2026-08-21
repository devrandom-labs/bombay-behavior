//! Expected to fail: a worker capability cannot enter a queue delivery even
//! though both protocols use the same address namespace and message type.

use established_recipient_probe::{
    ActorRef, EstablishedDelivery, EstablishedRecipient, Queue, RuntimeAddr, Worker,
};

fn main() {
    let worker = EstablishedRecipient::<Worker>::issued(ActorRef::issued(RuntimeAddr(3), 7));
    let _ = EstablishedDelivery::<Queue>::new(worker, 1);
}
