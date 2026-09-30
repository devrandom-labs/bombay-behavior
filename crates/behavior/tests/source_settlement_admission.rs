use behavior::{
    ActionItem, ActionItemResult, ActionSettlement, BehaviorActed, ClassifySettlement, Creations,
    EventIngress, InterpretItem, InterpretSends, Interpretation, InterpreterFault, ItemSettlement,
    MailAddr, Never, SendEffects, SendLayer, SettledItem, SettlementStatus, SourceAction,
    SourceActions, SourceAdmission, SourceCustody, SourceSettlementCustody, Step,
};
use core::future::Future;
use std::collections::VecDeque;

struct ProxyOwner;
struct PoolOwner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OperationTicket(u8);

#[derive(Debug, Eq, PartialEq)]
struct ProxyOperation {
    ticket: OperationTicket,
    command: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProxyRejection {
    Closed,
}

impl ActionItem for ProxyOperation {
    type Accepted = OperationTicket;
    type Rejection = ProxyRejection;
    type Prerequisite = Never;
}

impl SourceAction for ProxyOperation {
    type Source = ProxyOwner;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AssignmentToken(u8);

#[derive(Debug, Eq, PartialEq)]
struct AssignmentDelivery {
    token: AssignmentToken,
    payload: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AssignmentRejection {
    WorkerStopped,
}

impl ActionItem for AssignmentDelivery {
    type Accepted = AssignmentToken;
    type Rejection = AssignmentRejection;
    type Prerequisite = Never;
}

impl SourceAction for AssignmentDelivery {
    type Source = PoolOwner;
}

enum SystemEvent {
    Proxy(ActionItemResult<ProxyOperation>),
    Assignment(ActionItemResult<AssignmentDelivery>),
}

impl EventIngress<ProxyOwner, ActionItemResult<ProxyOperation>> for SystemEvent {
    fn ingress(input: ActionItemResult<ProxyOperation>) -> Self {
        Self::Proxy(input)
    }
}

impl EventIngress<PoolOwner, ActionItemResult<AssignmentDelivery>> for SystemEvent {
    fn ingress(input: ActionItemResult<AssignmentDelivery>) -> Self {
        Self::Assignment(input)
    }
}

#[allow(dead_code, reason = "compile-only generated source-admission contract")]
struct Generated;

#[derive(behavior_macros::SendProduct)]
struct SourceAdmissionSends<ProxySends, AssignmentSends> {
    proxy: ProxySends,
    assignment: AssignmentSends,
}

#[behavior::behavior(
    addr = MailAddr,
    message = Never,
    sends = {
        proxy: SourceActions<ProxyOperation>,
        assignment: SourceActions<AssignmentDelivery>,
    },
)]
impl Generated {
    #[allow(
        dead_code,
        reason = "the message type is uninhabited in this compile contract"
    )]
    fn receive(&mut self, _: MailAddr, message: Never) -> BehaviorActed<Self> {
        match message {}
    }
}

#[derive(Clone, Copy)]
enum AttemptPlan {
    Accept,
    Reject,
    Corrupt,
}

struct Runtime {
    proxy: VecDeque<AttemptPlan>,
    assignments: VecDeque<AttemptPlan>,
}

impl Runtime {
    fn new(proxy: Vec<AttemptPlan>, assignments: Vec<AttemptPlan>) -> Self {
        Self {
            proxy: proxy.into(),
            assignments: assignments.into(),
        }
    }
}

impl<RootEvent, Path> InterpretItem<ProxyOperation, RootEvent, Path> for Runtime {
    fn interpret_item(
        &mut self,
        item: ProxyOperation,
    ) -> impl Future<Output = ItemSettlement<ProxyOperation, OperationTicket, ProxyRejection, Never>>
    + Send {
        let plan = self.proxy.pop_front();
        async move {
            match plan {
                Some(AttemptPlan::Accept) => ItemSettlement::Accepted(item.ticket),
                Some(AttemptPlan::Reject) => ItemSettlement::Rejected {
                    item,
                    reason: ProxyRejection::Closed,
                },
                Some(AttemptPlan::Corrupt) | None => ItemSettlement::Corrupt {
                    item,
                    fault: InterpreterFault::CorruptTraversal,
                },
            }
        }
    }
}

