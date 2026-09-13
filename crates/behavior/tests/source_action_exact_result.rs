use behavior::{
    ActionItem, Never, Own, SendEffects, SendInput, SendSettlements, SettledItem, SourceAction,
    SourceActions,
};

struct Owner;

#[derive(Debug, Eq, PartialEq)]
struct Operation(String);

impl ActionItem for Operation {
    type Accepted = ();
    type Rejection = Never;
    type Prerequisite = Never;
}

impl SourceAction for Operation {
    type Source = Owner;
}

#[test]
fn source_action_returns_the_exact_unattempted_operation() {
    let operation = Operation("owned operation".to_owned());
    let mut actions = <SourceActions<Operation> as SendEffects>::empty();
    SendInput::<Operation, Own>::emit(&mut actions, operation);

    let results = SendSettlements::unattempted(actions).into_inputs();

    assert_eq!(
        results,
        [SettledItem::Unattempted(Operation(
            "owned operation".to_owned()
        ))]
    );
}
