use behavior::{
    ActionItem, Here, InterpretItem, InterpretSends, Interpretation, InterpreterFault,
    InterpreterRequests, ItemSettlement, SendSettlements,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Timer(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservationAccepted;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservationRejected;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObservationPrerequisite;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerAccepted;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerRejected;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TimerPrerequisite;

impl ActionItem for Observation {
    type Accepted = ObservationAccepted;
    type Rejection = ObservationRejected;
    type Prerequisite = ObservationPrerequisite;
}

impl ActionItem for Timer {
    type Accepted = TimerAccepted;
    type Rejection = TimerRejected;
    type Prerequisite = TimerPrerequisite;
}

struct RejectedObservationRuntime;
struct CorruptTimerRuntime;

impl InterpretItem<Observation, (), Here> for RejectedObservationRuntime {
    async fn interpret_item(
        &mut self,
        item: Observation,
    ) -> ItemSettlement<
        Observation,
        <Observation as ActionItem>::Accepted,
        <Observation as ActionItem>::Rejection,
        <Observation as ActionItem>::Prerequisite,
    > {
        ItemSettlement::Rejected {
            item,
            reason: ObservationRejected,
        }
    }
}

impl InterpretItem<Observation, (), Here> for CorruptTimerRuntime {
    async fn interpret_item(
        &mut self,
        _: Observation,
    ) -> ItemSettlement<
        Observation,
        <Observation as ActionItem>::Accepted,
        <Observation as ActionItem>::Rejection,
        <Observation as ActionItem>::Prerequisite,
    > {
        ItemSettlement::Accepted(ObservationAccepted)
    }
}

impl InterpretItem<Timer, (), Here> for RejectedObservationRuntime {
    async fn interpret_item(
        &mut self,
        item: Timer,
    ) -> ItemSettlement<
        Timer,
        <Timer as ActionItem>::Accepted,
        <Timer as ActionItem>::Rejection,
        <Timer as ActionItem>::Prerequisite,
    > {
        ItemSettlement::Blocked {
            item,
            prerequisite: TimerPrerequisite,
        }
    }
}

impl InterpretItem<Timer, (), Here> for CorruptTimerRuntime {
    async fn interpret_item(
        &mut self,
        item: Timer,
    ) -> ItemSettlement<
        Timer,
        <Timer as ActionItem>::Accepted,
        <Timer as ActionItem>::Rejection,
        <Timer as ActionItem>::Prerequisite,
    > {
        ItemSettlement::Corrupt {
            item,
            fault: InterpreterFault::CorruptTraversal,
        }
    }
}

type ObservationSettlements = <InterpreterRequests<Observation> as SendSettlements>::Settlements;

async fn settle_observations<Runtime>(
    runtime: &mut Runtime,
    observation: Observation,
) -> Interpretation<ObservationSettlements>
where
    Runtime: InterpretItem<Observation, (), Here>,
    InterpreterRequests<Observation>: InterpretSends<Runtime, (), Here>,
{
    <InterpreterRequests<Observation> as InterpretSends<Runtime, (), Here>>::interpret(
        InterpreterRequests::one(observation),
        runtime,
    )
    .await
}

#[tokio::test]
async fn capability_vocabularies_are_identical_across_runtime_implementations() {
    let rejected = RejectedObservationRuntime
        .interpret_item(Observation(1))
        .await;
    let accepted = CorruptTimerRuntime.interpret_item(Observation(2)).await;
    let blocked = RejectedObservationRuntime.interpret_item(Timer(3)).await;
    let corrupt = CorruptTimerRuntime.interpret_item(Timer(4)).await;

    assert_eq!(
        rejected,
        ItemSettlement::Rejected {
            item: Observation(1),
            reason: ObservationRejected,
        }
    );
    assert_eq!(accepted, ItemSettlement::Accepted(ObservationAccepted));
    assert_eq!(
        blocked,
        ItemSettlement::Blocked {
            item: Timer(3),
            prerequisite: TimerPrerequisite,
        }
    );
    assert_eq!(
        corrupt,
        ItemSettlement::Corrupt {
            item: Timer(4),
            fault: InterpreterFault::CorruptTraversal,
        }
    );
}

#[tokio::test]
async fn send_product_has_one_settlement_type_for_every_runtime() {
    let rejected = settle_observations(&mut RejectedObservationRuntime, Observation(1)).await;
    let accepted = settle_observations(&mut CorruptTimerRuntime, Observation(2)).await;

    assert!(matches!(rejected, Interpretation::Complete(_)));
    assert!(matches!(accepted, Interpretation::Complete(_)));
}