impl<RootEvent, Path> InterpretItem<AssignmentDelivery, RootEvent, Path> for Runtime {
    fn interpret_item(
        &mut self,
        item: AssignmentDelivery,
    ) -> impl Future<
        Output = ItemSettlement<AssignmentDelivery, AssignmentToken, AssignmentRejection, Never>,
    > + Send {
        let plan = self.assignments.pop_front();
        async move {
            match plan {
                Some(AttemptPlan::Accept) => ItemSettlement::Accepted(item.token),
                Some(AttemptPlan::Reject) => ItemSettlement::Rejected {
                    item,
                    reason: AssignmentRejection::WorkerStopped,
                },
                Some(AttemptPlan::Corrupt) | None => ItemSettlement::Corrupt {
                    item,
                    fault: InterpreterFault::CorruptTraversal,
                },
            }
        }
    }
}

#[derive(Clone, Copy)]
enum AdmissionWindow {
    Unlimited,
    One,
    Closed,
}

#[derive(Debug, Eq, PartialEq)]
enum AdmissionTrace {
    Proxy(OperationTicket),
    Assignment(AssignmentToken),
}

struct Host {
    window: AdmissionWindow,
    trace: Vec<AdmissionTrace>,
}

impl Host {
    fn new(window: AdmissionWindow) -> Self {
        Self {
            window,
            trace: Vec::new(),
        }
    }

    fn offer<Input>(&mut self, input: Input, trace: AdmissionTrace) -> Result<(), Input> {
        match self.window {
            AdmissionWindow::Unlimited => {
                self.trace.push(trace);
                Ok(())
            }
            AdmissionWindow::One => {
                self.trace.push(trace);
                self.window = AdmissionWindow::Closed;
                Ok(())
            }
            AdmissionWindow::Closed => Err(input),
        }
    }
}

impl SourceAdmission<SystemEvent, ProxyOwner, ActionItemResult<ProxyOperation>> for Host {
    fn admit_source(
        &mut self,
        input: ActionItemResult<ProxyOperation>,
    ) -> impl Future<Output = Result<(), ActionItemResult<ProxyOperation>>> + Send {
        async move {
            let input = match SystemEvent::ingress(input) {
                SystemEvent::Proxy(input) => input,
                SystemEvent::Assignment(_) => panic!("proxy ingress selected the wrong variant"),
            };
            let ticket = match &input {
                SettledItem::Attempted(ItemSettlement::Accepted(ticket)) => *ticket,
                SettledItem::Attempted(
                    ItemSettlement::Rejected {
                        item: operation, ..
                    }
                    | ItemSettlement::Corrupt {
                        item: operation, ..
                    },
                )
                | SettledItem::Unattempted(operation) => operation.ticket,
                SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                    match *prerequisite {}
                }
            };
            self.offer(input, AdmissionTrace::Proxy(ticket))
        }
    }
}

impl SourceAdmission<SystemEvent, PoolOwner, ActionItemResult<AssignmentDelivery>> for Host {
    fn admit_source(
        &mut self,
        input: ActionItemResult<AssignmentDelivery>,
    ) -> impl Future<Output = Result<(), ActionItemResult<AssignmentDelivery>>> + Send {
        async move {
            let input = match SystemEvent::ingress(input) {
                SystemEvent::Assignment(input) => input,
                SystemEvent::Proxy(_) => panic!("assignment ingress selected the wrong variant"),
            };
            let token = match &input {
                SettledItem::Attempted(ItemSettlement::Accepted(token)) => *token,
                SettledItem::Attempted(
                    ItemSettlement::Rejected { item: delivery, .. }
                    | ItemSettlement::Corrupt { item: delivery, .. },
                )
                | SettledItem::Unattempted(delivery) => delivery.token,
                SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
                    match *prerequisite {}
                }
            };
            self.offer(input, AdmissionTrace::Assignment(token))
        }
    }
}

