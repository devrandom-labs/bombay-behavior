use behavior::{Actions, MailAddr};
use behavior_actors::atomic::{Assignment, pool_worker};

struct Clerk;

#[pool_worker(addr = MailAddr, result = u64)]
impl Clerk {
    fn transition(&mut self, assignment: Assignment<u64>) -> WorkerActed<Self> {
        let value = *assignment.payload();
        Ok(Actions::cont().with_send(assignment.complete(value)))
    }
}

fn main() {
    let _clerk = Clerk;
}
