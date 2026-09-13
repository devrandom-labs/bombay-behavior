use behavior::{
    ActionItem, ActionSettlement, ActionSettlements, Actions, Creations, InterpreterRequests,
    MailAddr, Never, NoBirths, SendSettlements,
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

fn requires_exact_settlement<T>()
where
    T: ActionSettlements<Settlements = ExpectedSettlement>,
{
}

#[test]
fn one_actions_type_selects_one_runtime_independent_settlement() {
    requires_exact_settlement::<WorkerActions>();
}
