//! Bounded keyed supervision through one stable proxy per service.

use core::cmp::Ordering;
use core::num::NonZeroU64;
use core::ops::Bound::{Excluded, Unbounded};
use std::collections::BTreeMap;

use behavior::{
    Actions, ActiveTurn, Address, Behavior, BehaviorActed, BehaviorAddr, BehaviorBase, Births,
    ChildHead, ChildReport, CreateChild, CreationId, CreationSequence, Creations, EndpointAddress,
    EstablishedActor, InitializationTurn, InterpreterRequests, ItemSettlement, MessageProtocol,
    Never, SendEffects, SettledItem, SourceActions, Step, User,
};

use crate::{
    ActivationPlan, ActorDrainPolicy, ChildStopped, DeliveryRoute, DiagnosticAction,
    DiagnosticDisposition, InitialWorkerOutcome, ObserveChild, ProxyOutcome, ReplacementOutcome,
    ReplyRoute, ScheduleAfter, WorkerStartResult, WorkerSubmission,
};

use super::drain::ShutdownDeadline;
use super::{ActivationPolicy, EntryCapacity, ProxyInputResult, ProxyOperation, StableProxy};
use crate::atomic::proxy_creation::{StableProxyCreation, identity, resolve};
use crate::atomic::stable_proxy::ProxyOperationWitness;

mod diagnostic;
mod entry;
mod event;
mod lifecycle;
mod protocol;
mod requests;

pub use diagnostic::DynamicDiagnostic;
pub use event::DynamicSupervisorEvent;
pub use lifecycle::{
    CancellationOutcome, DynamicLifecycle, EntryRetirement, EntryStopFailure,
    EntryStopFailureReason, InterruptedWorker, ReplacementFailure, WorkerChange,
    WorkerChangeInterruption,
};
pub use protocol::{
    CancelAuthority, CancellationReceipt, DynamicCommand, DynamicStatus, QueryReply,
    ReplaceRejection, StartRejection, StopRejection, WorkerChangeReceipt, WorkerChangeRejection,
};
pub use requests::DynamicSupervisorRequests;

use entry::{
    DynamicEntry, DynamicEntryPhase, ProxyInputCustody, ProxyInputPurpose, ProxyRetirement,
    ProxyShutdown, RetiringWork, ServiceAvailability, WorkerChangeDisposition,
};

/// Policy after an unrequested exact worker stop leaves its proxy empty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnexpectedExit {
    /// Retain the key and empty stable proxy for explicit replacement or stop.
    KeepEmpty,
    /// Drain the stable proxy and release the key.
    Retire,
}

enum SupervisorAvailability {
    Accepting(ActorDrainPolicy),
    ShuttingDown(ShutdownDeadline),
}

/// One bounded keyed supervisor with a stable proxy per retained service.
pub struct DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRoute>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    services: BTreeMap<Key, DynamicEntry<Worker, Plan>>,
    entries: EntryCapacity,
    activation: ActivationPolicy,
    generations: Option<NonZeroU64>,
    operations: Option<NonZeroU64>,
    creations: CreationSequence,
    unexpected_exit: UnexpectedExit,
    availability: SupervisorAvailability,
    lifecycle: LifecycleRoute,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
}

/// Construct one dynamic supervisor from its six semantic policies.
#[must_use]
pub fn dynamic<Key, Worker, Plan, LifecycleRoute, DiagnosticRoute>(
    entries: EntryCapacity,
    activation: ActivationPolicy,
    unexpected_exit: UnexpectedExit,
    actor_drain: ActorDrainPolicy,
    lifecycle: LifecycleRoute,
    diagnostics: DiagnosticDisposition<DiagnosticRoute>,
) -> DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRoute>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    DynamicSupervisor {
        services: BTreeMap::new(),
        entries,
        activation,
        generations: Some(NonZeroU64::MIN),
        operations: Some(NonZeroU64::MIN),
        creations: CreationSequence::new(),
        unexpected_exit,
        availability: SupervisorAvailability::Accepting(actor_drain),
        lifecycle,
        diagnostics,
    }
}

impl<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType> BehaviorBase
    for DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType>
    DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType>
