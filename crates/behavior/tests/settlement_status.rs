//! Generic status projection over exact total action settlements.

use behavior::{
    ActionItem, ActionSettlement, ClassifySettlement, Interpretation, InterpreterFault,
    ItemSettlement, Never, ParentReportReason, ReportToParent, SendLayer, SettledItem,
    SettlementStatus, Step,
};

#[derive(Debug, Eq, PartialEq)]
struct Dependent(u8);

#[derive(Debug, Eq, PartialEq)]
enum DeliveryRejection {
    ParentClosed,
}

#[derive(Debug, Eq, PartialEq)]
enum DeliveryDependency {
    WorkerCreation,
}

impl ActionItem for Dependent {
    type Accepted = ();
    type Rejection = DeliveryRejection;
    type Prerequisite = DeliveryDependency;
}

#[test]
fn two_unrelated_item_kinds_classify_without_losing_the_product() {
    let reports = vec![SettledItem::<ReportToParent<u8>, _>::Attempted(
        ItemSettlement::<ReportToParent<u8>, (), ParentReportReason, Never>::Accepted(()),
    )];
    let dependent = vec![SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Rejected {
        item: Dependent(7),
        reason: DeliveryRejection::ParentClosed,
    })];
    let product = SendLayer::new(reports, dependent);

    assert_eq!(product.settlement_status(), SettlementStatus::Rejected);
    assert_eq!(product.owned.len(), 1);
    assert_eq!(product.inner.len(), 1);
}

#[test]
fn both_wrapper_orders_apply_one_identical_status_law() {
    let accepted_reports = vec![SettledItem::<ReportToParent<u8>, _>::Attempted(
        ItemSettlement::<ReportToParent<u8>, (), ParentReportReason, Never>::Accepted(()),
    )];
    let accepted_dependent = vec![SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Accepted(()))];
    let report_outer = SendLayer::new(accepted_reports, accepted_dependent);
    assert_eq!(report_outer.settlement_status(), SettlementStatus::Accepted);

    let blocked_dependent = vec![SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Blocked {
        item: Dependent(11),
        prerequisite: DeliveryDependency::WorkerCreation,
    })];
    let accepted_reports = vec![SettledItem::<ReportToParent<u8>, _>::Attempted(
        ItemSettlement::<ReportToParent<u8>, (), ParentReportReason, Never>::Accepted(()),
    )];
    let dependent_outer = SendLayer::new(blocked_dependent, accepted_reports);
    assert_eq!(
        dependent_outer.settlement_status(),
        SettlementStatus::Rejected
    );
}

#[test]
fn corruption_and_unattempted_suffix_dominate_expected_rejection() {
    let rejected = SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Rejected {
        item: Dependent(3),
        reason: DeliveryRejection::ParentClosed,
    });
    let corrupt = SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Corrupt {
        item: Dependent(5),
        fault: InterpreterFault::CorruptTraversal,
    });
    let untouched = SettledItem::Unattempted(Dependent(7));
    let complete = vec![rejected, corrupt, untouched];

    assert_eq!(complete.settlement_status(), SettlementStatus::Corrupt);
}

#[test]
fn complete_action_status_preserves_next_decision_and_every_value() {
    let creations = vec![SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
        Dependent,
        (),
        DeliveryRejection,
        DeliveryDependency,
    >::Accepted(()))];
    let sends = SendLayer::new(
        vec![SettledItem::<Dependent, _>::Attempted(ItemSettlement::<
            Dependent,
            (),
            DeliveryRejection,
            DeliveryDependency,
        >::Rejected {
            item: Dependent(13),
            reason: DeliveryRejection::ParentClosed,
        })],
        vec![SettledItem::<ReportToParent<u8>, _>::Attempted(
            ItemSettlement::<ReportToParent<u8>, (), ParentReportReason, Never>::Accepted(()),
        )],
    );
    let settlement = Interpretation::Complete(ActionSettlement::<_, _, Never> {
        creations,
        sends,
        become_: Step::Continue,
    });

    assert_eq!(settlement.settlement_status(), SettlementStatus::Rejected);
    let retained = settlement.into_settlement();
    assert_eq!(retained.creations.len(), 1);
    assert_eq!(retained.sends.owned.len(), 1);
    assert_eq!(retained.sends.inner.len(), 1);
    assert!(matches!(retained.become_, Step::Continue));
}