fn proxy(ticket: u8, command: &str) -> ProxyOperation {
    ProxyOperation {
        ticket: OperationTicket(ticket),
        command: command.to_owned(),
    }
}

fn assignment(token: u8, payload: &str) -> AssignmentDelivery {
    AssignmentDelivery {
        token: AssignmentToken(token),
        payload: payload.to_owned(),
    }
}

fn source_actions<Item>(items: impl IntoIterator<Item = Item>) -> SourceActions<Item>
where
    Item: SourceAction,
{
    let mut actions = <SourceActions<Item> as behavior::SendEffects>::empty();
    for item in items {
        behavior::SendInput::<Item, behavior::Own>::emit(&mut actions, item);
    }
    actions
}

#[tokio::test]
async fn authored_named_product_preserves_source_admission_and_corrupt_suffix() {
    type Sends =
        SourceAdmissionSends<SourceActions<ProxyOperation>, SourceActions<AssignmentDelivery>>;
    type Settled = SourceAdmissionSends<
        behavior::SourceSettlements<ProxyOperation>,
        behavior::SourceSettlements<AssignmentDelivery>,
    >;
    type Actual = <Sends as behavior::SendSettlements>::Settlements;
    let _: core::marker::PhantomData<Settled> = core::marker::PhantomData::<Actual>;

    let mut sends = Sends::empty();
    sends.append(Sends {
        proxy: source_actions([proxy(1, "accepted")]),
        assignment: source_actions([assignment(2, "closed")]),
    });
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept]);
    let Interpretation::Complete(settled) = <Sends as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(sends, &mut runtime)
    .await
    else {
        panic!("both authored lanes must settle");
    };
    assert_eq!(settled.settlement_status(), SettlementStatus::Accepted);
    let mut host = Host::new(AdmissionWindow::One);
    let SourceCustody::Admitted(settled) = settled.offer_next_to_source(&mut host).await else {
        panic!("the first source result must be admitted");
    };
    let SourceCustody::Closed(residual) = settled.offer_next_to_source(&mut host).await else {
        panic!("closed admission must return the second result");
    };
    assert_eq!(host.trace, [AdmissionTrace::Proxy(OperationTicket(1))]);
    let proxy_results = residual.proxy.into_inputs();
    let assignment_results = residual.assignment.into_inputs();
    assert!(proxy_results.is_empty());
    assert!(matches!(
        assignment_results.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(
            AssignmentToken(2)
        ))]
    ));

    let sends = Sends {
        proxy: source_actions([proxy(3, "corrupt")]),
        assignment: source_actions([assignment(4, "untouched")]),
    };
    let mut runtime = Runtime::new(vec![AttemptPlan::Corrupt], vec![AttemptPlan::Accept]);
    let Interpretation::Corrupt(settled) = <Sends as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(sends, &mut runtime)
    .await
    else {
        panic!("corruption must retain the untouched assignment suffix");
    };
    assert_eq!(settled.settlement_status(), SettlementStatus::Corrupt);
    assert_eq!(runtime.assignments.len(), 1);
    let mut host = Host::new(AdmissionWindow::Closed);
    let SourceCustody::Closed(residual) = settled.offer_next_to_source(&mut host).await else {
        panic!("closed admission must return the complete corrupt product");
    };
    assert!(host.trace.is_empty());
    let proxy_results = residual.proxy.into_inputs();
    let assignment_results = residual.assignment.into_inputs();
    assert!(matches!(
        proxy_results.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Corrupt { item, .. })]
            if item.ticket == OperationTicket(3) && item.command == "corrupt"
    ));
    assert!(matches!(
        assignment_results.as_slice(),
        [SettledItem::Unattempted(item)]
            if item.token == AssignmentToken(4) && item.payload == "untouched"
    ));

    let retained = SourceAdmissionSends {
        proxy: vec![SettledItem::Attempted(ItemSettlement::Rejected {
            item: proxy(5, "retained"),
            reason: ProxyRejection::Closed,
        })],
        assignment: Vec::<ActionItemResult<AssignmentDelivery>>::new(),
    };
    assert_eq!(retained.settlement_status(), SettlementStatus::Rejected);
    let mut host = Host::new(AdmissionWindow::Unlimited);
    let SourceCustody::Retained(residual) =
        SourceSettlementCustody::<Host, SystemEvent>::offer_next_to_source(retained, &mut host)
            .await
    else {
        panic!("a rejected source result must remain retained");
    };
    assert!(host.trace.is_empty());
    assert!(matches!(
        residual.proxy.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Rejected { item, .. })]
            if item.ticket == OperationTicket(5) && item.command == "retained"
    ));
    assert!(residual.assignment.is_empty());
}

