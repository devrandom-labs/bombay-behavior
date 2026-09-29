use core::convert::Infallible;
use core::future::Future;

use behavior::{
    ActionItem, ActionItemResult, EventIngress, ItemSettlement, Own, SendEffects, SendLayer,
    SendSettlements, SettledItem, SourceAction, SourceActions, SourceAdmission, SourceCustody,
    SourceSettlementCustody,
};
use behavior_actors::atomic::{DiagnosticAccepted, DiagnosticAction};

struct ReplySource;

struct ReplyRequest(Box<str>);

impl ActionItem for ReplyRequest {
    type Accepted = ();
    type Rejection = Infallible;
    type Prerequisite = Infallible;
}

impl SourceAction for ReplyRequest {
    type Source = ReplySource;
}

struct ReturnEvent(ActionItemResult<ReplyRequest>);

impl EventIngress<ReplySource, ActionItemResult<ReplyRequest>> for ReturnEvent {
    fn ingress(input: ActionItemResult<ReplyRequest>) -> Self {
        Self(input)
    }
}

enum AdmissionWindow {
    Open,
    Closed,
}

struct ReturnHost {
    window: AdmissionWindow,
    admitted: Vec<ReturnEvent>,
}

impl SourceAdmission<ReturnEvent, ReplySource, ActionItemResult<ReplyRequest>> for ReturnHost {
    fn admit_source(
        &mut self,
        input: ActionItemResult<ReplyRequest>,
    ) -> impl Future<Output = Result<(), ActionItemResult<ReplyRequest>>> + Send {
        async move {
            match self.window {
                AdmissionWindow::Open => {
                    self.admitted.push(ReturnEvent::ingress(input));
                    Ok(())
                }
                AdmissionWindow::Closed => Err(input),
            }
        }
    }
}

type TerminalDiagnostic = DiagnosticAction<Infallible, Box<str>>;

fn retained_diagnostic() -> Vec<ActionItemResult<TerminalDiagnostic>> {
    vec![SettledItem::Attempted(ItemSettlement::Accepted(
        DiagnosticAccepted::terminal(Box::from("kept")),
    ))]
}

fn source_result() -> behavior::SourceSettlements<ReplyRequest> {
    let source = <SourceActions<ReplyRequest> as SendEffects>::sending::<ReplyRequest, Own>(
        ReplyRequest(Box::from("returned")),
    );
    source.unattempted()
}

fn assert_admitted(host: &mut ReturnHost) {
    let ReturnEvent(SettledItem::Unattempted(ReplyRequest(value))) =
        host.admitted.pop().expect("one exact admitted input")
    else {
        panic!("the source receives its original untouched request");
    };
    assert_eq!(&*value, "returned");
    assert!(host.admitted.is_empty());
}

fn assert_terminal(residual: Vec<ActionItemResult<TerminalDiagnostic>>) {
    let retained: [ActionItemResult<TerminalDiagnostic>; 1] = residual
        .try_into()
        .unwrap_or_else(|_| panic!("one terminal diagnostic survives"));
    let [SettledItem::Attempted(ItemSettlement::Accepted(DiagnosticAccepted::Terminal(value)))] =
        retained
    else {
        panic!("the accepted terminal value remains unchanged");
    };
    assert_eq!(&*value, "kept");
}

#[tokio::test]
async fn retained_inner_diagnostic_allows_later_owned_source_admission() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Open,
        admitted: Vec::new(),
    };
    let product = SendLayer::new(source_result(), retained_diagnostic());
    let SourceCustody::Admitted(residual) = <_ as SourceSettlementCustody<
        ReturnHost,
        ReturnEvent,
    >>::offer_next_to_source(product, &mut host)
    .await
    else {
        panic!("the later source lane must progress");
    };
    assert_admitted(&mut host);

    let SourceCustody::Retained(residual) = residual.offer_next_to_source(&mut host).await else {
        panic!("terminal custody survives the admitted source input");
    };
    assert_terminal(residual.inner);
    assert!(residual.owned.into_inputs().is_empty());
}

#[tokio::test]
async fn inner_source_admission_precedes_owned_terminal_custody() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Open,
        admitted: Vec::new(),
    };
    let product = SendLayer::new(retained_diagnostic(), source_result());
    let SourceCustody::Admitted(residual) = <_ as SourceSettlementCustody<
        ReturnHost,
        ReturnEvent,
    >>::offer_next_to_source(product, &mut host)
    .await
    else {
        panic!("the inner source lane must progress first");
    };
    assert_admitted(&mut host);

    let SourceCustody::Retained(residual) = residual.offer_next_to_source(&mut host).await else {
        panic!("terminal custody survives after source admission");
    };
    assert!(residual.inner.into_inputs().is_empty());
    assert_terminal(residual.owned);
}

#[tokio::test]
async fn closed_source_retains_its_request_and_terminal_sibling() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Closed,
        admitted: Vec::new(),
    };
    let product = SendLayer::new(source_result(), retained_diagnostic());
    let SourceCustody::Closed(residual) =
        <_ as SourceSettlementCustody<ReturnHost, ReturnEvent>>::offer_next_to_source(
            product, &mut host,
        )
        .await
    else {
        panic!("closed admission retains the entire remaining product");
    };
    assert!(host.admitted.is_empty());
    assert_terminal(residual.inner);
    let source: [ActionItemResult<ReplyRequest>; 1] = residual
        .owned
        .into_inputs()
        .try_into()
        .unwrap_or_else(|_| panic!("one source request remains"));
    let [SettledItem::Unattempted(ReplyRequest(value))] = source else {
        panic!("the closed source request remains exact");
    };
    assert_eq!(&*value, "returned");
}
