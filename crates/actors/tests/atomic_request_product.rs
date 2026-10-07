use core::future::{Future, ready};

use behavior::{
    Behavior, BehaviorAddr, ClassifySettlement, EndpointAddress, InterpretSends, Interpretation,
    InterpretationProgress, SendEffects, SendSettlements, SettlementStatus,
};
use behavior_actors::atomic::{
    DynamicSupervisorRequests, FixedSupervisorRequests, InitializeWorker,
    WorkerInitializationOutcome, WorkerInitializationReport,
};

#[expect(
    dead_code,
    reason = "this compile contract names the three worker initialization protocol roles"
)]
fn worker_initialization_protocol<W, P>(
    _: InitializeWorker<W, P>,
    outcome: WorkerInitializationOutcome<W>,
    report: WorkerInitializationReport<W, P>,
) where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    match outcome {
        WorkerInitializationOutcome::ReadyForActivation
        | WorkerInitializationOutcome::EffectsRejected(_)
        | WorkerInitializationOutcome::Stopped(_) => {}
    }
    match report {
        WorkerInitializationReport::ReadyForActivation { .. }
        | WorkerInitializationReport::EffectsRejected { .. }
        | WorkerInitializationReport::Stopped { .. } => {}
    }
}

enum LaneOutcome {
    Accepted,
    Corrupt,
}

struct Lane<const NUMBER: u8> {
    outcomes: Vec<LaneOutcome>,
}

enum LaneSettlement {
    Accepted(u8),
    Corrupt(u8),
    Unattempted,
}

impl ClassifySettlement for LaneSettlement {
    fn settlement_status(&self) -> SettlementStatus {
        match self {
            Self::Accepted(_) => SettlementStatus::Accepted,
            Self::Corrupt(_) | Self::Unattempted => SettlementStatus::Corrupt,
        }
    }
}

impl<const NUMBER: u8> Lane<NUMBER> {
    fn accepted() -> Self {
        Self {
            outcomes: vec![LaneOutcome::Accepted],
        }
    }

    fn corrupt() -> Self {
        Self {
            outcomes: vec![LaneOutcome::Corrupt],
        }
    }
}

impl<const NUMBER: u8> SendEffects for Lane<NUMBER> {
    fn empty() -> Self {
        Self {
            outcomes: Vec::new(),
        }
    }

    fn append(&mut self, mut other: Self) {
        self.outcomes.append(&mut other.outcomes);
    }
}

impl<const NUMBER: u8> SendSettlements for Lane<NUMBER> {
    type Settlements = Vec<LaneSettlement>;
    type SourceCustody = Self::Settlements;
    type InterpretationCustody = (Option<Self>, Option<Interpretation<Self::Settlements>>);

    fn prepare_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(input)) => {
                *progress = Some(InterpretationProgress::Interpreting((Some(input), None)));
            }
            retained => *progress = retained,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        if !matches!(
            progress,
            Some(InterpretationProgress::Interpreting((None, Some(_))))
        ) {
            return;
        }
        match progress.take() {
            Some(InterpretationProgress::Interpreting((None, Some(received)))) => {
                *progress = Some(InterpretationProgress::Completed(received));
            }
            retained => *progress = retained,
        }
    }
    fn unattempted(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
    ) {
        match progress.take() {
            Some(InterpretationProgress::Original(input)) => {
                let received = input
                    .outcomes
                    .into_iter()
                    .map(|_| LaneSettlement::Unattempted)
                    .collect();
                *progress = Some(InterpretationProgress::Completed(Interpretation::Complete(
                    received,
                )));
            }
            retained => *progress = retained,
        }
    }
}

impl<const NUMBER: u8> InterpretSends<Vec<u8>, (), behavior::Here> for Lane<NUMBER> {
    fn interpret(
        progress: &mut Option<
            InterpretationProgress<Self, Self::InterpretationCustody, Self::Settlements>,
        >,
        trace: &mut Vec<u8>,
    ) -> impl Future<Output = ()> + Send {
        Self::prepare_interpretation(progress);
        let Some(InterpretationProgress::Interpreting((input, received))) = progress else {
            return ready(());
        };
        if received.is_some() {
            return ready(());
        }
        let Some(input) = input.take() else {
            return ready(());
        };
        trace.push(NUMBER);
        let mut settlements = Vec::new();
        let mut remaining = input.outcomes.into_iter();
        let disposition = loop {
            match remaining.next() {
                Some(LaneOutcome::Accepted) => settlements.push(LaneSettlement::Accepted(NUMBER)),
                Some(LaneOutcome::Corrupt) => {
                    settlements.push(LaneSettlement::Corrupt(NUMBER));
                    break Interpretation::Corrupt(settlements);
                }
                None => break Interpretation::Complete(settlements),
            }
        };
        *received = Some(disposition);
        Self::finish_interpretation(progress);
        ready(())
    }
}

