use behavior::{
    ActionItem, ActionSettlement, ActionSettlements, Actions, BehaviorSettlements, Creations,
    InterpreterRequests, MailAddr, Never, NoBirths, SendSettlements,
};

struct Observation;

impl ActionItem for Observation {
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