#[tokio::test]
async fn authored_product_preserves_custody_in_both_send_layer_orders() {
    type Product =
        SourceAdmissionSends<SourceActions<ProxyOperation>, SourceActions<AssignmentDelivery>>;

    let product = Product {
        proxy: source_actions([proxy(1, "first")]),
        assignment: source_actions([assignment(2, "second")]),
    };
    let sends = SendLayer::new(source_actions([assignment(3, "outer")]), product);
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept; 2]);
    let Interpretation::Complete(mut settled) =
        <SendLayer<SourceActions<AssignmentDelivery>, Product> as InterpretSends<
            Runtime,
            SystemEvent,
            behavior::Here,
        >>::interpret(sends, &mut runtime)
        .await
    else {
        panic!("the nested authored product must settle");
    };
    let mut host = Host::new(AdmissionWindow::Unlimited);
    for expected in [
        AdmissionTrace::Proxy(OperationTicket(1)),
        AdmissionTrace::Assignment(AssignmentToken(2)),
        AdmissionTrace::Assignment(AssignmentToken(3)),
    ] {
        let SourceCustody::Admitted(next) = settled.offer_next_to_source(&mut host).await else {
            panic!("each nested source result must transfer once");
        };
        settled = next;
        assert_eq!(host.trace.last(), Some(&expected));
    }
    let exhausted = settled.offer_next_to_source(&mut host).await;
    assert!(matches!(exhausted, SourceCustody::Exhausted(_)));
    assert_eq!(host.trace.len(), 3);

    let product = Product {
        proxy: source_actions([proxy(5, "second")]),
        assignment: source_actions([assignment(6, "third")]),
    };
    let sends = SendLayer::new(product, source_actions([proxy(4, "inner")]));
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept; 2], vec![AttemptPlan::Accept]);
    let Interpretation::Complete(mut settled) =
        <SendLayer<Product, SourceActions<ProxyOperation>> as InterpretSends<
            Runtime,
            SystemEvent,
            behavior::Here,
        >>::interpret(sends, &mut runtime)
        .await
    else {
        panic!("the outer authored product must settle");
    };
    let mut host = Host::new(AdmissionWindow::Unlimited);
    for expected in [
        AdmissionTrace::Proxy(OperationTicket(4)),
        AdmissionTrace::Proxy(OperationTicket(5)),
        AdmissionTrace::Assignment(AssignmentToken(6)),
    ] {
        let SourceCustody::Admitted(next) = settled.offer_next_to_source(&mut host).await else {
            panic!("each outer source result must transfer once");
        };
        settled = next;
        assert_eq!(host.trace.last(), Some(&expected));
    }
    let exhausted = settled.offer_next_to_source(&mut host).await;
    assert!(matches!(exhausted, SourceCustody::Exhausted(_)));
    assert_eq!(host.trace.len(), 3);
}

