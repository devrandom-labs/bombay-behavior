use behavior_actors::atomic::Assignment;

struct OrdinaryDirect;

#[behavior::behavior(addr = behavior::MailAddr, message = u8)]
impl OrdinaryDirect {
    fn receive(&mut self, _: behavior::MailAddr, _: u8) -> behavior::BehaviorActed<Self> {
        Ok(behavior::Actions::cont())
    }
}

struct OrdinaryWorker;

#[behavior_actors::atomic::pool_worker(addr = behavior::MailAddr, result = u16)]
impl OrdinaryWorker {
    fn transition(&mut self, assignment: Assignment<u8>) -> WorkerActed<Self> {
        Ok(behavior::Actions::cont().with_send(assignment.complete(5)))
    }
}