where
    Key: Clone + Ord + Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Clone + Send,
    LifecycleRoute: DeliveryRoute<
            Protocol = MessageProtocol<BehaviorAddr<Worker>, DynamicLifecycle<Key, Worker, Plan>>,
        > + Clone,
    LifecycleRoute::Sends: behavior::SendsFor<DynamicSupervisorEvent<Key, Worker, Plan>>,
    DiagnosticRouteType: crate::DiagnosticRoute<DynamicDiagnostic<Key, Worker, Plan>> + Clone,
{
    fn take_proxy_entry(
        &mut self,
        creation: CreationId,
    ) -> Option<(Key, DynamicEntry<Worker, Plan>)> {
        self.services
            .extract_if(.., |_, entry| entry.creation == creation)
            .next()
    }

    fn reject_input(
        &mut self,
        input: DynamicSupervisorEvent<Key, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        Err(input)
    }

    fn authorize_waiting(&mut self) -> SourceActions<ProxyOperation<behavior::Here, Worker, Plan>> {
        let occupied: usize = self
            .services
            .values()
            .map(|service| match service.phase {
                DynamicEntryPhase::WaitingForProxyInput { .. }
                | DynamicEntryPhase::WaitingForProxyOutcome { .. }
                | DynamicEntryPhase::ReplacementAwaitingReceipt { .. }
                | DynamicEntryPhase::ReplacementAwaitingOutcome { .. }
                | DynamicEntryPhase::Retiring(ProxyRetirement {
                    work:
                        RetiringWork::Transferred {
                            input:
                                ProxyInputCustody::AwaitingSettlement(_)
                                | ProxyInputCustody::AwaitingOutcome(_),
                            ..
                        },
                    ..
                }) => 1,
                _ => 0,
            })
            .sum();
        match occupied.cmp(&self.activation.maximum()) {
            Ordering::Equal | Ordering::Greater => return SourceActions::empty(),
            Ordering::Less => {}
        }
        let waiting = self
            .services
            .iter()
            .filter_map(|(key, entry)| match entry.phase {
                DynamicEntryPhase::WaitingForActivation { .. }
                | DynamicEntryPhase::ReplacementQueued { .. } => {
                    Some((entry.operation, key.clone()))
                }
                _ => None,
            })
            .min_by_key(|(operation, _)| *operation);
        let Some((_, key)) = waiting else {
            return SourceActions::empty();
        };
        let Some(entry) = self.services.remove(&key) else {
            return SourceActions::empty();
        };
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        match phase {
            DynamicEntryPhase::WaitingForActivation { proxy, submission } => {
                let (witness, request) = ProxyOperation::initial(creation, submission);
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::WaitingForProxyInput { proxy, witness },
                    },
                );
                SourceActions::sending(request)
            }
            DynamicEntryPhase::ReplacementQueued {
                proxy,
                service,
                submission,
            } => {
                let (witness, request) = ProxyOperation::replacement(creation, submission);
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::ReplacementAwaitingReceipt {
                            proxy,
                            service,
                            witness,
                        },
                    },
                );
                SourceActions::sending(request)
            }
            phase => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                SourceActions::empty()
            }
        }
    }

    fn start(
        &mut self,
        key: Key,
        submission: WorkerSubmission<Worker, Plan>,
        reply_to: ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, StartRejection>,
                >,
            >,
        >,
    ) -> BehaviorActed<Self> {
        let admission = if self.services.contains_key(&key) {
            Err(StartRejection::AlreadyExists)
        } else if self.services.len() >= self.entries.maximum() {
            Err(StartRejection::AtCapacity)
        } else {
            match (self.generations, self.operations) {
                (Some(generation), Some(operation)) => Ok((generation, operation)),
                (None, _) => Err(StartRejection::EntryGenerationExhausted),
                (Some(_), None) => Err(StartRejection::OperationExhausted),
            }
        };
        let (generation, operation) = match admission {
            Ok(admission) => admission,
            Err(reason) => {
                let mut requests = DynamicSupervisorRequests::empty();
                requests.start_replies = reply_to.deliver(Err(WorkerChangeRejection {
                    key,
                    submission,
                    reason,
                }));
                return Ok(Actions::send(requests));
            }
        };
        let creation = match self.creations.issue() {
            Some(creation) => creation,
            None => {
                let mut requests = DynamicSupervisorRequests::empty();
                requests.start_replies = reply_to.deliver(Err(WorkerChangeRejection {
                    key,
                    submission,
                    reason: StartRejection::ProxyCreationExhausted,
                }));
                return Ok(Actions::send(requests));
            }
        };
        self.generations = generation.checked_add(1);
        self.operations = operation.checked_add(1);
        let authority = CancelAuthority::issued(key.clone(), operation.get());
        let replaced = self.services.insert(
            key.clone(),
            DynamicEntry {
                creation,
                generation,
                operation,
                phase: DynamicEntryPhase::CreatingProxy { submission },
            },
        );
        debug_assert!(replaced.is_none(), "duplicate admission was rejected");
        let mut requests = DynamicSupervisorRequests::empty();
        requests.proxy_observations =
            InterpreterRequests::one(ObserveChild::<Worker::Protocol, ChildHead>::new(creation));
        requests.start_replies = reply_to.deliver(Ok(WorkerChangeReceipt {
            key: key.clone(),
            cancel: authority,
        }));
        let creations = Creations::one(CreateChild::birth(
            creation,
            StableProxy::<Worker, Plan>::activated(),
        ));
        Ok(Actions::new(requests, creations, Step::Continue))
    }

    fn replace(
        &mut self,
        key: Key,
        submission: WorkerSubmission<Worker, Plan>,
        reply_to: ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, ReplaceRejection>,
                >,
            >,
        >,
    ) -> BehaviorActed<Self> {
        let Some(entry) = self.services.remove(&key) else {
            let mut requests = DynamicSupervisorRequests::empty();
            requests.replace_replies = reply_to.deliver(Err(WorkerChangeRejection {
                key,
                submission,
                reason: ReplaceRejection::Unknown,
            }));
            return Ok(Actions::send(requests));
        };
        let DynamicEntry {
            creation,
            generation,
            operation: previous_operation,
            phase,
        } = entry;
        let (creation, proxy, service, latest) = match phase {
            DynamicEntryPhase::Available {
                proxy,
                service,
                latest,
            } => (creation, proxy, service, latest),
            phase => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation: previous_operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.replace_replies = reply_to.deliver(Err(WorkerChangeRejection {
                    key,
                    submission,
                    reason: ReplaceRejection::Unavailable,
                }));
                return Ok(Actions::send(requests));
            }
        };
        let Some(operation) = self.operations else {
            self.services.insert(
                key.clone(),
                DynamicEntry {
                    creation,
                    generation,
                    operation: previous_operation,
                    phase: DynamicEntryPhase::Available {
                        proxy,
                        service,
                        latest,
                    },
                },
            );
            let mut requests = DynamicSupervisorRequests::empty();
            requests.replace_replies = reply_to.deliver(Err(WorkerChangeRejection {
                key,
                submission,
                reason: ReplaceRejection::OperationExhausted,
            }));
            return Ok(Actions::send(requests));
        };
        self.operations = operation.checked_add(1);
        let authority = CancelAuthority::issued(key.clone(), operation.get());
        self.services.insert(
            key.clone(),
            DynamicEntry {
                creation,
                generation,
                operation,
                phase: DynamicEntryPhase::ReplacementQueued {
                    proxy,
                    service,
                    submission,
                },
            },
        );
        let mut requests = DynamicSupervisorRequests::empty();
        requests.proxy_operations = self.authorize_waiting();
        requests.replace_replies = reply_to.deliver(Ok(WorkerChangeReceipt {
            key,
            cancel: authority,
        }));
        Ok(Actions::send(requests))
    }

    fn stop(
        &mut self,
        key: Key,
        reply_to: ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, Result<Key, StopRejection<Key>>>,
        >,
    ) -> BehaviorActed<Self> {
        let Some(entry) = self.services.remove(&key) else {
            let mut requests = DynamicSupervisorRequests::empty();
            requests.stop_replies = reply_to.deliver(Err(StopRejection::Unknown { key }));
            return Ok(Actions::send(requests));
        };
        let DynamicEntry {
            creation,
            generation,
            operation: previous_operation,
            phase,
        } = entry;
        let (creation, proxy, restorable, operation, lifecycle) = match phase {
            DynamicEntryPhase::Available { proxy, service, .. } => {
                let Some(operation) = self.operations else {
                    return self.stop_operation_exhausted(
                        key,
                        creation,
                        generation,
                        previous_operation,
                        DynamicEntryPhase::Available {
                            proxy,
                            service,
                            latest: WorkerChangeDisposition::Committed,
                        },
                        reply_to,
                    );
                };
                self.operations = operation.checked_add(1);
                (
                    creation,
                    proxy,
                    Some(service),
                    operation,
                    SendEffects::empty(),
                )
            }
            DynamicEntryPhase::CreatingProxy { submission } => {
                let Some(operation) = self.operations else {
                    return self.stop_operation_exhausted(
                        key,
                        creation,
                        generation,
                        previous_operation,
                        DynamicEntryPhase::CreatingProxy { submission },
                        reply_to,
                    );
                };
                self.operations = operation.checked_add(1);
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::StoppingProxyCreation {
                            start_operation: previous_operation,
                            submission,
                        },
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.stop_replies = reply_to.deliver(Ok(key));
                return Ok(Actions::send(requests));
            }
            DynamicEntryPhase::WaitingForActivation { proxy, submission } => {
                let Some(operation) = self.operations else {
                    return self.stop_operation_exhausted(
                        key,
                        creation,
                        generation,
                        previous_operation,
                        DynamicEntryPhase::WaitingForActivation { proxy, submission },
                        reply_to,
                    );
                };
                self.operations = operation.checked_add(1);
                let lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::WorkerChangeInterrupted {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: previous_operation.get(),
                            interruption: WorkerChangeInterruption::ExplicitStop {
                                operation: operation.get(),
                            },
                            worker: InterruptedWorker::Submission(submission),
                        });
                (creation, proxy, None, operation, lifecycle)
            }
            DynamicEntryPhase::WaitingForProxyInput { proxy, witness } => {
                let Some(stop_operation) = self.operations else {
                    return self.stop_operation_exhausted(
                        key,
                        creation,
                        generation,
                        previous_operation,
                        DynamicEntryPhase::WaitingForProxyInput { proxy, witness },
                        reply_to,
                    );
                };
                self.operations = stop_operation.checked_add(1);
                let (retirement, shutdown) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::InterruptedStart(previous_operation),
                        input: ProxyInputCustody::AwaitingSettlement(witness),
                    },
                );
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation: stop_operation,
                        phase: DynamicEntryPhase::Retiring(retirement),
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = SourceActions::sending(shutdown);
                requests.stop_replies = reply_to.deliver(Ok(key));
                return Ok(Actions::send(requests));
            }
            DynamicEntryPhase::WaitingForProxyOutcome {
                proxy,
                operation: proxy_operation,
            } => {
                let Some(stop_operation) = self.operations else {
                    return self.stop_operation_exhausted(
                        key,
                        creation,
                        generation,
                        previous_operation,
                        DynamicEntryPhase::WaitingForProxyOutcome {
                            proxy,
                            operation: proxy_operation,
                        },
                        reply_to,
                    );
                };
                self.operations = stop_operation.checked_add(1);
                let (retirement, shutdown) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::InterruptedStart(previous_operation),
                        input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                    },
                );
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation: stop_operation,
                        phase: DynamicEntryPhase::Retiring(retirement),
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = SourceActions::sending(shutdown);
                requests.stop_replies = reply_to.deliver(Ok(key));
                return Ok(Actions::send(requests));
            }
            phase @ (DynamicEntryPhase::Stopping { .. }
            | DynamicEntryPhase::StoppingProxyCreation { .. }) => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation: previous_operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.stop_replies =
                    reply_to.deliver(Err(StopRejection::AlreadyStopping { key }));
                return Ok(Actions::send(requests));
            }
            phase => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation: previous_operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.stop_replies = reply_to.deliver(Err(StopRejection::Unavailable { key }));
                return Ok(Actions::send(requests));
            }
        };
        let (witness, shutdown) = ProxyOperation::shutdown(creation);
        self.services.insert(
            key.clone(),
            DynamicEntry {
                creation,
                generation,
                operation,
                phase: DynamicEntryPhase::Stopping {
                    proxy,
                    restorable,
                    shutdown: ProxyShutdown::AwaitingSettlement(witness),
                    stopped: None,
                },
            },
        );
        let mut requests = DynamicSupervisorRequests::empty();
        requests.proxy_operations = SourceActions::sending(shutdown);
        requests.stop_replies = reply_to.deliver(Ok(key));
        requests.lifecycle = lifecycle;
        Ok(Actions::send(requests))
    }

    fn stop_operation_exhausted(
        &mut self,
        key: Key,
        creation: CreationId,
        generation: NonZeroU64,
        operation: NonZeroU64,
        phase: DynamicEntryPhase<Worker, Plan>,
        reply_to: ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, Result<Key, StopRejection<Key>>>,
        >,
    ) -> BehaviorActed<Self> {
        self.services.insert(
            key.clone(),
            DynamicEntry {
                creation,
                generation,
                operation,
                phase,
            },
        );
        let mut requests = DynamicSupervisorRequests::empty();
        requests.stop_replies = reply_to.deliver(Err(StopRejection::OperationExhausted { key }));
        Ok(Actions::send(requests))
    }

    fn next_service_key(&self, previous: Option<&Key>) -> Option<Key> {
        match previous {
            Some(previous) => self.services.range((Excluded(previous), Unbounded)).next(),
            None => self.services.first_key_value(),
        }
        .map(|(key, _)| key.clone())
    }

    fn begin_shutdown(&mut self) -> BehaviorActed<Self> {
        let actor_drain = match &self.availability {
            SupervisorAvailability::Accepting(actor_drain) => *actor_drain,
            SupervisorAvailability::ShuttingDown(_) => return Ok(Actions::cont()),
        };
        let (deadline, schedule) = ShutdownDeadline::begin(actor_drain);
        self.availability = SupervisorAvailability::ShuttingDown(deadline);
        let mut requests: <Self as Behavior>::Sends = SendEffects::empty();
        match schedule {
            Some(schedule) => requests.shutdown_schedules.send(schedule),
            None => {}
        }

        let mut next = self.next_service_key(None);
        while let Some(key) = next {
            let entry = match self.services.remove(&key) {
                Some(entry) => entry,
                None => {
                    next = self.next_service_key(Some(&key));
                    continue;
                }
            };
            next = self.next_service_key(Some(&key));
            let DynamicEntry {
                creation,
                generation,
                operation,
                phase,
            } = entry;
            let (phase, shutdown, interruption) = phase.shutdown(creation, operation);
            match shutdown {
                Some(shutdown) => requests.proxy_operations.send(shutdown),
                None => {}
            }
            match interruption {
                Some((operation, interruption, submission)) => {
                    requests.lifecycle.append(self.lifecycle.clone().deliver(
                        DynamicLifecycle::WorkerChangeInterrupted {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: operation.get(),
                            interruption,
                            worker: InterruptedWorker::Submission(submission),
                        },
                    ))
                }
                None => {}
            }
            self.services.insert(
                key,
                DynamicEntry {
                    creation,
                    generation,
                    operation,
                    phase,
                },
            );
        }
        Ok(Actions::send(requests))
    }

    fn apply_shutdown_step(&self, acted: BehaviorActed<Self>) -> BehaviorActed<Self> {
        acted.map(|mut actions| {
            match (&self.availability, self.services.first_key_value()) {
                (SupervisorAvailability::Accepting(_), _) => {}
                (SupervisorAvailability::ShuttingDown(_), None) => {
                    actions.become_ = Step::Stop(behavior::Stopped);
                }
                (
                    SupervisorAvailability::ShuttingDown(ShutdownDeadline::NotScheduled(returned)),
                    Some(_),
                ) => {
                    let _ = returned;
                    actions.become_ = Step::Stop(behavior::Stopped);
                }
                (
                    SupervisorAvailability::ShuttingDown(ShutdownDeadline::Elapsed(elapsed)),
                    Some(_),
                ) => {
                    let _ = elapsed;
                    actions.become_ = Step::Stop(behavior::Stopped);
                }
                (SupervisorAvailability::ShuttingDown(_), Some(_)) => {}
            }
            actions
        })
    }

    fn command(&mut self, command: DynamicCommand<Key, Worker, Plan>) -> BehaviorActed<Self> {
        match &self.availability {
            SupervisorAvailability::Accepting(_) => {}
            SupervisorAvailability::ShuttingDown(_) => {
                return self.command_while_shutting_down(command);
            }
        }
        match command {
            DynamicCommand::Start {
                key,
                submission,
                reply_to,
            } => self.start(key, submission, reply_to),
            DynamicCommand::Replace {
                key,
                submission,
                reply_to,
            } => self.replace(key, submission, reply_to),
            DynamicCommand::Query { key, reply_to } => {
                let reply = match self.services.get(&key) {
                    Some(entry) => QueryReply::Known {
                        key,
                        status: entry.phase.status(),
                    },
                    None => QueryReply::Unknown { key },
                };
                let mut requests = DynamicSupervisorRequests::empty();
                requests.query_replies = reply_to.deliver(reply);
                Ok(Actions::send(requests))
            }
            DynamicCommand::Cancel {
                authority,
                reply_to,
            } => self.cancel(authority, reply_to),
            DynamicCommand::Stop { key, reply_to } => self.stop(key, reply_to),
        }
    }

    fn command_while_shutting_down(
        &mut self,
        command: DynamicCommand<Key, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let mut requests = DynamicSupervisorRequests::empty();
        match command {
            DynamicCommand::Start {
                key,
                submission,
                reply_to,
            } => {
                requests.start_replies = reply_to.deliver(Err(WorkerChangeRejection {
                    key,
                    submission,
                    reason: StartRejection::ShuttingDown,
                }));
            }
            DynamicCommand::Replace {
                key,
                submission,
                reply_to,
            } => {
                requests.replace_replies = reply_to.deliver(Err(WorkerChangeRejection {
                    key,
                    submission,
                    reason: ReplaceRejection::ShuttingDown,
                }));
            }
            DynamicCommand::Stop { key, reply_to } => {
                requests.stop_replies = reply_to.deliver(Err(StopRejection::ShuttingDown { key }));
            }
            DynamicCommand::Query { key, reply_to } => {
                let reply = match self.services.get(&key) {
                    Some(_) => QueryReply::Known {
                        key,
                        status: DynamicStatus::Draining,
                    },
                    None => QueryReply::Unknown { key },
                };
                requests.query_replies = reply_to.deliver(reply);
            }
            DynamicCommand::Cancel {
                authority,
                reply_to,
            } => {
                requests.cancel_replies =
                    reply_to.deliver(CancellationReceipt::Draining { authority });
            }
        }
        Ok(Actions::send(requests))
    }

    fn cancel(
        &mut self,
        authority: CancelAuthority<Key>,
        reply_to: ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, CancellationReceipt<Key, Worker, Plan>>,
        >,
    ) -> BehaviorActed<Self> {
        let key = authority.key().clone();
        let Some(entry) = self.services.remove(&key) else {
            let mut requests = DynamicSupervisorRequests::empty();
            requests.cancel_replies = reply_to.deliver(CancellationReceipt::Stale { authority });
            return Ok(Actions::send(requests));
        };
        if authority.operation() != entry.operation.get() {
            self.services.insert(key, entry);
            let mut requests = DynamicSupervisorRequests::empty();
            requests.cancel_replies = reply_to.deliver(CancellationReceipt::Stale { authority });
            return Ok(Actions::send(requests));
        }
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        let mut proxy_operations = SourceActions::empty();
        let (phase, receipt) = match phase {
            DynamicEntryPhase::CreatingProxy { submission } => (
                DynamicEntryPhase::CancellingProxyCreation,
                CancellationReceipt::Returned {
                    authority,
                    submission,
                },
            ),
            DynamicEntryPhase::WaitingForActivation { proxy, submission } => {
                let (retirement, shutdown_request) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::CancelledWorkerReturned(WorkerChange::Start),
                );
                proxy_operations.send(shutdown_request);
                (
                    DynamicEntryPhase::Retiring(retirement),
                    CancellationReceipt::Returned {
                        authority,
                        submission,
                    },
                )
            }
            DynamicEntryPhase::WaitingForProxyInput { proxy, witness } => {
                let (retirement, shutdown_request) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::Cancellation(WorkerChange::Start),
                        input: ProxyInputCustody::AwaitingSettlement(witness),
                    },
                );
                proxy_operations.send(shutdown_request);
                (
                    DynamicEntryPhase::Retiring(retirement),
                    CancellationReceipt::Pending {
                        authority,
                        phase: DynamicStatus::AwaitingProxy,
                    },
                )
            }
            DynamicEntryPhase::WaitingForProxyOutcome {
                proxy,
                operation: proxy_operation,
            } => {
                let (retirement, shutdown_request) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::Cancellation(WorkerChange::Start),
                        input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                    },
                );
                proxy_operations.send(shutdown_request);
                (
                    DynamicEntryPhase::Retiring(retirement),
                    CancellationReceipt::Pending {
                        authority,
                        phase: DynamicStatus::AwaitingProxy,
                    },
                )
            }
            phase @ DynamicEntryPhase::CancellingProxyCreation => {
                (phase, CancellationReceipt::Cancelled { authority })
            }
            phase @ DynamicEntryPhase::Retiring(ProxyRetirement {
                work:
                    RetiringWork::CancelledWorkerReturned(_)
                    | RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::Cancellation(_),
                        ..
                    },
                ..
            }) => (phase, CancellationReceipt::Cancelled { authority }),
            phase @ DynamicEntryPhase::Retiring(_) => (
                phase,
                CancellationReceipt::Committed {
                    authority,
                    resulting_phase: DynamicStatus::Retiring,
                },
            ),
            phase @ (DynamicEntryPhase::ShuttingDownProxyCreation
            | DynamicEntryPhase::DrainingProxyCreationStop) => {
                (phase, CancellationReceipt::Draining { authority })
            }
            DynamicEntryPhase::Available {
                proxy,
                service,
                latest,
            } => {
                let resulting_phase = match &service {
                    ServiceAvailability::Ready { .. } => DynamicStatus::Ready {
                        proxy: proxy.recipient(),
                    },
                    ServiceAvailability::Empty { .. } => DynamicStatus::Empty,
                };
                match latest {
                    WorkerChangeDisposition::Committed => (
                        DynamicEntryPhase::Available {
                            proxy,
                            service,
                            latest: WorkerChangeDisposition::Committed,
                        },
                        CancellationReceipt::Committed {
                            authority,
                            resulting_phase,
                        },
                    ),
                    WorkerChangeDisposition::Cancelled => (
                        DynamicEntryPhase::Available {
                            proxy,
                            service,
                            latest: WorkerChangeDisposition::Cancelled,
                        },
                        CancellationReceipt::Cancelled { authority },
                    ),
                }
            }
            DynamicEntryPhase::ReplacementQueued {
                proxy,
                service,
                submission,
            } => (
                DynamicEntryPhase::Available {
                    proxy,
                    service,
                    latest: WorkerChangeDisposition::Cancelled,
                },
                CancellationReceipt::Returned {
                    authority,
                    submission,
                },
            ),
            DynamicEntryPhase::ReplacementAwaitingReceipt {
                proxy,
                service: _,
                witness,
            } => {
                let (retirement, shutdown_request) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::Cancellation(WorkerChange::Replacement),
                        input: ProxyInputCustody::AwaitingSettlement(witness),
                    },
                );
                proxy_operations.send(shutdown_request);
                (
                    DynamicEntryPhase::Retiring(retirement),
                    CancellationReceipt::Pending {
                        authority,
                        phase: DynamicStatus::Replacing,
                    },
                )
            }
            DynamicEntryPhase::ReplacementAwaitingOutcome {
                proxy,
                service: _,
                operation: proxy_operation,
            } => {
                let (retirement, shutdown_request) = ProxyRetirement::begin(
                    creation,
                    proxy,
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::Cancellation(WorkerChange::Replacement),
                        input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                    },
                );
                proxy_operations.send(shutdown_request);
                (
                    DynamicEntryPhase::Retiring(retirement),
                    CancellationReceipt::Pending {
                        authority,
                        phase: DynamicStatus::Replacing,
                    },
                )
            }
            phase @ (DynamicEntryPhase::Stopping { .. }
            | DynamicEntryPhase::StoppingProxyCreation { .. }) => (
                phase,
                CancellationReceipt::Committed {
                    authority,
                    resulting_phase: DynamicStatus::Stopping,
                },
            ),
        };
        self.services.insert(
            key,
            DynamicEntry {
                creation,
                generation,
                operation,
                phase,
            },
        );
        let mut requests = DynamicSupervisorRequests::empty();
        requests.proxy_operations = proxy_operations;
        requests.cancel_replies = reply_to.deliver(receipt);
        Ok(Actions::send(requests))
    }

    fn accept_creations(
        &mut self,
        proxies: behavior::CreationsSettled<BehaviorAddr<Worker>, StableProxy<Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        let settlements = match proxies.into_settlement() {
            behavior::CreationSettlement::Settled(settlements) => settlements,
            settlement => {
                return self.reject_input(DynamicSupervisorEvent::ProxyCreationsSettled(
                    behavior::CreationsSettled::new(settlement),
                ));
            }
        };
        if settlements.len() != 1 {
            return self.reject_input(DynamicSupervisorEvent::ProxyCreationsSettled(
                behavior::CreationsSettled::new(behavior::CreationSettlement::Settled(settlements)),
            ));
        }
        let settlement = match settlements.into_iter().next() {
            Some(settlement) => settlement,
            None => {
                return self.reject_input(DynamicSupervisorEvent::ProxyCreationsSettled(
                    behavior::CreationsSettled::new(behavior::CreationSettlement::Settled(
                        Creations::empty(),
                    )),
                ));
            }
        };
        let (creation, _) = identity(&settlement);
        let Some((key, entry)) = self.take_proxy_entry(creation) else {
            return self.reject_input(DynamicSupervisorEvent::ProxyCreationsSettled(
                behavior::CreationsSettled::new(behavior::CreationSettlement::Settled(
                    Creations::one(settlement),
                )),
            ));
        };
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        let (creation, submission) = match phase {
            DynamicEntryPhase::CreatingProxy { submission } => (creation, submission),
            DynamicEntryPhase::ShuttingDownProxyCreation => match resolve(settlement) {
                StableProxyCreation::Committed(proxy) => {
                    let (retirement, shutdown_request) = ProxyRetirement::begin(
                        creation,
                        proxy,
                        RetiringWork::SupervisorShutdown(None),
                    );
                    self.services.insert(
                        key,
                        DynamicEntry {
                            creation,
                            generation,
                            operation,
                            phase: DynamicEntryPhase::Retiring(retirement),
                        },
                    );
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.proxy_operations = SourceActions::sending(shutdown_request);
                    return Ok(Actions::send(requests));
                }
                StableProxyCreation::Rejected(creation_settlement) => {
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.lifecycle = self.entry_retired(
                        key.clone(),
                        generation.get(),
                        EntryRetirement::Shutdown,
                    );
                    requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                        DynamicDiagnostic::ProxyCreationRejected {
                            key,
                            generation: generation.get(),
                            creation: creation_settlement,
                        },
                    ));
                    return Ok(Actions::send(requests));
                }
            },
            DynamicEntryPhase::DrainingProxyCreationStop => match resolve(settlement) {
                StableProxyCreation::Committed(proxy) => {
                    let (witness, shutdown_request) = ProxyOperation::shutdown(creation);
                    self.services.insert(
                        key,
                        DynamicEntry {
                            creation,
                            generation,
                            operation,
                            phase: DynamicEntryPhase::Stopping {
                                proxy,
                                restorable: None,
                                shutdown: ProxyShutdown::AwaitingSettlement(witness),
                                stopped: None,
                            },
                        },
                    );
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.proxy_operations = SourceActions::sending(shutdown_request);
                    return Ok(Actions::send(requests));
                }
                StableProxyCreation::Rejected(creation_settlement) => {
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.lifecycle =
                        self.entry_retired(key.clone(), generation.get(), EntryRetirement::Stop);
                    requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                        DynamicDiagnostic::ProxyCreationRejected {
                            key,
                            generation: generation.get(),
                            creation: creation_settlement,
                        },
                    ));
                    return Ok(Actions::send(requests));
                }
            },
            DynamicEntryPhase::CancellingProxyCreation => match resolve(settlement) {
                StableProxyCreation::Committed(proxy) => {
                    let (retirement, shutdown_request) = ProxyRetirement::begin(
                        creation,
                        proxy,
                        RetiringWork::CancelledWorkerReturned(WorkerChange::Start),
                    );
                    self.services.insert(
                        key,
                        DynamicEntry {
                            creation,
                            generation,
                            operation,
                            phase: DynamicEntryPhase::Retiring(retirement),
                        },
                    );
                    let mut proxy_operations = SourceActions::empty();
                    proxy_operations.send(shutdown_request);
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.proxy_operations = proxy_operations;
                    return Ok(Actions::send(requests));
                }
                StableProxyCreation::Rejected(creation_settlement) => {
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.lifecycle = self.operation_cancelled(
                        key.clone(),
                        generation.get(),
                        operation.get(),
                        WorkerChange::Start,
                        CancellationOutcome::WorkerReturned,
                    );
                    requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                        DynamicDiagnostic::ProxyCreationRejected {
                            key,
                            generation: generation.get(),
                            creation: creation_settlement,
                        },
                    ));
                    return Ok(Actions::send(requests));
                }
            },
            DynamicEntryPhase::StoppingProxyCreation {
                start_operation,
                submission,
            } => match resolve(settlement) {
                StableProxyCreation::Committed(proxy) => {
                    let (witness, shutdown) = ProxyOperation::shutdown(creation);
                    self.services.insert(
                        key.clone(),
                        DynamicEntry {
                            creation,
                            generation,
                            operation,
                            phase: DynamicEntryPhase::Stopping {
                                proxy,
                                restorable: None,
                                shutdown: ProxyShutdown::AwaitingSettlement(witness),
                                stopped: None,
                            },
                        },
                    );
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.proxy_operations = SourceActions::sending(shutdown);
                    requests.lifecycle =
                        self.lifecycle
                            .clone()
                            .deliver(DynamicLifecycle::WorkerChangeInterrupted {
                                key,
                                generation: generation.get(),
                                operation: start_operation.get(),
                                interruption: WorkerChangeInterruption::ExplicitStop {
                                    operation: operation.get(),
                                },
                                worker: InterruptedWorker::Submission(submission),
                            });
                    return Ok(Actions::send(requests));
                }
                StableProxyCreation::Rejected(creation_settlement) => {
                    let mut requests = DynamicSupervisorRequests::empty();
                    requests.lifecycle = self
                        .lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::WorkerChangeInterrupted {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: start_operation.get(),
                            interruption: WorkerChangeInterruption::ExplicitStop {
                                operation: operation.get(),
                            },
                            worker: InterruptedWorker::Submission(submission),
                        })
                        .combine(self.entry_retired(
                            key.clone(),
                            generation.get(),
                            EntryRetirement::Stop,
                        ));
                    requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                        DynamicDiagnostic::ProxyCreationRejected {
                            key,
                            generation: generation.get(),
                            creation: creation_settlement,
                        },
                    ));
                    return Ok(Actions::send(requests));
                }
            },
            phase => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                return self.reject_input(DynamicSupervisorEvent::ProxyCreationsSettled(
                    behavior::CreationsSettled::new(behavior::CreationSettlement::Settled(
                        Creations::one(settlement),
                    )),
                ));
            }
        };
        match resolve(settlement) {
            StableProxyCreation::Committed(proxy) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::WaitingForActivation { proxy, submission },
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = self.authorize_waiting();
                Ok(Actions::send(requests))
            }
            StableProxyCreation::Rejected(creation_settlement) => {
                let mut requests = DynamicSupervisorRequests::empty();
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::StartCreationRejected {
                            key,
                            generation: generation.get(),
                            submission,
                            creation: creation_settlement,
                        });
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_proxy_input(
        &mut self,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let creation = proxy_input_creation(&input);
        let Some((key, entry)) = self.take_proxy_entry(creation) else {
            return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
        };
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        match phase {
            DynamicEntryPhase::WaitingForProxyInput { proxy, witness } => {
                self.accept_start_input(key, generation, operation, creation, proxy, witness, input)
            }
            DynamicEntryPhase::ReplacementAwaitingReceipt {
                proxy,
                service,
                witness,
            } => self.accept_replacement_input(
                key,
                DynamicEntry {
                    creation,
                    generation,
                    operation,
                    phase: DynamicEntryPhase::ReplacementAwaitingReceipt {
                        proxy,
                        service,
                        witness,
                    },
                },
                input,
            ),
            DynamicEntryPhase::Stopping {
                proxy,
                restorable,
                shutdown: ProxyShutdown::AwaitingSettlement(witness),
                stopped,
            } => self.accept_stopping_input(
                key, generation, operation, creation, proxy, restorable, witness, stopped, input,
            ),
            phase @ DynamicEntryPhase::Retiring(_) => self.accept_retiring_input(
                key,
                DynamicEntry {
                    creation,
                    generation,
                    operation,
                    phase,
                },
                input,
            ),
            phase => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input))
            }
        }
    }

    fn accept_start_input(
        &mut self,
        key: Key,
        generation: NonZeroU64,
        operation: NonZeroU64,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        witness: ProxyOperationWitness,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let input = match witness.admit(input) {
            Ok(input) => input,
            Err((witness, input)) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::WaitingForProxyInput { proxy, witness },
                    },
                );
                return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
            }
        };
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => {
                let (_, proxy, proxy_operation) = receipt.into_parts();
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::WaitingForProxyOutcome {
                            proxy,
                            operation: proxy_operation,
                        },
                    },
                );
                Ok(Actions::cont())
            }
            input => {
                let (retirement, operation_request) =
                    ProxyRetirement::begin(creation, proxy, RetiringWork::StartFailed);
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Retiring(retirement),
                    },
                );
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(operation_request);
                let waiting = self.authorize_waiting();
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.proxy_operations.append(waiting);
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::StartInputRejected {
                            key: key.clone(),
                            generation: generation.get(),
                        });
                requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                    DynamicDiagnostic::ProxyInputRejected {
                        key,
                        generation: generation.get(),
                        input,
                    },
                ));
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_replacement_input(
        &mut self,
        key: Key,
        entry: DynamicEntry<Worker, Plan>,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase:
                DynamicEntryPhase::ReplacementAwaitingReceipt {
                    proxy,
                    service,
                    witness,
                },
        } = entry
        else {
            return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
        };
        let input = match witness.admit(input) {
            Ok(input) => input,
            Err((witness, input)) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::ReplacementAwaitingReceipt {
                            proxy,
                            service,
                            witness,
                        },
                    },
                );
                return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
            }
        };
        match input {
            SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => {
                let (_, proxy, proxy_operation) = receipt.into_parts();
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::ReplacementAwaitingOutcome {
                            proxy,
                            service,
                            operation: proxy_operation,
                        },
                    },
                );
                Ok(Actions::cont())
            }
            input => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Available {
                            proxy,
                            service,
                            latest: WorkerChangeDisposition::Committed,
                        },
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = self.authorize_waiting();
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::ReplacementInputRejected {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: operation.get(),
                        });
                requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                    DynamicDiagnostic::ProxyInputRejected {
                        key,
                        generation: generation.get(),
                        input,
                    },
                ));
                Ok(Actions::send(requests))
            }
        }
    }

    fn accept_stopping_input(
        &mut self,
        key: Key,
        generation: NonZeroU64,
        operation: NonZeroU64,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        restorable: Option<ServiceAvailability<Plan::Ready>>,
        witness: ProxyOperationWitness,
        stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let input = match witness.admit(input) {
            Ok(input) => input,
            Err((witness, input)) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Stopping {
                            proxy,
                            restorable,
                            shutdown: ProxyShutdown::AwaitingSettlement(witness),
                            stopped,
                        },
                    },
                );
                return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
            }
        };
        let (outcome, input) = match entry_stop_result(stopped, input) {
            Ok(receipt) => {
                let (_, proxy, shutdown) = receipt.into_parts();
                let mut requests = DynamicSupervisorRequests::empty();
                match stopped {
                    Some(stopped) => {
                        requests.lifecycle = self
                            .lifecycle
                            .clone()
                            .deliver(DynamicLifecycle::StopFinished {
                                key: key.clone(),
                                generation: generation.get(),
                                operation: operation.get(),
                                proxy,
                                result: Ok(stopped),
                            })
                            .combine(self.entry_retired(
                                key,
                                generation.get(),
                                EntryRetirement::Stop,
                            ));
                    }
                    None => {
                        self.services.insert(
                            key,
                            DynamicEntry {
                                creation,
                                generation,
                                operation,
                                phase: DynamicEntryPhase::Stopping {
                                    proxy,
                                    restorable,
                                    shutdown: ProxyShutdown::Accepted(shutdown),
                                    stopped: None,
                                },
                            },
                        );
                    }
                }
                return Ok(Actions::send(requests));
            }
            Err(rejected) => rejected,
        };
        let mut requests = DynamicSupervisorRequests::empty();
        requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
            DynamicDiagnostic::ProxyShutdownRejected {
                key: key.clone(),
                generation: generation.get(),
                input,
            },
        ));
        requests.lifecycle = match (outcome, restorable) {
            (failure @ EntryStopFailure { stopped: None, .. }, Some(service)) => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Available {
                            proxy: proxy.clone(),
                            service,
                            latest: WorkerChangeDisposition::Committed,
                        },
                    },
                );
                self.lifecycle
                    .clone()
                    .deliver(DynamicLifecycle::StopFinished {
                        key,
                        generation: generation.get(),
                        operation: operation.get(),
                        proxy,
                        result: Err(failure),
                    })
            }
            (failure @ EntryStopFailure { stopped: None, .. }, None) => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Retiring(ProxyRetirement {
                            proxy: proxy.clone(),
                            shutdown: ProxyShutdown::Rejected,
                            work: RetiringWork::InterruptedStart,
                            stopped: None,
                        }),
                    },
                );
                self.lifecycle
                    .clone()
                    .deliver(DynamicLifecycle::StopFinished {
                        key,
                        generation: generation.get(),
                        operation: operation.get(),
                        proxy,
                        result: Err(failure),
                    })
            }
            (
                failure @ EntryStopFailure {
                    stopped: Some(_), ..
                },
                _,
            ) => self
                .lifecycle
                .clone()
                .deliver(DynamicLifecycle::StopFinished {
                    key: key.clone(),
                    generation: generation.get(),
                    operation: operation.get(),
                    proxy,
                    result: Err(failure),
                })
                .combine(self.entry_retired(key, generation.get(), EntryRetirement::Stop)),
        };
        Ok(Actions::send(requests))
    }

    fn accept_retiring_input(
        &mut self,
        key: Key,
        entry: DynamicEntry<Worker, Plan>,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> BehaviorActed<Self> {
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase:
                DynamicEntryPhase::Retiring(ProxyRetirement {
                    proxy,
                    shutdown,
                    work,
                    stopped,
                }),
        } = entry
        else {
            return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
        };
        let mut requests = DynamicSupervisorRequests::empty();
        let (proxy, shutdown, remaining_input) = match shutdown {
            ProxyShutdown::AwaitingSettlement(witness) => match witness.admit(input) {
                Ok(SettledItem::Attempted(ItemSettlement::Accepted(receipt))) => {
                    let (_, proxy, operation) = receipt.into_parts();
                    (proxy, ProxyShutdown::Accepted(operation), None)
                }
                Ok(input) => match &work {
                    RetiringWork::Transferred {
                        purpose: ProxyInputPurpose::InterruptedStart(_),
                        ..
                    } => match entry_stop_result(stopped, input) {
                        Ok(receipt) => {
                            let (_, proxy, operation) = receipt.into_parts();
                            (proxy, ProxyShutdown::Accepted(operation), None)
                        }
                        Err((failure, input)) => {
                            requests.lifecycle =
                                self.lifecycle
                                    .clone()
                                    .deliver(DynamicLifecycle::StopFinished {
                                        key: key.clone(),
                                        generation: generation.get(),
                                        operation: operation.get(),
                                        proxy: proxy.clone(),
                                        result: Err(failure),
                                    });
                            requests.diagnostics = InterpreterRequests::one(
                                self.diagnostics
                                    .action(DynamicDiagnostic::ProxyShutdownRejected {
                                        key: key.clone(),
                                        generation: generation.get(),
                                        input,
                                    }),
                            );
                            (proxy, ProxyShutdown::Rejected, None)
                        }
                    },
                    _ => {
                        requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                            DynamicDiagnostic::ProxyShutdownRejected {
                                key: key.clone(),
                                generation: generation.get(),
                                input,
                            },
                        ));
                        (proxy, ProxyShutdown::Rejected, None)
                    }
                },
                Err((witness, input)) => (
                    proxy,
                    ProxyShutdown::AwaitingSettlement(witness),
                    Some(input),
                ),
            },
            shutdown => (proxy, shutdown, Some(input)),
        };
        let work = match remaining_input {
            None => work,
            Some(input) => match work.settle_input(input) {
                Ok((work, rejected)) => {
                    if let Some(input) = rejected {
                        requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                            DynamicDiagnostic::ProxyInputRejected {
                                key: key.clone(),
                                generation: generation.get(),
                                input,
                            },
                        ));
                    }
                    work
                }
                Err((work, input)) => {
                    self.services.insert(
                        key,
                        DynamicEntry {
                            creation,
                            generation,
                            operation,
                            phase: DynamicEntryPhase::Retiring(ProxyRetirement {
                                proxy,
                                shutdown,
                                work,
                                stopped,
                            }),
                        },
                    );
                    return self.reject_input(DynamicSupervisorEvent::ProxyInputSettled(input));
                }
            },
        };
        let retirement = ProxyRetirement {
            proxy,
            shutdown,
            work,
            stopped,
        };
        requests.lifecycle = requests
            .lifecycle
            .combine(self.settle_retirement(key, creation, generation, operation, retirement));
        requests.proxy_operations = self.authorize_waiting();
        Ok(Actions::send(requests))
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "compile-only proof of closed retirement lifecycle construction"
    )]
    fn retirement_lifecycle_is_closed(
        &self,
        key: Key,
        generation: u64,
        operation: u64,
        cause: EntryRetirement,
        change: WorkerChange,
        outcome: CancellationOutcome<Worker, Plan>,
    ) {
        let _ = self.entry_retired(key.clone(), generation, cause);
        let _ = self.operation_cancelled(key, generation, operation, change, outcome);
    }

    fn entry_retired(
        &self,
        key: Key,
        generation: u64,
        cause: EntryRetirement,
    ) -> LifecycleRoute::Sends {
        self.lifecycle
            .clone()
            .deliver(DynamicLifecycle::EntryRetired {
                key,
                generation,
                cause,
            })
    }

    fn operation_cancelled(
        &self,
        key: Key,
        generation: u64,
        operation: u64,
        change: WorkerChange,
        outcome: CancellationOutcome<Worker, Plan>,
    ) -> LifecycleRoute::Sends {
        let lifecycle = self.lifecycle.clone();
        lifecycle
            .clone()
            .deliver(DynamicLifecycle::OperationCancelled {
                key: key.clone(),
                generation,
                operation,
                change,
                outcome,
            })
            .combine(lifecycle.deliver(DynamicLifecycle::EntryRetired {
                key,
                generation,
                cause: EntryRetirement::Cancellation,
            }))
    }

    fn settle_retirement(
        &mut self,
        key: Key,
        creation: CreationId,
        generation: NonZeroU64,
        operation: NonZeroU64,
        retirement: ProxyRetirement<Worker, Plan>,
    ) -> LifecycleRoute::Sends {
        match self.close_retirement(key.clone(), generation.get(), operation.get(), retirement) {
            Ok(lifecycle) => lifecycle,
            Err(retirement) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Retiring(retirement),
                    },
                );
                SendEffects::empty()
            }
        }
    }

    fn close_retirement(
        &self,
        key: Key,
        generation: u64,
        operation: u64,
        retirement: ProxyRetirement<Worker, Plan>,
    ) -> Result<LifecycleRoute::Sends, ProxyRetirement<Worker, Plan>> {
        let ProxyRetirement {
            proxy,
            shutdown,
            work,
            stopped,
        } = retirement;
        let stopped = match (shutdown, stopped) {
            (shutdown @ (ProxyShutdown::Accepted(_) | ProxyShutdown::Rejected), Some(stopped)) => {
                (shutdown, stopped)
            }
            (shutdown, stopped) => {
                return Err(ProxyRetirement {
                    proxy,
                    shutdown,
                    work,
                    stopped,
                });
            }
        };
        match work {
            RetiringWork::StartFailed => {
                Ok(self.entry_retired(key, generation, EntryRetirement::StartFailed))
            }
            RetiringWork::UnexpectedWorkerStopped => {
                Ok(self.entry_retired(key, generation, EntryRetirement::UnexpectedWorkerStopped))
            }
            RetiringWork::InterruptedStart => {
                Ok(self.entry_retired(key, generation, EntryRetirement::Stop))
            }
            RetiringWork::SupervisorShutdown(service) => {
                drop(service);
                Ok(self.entry_retired(key, generation, EntryRetirement::Shutdown))
            }
            RetiringWork::CancelledWorkerReturned(change) => Ok(self.operation_cancelled(
                key,
                generation,
                operation,
                change,
                CancellationOutcome::WorkerReturned,
            )),
            RetiringWork::Transferred {
                purpose: ProxyInputPurpose::Cancellation(change),
                input,
            } => {
                let outcome = match input {
                    ProxyInputCustody::InputRejected => CancellationOutcome::ProxyInputRejected,
                    ProxyInputCustody::ProxyReported(outcome) => {
                        CancellationOutcome::ProxyReported { outcome }
                    }
                    input => {
                        return Err(ProxyRetirement {
                            proxy,
                            shutdown: stopped.0,
                            work: RetiringWork::Transferred {
                                purpose: ProxyInputPurpose::Cancellation(change),
                                input,
                            },
                            stopped: Some(stopped.1),
                        });
                    }
                };
                Ok(self.operation_cancelled(key, generation, operation, change, outcome))
            }
            RetiringWork::Transferred {
                purpose: ProxyInputPurpose::InterruptedStart(start_operation),
                input,
            } => {
                let shutdown = match stopped.0 {
                    ProxyShutdown::Accepted(operation) => Ok(operation),
                    ProxyShutdown::Rejected => Err(()),
                    ProxyShutdown::AwaitingSettlement(witness) => {
                        return Err(ProxyRetirement {
                            proxy,
                            shutdown: ProxyShutdown::AwaitingSettlement(witness),
                            work: RetiringWork::Transferred {
                                purpose: ProxyInputPurpose::InterruptedStart(start_operation),
                                input,
                            },
                            stopped: Some(stopped.1),
                        });
                    }
                };
                let worker = match input.into_interrupted_worker() {
                    Ok(worker) => worker,
                    Err(input) => {
                        return Err(ProxyRetirement {
                            proxy,
                            shutdown: match shutdown {
                                Ok(operation) => ProxyShutdown::Accepted(operation),
                                Err(()) => ProxyShutdown::Rejected,
                            },
                            work: RetiringWork::Transferred {
                                purpose: ProxyInputPurpose::InterruptedStart(start_operation),
                                input,
                            },
                            stopped: Some(stopped.1),
                        });
                    }
                };
                let interrupted =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::WorkerChangeInterrupted {
                            key: key.clone(),
                            generation,
                            operation: start_operation.get(),
                            interruption: WorkerChangeInterruption::ExplicitStop { operation },
                            worker,
                        });
                let retired = self.entry_retired(key.clone(), generation, EntryRetirement::Stop);
                match shutdown {
                    Ok(shutdown_operation) => {
                        drop(shutdown_operation);
                        Ok(interrupted
                            .combine(self.lifecycle.clone().deliver(
                                DynamicLifecycle::StopFinished {
                                    key,
                                    generation,
                                    operation,
                                    proxy,
                                    result: Ok(stopped.1),
                                },
                            ))
                            .combine(retired))
                    }
                    Err(()) => {
                        drop(proxy);
                        Ok(interrupted.combine(retired))
                    }
                }
            }
            RetiringWork::Transferred { purpose, input } => {
                let (worker_operation, change) = match &purpose {
                    ProxyInputPurpose::SupervisorStart(operation) => {
                        (*operation, WorkerChange::Start)
                    }
                    ProxyInputPurpose::SupervisorReplacement { operation, service } => {
                        let _ = service;
                        (*operation, WorkerChange::Replacement)
                    }
                    ProxyInputPurpose::Cancellation(_) | ProxyInputPurpose::InterruptedStart(_) => {
                        return Err(ProxyRetirement {
                            proxy,
                            shutdown: stopped.0,
                            work: RetiringWork::Transferred { purpose, input },
                            stopped: Some(stopped.1),
                        });
                    }
                };
                let worker = match input.into_interrupted_worker() {
                    Ok(worker) => worker,
                    Err(input) => {
                        return Err(ProxyRetirement {
                            proxy,
                            shutdown: stopped.0,
                            work: RetiringWork::Transferred { purpose, input },
                            stopped: Some(stopped.1),
                        });
                    }
                };
                drop((proxy, stopped));
                Ok(self
                    .lifecycle
                    .clone()
                    .deliver(DynamicLifecycle::WorkerChangeInterrupted {
                        key: key.clone(),
                        generation,
                        operation: worker_operation.get(),
                        interruption: WorkerChangeInterruption::SupervisorShutdown { change },
                        worker,
                    })
                    .combine(self.entry_retired(key, generation, EntryRetirement::Shutdown)))
            }
        }
    }

    fn accept_proxy_stop(
        &mut self,
        stopped: crate::ChildStopped<BehaviorAddr<Worker>>,
    ) -> BehaviorActed<Self> {
        let Some((key, entry)) = self.take_proxy_entry(stopped.child) else {
            let mut requests = DynamicSupervisorRequests::empty();
            requests.diagnostics = InterpreterRequests::one(
                self.diagnostics
                    .action(DynamicDiagnostic::RejectedProxyStop { stopped }),
            );
            return Ok(Actions::send(requests));
        };
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        match phase {
            DynamicEntryPhase::Stopping {
                proxy,
                restorable,
                shutdown: ProxyShutdown::AwaitingSettlement(witness),
                stopped: None,
            } => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Stopping {
                            proxy,
                            restorable,
                            shutdown: ProxyShutdown::AwaitingSettlement(witness),
                            stopped: Some(stopped),
                        },
                    },
                );
                Ok(Actions::cont())
            }
            DynamicEntryPhase::Stopping {
                proxy,
                restorable: _,
                shutdown: ProxyShutdown::Accepted(shutdown),
                stopped: None,
            } => {
                drop(shutdown);
                let mut requests = DynamicSupervisorRequests::empty();
                requests.lifecycle = self
                    .lifecycle
                    .clone()
                    .deliver(DynamicLifecycle::StopFinished {
                        key: key.clone(),
                        generation: generation.get(),
                        operation: operation.get(),
                        proxy,
                        result: Ok(stopped),
                    })
                    .combine(self.entry_retired(key, generation.get(), EntryRetirement::Stop));
                Ok(Actions::send(requests))
            }
            DynamicEntryPhase::Retiring(ProxyRetirement {
                proxy,
                shutdown,
                work,
                stopped: None,
            }) => {
                let mut requests = DynamicSupervisorRequests::empty();
                let retirement = ProxyRetirement {
                    proxy,
                    shutdown,
                    work,
                    stopped: Some(stopped),
                };
                requests.lifecycle =
                    self.settle_retirement(key, creation, generation, operation, retirement);
                Ok(Actions::send(requests))
            }
            phase => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.diagnostics = InterpreterRequests::one(
                    self.diagnostics
                        .action(DynamicDiagnostic::RejectedProxyStop { stopped }),
                );
                Ok(Actions::send(requests))
            }
        }
    }

    fn replacement_rejected(
        &mut self,
        key: Key,
        entry: DynamicEntry<Worker, Plan>,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        self.services.insert(key, entry);
        let mut requests = DynamicSupervisorRequests::empty();
        requests.diagnostics = InterpreterRequests::one(
            self.diagnostics
                .action(DynamicDiagnostic::RejectedProxyOutcome { report }),
        );
        Ok(Actions::send(requests))
    }

    fn accept_proxy_report(
        &mut self,
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    ) -> BehaviorActed<Self> {
        let child = report.child;
        let Some((key, entry)) = self.take_proxy_entry(child) else {
            let mut requests = DynamicSupervisorRequests::empty();
            requests.diagnostics = InterpreterRequests::one(
                self.diagnostics
                    .action(DynamicDiagnostic::RejectedProxyOutcome { report }),
            );
            return Ok(Actions::send(requests));
        };
        let DynamicEntry {
            creation,
            generation,
            operation,
            phase,
        } = entry;
        match (phase, report.report) {
            (
                DynamicEntryPhase::WaitingForProxyOutcome {
                    proxy,
                    operation: proxy_operation,
                },
                ProxyOutcome::Initial {
                    outcome:
                        InitialWorkerOutcome::Resolved {
                            result: WorkerStartResult::Ready { attempt, readiness },
                        },
                },
            ) => {
                drop(proxy_operation);
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Available {
                            proxy: proxy.clone(),
                            service: ServiceAvailability::Ready {
                                worker: attempt,
                                readiness,
                            },
                            latest: WorkerChangeDisposition::Committed,
                        },
                    },
                );
                let waiting = self.authorize_waiting();
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = waiting;
                requests.lifecycle = self.lifecycle.clone().deliver(DynamicLifecycle::Started {
                    key,
                    generation: generation.get(),
                    operation: operation.get(),
                    proxy,
                });
                Ok(Actions::send(requests))
            }
            (
                DynamicEntryPhase::WaitingForProxyOutcome {
                    proxy,
                    operation: proxy_operation,
                },
                ProxyOutcome::Initial { outcome },
            ) => {
                drop(proxy_operation);
                let (retirement, shutdown_request) =
                    ProxyRetirement::begin(creation, proxy, RetiringWork::StartFailed);
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Retiring(retirement),
                    },
                );
                let mut proxy_operations = SourceActions::empty();
                proxy_operations.send(shutdown_request);
                let waiting = self.authorize_waiting();
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.proxy_operations.append(waiting);
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::StartOutcomeRejected {
                            key,
                            generation: generation.get(),
                            operation: operation.get(),
                            outcome,
                        });
                Ok(Actions::send(requests))
            }
            (
                DynamicEntryPhase::ReplacementAwaitingOutcome {
                    proxy,
                    service,
                    operation: proxy_operation,
                },
                ProxyOutcome::Replacement { outcome },
            ) => {
                let current_worker = match &service {
                    ServiceAvailability::Ready { worker, .. }
                    | ServiceAvailability::Empty { previous: worker } => worker,
                };
                match &outcome {
                    ReplacementOutcome::WorkerAttemptsExhausted { replaces, .. }
                    | ReplacementOutcome::CancelledBeforeBirth { replaces, .. }
                    | ReplacementOutcome::Resolved { replaces, .. }
                        if replaces != current_worker =>
                    {
                        return self.replacement_rejected(
                            key,
                            DynamicEntry {
                                creation,
                                generation,
                                operation,
                                phase: DynamicEntryPhase::ReplacementAwaitingOutcome {
                                    proxy,
                                    service,
                                    operation: proxy_operation,
                                },
                            },
                            ChildReport::new(child, ProxyOutcome::Replacement { outcome }),
                        );
                    }
                    _ => {}
                }

                let replacement = match outcome {
                    ReplacementOutcome::Resolved {
                        replaces: _,
                        result: WorkerStartResult::Ready { attempt, readiness },
                    } => Ok(ServiceAvailability::Ready {
                        worker: attempt,
                        readiness,
                    }),
                    ReplacementOutcome::NotReplaceable {
                        worker,
                        activation,
                        phase,
                    } => Err((
                        service,
                        ReplacementFailure::ProxyRefused {
                            worker,
                            activation,
                            phase,
                        },
                    )),
                    ReplacementOutcome::WorkerAttemptsExhausted {
                        replaces: _,
                        worker,
                        activation,
                    } => Err((
                        service,
                        ReplacementFailure::WorkerAttemptsExhausted { worker, activation },
                    )),
                    ReplacementOutcome::Resolved {
                        replaces,
                        result:
                            WorkerStartResult::CreationRejected {
                                rejection,
                                activation,
                                stopped,
                            },
                    } => Err((
                        ServiceAvailability::Empty { previous: replaces },
                        ReplacementFailure::WorkerCreationRejected {
                            rejection,
                            activation,
                            stopped,
                        },
                    )),
                    ReplacementOutcome::Resolved {
                        replaces: _,
                        result: WorkerStartResult::Unavailable { attempt, drain },
                    } => Err((
                        ServiceAvailability::Empty { previous: attempt },
                        ReplacementFailure::WorkerUnavailable { drain },
                    )),
                    outcome @ ReplacementOutcome::CancelledBeforeBirth { .. } => {
                        return self.replacement_rejected(
                            key,
                            DynamicEntry {
                                creation,
                                generation,
                                operation,
                                phase: DynamicEntryPhase::ReplacementAwaitingOutcome {
                                    proxy,
                                    service,
                                    operation: proxy_operation,
                                },
                            },
                            ChildReport::new(child, ProxyOutcome::Replacement { outcome }),
                        );
                    }
                };
                let (service, lifecycle) = match replacement {
                    Ok(service) => (
                        service,
                        DynamicLifecycle::Replaced {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: operation.get(),
                            proxy: proxy.clone(),
                        },
                    ),
                    Err((service, failure)) => (
                        service,
                        DynamicLifecycle::ReplacementFailed {
                            key: key.clone(),
                            generation: generation.get(),
                            operation: operation.get(),
                            failure,
                        },
                    ),
                };
                drop(proxy_operation);
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase: DynamicEntryPhase::Available {
                            proxy,
                            service,
                            latest: WorkerChangeDisposition::Committed,
                        },
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = self.authorize_waiting();
                requests.lifecycle = self.lifecycle.clone().deliver(lifecycle);
                Ok(Actions::send(requests))
            }
            (
                DynamicEntryPhase::Retiring(ProxyRetirement {
                    proxy,
                    shutdown,
                    work:
                        RetiringWork::Transferred {
                            purpose,
                            input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                        },
                    stopped,
                }),
                outcome @ (ProxyOutcome::Initial { .. } | ProxyOutcome::Replacement { .. }),
            ) => {
                match (&purpose, &outcome) {
                    (
                        ProxyInputPurpose::Cancellation(WorkerChange::Start)
                        | ProxyInputPurpose::InterruptedStart(_)
                        | ProxyInputPurpose::SupervisorStart(_),
                        ProxyOutcome::Initial { .. },
                    )
                    | (
                        ProxyInputPurpose::Cancellation(WorkerChange::Replacement)
                        | ProxyInputPurpose::SupervisorReplacement { .. },
                        ProxyOutcome::Replacement { .. },
                    ) => {}
                    _ => {
                        return self.replacement_rejected(
                            key,
                            DynamicEntry {
                                creation,
                                generation,
                                operation,
                                phase: DynamicEntryPhase::Retiring(ProxyRetirement {
                                    proxy,
                                    shutdown,
                                    work: RetiringWork::Transferred {
                                        purpose,
                                        input: ProxyInputCustody::AwaitingOutcome(proxy_operation),
                                    },
                                    stopped,
                                }),
                            },
                            ChildReport::new(child, outcome),
                        );
                    }
                }
                drop(proxy_operation);
                let retirement = ProxyRetirement {
                    proxy,
                    shutdown,
                    work: RetiringWork::Transferred {
                        purpose,
                        input: ProxyInputCustody::ProxyReported(outcome),
                    },
                    stopped,
                };
                let mut requests = DynamicSupervisorRequests::empty();
                requests.lifecycle =
                    self.settle_retirement(key, creation, generation, operation, retirement);
                requests.proxy_operations = self.authorize_waiting();
                Ok(Actions::send(requests))
            }
            (
                phase,
                ProxyOutcome::Unavailable {
                    sender,
                    phase: proxy_phase,
                    command,
                },
            ) => {
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::CommandUnavailable {
                            key,
                            generation: generation.get(),
                            sender,
                            proxy_phase,
                            command,
                        });
                Ok(Actions::send(requests))
            }
            (
                DynamicEntryPhase::Available {
                    proxy,
                    service: ServiceAvailability::Ready { worker, readiness },
                    latest,
                },
                ProxyOutcome::WorkerStopped {
                    worker: stopped_worker,
                    stopped,
                },
            ) if worker == stopped_worker => {
                let disposition = self.unexpected_exit;
                let mut proxy_operations = SourceActions::empty();
                let phase = match disposition {
                    UnexpectedExit::KeepEmpty => DynamicEntryPhase::Available {
                        proxy,
                        service: ServiceAvailability::Empty { previous: worker },
                        latest,
                    },
                    UnexpectedExit::Retire => {
                        let (retirement, shutdown_request) = ProxyRetirement::begin(
                            creation,
                            proxy,
                            RetiringWork::UnexpectedWorkerStopped,
                        );
                        proxy_operations.send(shutdown_request);
                        DynamicEntryPhase::Retiring(retirement)
                    }
                };
                self.services.insert(
                    key.clone(),
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.proxy_operations = proxy_operations;
                requests.lifecycle =
                    self.lifecycle
                        .clone()
                        .deliver(DynamicLifecycle::UnexpectedWorkerStopped {
                            key,
                            generation: generation.get(),
                            worker: stopped_worker,
                            readiness,
                            stopped,
                            disposition,
                        });
                Ok(Actions::send(requests))
            }
            (phase, outcome) => {
                self.services.insert(
                    key,
                    DynamicEntry {
                        creation,
                        generation,
                        operation,
                        phase,
                    },
                );
                let mut requests = DynamicSupervisorRequests::empty();
                requests.diagnostics = InterpreterRequests::one(self.diagnostics.action(
                    DynamicDiagnostic::RejectedProxyOutcome {
                        report: ChildReport::new(child, outcome),
                    },
                ));
                Ok(Actions::send(requests))
            }
        }
    }
}