#[tokio::test]
async fn source_actions_normalize_rejection_corruption_and_unattempted_values() {
    let actions = source_actions([
        proxy(1, "accepted"),
        proxy(2, "rejected"),
        proxy(3, "corrupt"),
        proxy(4, "unattempted"),
    ]);
    let mut runtime = Runtime::new(
        vec![
            AttemptPlan::Accept,
            AttemptPlan::Reject,
            AttemptPlan::Corrupt,
        ],
        Vec::new(),
    );
    let interpreted = <SourceActions<ProxyOperation> as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(actions, &mut runtime)
    .await;
    let Interpretation::Corrupt(settlements) = interpreted else {
        panic!("the third source action must corrupt interpretation");
    };
    let mut host = Host::new(AdmissionWindow::Closed);
    let SourceCustody::Closed(residual) = settlements.offer_next_to_source(&mut host).await else {
        panic!("closed source admission must retain every result");
    };

    let residual = residual.into_inputs();
    assert_eq!(residual.len(), 4);
    assert!(matches!(
        &residual[1],
        SettledItem::Attempted(ItemSettlement::Rejected { item: operation, .. })
            if operation.command == "rejected"
    ));
    assert!(matches!(
        &residual[3],
        SettledItem::Unattempted(operation) if operation.command == "unattempted"
    ));
}

#[tokio::test]
async fn both_send_layer_orders_match_interpretation_order() {
    let sends = SendLayer::new(
        source_actions([proxy(2, "outer")]),
        source_actions([assignment(1, "inner")]),
    );
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept]);
    let interpreted = <SendLayer<
        SourceActions<ProxyOperation>,
        SourceActions<AssignmentDelivery>,
    > as InterpretSends<Runtime, SystemEvent, behavior::Here>>::interpret(
        sends, &mut runtime
    )
    .await;
    let Interpretation::Complete(settlements) = interpreted else {
        panic!("both source actions must settle");
    };
    let mut host = Host::new(AdmissionWindow::Unlimited);
    let SourceCustody::Admitted(settlements) = settlements.offer_next_to_source(&mut host).await
    else {
        panic!("the inner assignment result must be admitted first");
    };
    assert_eq!(host.trace, [AdmissionTrace::Assignment(AssignmentToken(1))]);
    let SourceCustody::Admitted(settlements) = settlements.offer_next_to_source(&mut host).await
    else {
        panic!("the outer proxy result must be admitted second");
    };
    let exhausted = settlements.offer_next_to_source(&mut host).await;
    assert!(matches!(exhausted, SourceCustody::Exhausted(_)));
    assert_eq!(
        host.trace,
        [
            AdmissionTrace::Assignment(AssignmentToken(1)),
            AdmissionTrace::Proxy(OperationTicket(2)),
        ]
    );

    let sends = SendLayer::new(
        source_actions([assignment(4, "outer")]),
        source_actions([proxy(3, "inner")]),
    );
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept]);
    let interpreted = <SendLayer<
        SourceActions<AssignmentDelivery>,
        SourceActions<ProxyOperation>,
    > as InterpretSends<Runtime, SystemEvent, behavior::Here>>::interpret(
        sends, &mut runtime
    )
    .await;
    let Interpretation::Complete(settlements) = interpreted else {
        panic!("both source actions must settle");
    };
    let mut host = Host::new(AdmissionWindow::Unlimited);
    let SourceCustody::Admitted(settlements) = settlements.offer_next_to_source(&mut host).await
    else {
        panic!("the inner proxy result must be admitted first");
    };
    assert_eq!(host.trace, [AdmissionTrace::Proxy(OperationTicket(3))]);
    let SourceCustody::Admitted(settlements) = settlements.offer_next_to_source(&mut host).await
    else {
        panic!("the outer assignment result must be admitted second");
    };
    let exhausted = settlements.offer_next_to_source(&mut host).await;
    assert!(matches!(exhausted, SourceCustody::Exhausted(_)));
    assert_eq!(
        host.trace,
        [
            AdmissionTrace::Proxy(OperationTicket(3)),
            AdmissionTrace::Assignment(AssignmentToken(4)),
        ]
    );
}

