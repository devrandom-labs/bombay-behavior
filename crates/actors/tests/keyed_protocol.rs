use behavior_actors::atomic::{
    BindingExpectation, BindingReply, BindingRequestId, KeyedAdmissionRejection, KeyedCommand,
    KeyedOutcome, SubmissionId,
};
use behavior_actors::{Address, EndpointAddress, MessageProtocol, Protocol, Recipient};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

#[derive(Debug, Eq, PartialEq)]
struct Account(u64);

#[derive(Debug, Eq, PartialEq)]
enum SearchRole {
    Replica,
}

#[derive(Debug, Eq, PartialEq)]
struct SearchJob(u8);

#[derive(Debug, Eq, PartialEq)]
struct SearchResult(u16);

fn accept_command(_: KeyedCommand<RuntimeAddr, Account, SearchRole, SearchJob, SearchResult>) {}

#[test]
fn commands_use_only_domain_values_and_typed_recipients() {
    let customer = Recipient::<
        MessageProtocol<RuntimeAddr, KeyedOutcome<Account, SearchRole, SearchJob, SearchResult>>,
    >::global(RuntimeAddr(80));
    let management = Recipient::<
        MessageProtocol<RuntimeAddr, BindingReply<RuntimeAddr, Account, SearchRole>>,
    >::global(RuntimeAddr(81));

    accept_command(KeyedCommand::submit(
        SubmissionId::new(1),
        Account(42),
        SearchJob(9),
        customer,
    ));
    accept_command(KeyedCommand::rebalance(
        BindingRequestId::new(2),
        Account(7),
        BindingExpectation::Absent,
        SearchRole::Replica,
        management,
    ));
    accept_command(KeyedCommand::unbind(
        BindingRequestId::new(3),
        Account(99),
        BindingExpectation::Absent,
        management,
    ));
    accept_command(KeyedCommand::shutdown());
}

#[test]
fn binding_request_identity_round_trips_exactly() {
    assert_eq!(BindingRequestId::new(42).get(), 42);
}

#[test]
fn keyed_admission_keeps_its_distinct_role_rejection() {
    let outcome: KeyedOutcome<Account, SearchRole, SearchJob, SearchResult> =
        KeyedOutcome::Rejected {
            submission: SubmissionId::new(4),
            key: Account(11),
            payload: SearchJob(5),
            reason: KeyedAdmissionRejection::UnknownSelectedRole,
        };

    let KeyedOutcome::Rejected { reason, .. } = outcome else {
        panic!("the constructed outcome must remain rejected");
    };
    assert_eq!(reason, KeyedAdmissionRejection::UnknownSelectedRole);
}
