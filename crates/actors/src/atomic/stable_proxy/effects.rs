//! Named StableProxy action lanes and their total ordered settlement.

use behavior::{ClassifySettlement, InterpretSends, Interpretation, SendEffects, SendSettlements};

/// Named worker lifecycle, delivery, owner-outcome, and diagnostic lanes.
#[doc(hidden)]
pub struct ProxyEffects<
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> {
    pub worker_observations: WorkerObservations,
    pub worker_initializations: WorkerInitializations,
    pub worker_activations: WorkerActivations,
    pub worker_shutdowns: WorkerShutdowns,
    pub worker_deliveries: WorkerDeliveries,
    pub owner_outcomes: OwnerOutcomes,
    pub diagnostics: Diagnostics,
}

impl<
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> SendEffects
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    WorkerObservations: SendEffects,
    WorkerInitializations: SendEffects,
    WorkerActivations: SendEffects,
    WorkerShutdowns: SendEffects,
    WorkerDeliveries: SendEffects,
    OwnerOutcomes: SendEffects,
    Diagnostics: SendEffects,
{
    fn empty() -> Self {
        Self {
            worker_observations: WorkerObservations::empty(),
            worker_initializations: WorkerInitializations::empty(),
            worker_activations: WorkerActivations::empty(),
            worker_shutdowns: WorkerShutdowns::empty(),
            worker_deliveries: WorkerDeliveries::empty(),
            owner_outcomes: OwnerOutcomes::empty(),
            diagnostics: Diagnostics::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.worker_observations.append(other.worker_observations);
        self.worker_initializations
            .append(other.worker_initializations);
        self.worker_activations.append(other.worker_activations);
        self.worker_shutdowns.append(other.worker_shutdowns);
        self.worker_deliveries.append(other.worker_deliveries);
        self.owner_outcomes.append(other.owner_outcomes);
        self.diagnostics.append(other.diagnostics);
    }
}

impl<
    Host,
    RootEvent,
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> behavior::SourceSettlementCustody<Host, RootEvent>
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    Host: Send,
    WorkerObservations: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    WorkerInitializations: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    WorkerActivations: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    WorkerShutdowns: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    WorkerDeliveries: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    OwnerOutcomes: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
    Diagnostics: behavior::SourceSettlementCustody<Host, RootEvent> + Send,
{
    fn offer_next_to_source(
        self,
        host: &mut Host,
    ) -> impl core::future::Future<Output = behavior::SourceCustody<Self>> + Send {
        async move {
            let settlements = (
                (
                    (
                        (
                            (
                                (self.worker_observations, self.worker_initializations),
                                self.worker_activations,
                            ),
                            self.worker_shutdowns,
                        ),
                        self.worker_deliveries,
                    ),
                    self.owner_outcomes,
                ),
                self.diagnostics,
            );
            settlements.offer_next_to_source(host).await.map(
                |(
                    (
                        (
                            (
                                ((worker_observations, worker_initializations), worker_activations),
                                worker_shutdowns,
                            ),
                            worker_deliveries,
                        ),
                        owner_outcomes,
                    ),
                    diagnostics,
                )| ProxyEffects {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    worker_shutdowns,
                    worker_deliveries,
                    owner_outcomes,
                    diagnostics,
                },
            )
        }
    }
}

impl<
    Event,
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> behavior::SendsFor<Event>
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    WorkerObservations: SendEffects + behavior::SendsFor<Event>,
    WorkerInitializations: SendEffects + behavior::SendsFor<Event>,
    WorkerActivations: SendEffects + behavior::SendsFor<Event>,
    WorkerShutdowns: SendEffects + behavior::SendsFor<Event>,
    WorkerDeliveries: SendEffects + behavior::SendsFor<Event>,
    OwnerOutcomes: SendEffects + behavior::SendsFor<Event>,
    Diagnostics: SendEffects + behavior::SendsFor<Event>,
{
}

impl<
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> ClassifySettlement
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    WorkerObservations: ClassifySettlement,
    WorkerInitializations: ClassifySettlement,
    WorkerActivations: ClassifySettlement,
    WorkerShutdowns: ClassifySettlement,
    WorkerDeliveries: ClassifySettlement,
    OwnerOutcomes: ClassifySettlement,
    Diagnostics: ClassifySettlement,
{
    fn settlement_status(&self) -> behavior::SettlementStatus {
        self.worker_observations
            .settlement_status()
            .combine(self.worker_initializations.settlement_status())
            .combine(self.worker_activations.settlement_status())
            .combine(self.worker_shutdowns.settlement_status())
            .combine(self.worker_deliveries.settlement_status())
            .combine(self.owner_outcomes.settlement_status())
            .combine(self.diagnostics.settlement_status())
    }
}

impl<
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> SendSettlements
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    WorkerObservations: SendSettlements,
    WorkerInitializations: SendSettlements,
    WorkerActivations: SendSettlements,
    WorkerShutdowns: SendSettlements,
    WorkerDeliveries: SendSettlements,
    OwnerOutcomes: SendSettlements,
    Diagnostics: SendSettlements,
{
    type Settlements = ProxyEffects<
        WorkerObservations::Settlements,
        WorkerInitializations::Settlements,
        WorkerActivations::Settlements,
        WorkerShutdowns::Settlements,
        WorkerDeliveries::Settlements,
        OwnerOutcomes::Settlements,
        Diagnostics::Settlements,
    >;

    fn unattempted(self) -> Self::Settlements {
        ProxyEffects {
            worker_observations: self.worker_observations.unattempted(),
            worker_initializations: self.worker_initializations.unattempted(),
            worker_activations: self.worker_activations.unattempted(),
            worker_shutdowns: self.worker_shutdowns.unattempted(),
            worker_deliveries: self.worker_deliveries.unattempted(),
            owner_outcomes: self.owner_outcomes.unattempted(),
            diagnostics: self.diagnostics.unattempted(),
        }
    }
}

impl<
    Interpreter,
    RootEvent,
    Path,
    WorkerObservations,
    WorkerInitializations,
    WorkerActivations,
    WorkerShutdowns,
    WorkerDeliveries,
    OwnerOutcomes,
    Diagnostics,
> InterpretSends<Interpreter, RootEvent, Path>
    for ProxyEffects<
        WorkerObservations,
        WorkerInitializations,
        WorkerActivations,
        WorkerShutdowns,
        WorkerDeliveries,
        OwnerOutcomes,
        Diagnostics,
    >
where
    Interpreter: Send,
    WorkerObservations: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    WorkerInitializations: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    WorkerActivations: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    WorkerShutdowns: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    WorkerDeliveries: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    OwnerOutcomes: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
    Diagnostics: SendEffects + InterpretSends<Interpreter, RootEvent, Path>,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl core::future::Future<Output = Interpretation<Self::Settlements>> + Send {
        async move {
            let worker_observations = match self.worker_observations.interpret(interpreter).await {
                Interpretation::Complete(worker_observations) => worker_observations,
                Interpretation::Corrupt(worker_observations) => {
                    return Interpretation::Corrupt(ProxyEffects {
                        worker_observations,
                        worker_initializations: self.worker_initializations.unattempted(),
                        worker_activations: self.worker_activations.unattempted(),
                        worker_shutdowns: self.worker_shutdowns.unattempted(),
                        worker_deliveries: self.worker_deliveries.unattempted(),
                        owner_outcomes: self.owner_outcomes.unattempted(),
                        diagnostics: self.diagnostics.unattempted(),
                    });
                }
            };
            let worker_initializations =
                match self.worker_initializations.interpret(interpreter).await {
                    Interpretation::Complete(worker_initializations) => worker_initializations,
                    Interpretation::Corrupt(worker_initializations) => {
                        return Interpretation::Corrupt(ProxyEffects {
                            worker_observations,
                            worker_initializations,
                            worker_activations: self.worker_activations.unattempted(),
                            worker_shutdowns: self.worker_shutdowns.unattempted(),
                            worker_deliveries: self.worker_deliveries.unattempted(),
                            owner_outcomes: self.owner_outcomes.unattempted(),
                            diagnostics: self.diagnostics.unattempted(),
                        });
                    }
                };
            let worker_activations = match self.worker_activations.interpret(interpreter).await {
                Interpretation::Complete(worker_activations) => worker_activations,
                Interpretation::Corrupt(worker_activations) => {
                    return Interpretation::Corrupt(ProxyEffects {
                        worker_observations,
                        worker_initializations,
                        worker_activations,
                        worker_shutdowns: self.worker_shutdowns.unattempted(),
                        worker_deliveries: self.worker_deliveries.unattempted(),
                        owner_outcomes: self.owner_outcomes.unattempted(),
                        diagnostics: self.diagnostics.unattempted(),
                    });
                }
            };
            let worker_shutdowns = match self.worker_shutdowns.interpret(interpreter).await {
                Interpretation::Complete(worker_shutdowns) => worker_shutdowns,
                Interpretation::Corrupt(worker_shutdowns) => {
                    return Interpretation::Corrupt(ProxyEffects {
                        worker_observations,
                        worker_initializations,
                        worker_activations,
                        worker_shutdowns,
                        worker_deliveries: self.worker_deliveries.unattempted(),
                        owner_outcomes: self.owner_outcomes.unattempted(),
                        diagnostics: self.diagnostics.unattempted(),
                    });
                }
            };
            let worker_deliveries = match self.worker_deliveries.interpret(interpreter).await {
                Interpretation::Complete(worker_deliveries) => worker_deliveries,
                Interpretation::Corrupt(worker_deliveries) => {
                    return Interpretation::Corrupt(ProxyEffects {
                        worker_observations,
                        worker_initializations,
                        worker_activations,
                        worker_shutdowns,
                        worker_deliveries,
                        owner_outcomes: self.owner_outcomes.unattempted(),
                        diagnostics: self.diagnostics.unattempted(),
                    });
                }
            };
            behavior::settle_in_order(self.owner_outcomes, self.diagnostics, interpreter)
                .await
                .map(|(owner_outcomes, diagnostics)| ProxyEffects {
                    worker_observations,
                    worker_initializations,
                    worker_activations,
                    worker_shutdowns,
                    worker_deliveries,
                    owner_outcomes,
                    diagnostics,
                })
        }
    }
}
