use core::future::Future;

use behavior::{
    ActionItem, Here, InterpretItem, InterpretSends, Interpretation, InterpretationProgress,
    InterpreterFault, InterpreterRequests, ItemSettlement, SendSettlements, finish_item,
    prepare_item,
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

    type Accepted = ObservationAccepted;
    type Rejection = ObservationRejected;
    type Prerequisite = ObservationPrerequisite;
}

impl ActionItem for Timer {
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

    type Accepted = TimerAccepted;
    type Rejection = TimerRejected;
    type Prerequisite = TimerPrerequisite;
}

struct RejectedObservationRuntime;
struct CorruptTimerRuntime;

impl InterpretItem<Observation, (), Here> for RejectedObservationRuntime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Observation>,
        received: &'a mut Option<<Observation as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Observation: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            *received = Some(ItemSettlement::Rejected {
                item,
                reason: ObservationRejected,
            });
        }
    }
}

impl InterpretItem<Observation, (), Here> for CorruptTimerRuntime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Observation>,
        received: &'a mut Option<<Observation as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Observation: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(_item) = input.take() else {
                return;
            };
            *received = Some(ItemSettlement::Accepted(ObservationAccepted));
        }
    }
}

impl InterpretItem<Timer, (), Here> for RejectedObservationRuntime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Timer>,
        received: &'a mut Option<<Timer as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Timer: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            *received = Some(ItemSettlement::Blocked {
                item,
                prerequisite: TimerPrerequisite,
            });
        }
    }
}

impl InterpretItem<Timer, (), Here> for CorruptTimerRuntime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Timer>,
        received: &'a mut Option<<Timer as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Timer: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            *received = Some(ItemSettlement::Corrupt {
                item,
                fault: InterpreterFault::CorruptTraversal,
            });
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
    let mut progress = Some(InterpretationProgress::Original(InterpreterRequests::one(
        observation,
    )));
    <InterpreterRequests<Observation> as InterpretSends<Runtime, (), Here>>::interpret(
        &mut progress,
        runtime,
    )
    .await;
    let Some(InterpretationProgress::Completed(settlement)) = progress else {
        panic!("the observation product must return its actual settlement");
    };
    settlement
}

#[tokio::test]
async fn capability_vocabularies_are_identical_across_runtime_implementations() {
    let mut rejected_input = Some(Observation(1));
    let mut rejected_received = None;
    <RejectedObservationRuntime as InterpretItem<Observation, (), Here>>::interpret_item(
        &mut RejectedObservationRuntime,
        &mut rejected_input,
        &mut rejected_received,
    )
    .await;
    let rejected =
        rejected_received.expect("the concrete runtime returns the actual promised settlement");
    let mut accepted_input = Some(Observation(2));
    let mut accepted_received = None;
    <CorruptTimerRuntime as InterpretItem<Observation, (), Here>>::interpret_item(
        &mut CorruptTimerRuntime,
        &mut accepted_input,
        &mut accepted_received,
    )
    .await;
    let accepted =
        accepted_received.expect("the concrete runtime returns the actual promised settlement");
    let mut blocked_input = Some(Timer(3));
    let mut blocked_received = None;
    <RejectedObservationRuntime as InterpretItem<Timer, (), Here>>::interpret_item(
        &mut RejectedObservationRuntime,
        &mut blocked_input,
        &mut blocked_received,
    )
    .await;
    let blocked =
        blocked_received.expect("the concrete runtime returns the actual promised settlement");
    let mut corrupt_input = Some(Timer(4));
    let mut corrupt_received = None;
    <CorruptTimerRuntime as InterpretItem<Timer, (), Here>>::interpret_item(
        &mut CorruptTimerRuntime,
        &mut corrupt_input,
        &mut corrupt_received,
    )
    .await;
    let corrupt =
        corrupt_received.expect("the concrete runtime returns the actual promised settlement");

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
