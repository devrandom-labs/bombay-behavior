//! Proven construction orders spanning existing behavior templates.

use behavior::{Address, Behavior, Recipient};

use crate::{
    AffinitySelector, Backoff, BackoffSuperviseWithParent, BackoffWorkersWithParent, ChildTopology,
    DeliveryRoute, DeliveryRouteProtocol, DynamicSupervisorOutcome, DynamicSupervisorWithParent,
    FleetError, KeyedWorkerPoolProtocol, KeyedWorkerPoolWithParent, PoolAssignment,
    PoolConfigError, PoolConfiguration, PoolResponse, Protocol, ProxyParentIngress,
    RestartConfiguration, SuperviseWithParent, SupervisedWorkersWithParent, TimerId,
    WorkerPoolProtocol, WorkerPoolWithParent,
};

/// Compose an application behavior with ownership of its fixed child fleet.
///
/// The enclosing actor type infers the final path used by stable proxies to
/// return lifecycle facts.
pub fn supervise<B, C, ParentPath>(
    inner: B,
    topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
    restart: RestartConfiguration,
) -> Result<
    SuperviseWithParent<B, C, ParentPath>,
    FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>,
>
where
    B: Behavior<Birth = behavior::Births<C>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: Protocol<Addr = crate::BehaviorAddr<B>>,
{
    SuperviseWithParent::with_parent(inner, topology, restart, ProxyParentIngress::new())
}

/// Compose application-owned fixed children with generation-safe restart delay.
pub fn supervise_backoff<B, C, ParentPath>(
    inner: B,
    topology: ChildTopology<<crate::BehaviorAddr<B> as Address>::Nonce, C>,
    restart: RestartConfiguration,
    backoff: Backoff,
    timer: fn(<crate::BehaviorAddr<B> as Address>::Nonce) -> TimerId,
) -> Result<
    BackoffSuperviseWithParent<B, C, ParentPath>,
    FleetError<<crate::BehaviorAddr<B> as Address>::Nonce>,
>
where
    B: Behavior<Birth = behavior::Births<C>>,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: Protocol<Addr = crate::BehaviorAddr<B>>,
{
    supervise(inner, topology, restart)
        .map(|owned| BackoffSuperviseWithParent::new(owned, backoff, timer))
}

/// Construct a dynamic stable-child owner with its final report path inferred.
#[must_use]
pub fn dynamic_supervisor<A, C, Route, ParentPath>()
-> DynamicSupervisorWithParent<A, C, Route, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: Protocol<Addr = A>,
    Route: DeliveryRouteProtocol,
    Route::Protocol: Protocol<Addr = A, Msg = DynamicSupervisorOutcome<A, C>>,
{
    DynamicSupervisorWithParent::with_parent(ProxyParentIngress::new())
}

/// Construct a FIFO worker pool with its final proxy-report path inferred.
pub fn worker_pool<A, D, J, R, C, Route, ParentPath>(
    topology: ChildTopology<A::Nonce, C>,
    configuration: PoolConfiguration,
    complete_to: Recipient<WorkerPoolProtocol<A, D, J, R, Route>>,
) -> Result<WorkerPoolWithParent<A, D, J, R, C, Route, ParentPath>, PoolConfigError<A::Nonce>>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: Protocol<Addr = A, Msg = PoolAssignment<WorkerPoolProtocol<A, D, J, R, Route>>>,
{
    WorkerPoolWithParent::with_parent(
        topology,
        configuration,
        complete_to,
        ProxyParentIngress::new(),
    )
}

/// Construct a keyed worker pool with its final proxy-report path inferred.
pub fn keyed_worker_pool<A, D, K, J, R, C, Route, Select, ParentPath>(
    topology: ChildTopology<A::Nonce, C>,
    configuration: PoolConfiguration,
    select: Select,
    complete_to: Recipient<KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>,
) -> Result<
    KeyedWorkerPoolWithParent<A, D, K, J, R, C, Route, Select, ParentPath>,
    PoolConfigError<A::Nonce>,
>
where
    A: Address,
    A::Nonce: From<u64>,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D> + Clone,
    K: Eq,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol:
        Protocol<Addr = A, Msg = PoolAssignment<KeyedWorkerPoolProtocol<A, D, K, J, R, Route>>>,
    Select: AffinitySelector<K, A::Nonce>,
{
    KeyedWorkerPoolWithParent::with_parent(
        topology,
        configuration,
        select,
        complete_to,
        ProxyParentIngress::new(),
    )
}