async fn interpret<Product>(
    product: Product,
    trace: &mut Vec<u8>,
) -> Interpretation<Product::Settlements>
where
    Product: InterpretSends<Vec<u8>, (), behavior::Here>,
{
    let mut progress = Some(InterpretationProgress::Original(product));
    <Product as InterpretSends<Vec<u8>, (), behavior::Here>>::interpret(&mut progress, trace).await;
    let Some(InterpretationProgress::Completed(settlement)) = progress else {
        panic!("the exact fixture product must return its full settlement");
    };
    settlement
}

#[tokio::test]
async fn dynamic_product_preserves_the_exact_corrupt_suffix() {
    let requests = DynamicSupervisorRequests {
        proxy_observations: Lane::<1>::accepted(),
        proxy_operations: Lane::<2>::corrupt(),
        shutdown_schedules: Lane::<3>::accepted(),
        start_replies: Lane::<4>::accepted(),
        replace_replies: Lane::<5>::accepted(),
        stop_replies: Lane::<6>::accepted(),
        query_replies: Lane::<7>::accepted(),
        cancel_replies: Lane::<8>::accepted(),
        lifecycle: Lane::<9>::accepted(),
        diagnostics: Lane::<10>::accepted(),
    };
    let mut trace = Vec::new();
    let Interpretation::Corrupt(settled) = interpret(requests, &mut trace).await else {
        panic!("the corrupt lane must retain its exact suffix");
    };
    assert_eq!(trace, vec![1, 2]);
    assert!(matches!(
        settled.proxy_observations.as_slice(),
        [LaneSettlement::Accepted(1)]
    ));
    assert!(matches!(
        settled.proxy_operations.as_slice(),
        [LaneSettlement::Corrupt(2)]
    ));
    assert!(matches!(
        settled.shutdown_schedules.as_slice(),
        [LaneSettlement::Unattempted]
    ));
    assert!(matches!(
        settled.diagnostics.as_slice(),
        [LaneSettlement::Unattempted]
    ));
    assert_eq!(settled.settlement_status(), SettlementStatus::Corrupt);
}

#[tokio::test]
async fn fixed_product_uses_its_declared_domain_order() {
    let requests = FixedSupervisorRequests {
        proxy_observations: Lane::<1>::accepted(),
        worker_preparations: Lane::<2>::accepted(),
        proxy_operations: Lane::<3>::accepted(),
        restart_schedules: Lane::<4>::corrupt(),
        lifecycle: Lane::<5>::accepted(),
        status_replies: Lane::<6>::accepted(),
        capability_replies: Lane::<7>::accepted(),
        diagnostics: Lane::<8>::accepted(),
    };
    let mut trace = Vec::new();
    let Interpretation::Corrupt(settled) = interpret(requests, &mut trace).await else {
        panic!("the corrupt restart request must retain its exact suffix");
    };
    assert_eq!(trace, vec![1, 2, 3, 4]);
    assert!(matches!(
        settled.proxy_operations.as_slice(),
        [LaneSettlement::Accepted(3)]
    ));
    assert!(matches!(
        settled.restart_schedules.as_slice(),
        [LaneSettlement::Corrupt(4)]
    ));
    assert!(matches!(
        settled.lifecycle.as_slice(),
        [LaneSettlement::Unattempted]
    ));
    assert!(matches!(
        settled.status_replies.as_slice(),
        [LaneSettlement::Unattempted]
    ));
    assert!(matches!(
        settled.capability_replies.as_slice(),
        [LaneSettlement::Unattempted]
    ));
    assert!(matches!(
        settled.diagnostics.as_slice(),
        [LaneSettlement::Unattempted]
    ));
}
