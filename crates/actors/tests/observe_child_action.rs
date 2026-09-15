use behavior::{
    ActionItem, ChildHead, CreationCorrelation, CreationSequence, Here, InterpreterRequest,
    MailAddr, Never, Protocol, ReturnsToEmitter,
};
use behavior_actors::{ChildStopped, ObserveChild};

struct Worker;

impl Protocol for Worker {
    type Addr = MailAddr;
    type Msg = u8;
}

struct Account;

impl Protocol for Account {
    type Addr = MailAddr;
    type Msg = u8;
}

fn requires_worker_observation<Item>()
where
    Item: ActionItem<
            Accepted = (),
            Rejection = Never,
            Prerequisite = CreationCorrelation<Worker, ChildHead>,
        > + InterpreterRequest<ReturnToEmitter = ReturnsToEmitter<ChildStopped<MailAddr>, Here>>,
{
}

fn worker_observation(_: ObserveChild<Worker, ChildHead>) {}

fn account_observation(_: ObserveChild<Account, ChildHead>) {}

#[test]
fn child_observations_retain_issued_ids_and_static_protocols() {
    requires_worker_observation::<ObserveChild<Worker, ChildHead>>();

    let mut creations = CreationSequence::new();
    let worker = creations.issue().expect("the worker creation ID exists");
    let account = creations.issue().expect("the account creation ID exists");

    worker_observation(ObserveChild::new(worker));
    account_observation(ObserveChild::new(account));
}