/// Construct fixed workers that accept their application command directly.
///
/// The final actor composition infers the path used by stable proxies to
/// return lifecycle facts. `select` is the only routing policy.
///
/// # Errors
/// Returns the exact fixed-topology rejection.
pub fn supervised<A, C, Select, ParentPath>(
    topology: ChildTopology<A::Nonce, C>,
    restart: RestartConfiguration,
    select: Select,
) -> Result<SupervisedWorkersWithParent<A, C, Select, ParentPath>, FleetError<A::Nonce>>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Select: Fn(&<C::Protocol as crate::Protocol>::Msg) -> A::Nonce,
{
    SupervisedWorkersWithParent::with_parent(topology, restart, select, ProxyParentIngress::new())
}

/// Construct fixed supervised workers with generation-safe restart delays.
///
/// The returned actor accepts the worker's application message directly.
/// `select` chooses one configured stable worker nonce for every message; no
/// default routing policy is inferred. Successful messages travel through the
/// supervisor-owned stable proxy route, while proxy addresses and commands
/// remain private to the composition. Restart delay affects replacement
/// commands only.
///
/// This stable-route construction and its routing failures are derived Bombay
/// policy, not actor-model laws.
///
/// # Errors
///
/// Returns the exact [`FleetError`] produced while configuring the fixed
/// supplied topology cannot establish a fresh, unambiguous fixed fleet.
pub fn supervised_backoff<A, C, Select, ParentPath>(
    topology: ChildTopology<A::Nonce, C>,
    restart: RestartConfiguration,
    backoff: Backoff,
    select: Select,
) -> Result<BackoffWorkersWithParent<A, C, Select, ParentPath>, FleetError<A::Nonce>>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = behavior::Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    Select: Fn(&<C::Protocol as crate::Protocol>::Msg) -> A::Nonce,
{
    supervised(topology, restart, select)
        .map(|workers| BackoffWorkersWithParent::new(workers, backoff))
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use behavior::{Actions, Here, MailAddr, Never, NoBirths, User};

    use super::*;
    use crate::{
        Activate as _, BackoffWorkers, Crash, CreationKind, CreationResolved, RestartPolicy,
        Strategy, TimerElapsed, TimerGeneration, TimerId, WorkerCreationResolved, WorkerStopped,
    };

    struct Child;

    impl behavior::Protocol for Child {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Child {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }

    fn restart() -> RestartConfiguration {
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            2,
            Duration::from_secs(30),
        )
    }

    fn select(_: &()) -> u64 {
        1
    }

    fn policy() -> Backoff {
        Backoff::linear(Duration::from_secs(3), Duration::from_secs(9)).unwrap()
    }

    type Subject = BackoffWorkers<MailAddr, Child, fn(&()) -> u64>;

    #[derive(Debug, PartialEq, Eq)]
    struct PaymentCommand {
        payment_id: u64,
        amount: u64,
    }

    struct PaymentWorker;

    impl behavior::Protocol for PaymentWorker {
        type Addr = MailAddr;
        type Msg = PaymentCommand;
    }

    impl Behavior for PaymentWorker {
        type Protocol = Self;
        type Event = User<MailAddr, PaymentCommand>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> crate::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn payment_worker(_: usize) -> Option<PaymentWorker> {
        Some(PaymentWorker)
    }

    fn payment_slot(command: &PaymentCommand) -> u64 {
        command.payment_id
    }

    type Payments = BackoffWorkers<MailAddr, PaymentWorker, fn(&PaymentCommand) -> u64>;

    fn assert_command_not_accepted(
        result: crate::BehaviorActed<Payments>,
        worker: u64,
        reason: crate::WorkerUnavailable,
        from: MailAddr,
        command: PaymentCommand,
    ) {
        let Err(crate::BackoffSupervisorError::Supervision(
            crate::SupervisedWorkersError::CommandNotAccepted {
                worker: observed_worker,
                reason: observed_reason,
                command: observed_command,
            },
        )) = result
        else {
            panic!("command was not returned through the typed admission failure");
        };
        assert_eq!(observed_worker, worker);
        assert_eq!(observed_reason, reason);
        assert_eq!(observed_command, User::new(from, command));
    }

    #[test]
    fn exact_type_and_topology_error_are_preserved() {
        fn exact(_: Subject) {}

        exact(
            supervised_backoff::<_, _, _, Here>(
                ChildTopology::new([1], child),
                restart(),
                policy(),
                select as fn(&()) -> u64,
            )
            .unwrap(),
        );
        assert!(matches!(
            supervised_backoff::<_, _, _, Here>(
                ChildTopology::new([7, 7], child),
                restart(),
                policy(),
                select,
            ),
            Err(FleetError::DuplicateChild(7))
        ));
    }

    #[test]
    fn application_commands_keep_the_selected_stable_worker_across_replacement() {
        fn public_protocol<P: behavior::Protocol<Addr = MailAddr, Msg = PaymentCommand>>() {}
        public_protocol::<<Payments as Behavior>::Protocol>();

        let mut payments = supervised_backoff(
            ChildTopology::new([7], payment_worker),
            restart(),
            policy(),
            payment_slot as fn(&PaymentCommand) -> u64,
        )
        .unwrap()
        .initialize()
        .unwrap()
        .behavior;

        let starting = payments.receive(
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 10,
            },
        );
        assert_command_not_accepted(
            starting,
            7,
            crate::WorkerUnavailable::Starting,
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 10,
            },
        );

        payments
            .on_path(CreationResolved::birth(7, MailAddr(70)))
            .unwrap();
        payments
            .on_path(WorkerCreationResolved::new(
                7,
                700,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        let before = payments
            .receive(
                MailAddr(90),
                PaymentCommand {
                    payment_id: 7,
                    amount: 11,
                },
            )
            .unwrap();
        assert_eq!(before.sends.supervision.worker_commands.len(), 1);
        let delivery = &before.sends.supervision.worker_commands[0];
        assert_eq!(delivery.nonce, 7);
        assert!(matches!(
            &delivery.message,
            crate::ProxyCommand::Forward(PaymentCommand {
                payment_id: 7,
                amount: 11
            })
        ));

        let failed_at = Instant::now();
        payments
            .on_path(WorkerStopped::new(7, 700, Err(Crash::Panicked), failed_at))
            .unwrap();
        let restarting = payments.receive(
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 12,
            },
        );
        assert_command_not_accepted(
            restarting,
            7,
            crate::WorkerUnavailable::Restarting,
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 12,
            },
        );
        payments
            .on_path(TimerElapsed::new(TimerId(0), TimerGeneration(0)))
            .unwrap();
        payments
            .on_path(WorkerCreationResolved::new(
                7,
                701,
                CreationKind::replacement_of(700),
                Ok(()),
            ))
            .unwrap();
        let after = payments
            .receive(
                MailAddr(90),
                PaymentCommand {
                    payment_id: 7,
                    amount: 13,
                },
            )
            .unwrap();
        assert_eq!(after.sends.supervision.worker_commands[0].nonce, 7);

        let unknown = payments.receive(
            MailAddr(90),
            PaymentCommand {
                payment_id: 99,
                amount: 14,
            },
        );
        assert_command_not_accepted(
            unknown,
            99,
            crate::WorkerUnavailable::Unknown,
            MailAddr(90),
            PaymentCommand {
                payment_id: 99,
                amount: 14,
            },
        );

        payments.on_path(crate::ShutdownRequested).unwrap();
        let shutting_down = payments.receive(
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 15,
            },
        );
        assert_command_not_accepted(
            shutting_down,
            7,
            crate::WorkerUnavailable::ShuttingDown,
            MailAddr(90),
            PaymentCommand {
                payment_id: 7,
                amount: 15,
            },
        );
    }

    #[test]
    fn stable_proxy_never_silently_consumes_a_command_during_replacement() {
        let initialized = crate::Proxy::new(PaymentWorker).initialize().unwrap();
        let mut proxy = initialized.behavior;
        proxy
            .on_path(CreationResolved::birth(0, MailAddr(70)))
            .unwrap();

        proxy
            .receive(MailAddr(1), crate::ProxyCommand::Replace(PaymentWorker))
            .unwrap();
        let still_running = proxy
            .receive(
                MailAddr(1),
                crate::ProxyCommand::Forward(PaymentCommand {
                    payment_id: 7,
                    amount: 20,
                }),
            )
            .unwrap();
        assert_eq!(still_running.sends.deliveries[0].nonce, 0);

        proxy
            .on_path(crate::ChildStopped::new(
                0,
                Ok(crate::Exit::Normal),
                Instant::now(),
            ))
            .unwrap();
        let rejected = proxy.receive(
            MailAddr(1),
            crate::ProxyCommand::Forward(PaymentCommand {
                payment_id: 7,
                amount: 21,
            }),
        );
        assert!(matches!(
            rejected,
            Err(crate::ProxyError::CommandNotAccepted {
                phase: crate::IncarnationPhase::Installing {
                    attempt: 1,
                    kind: CreationKind::ReplacementIncarnation { replaces: 0 },
                },
                command: PaymentCommand {
                    payment_id: 7,
                    amount: 21,
                },
            })
        ));

        proxy
            .on_path(CreationResolved::replacement_incarnation(
                1,
                0,
                MailAddr(71),
            ))
            .unwrap();
        let replaced = proxy
            .receive(
                MailAddr(1),
                crate::ProxyCommand::Forward(PaymentCommand {
                    payment_id: 7,
                    amount: 22,
                }),
            )
            .unwrap();
        assert_eq!(replaced.sends.deliveries[0].nonce, 1);
    }
}
