//! Expected to fail: behavior-side code can retain and emit the capability but
//! cannot extract the runtime endpoint or invoke it directly.

use established_recipient_probe::{ActorRef, EstablishedRecipient, Worker};

fn main() {
    let worker = EstablishedRecipient::<Worker>::issued(ActorRef::issued(7));
    let _ = worker.into_endpoint();
}
