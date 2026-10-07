use behavior::{
    ActionItem, Interpretation, InterpretationProgress, ItemSettlement, Never, Own, SendEffects,
    SendInput, SendSettlements, SettledItem, SourceAction, SourceActions, finish_item,
    prepare_item,
};

struct Owner;

#[derive(Debug, Eq, PartialEq)]
struct Operation(String);

impl ActionItem for Operation {
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

    let mut progress = Some(InterpretationProgress::Original(actions));
    <SourceActions<Operation> as SendSettlements>::unattempted(&mut progress);
    let Some(InterpretationProgress::Completed(Interpretation::Complete(results))) = progress
    else {
        panic!("the cold source retains the exact unattempted operation");
    };
    let results = results.into_inputs();

    assert_eq!(
        results,
        [SettledItem::Unattempted(Operation(
            "owned operation".to_owned()
        ))]
    );
}
