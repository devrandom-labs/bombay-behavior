use core::convert::Infallible;

use behavior::{
    ActionItem, ActionItemResult, Delivery, InterpreterFault, ItemSettlement, MailAddr,
    MessageProtocol, SettledItem, SourceCustody, SourceSettlementCustody,
};
use behavior_actors::atomic::{DiagnosticAccepted, DiagnosticAction};

struct OwnedRequest(Box<str>);

impl ActionItem for OwnedRequest {
    type Accepted = ();
    type Rejection = u8;
    type Prerequisite = u16;
}

type TerminalDiagnostic = DiagnosticAction<Infallible, Box<str>>;
type OrdinaryDelivery = Delivery<MessageProtocol<MailAddr, u8>>;

async fn offer<Item: ActionItem>(
    settlements: Vec<ActionItemResult<Item>>,
) -> SourceCustody<Vec<ActionItemResult<Item>>> {
    <Vec<ActionItemResult<Item>> as SourceSettlementCustody<(), ()>>::offer_next_to_source(
        settlements,
        &mut (),
    )
    .await
}

#[tokio::test]
async fn terminal_diagnostic_remains_owned_across_repeated_offers() {
    let diagnostic: Box<str> = "terminal".into();
    let identity = diagnostic.as_ptr();
    let settlements: Vec<ActionItemResult<TerminalDiagnostic>> = vec![SettledItem::Attempted(
        ItemSettlement::Accepted(DiagnosticAccepted::terminal(diagnostic)),
    )];

    let SourceCustody::Retained(mut residual) = offer(settlements).await else {
        panic!("terminal diagnostic must remain in custody");
    };
    for _ in 0..3 {
        residual.push(SettledItem::Attempted(ItemSettlement::Accepted(
            DiagnosticAccepted::delivered(),
        )));
        let SourceCustody::Retained(next) = offer(residual).await else {
            panic!("a later continuing turn must preserve terminal custody");
        };
        assert_eq!(next.len(), 1, "discharged receipts must not accumulate");
        residual = next;
    }
    let SettledItem::Attempted(ItemSettlement::Accepted(DiagnosticAccepted::Terminal(returned))) =
        residual.pop().expect("one retained diagnostic")
    else {
        panic!("the exact accepted terminal diagnostic must survive");
    };
    assert_eq!(returned.as_ptr(), identity);
    assert_eq!(&*returned, "terminal");
    assert!(residual.is_empty());
}

#[tokio::test]
async fn delivered_diagnostics_discharge_without_retained_history() {
    let settlements: Vec<ActionItemResult<TerminalDiagnostic>> = (0..100)
        .map(|_| SettledItem::Attempted(ItemSettlement::Accepted(DiagnosticAccepted::delivered())))
        .collect();

    let SourceCustody::Exhausted(residual) = offer(settlements).await else {
        panic!("delivered diagnostics have no terminal custody");
    };
    assert!(residual.is_empty());
}

#[tokio::test]
async fn ordinary_delivery_receipts_discharge_without_retained_history() {
    let settlements: Vec<ActionItemResult<OrdinaryDelivery>> = (0..100)
        .map(|_| SettledItem::Attempted(ItemSettlement::Accepted(())))
        .collect();

    let SourceCustody::Exhausted(residual) = offer(settlements).await else {
        panic!("ordinary delivery receipts have no terminal custody");
    };
    assert!(residual.is_empty());
}

#[tokio::test]
async fn source_free_failures_and_untouched_requests_remain_complete() {
    let settlements: Vec<ActionItemResult<OwnedRequest>> = vec![
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: OwnedRequest("rejected".into()),
            reason: 7,
        }),
        SettledItem::Attempted(ItemSettlement::Blocked {
            item: OwnedRequest("blocked".into()),
            prerequisite: 11,
        }),
        SettledItem::Attempted(ItemSettlement::Corrupt {
            item: OwnedRequest("corrupt".into()),
            fault: InterpreterFault::MissingCapability,
        }),
        SettledItem::Unattempted(OwnedRequest("unattempted".into())),
    ];

    let SourceCustody::Retained(residual) = offer(settlements).await else {
        panic!("source-free failures remain terminal custody");
    };
    let retained: [ActionItemResult<OwnedRequest>; 4] = residual
        .try_into()
        .unwrap_or_else(|_| panic!("four complete settlements remain"));
    let [
        SettledItem::Attempted(ItemSettlement::Rejected {
            item: OwnedRequest(rejected),
            reason: 7,
        }),
        SettledItem::Attempted(ItemSettlement::Blocked {
            item: OwnedRequest(blocked),
            prerequisite: 11,
        }),
        SettledItem::Attempted(ItemSettlement::Corrupt {
            item: OwnedRequest(corrupt),
            fault: InterpreterFault::MissingCapability,
        }),
        SettledItem::Unattempted(OwnedRequest(unattempted)),
    ] = retained
    else {
        panic!("each failure must retain its original item and reason");
    };
    assert_eq!(&*rejected, "rejected");
    assert_eq!(&*blocked, "blocked");
    assert_eq!(&*corrupt, "corrupt");
    assert_eq!(&*unattempted, "unattempted");
}
