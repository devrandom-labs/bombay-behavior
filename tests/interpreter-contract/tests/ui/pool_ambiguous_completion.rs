use behavior::{Actions, BehaviorActed, Delivery, Protocol, Recipient};
use behavior::{InterpreterRequests, ReportToParent};
use behavior_actors::diagnostic::DiagnosticDisposition;
use behavior_actors::lifecycle::ActorDrainPolicy;
use behavior_actors::pool::Assignment;
use behavior_actors::pool::Completion;
use behavior_actors::pool::queue;
use behavior_actors::pool::{BacklogCapacity, Interruption, PoolFailureReaction, PoolRecovery};
use behavior_actors::worker::{OrderedRoles, StartupLimit, WorkerSubmission};

#[path = "../pool/address.rs"]
mod address;
use address::RuntimeAddr;

#[derive(Debug, Eq, PartialEq)]
enum Role {
    Clerk,
}

struct Audits;
impl Protocol for Audits {
    type Addr = RuntimeAddr;
    type Msg = u64;
}
struct Clerk {
    audits: Recipient<Audits>,
}
#[behavior::behavior(addr = RuntimeAddr, message = Assignment<u64>, sends = {
first: InterpreterRequests<ReportToParent<Completion<u64>>>,
    second: InterpreterRequests<ReportToParent<Completion<u64>>>,
    audits: Vec<Delivery<Audits>>,
})]
impl Clerk {
    fn receive(&mut self, _: RuntimeAddr, assignment: Assignment<u64>) -> BehaviorActed<Self> {
        Ok(Actions::cont().with_send(Delivery::new(self.audits, *assignment.payload())))
    }
}
fn main() {
    let _pool = queue::new(
        |_: &Role| {
            WorkerSubmission::immediate(Clerk {
                audits: Recipient::global(RuntimeAddr(17)),
            })
        },
        OrderedRoles::new(Role::Clerk, []).expect("one clerk"),
        queue::Config {
            startup: StartupLimit::new(1).expect("one clerk startup"),
            recovery: PoolRecovery::temporary(PoolFailureReaction::RetireRole),
            backlog: BacklogCapacity::new(2),
            interruption: Interruption::Retry,
            shutdown: ActorDrainPolicy::WaitForActorGraph,
            diagnostics: DiagnosticDisposition::terminate(),
        },
    )
    .build();
}