fn entry_stop_result<Worker, Plan>(
    stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
    input: ProxyInputResult<behavior::Here, Worker, Plan>,
) -> Result<
    crate::ProxyInputReceipt<Worker, Plan>,
    (
        EntryStopFailure<BehaviorAddr<Worker>>,
        ProxyInputResult<behavior::Here, Worker, Plan>,
    ),
>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    match input {
        SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => Ok(receipt),
        SettledItem::Attempted(ItemSettlement::Rejected { item, reason }) => Err((
            EntryStopFailure {
                reason: EntryStopFailureReason::ControlRejected(reason),
                stopped,
            },
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
        )),
        SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }) => Err((
            EntryStopFailure {
                reason: EntryStopFailureReason::InterpreterCorrupt(fault),
                stopped,
            },
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }),
        )),
        SettledItem::Unattempted(item) => Err((
            EntryStopFailure {
                reason: EntryStopFailureReason::InterpretationSkipped,
                stopped,
            },
            SettledItem::Unattempted(item),
        )),
        SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
            match prerequisite {}
        }
    }
}

fn proxy_input_creation<Worker, Plan>(
    input: &ProxyInputResult<behavior::Here, Worker, Plan>,
) -> CreationId
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    match input {
        SettledItem::Attempted(ItemSettlement::Accepted(receipt)) => receipt.creation(),
        SettledItem::Attempted(
            ItemSettlement::Rejected {
                item: operation, ..
            }
            | ItemSettlement::Corrupt {
                item: operation, ..
            },
        )
        | SettledItem::Unattempted(operation) => operation.creation(),
        SettledItem::Attempted(ItemSettlement::Blocked { prerequisite, .. }) => {
            match *prerequisite {}
        }
    }
}

