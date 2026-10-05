use core::convert::Infallible;

use behavior::{
    ActionItem, ActionItemResult, Delivery, InterpretationProgress, InterpreterFault,
    ItemSettlement, MailAddr, MessageProtocol, ReportToParent, SettledItem, SourceCustody,
    SourceProgress, SourceSettlementCustody, finish_item, prepare_item,
};
use behavior_actors::atomic::{DiagnosticAccepted, DiagnosticAction};

struct OwnedRequest(Box<str>);

impl ActionItem for OwnedRequest {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = ();
    type Rejection = u8;
    type Prerequisite = u16;
}

type TerminalDiagnostic = DiagnosticAction<Infallible, Box<str>>;
type OrdinaryDelivery = Delivery<MessageProtocol<MailAddr, u8>>;

async fn offer<Item: ActionItem>(
    settlements: Vec<ActionItemResult<Item>>,
) -> SourceCustody<Vec<ActionItemResult<Item>>> {
    {
        let mut source_progress = Some(SourceProgress::Original(settlements));
        <Vec<ActionItemResult<Item>> as SourceSettlementCustody<(), ()>>::prepare_source(
            &mut source_progress,
        );
        if let Some(SourceProgress::Offering(custody)) = &mut source_progress {
            <Vec<ActionItemResult<Item>> as SourceSettlementCustody<(), ()>>::offer_next_to_source(
                custody,
                &mut (),
            )
            .await;
        }
        <Vec<ActionItemResult<Item>> as SourceSettlementCustody<(), ()>>::finish_source(
            &mut source_progress,
        );
        let Some(SourceProgress::Completed(custody)) = source_progress else {
            panic!("the complete original source row did not finish");
        };
        custody
    }
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

#[tokio::test]
async fn cold_nonstatic_parent_report_keeps_the_actual_borrowed_slice() {
    let original = vec![17_u64, 31, 43];
    let original_pointer = original.as_ptr();
    let request = ReportToParent::new(original.as_slice());
    let settlements: Vec<ActionItemResult<ReportToParent<&[u64]>>> =
        vec![SettledItem::Unattempted(request)];
    let SourceCustody::Retained(mut returned) = offer(settlements).await else {
        panic!("the actual unattempted parent report stays in cold source custody");
    };
    let settled = returned
        .pop()
        .expect("the original parent report is retained");
    let SettledItem::Unattempted(request) = settled else {
        panic!("source custody attempted or reclassified the unattempted parent report");
    };
    let report = request.into_inner();
    assert_eq!(report.as_ptr(), original_pointer);
    assert_eq!(report, &[17, 31, 43]);
    assert!(returned.is_empty());
}
