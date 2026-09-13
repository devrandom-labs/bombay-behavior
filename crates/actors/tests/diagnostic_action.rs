use core::convert::Infallible;

use behavior_actors::atomic::{DiagnosticAccepted, DiagnosticAction};
use behavior_actors::{
    ActionItem, Address, EndpointAddress, EstablishedRecipient, ExactDeliveryReason,
    LogicalDeliveryReason, MessageProtocol, Never, Protocol, Recipient,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

#[derive(Clone, Eq, PartialEq)]
struct Endpoint(u64);

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = Endpoint
    where
        P: Protocol<Addr = Self>;
}

struct FixedDiagnostic(String);
struct PoolDiagnostic(Vec<u8>);

fn logical_contract<Item>()
where
    Item: ActionItem<
            Accepted = DiagnosticAccepted<FixedDiagnostic>,
            Rejection = LogicalDeliveryReason,
            Prerequisite = Never,
        >,
{
}

fn exact_contract<Item>()
where
    Item: ActionItem<
            Accepted = DiagnosticAccepted<PoolDiagnostic>,
            Rejection = ExactDeliveryReason,
            Prerequisite = Never,
        >,
{
}

fn terminal_contract<Item>()
where
    Item: ActionItem<
            Accepted = DiagnosticAccepted<FixedDiagnostic>,
            Rejection = Never,
            Prerequisite = Never,
        >,
{
}

#[test]
fn route_family_selects_the_exact_rejection_contract() {
    logical_contract::<
        DiagnosticAction<Recipient<MessageProtocol<RuntimeAddr, FixedDiagnostic>>, FixedDiagnostic>,
    >();
    exact_contract::<
        DiagnosticAction<
            EstablishedRecipient<MessageProtocol<RuntimeAddr, PoolDiagnostic>>,
            PoolDiagnostic,
        >,
    >();
    terminal_contract::<DiagnosticAction<Infallible, FixedDiagnostic>>();

    let route =
        EstablishedRecipient::<MessageProtocol<RuntimeAddr, PoolDiagnostic>>::issued(Endpoint(11));
    let action = DiagnosticAction::deliver(route, PoolDiagnostic(vec![1, 2, 3]));
    match action {
        DiagnosticAction::Deliver {
            diagnostic: PoolDiagnostic(bytes),
            ..
        } => assert_eq!(bytes, [1, 2, 3]),
        DiagnosticAction::Terminal { .. } => panic!("exact route remains a delivery"),
    }
}

#[test]
fn terminal_acceptance_retains_the_complete_affine_diagnostic() {
    let accepted = DiagnosticAccepted::terminal(FixedDiagnostic("search failed".to_owned()));

    match accepted {
        DiagnosticAccepted::Terminal(FixedDiagnostic(message)) => {
            assert_eq!(message, "search failed");
        }
        DiagnosticAccepted::Delivered => panic!("terminal custody is not route delivery"),
    }
}

#[test]
fn delivery_action_retains_route_and_affine_payload() {
    let route = Recipient::<MessageProtocol<RuntimeAddr, FixedDiagnostic>>::global(RuntimeAddr(7));
    let action = DiagnosticAction::deliver(route, FixedDiagnostic("index failed".to_owned()));

    match action {
        DiagnosticAction::Deliver {
            route,
            diagnostic: FixedDiagnostic(message),
        } => {
            assert_eq!(route.address(), RuntimeAddr(7));
            assert_eq!(message, "index failed");
        }
        DiagnosticAction::Terminal { .. } => panic!("a selected route stays a delivery"),
    }
}
