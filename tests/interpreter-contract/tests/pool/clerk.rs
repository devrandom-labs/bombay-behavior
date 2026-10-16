use behavior::{
    Actions, BehaviorActed, Delivery, InterpreterRequests, Protocol, Recipient, ReportToParent,
};
use behavior_actors::pool::{Assignment, Completion};

use super::address::RuntimeAddr;

pub(super) struct Audits;
impl Protocol for Audits {
    type Addr = RuntimeAddr;
    type Msg = u64;
}

pub(super) struct Clerk {
    pub(super) audits: Recipient<Audits>,
}

#[behavior::behavior(addr = RuntimeAddr, message = Assignment<u64>, sends = pub(super) {
    completions: InterpreterRequests<ReportToParent<Completion<u64>>>,
    audits: Vec<Delivery<Audits>>,
})]
impl Clerk {
    fn receive(&mut self, _: RuntimeAddr, assignment: Assignment<u64>) -> BehaviorActed<Self> {
        let amount = *assignment.payload();
        Ok(Actions::cont()
            .with_send(assignment.complete(amount))
            .with_send(Delivery::new(self.audits, amount)))
    }
}
