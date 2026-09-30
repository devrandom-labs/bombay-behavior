use core::convert::Infallible;
use core::future::Future;

use behavior::{
    ActionItem, ActionItemResult, ActionSettlement, ActiveTurn, Address, Behavior, BehaviorActed,
    ChildNamespaceExhausted, CreateChild, CreationSequence, CreationSettlement,
    CreationSettlements, Creations, EndpointAddress, EventIngress, ItemSettlement, MessageProtocol,
    Never, NoBirths, NoSends, Own, Protocol, RetirementBirths, SendEffects, SendLayer,
    SendSettlements, SettledItem, SourceAction, SourceActions, SourceAdmission, SourceCustody,
    SourceSettlementCustody, Step, Stopped, User,
};
use behavior_actors::atomic::{DiagnosticAccepted, DiagnosticAction};

mod installed_control;

struct ReplySource;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeAddr(u64);

impl Address for RuntimeAddr {
    type Nonce = u64;
}

impl EndpointAddress for RuntimeAddr {
    type Established<P>
        = u64
    where
        P: Protocol<Addr = Self>;

    type Installed<B>
        = installed_control::InstalledControl<B, u64>
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>;

    fn recipient<B>(installed: &Self::Installed<B>) -> u64
    where
        B: Behavior<Protocol: Protocol<Addr = Self>>,
    {
        *installed.endpoint()
    }
}

struct Child(Box<str>);

impl Behavior for Child {
    type Protocol = MessageProtocol<RuntimeAddr, Never>;
    type Event = User<RuntimeAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        match input.message {}
    }
}

type PendingRetirement = <RetirementBirths<Child> as CreationSettlements<RuntimeAddr>>::Settlements;

fn retained_creation() -> PendingRetirement {
    let id = CreationSequence::new()
        .issue()
        .expect("one child creation ID is available");
    behavior::RetirementCreationSettlement::new(CreationSettlement::Rejected {
        creations: Creations::one(CreateChild::birth(id, Child("uncreated child".into()))),
        reason: ChildNamespaceExhausted,
    })
}

fn assert_creation(settlement: PendingRetirement) {
    let CreationSettlement::Rejected { creations, .. } = settlement.into_settlement() else {
        panic!("the complete rejected creation batch remains owned");
    };
    let [creation]: [_; 1] = creations
        .into_iter()
        .collect::<Vec<_>>()
        .try_into()
        .unwrap_or_else(|_| panic!("one rejected child remains"));
    assert_eq!(creation.into_parts().1.0.as_ref(), "uncreated child");
}

struct IndependentRequest(Box<str>);

impl ActionItem for IndependentRequest {
    type Accepted = ();
    type Rejection = u8;
    type Prerequisite = Infallible;
}

fn independent_rejection() -> Vec<ActionItemResult<IndependentRequest>> {
    vec![SettledItem::Attempted(ItemSettlement::Rejected {
        item: IndependentRequest("independent rejection".into()),
        reason: 4,
    })]
}

fn assert_independent(mut residual: Vec<ActionItemResult<IndependentRequest>>) {
    let Some(SettledItem::Attempted(ItemSettlement::Rejected {
        item: IndependentRequest(value),
        reason: 4,
    })) = residual.pop()
    else {
        panic!("the independent request and exact reason remain owned");
    };
    assert_eq!(value.as_ref(), "independent rejection");
    assert!(residual.is_empty());
}

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

#[tokio::test]
async fn mixed_retirement_product_admits_source_after_retained_inner_lanes() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Open,
        admitted: Vec::new(),
    };
    let settlement = ActionSettlement {
        creations: retained_creation(),
        sends: SendLayer::new(
            source_result(),
            SendLayer::new(retained_diagnostic(), independent_rejection()),
        ),
        become_: Step::<Never, Stopped>::Continue,
    };
    let SourceCustody::Admitted(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("retained creation and inner lanes must not block source admission");
    };
    assert_admitted(&mut host);
    let SourceCustody::Retained(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("remaining creation, diagnostic, and rejection need terminal custody");
    };
    assert_creation(settlement.creations);
    assert!(settlement.sends.owned.into_inputs().is_empty());
    assert_terminal(settlement.sends.inner.owned);
    assert_independent(settlement.sends.inner.inner);
    assert!(matches!(settlement.become_, Step::Continue));
}

#[tokio::test]
async fn mixed_retirement_product_admits_source_through_the_other_layer_order() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Open,
        admitted: Vec::new(),
    };
    let settlement = ActionSettlement {
        creations: retained_creation(),
        sends: SendLayer::new(
            SendLayer::new(source_result(), independent_rejection()),
            retained_diagnostic(),
        ),
        become_: Step::<Never, Stopped>::Continue,
    };
    let SourceCustody::Admitted(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("retained creation and diagnostic must not block inner source admission");
    };
    assert_admitted(&mut host);
    let SourceCustody::Retained(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("remaining creation, diagnostic, and rejection need terminal custody");
    };
    assert_creation(settlement.creations);
    assert!(settlement.sends.owned.owned.into_inputs().is_empty());
    assert_independent(settlement.sends.owned.inner);
    assert_terminal(settlement.sends.inner);
    assert!(matches!(settlement.become_, Step::Continue));
}

#[tokio::test]
async fn mixed_retirement_product_returns_every_lane_when_source_closes() {
    let mut host = ReturnHost {
        window: AdmissionWindow::Closed,
        admitted: Vec::new(),
    };
    let settlement = ActionSettlement {
        creations: retained_creation(),
        sends: SendLayer::new(
            source_result(),
            SendLayer::new(retained_diagnostic(), independent_rejection()),
        ),
        become_: Step::<Never, Stopped>::Continue,
    };
    let SourceCustody::Closed(settlement) = settlement.offer_next_to_source(&mut host).await else {
        panic!("a closed source returns the complete mixed product");
    };
    assert!(host.admitted.is_empty());
    assert_creation(settlement.creations);
    let source: [ActionItemResult<ReplyRequest>; 1] = settlement
        .sends
        .owned
        .into_inputs()
        .try_into()
        .unwrap_or_else(|_| panic!("the source request remains pending"));
    let [SettledItem::Unattempted(ReplyRequest(value))] = source else {
        panic!("the exact source request remains untouched");
    };
    assert_eq!(value.as_ref(), "returned");
    assert_terminal(settlement.sends.inner.owned);
    assert_independent(settlement.sends.inner.inner);
    assert!(matches!(settlement.become_, Step::Continue));
}