#[tokio::test]
async fn nested_products_offer_only_the_first_remaining_source() {
    let first = source_actions([proxy(1, "first")]);
    let second = source_actions([assignment(2, "second")]);
    let third = source_actions([proxy(3, "third")]);
    let mut runtime = Runtime::new(
        vec![AttemptPlan::Accept, AttemptPlan::Accept],
        vec![AttemptPlan::Accept],
    );
    let Interpretation::Complete(first) = <SourceActions<ProxyOperation> as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(first, &mut runtime)
    .await
    else {
        panic!("the first source must settle");
    };
    let Interpretation::Complete(second) = <SourceActions<AssignmentDelivery> as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(second, &mut runtime)
    .await
    else {
        panic!("the second source must settle");
    };
    let Interpretation::Complete(third) = <SourceActions<ProxyOperation> as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(third, &mut runtime)
    .await
    else {
        panic!("the third source must settle");
    };
    let mut residual = ((first, second), third);
    let mut host = Host::new(AdmissionWindow::Unlimited);

    for expected in [
        AdmissionTrace::Proxy(OperationTicket(1)),
        AdmissionTrace::Assignment(AssignmentToken(2)),
        AdmissionTrace::Proxy(OperationTicket(3)),
    ] {
        let SourceCustody::Admitted(next) = residual.offer_next_to_source(&mut host).await else {
            panic!("exactly one nested source must transfer per offer");
        };
        residual = next;
        assert_eq!(host.trace.last(), Some(&expected));
    }
    let exhausted = residual.offer_next_to_source(&mut host).await;
    assert!(matches!(exhausted, SourceCustody::Exhausted(_)));
}

#[tokio::test]
async fn generated_product_stops_after_closed_source_and_retains_later_field() {
    let sends = GeneratedSends {
        proxy: source_actions([proxy(1, "proxy")]),
        assignment: source_actions([assignment(2, "assignment")]),
    };
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept]);
    let interpreted =
        <GeneratedSends as InterpretSends<Runtime, SystemEvent, behavior::Here>>::interpret(
            sends,
            &mut runtime,
        )
        .await;
    let Interpretation::Complete(settlements) = interpreted else {
        panic!("both generated fields must settle");
    };
    let mut host = Host::new(AdmissionWindow::One);
    let SourceCustody::Admitted(settlements) = settlements.offer_next_to_source(&mut host).await
    else {
        panic!("the proxy result must be the only first admission");
    };
    let SourceCustody::Closed(residual) = settlements.offer_next_to_source(&mut host).await else {
        panic!("the host closes before the assignment result transfers");
    };

    assert_eq!(host.trace, [AdmissionTrace::Proxy(OperationTicket(1))]);
    let proxy_inputs = residual.proxy.into_inputs();
    assert_eq!(proxy_inputs, []);
    let assignment_inputs = residual.assignment.into_inputs();
    assert!(matches!(
        assignment_inputs.as_slice(),
        [SettledItem::Attempted(ItemSettlement::Accepted(
            AssignmentToken(2)
        ))]
    ));
}

#[tokio::test]
async fn complete_action_custody_offers_only_one_source_result() {
    let sends = GeneratedSends {
        proxy: source_actions([proxy(1, "proxy")]),
        assignment: source_actions([assignment(2, "assignment")]),
    };
    let mut runtime = Runtime::new(vec![AttemptPlan::Accept], vec![AttemptPlan::Accept]);
    let Interpretation::Complete(sends) = <GeneratedSends as InterpretSends<
        Runtime,
        SystemEvent,
        behavior::Here,
    >>::interpret(sends, &mut runtime)
    .await
    else {
        panic!("both generated fields must settle");
    };
    let settlement = ActionSettlement {
        creations: Creations::<Never>::empty(),
        sends,
        become_: behavior::Become::<Never>::Continue,
    };
    let mut host = Host::new(AdmissionWindow::Unlimited);

    let SourceCustody::Admitted(settlement) = settlement.offer_next_to_source(&mut host).await
    else {
        panic!("complete action custody must admit one result");
    };

    assert_eq!(host.trace, [AdmissionTrace::Proxy(OperationTicket(1))]);
    let admitted = settlement.offer_next_to_source(&mut host).await;
    assert!(matches!(admitted, SourceCustody::Admitted(_)));
}

#[test]
fn generated_product_is_statically_lawful_for_both_source_inputs() {
    fn requires_source_inputs<S: behavior::SendsFor<SystemEvent>>() {}
    requires_source_inputs::<GeneratedSends>();
    assert_eq!(Step::<Never>::Continue, Step::Continue);
}
