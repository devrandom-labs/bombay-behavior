use behavior::{
    Actions, Births, CreateChild, CreationKind, CreationSequence, Creations, Inside,
    InterpreterRequests, MailAddr, Never, NoBirths, Own, ReportToParent, SendEffects, SendLayer,
    Step,
};

type ParentReports = InterpreterRequests<ReportToParent<Box<[u8]>>>;
type StatusReports = InterpreterRequests<ReportToParent<u32>>;
type ChildReports = InterpreterRequests<ReportToParent<u64>>;

#[test]
fn inferred_inner_append_preserves_owned_reports_and_the_original_allocation() {
    let payload: Box<[u8]> = Box::from([3, 7]);
    let allocation = payload.as_ptr();
    let sends = SendLayer::new(
        StatusReports::one(ReportToParent::new(11)),
        ParentReports::empty(),
    );
    let actions: Actions<MailAddr, Never, _, NoBirths> = Actions::send(sends)
        .with_send(ReportToParent::<Box<[u8]>>::new(payload))
        .with_send(ReportToParent::new(13_u32));
    assert!(matches!(actions.become_, Step::Continue));
    assert!(actions.creates.is_empty());
    assert_eq!(
        actions
            .sends
            .owned
            .iter()
            .map(|request| request.report)
            .collect::<Vec<_>>(),
        vec![11, 13]
    );
    let requests = actions.sends.inner.into_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].report.as_ptr(), allocation);
    assert_eq!(&*requests[0].report, &[3, 7]);
}

#[test]
fn inferred_append_crosses_two_layers_without_changing_other_lanes() {
    let mut sends = SendLayer::new(
        ChildReports::one(ReportToParent::new(17)),
        SendLayer::new(
            StatusReports::one(ReportToParent::new(19)),
            ParentReports::empty(),
        ),
    );
    let payload: Box<[u8]> = Box::from([23, 29]);
    let allocation = payload.as_ptr();
    sends.send(ReportToParent::<Box<[u8]>>::new(payload));
    assert_eq!(sends.owned.as_slice()[0].report, 17);
    assert_eq!(sends.inner.owned.as_slice()[0].report, 19);
    assert_eq!(sends.inner.inner.len(), 1);
    assert_eq!(sends.inner.inner.as_slice()[0].report.as_ptr(), allocation);
}

#[test]
fn repeated_report_capabilities_require_an_explicit_static_selection() {
    let mut sends = SendLayer::new(StatusReports::empty(), StatusReports::empty());
    sends.send::<_, (Own, Own)>(ReportToParent::new(31));
    sends.send::<_, Inside<Own>>(ReportToParent::new(37));
    assert_eq!(sends.owned.as_slice()[0].report, 31);
    assert_eq!(sends.inner.as_slice()[0].report, 37);
}

struct Child {
    payload: Box<[u8]>,
}

#[behavior::behavior(addr = MailAddr, message = Never)]
impl Child {
    fn receive(&mut self, _: MailAddr, message: Never) -> behavior::BehaviorActed<Self> {
        match message {}
    }
}

#[test]
fn appended_inner_report_preserves_replacement_request_and_stop() {
    let payload: Box<[u8]> = Box::from([41, 43]);
    let allocation = payload.as_ptr();
    let mut sequence = CreationSequence::new();
    let previous = sequence.issue().expect("two creation requests fit");
    let successor = sequence.issue().expect("two creation requests fit");
    let child = CreateChild::replacement(successor, previous, Child { payload });
    let sends = SendLayer::new(StatusReports::empty(), ParentReports::empty());
    let actions: Actions<MailAddr, Never, _, Births<Child>> =
        Actions::from((sends, Creations::one(child), Step::Stop(behavior::Stopped)))
            .with_send(ReportToParent::new(Box::<[u8]>::from([47, 53])));
    assert!(matches!(actions.become_, Step::Stop(_)));
    assert!(actions.sends.owned.is_empty());
    assert_eq!(actions.sends.inner.len(), 1);
    assert_eq!(&*actions.sends.inner.as_slice()[0].report, &[47, 53]);
    assert_eq!(actions.creates.len(), 1);
    let (id, child, kind) = actions
        .creates
        .into_iter()
        .next()
        .expect("original request survives")
        .into_parts();
    assert_eq!(id, successor);
    assert_eq!(kind, CreationKind::Replacement { previous });
    assert_eq!(child.payload.as_ptr(), allocation);
    assert_eq!(&*child.payload, &[41, 43]);
}
