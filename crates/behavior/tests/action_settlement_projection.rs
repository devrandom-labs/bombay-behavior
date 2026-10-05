use behavior::{
    ActionItem, ActionSettlement, ActionSettlements, Actions, BehaviorSettlements, Creations,
    InterpretationProgress, InterpreterRequests, ItemSettlement, MailAddr, Never, NoBirths,
    SendSettlements, finish_item, prepare_item,
};

struct Observation;

impl ActionItem for Observation {
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

type ObservationSettlements = <InterpreterRequests<Observation> as SendSettlements>::Settlements;
type ExpectedSettlement = ActionSettlement<Creations<Never>, ObservationSettlements, Never>;
type WorkerActions = Actions<MailAddr, Never, InterpreterRequests<Observation>, NoBirths>;

pub struct RetainedSettlement<B: BehaviorSettlements> {
    pub settlement: <B as BehaviorSettlements>::Settlements,
}

fn requires_exact_settlement<T>()
where
    T: ActionSettlements<Settlements = ExpectedSettlement>,
{
}

#[test]
fn one_actions_type_selects_one_runtime_independent_settlement() {
    requires_exact_settlement::<WorkerActions>();
}