impl<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType> Behavior
    for DynamicSupervisor<Key, Worker, Plan, LifecycleRoute, DiagnosticRouteType>
where
    Key: Clone + Ord + Send + Sync,
    Worker: Behavior + Send,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    <BehaviorAddr<Worker> as Address>::Nonce: Send,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    EstablishedActor<StableProxy<Worker, Plan>>: Clone + Send,
    LifecycleRoute: DeliveryRoute<
            Protocol = MessageProtocol<BehaviorAddr<Worker>, DynamicLifecycle<Key, Worker, Plan>>,
        > + Clone,
    LifecycleRoute::Sends: behavior::SendsFor<DynamicSupervisorEvent<Key, Worker, Plan>>,
    DiagnosticRouteType: crate::DiagnosticRoute<DynamicDiagnostic<Key, Worker, Plan>> + Clone,
{
    type Protocol = MessageProtocol<BehaviorAddr<Worker>, DynamicCommand<Key, Worker, Plan>>;
    type Event = DynamicSupervisorEvent<Key, Worker, Plan>;
    type Sends = DynamicSupervisorRequests<
        InterpreterRequests<ObserveChild<Worker::Protocol, ChildHead>>,
        SourceActions<ProxyOperation<behavior::Here, Worker, Plan>>,
        SourceActions<ScheduleAfter>,
        <ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, StartRejection>,
                >,
            >,
        > as DeliveryRoute>::Sends,
        <ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, ReplaceRejection>,
                >,
            >,
        > as DeliveryRoute>::Sends,
        <ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, Result<Key, StopRejection<Key>>>,
        > as DeliveryRoute>::Sends,
        <ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, QueryReply<Key, Worker::Protocol>>,
        > as DeliveryRoute>::Sends,
        <ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, CancellationReceipt<Key, Worker, Plan>>,
        > as DeliveryRoute>::Sends,
        LifecycleRoute::Sends,
        InterpreterRequests<
            DiagnosticAction<DiagnosticRouteType, DynamicDiagnostic<Key, Worker, Plan>>,
        >,
    >;
    type Ph = Never;
    type Error = DynamicSupervisorEvent<Key, Worker, Plan>;
    type Birth = Births<StableProxy<Worker, Plan>>;

    fn init(&mut self, _: InitializationTurn) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }

    fn transition(&mut self, _: ActiveTurn, input: Self::Event) -> BehaviorActed<Self> {
        let acted = match input {
            DynamicSupervisorEvent::Command(User { message, .. }) => self.command(message),
            DynamicSupervisorEvent::ProxyCreationsSettled(proxies) => {
                self.accept_creations(proxies)
            }
            DynamicSupervisorEvent::ProxyInputSettled(input) => self.accept_proxy_input(input),
            DynamicSupervisorEvent::ProxyReported(report) => self.accept_proxy_report(report),
            DynamicSupervisorEvent::ProxyStopped(stopped) => self.accept_proxy_stop(stopped),
            DynamicSupervisorEvent::Shutdown(_) => self.begin_shutdown(),
            DynamicSupervisorEvent::ShutdownScheduleSettled(input) => {
                match &mut self.availability {
                    SupervisorAvailability::ShuttingDown(deadline) => deadline
                        .accept_schedule(input)
                        .map(|()| Actions::cont())
                        .map_err(DynamicSupervisorEvent::ShutdownScheduleSettled),
                    SupervisorAvailability::Accepting(_) => {
                        Err(DynamicSupervisorEvent::ShutdownScheduleSettled(input))
                    }
                }
            }
            DynamicSupervisorEvent::ShutdownElapsed(elapsed) => match &mut self.availability {
                SupervisorAvailability::ShuttingDown(deadline) => deadline
                    .accept_elapsed(elapsed)
                    .map(|()| Actions::cont())
                    .map_err(DynamicSupervisorEvent::ShutdownElapsed),
                SupervisorAvailability::Accepting(_) => {
                    Err(DynamicSupervisorEvent::ShutdownElapsed(elapsed))
                }
            },
        };
        self.apply_shutdown_step(acted)
    }
}
