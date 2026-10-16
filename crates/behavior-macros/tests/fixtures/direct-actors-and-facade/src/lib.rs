//! The Bombay worker-pool shape: the consumer depends on the Bombay facade and
//! keeps a direct foundational actors dependency at the same time. Pool-worker
//! expansion must resolve the Actors catalogue through the facade so the
//! generated `Completion` path names the instance the facade re-exports
//! (`bombay::atomic`), not the direct dependency's revision.

use bombay::behavior::{Actions, MailAddr};
use bombay::pool::{Assignment, pool_worker};

struct BothWorker;

#[pool_worker(addr = MailAddr, result = u16)]
impl BothWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        Ok(Actions::cont().with_send(assignment.complete(7)))
    }
}
