//! Cross-adapter proofs for the shared fixed-fleet ownership fold.

use std::time::{Duration, Instant};

use behavior::{
    Acted, Actions, Activate as _, Backoff, Behavior, BehaviorActed, BehaviorBase, Births,
    ChildStopped, ChildTopology, Crash, Create, CreationKind, CreationRejection, CreationResolved,
    EventIngress, Exit, Here, MailAddr, Never, RestartConfiguration, RestartPolicy,
    ShutdownRequested, Step, Strategy, Supervise, SuperviseError, SupervisionEvent,
    SupervisionLifecycle, Supervisor, TimerElapsed, User, UserEvent, WorkerCreationResolved,
    WorkerStopped,
};

struct Child;

#[behavior::behavior(addr = MailAddr, message = (), sends = Vec<Never>, births = behavior::NoBirths, error = Never)]
impl Child {
    fn receive(
        &mut self,
        _from: MailAddr,
        _message: (),
    ) -> Acted<MailAddr, Never, Vec<Never>, behavior::NoBirths, Never> {
        Ok(Actions::cont())
    }
}

/// A real application composition: user input stages an additional child.
#[derive(Default)]
struct Application {
    lifecycle: Vec<SupervisionLifecycle<MailAddr>>,
}

enum ApplicationEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    User(User<MailAddr, u64>),
}

impl UserEvent for ApplicationEvent {
    type Addr = MailAddr;
    type Message = u64;

    fn user(from: MailAddr, message: u64) -> Self {
        Self::User(User::new(from, message))
    }

    fn into_user(self) -> Result<User<MailAddr, u64>, Self> {
        match self {
            Self::User(event) => Ok(event),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for ApplicationEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl foundation::Protocol for Application {
    type Addr = MailAddr;
    type Msg = u64;
}

impl BehaviorBase for Application {
    type Base = Self;
    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for Application {
    type Protocol = Self;
    type Event = ApplicationEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<Child>;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            ApplicationEvent::Lifecycle(lifecycle) => {
                self.lifecycle.push(lifecycle);
                Ok(Actions::cont())
            }
            ApplicationEvent::User(event) => {
                Ok(Actions::create(vec![Create::birth(event.message, Child)]))
            }
        }
    }
}

struct RejectingApplication {
    reject: fn(&SupervisionLifecycle<MailAddr>) -> bool,
    accepted: Vec<SupervisionLifecycle<MailAddr>>,
}

enum RejectingEvent {
    Lifecycle(SupervisionLifecycle<MailAddr>),
    #[allow(
        dead_code,
        reason = "the Never message makes this required UserEvent lane uninhabited"
    )]
    User(User<MailAddr, Never>),
}

impl UserEvent for RejectingEvent {
    type Addr = MailAddr;
    type Message = Never;

    fn user(_: MailAddr, message: Never) -> Self {
        match message {}
    }

    fn into_user(self) -> Result<User<MailAddr, Never>, Self> {
        match self {
            Self::User(event) => Ok(event),
            lifecycle => Err(lifecycle),
        }
    }
}

impl EventIngress<Here, SupervisionLifecycle<MailAddr>> for RejectingEvent {
    fn ingress(lifecycle: SupervisionLifecycle<MailAddr>) -> Self {
        Self::Lifecycle(lifecycle)
    }
}

impl foundation::Protocol for RejectingApplication {
    type Addr = MailAddr;
    type Msg = Never;
}

impl BehaviorBase for RejectingApplication {
    type Base = Self;

    fn base(&self) -> &Self::Base {
        self
    }
}

impl Behavior for RejectingApplication {
    type Protocol = Self;
    type Event = RejectingEvent;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = SupervisionLifecycle<MailAddr>;
    type Birth = behavior::NoBirths;

    fn transition(&mut self, _: foundation::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            RejectingEvent::Lifecycle(lifecycle) if (self.reject)(&lifecycle) => Err(lifecycle),
            RejectingEvent::Lifecycle(lifecycle) => {
                self.accepted.push(lifecycle);
                Ok(Actions::cont())
            }
            RejectingEvent::User(event) => match event.message {},
        }
    }
}

fn child(_: usize) -> Option<Child> {
    Some(Child)
}

fn stop_standalone_on_failure(_: &behavior::SupervisionFailure<MailAddr>) -> behavior::Become {
    Step::Stop(behavior::Stopped)
}

fn restart() -> RestartConfiguration {
    RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        4,
        Duration::from_secs(10),
        behavior::RestartTiming::Immediate,
    )
}

macro_rules! assert_quiet_application_owner {
    ($actions:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.owned.child_observations.is_empty());
        assert!(actions.sends.owned.creation_observations.is_empty());
        assert!(actions.sends.owned.schedules.is_empty());
        assert!(actions.sends.owned.replacement_inputs.is_empty());
        assert!(actions.sends.owned.failure_reports.is_empty());
        assert!(actions.sends.owned.shutdowns.is_empty());
        assert!(actions.sends.inner.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }};
}

macro_rules! assert_quiet_standalone_owner {
    ($actions:expr) => {{
        let actions = &$actions;
        assert!(actions.sends.child_observations.is_empty());
        assert!(actions.sends.creation_observations.is_empty());
        assert!(actions.sends.schedules.is_empty());
        assert!(actions.sends.replacement_inputs.is_empty());
        assert!(actions.sends.failure_reports.is_empty());
        assert!(actions.sends.shutdowns.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
    }};
}

fn one() -> Supervise<Application, Child, fn(Child) -> behavior::Proxy<Child>> {
    Supervise::new(
        Application::default(),
        ChildTopology::new([3], child),
        restart(),
        behavior::Proxy::new as fn(Child) -> behavior::Proxy<Child>,
    )
    .unwrap()
}

fn fleet(
    configuration: RestartConfiguration,
) -> Supervise<Application, Child, fn(Child) -> behavior::Proxy<Child>> {
    Supervise::new(
        Application::default(),
        ChildTopology::new([3, 5], child),
        configuration,
        behavior::Proxy::new as fn(Child) -> behavior::Proxy<Child>,
    )
    .unwrap()
}

#[test]
fn behavior_layer_owns_one_fixed_fleet_trace_without_a_parallel_supervisor_actor() {
    let composed = Application::default().layer(|inner| {
        Supervise::new(
            inner,
            ChildTopology::new([3, 5], child),
            restart(),
            behavior::Proxy::new,
        )
        .unwrap()
    });
    let composed = composed.initialize().unwrap();

    assert_eq!(
        composed
            .actions
            .creates
            .iter()
            .map(|create| create.nonce)
            .collect::<Vec<_>>(),
        [3, 5],
    );
    assert_eq!(composed.actions.sends.owned.child_observations.len(), 2);

    let mut composed = composed.behavior;
    for nonce in [3, 5] {
        let fact = CreationResolved::birth(nonce, MailAddr(100 + nonce));
        let committed = composed.on(fact).unwrap();
        assert_quiet_application_owner!(committed);
    }
    let stopped = WorkerStopped::new(3, 103, Err(Crash::Failed), Instant::now());
    let composed_actions = composed.on(stopped).unwrap();

    assert_eq!(
        composed_actions
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|request| request.nonce)
            .collect::<Vec<_>>(),
        [3],
    );
    assert_eq!(composed.child_count(), 2);
    assert_eq!(composed.restarts_in_window(), 1);
}

#[test]
fn composed_application_birth_remains_outside_the_fixed_ownership_occurrence() {
    let initialized = Application::default()
        .layer(|inner| {
            Supervise::new(
                inner,
                ChildTopology::new([3], child),
                restart(),
                behavior::Proxy::new,
            )
            .unwrap()
        })
        .initialize()
        .unwrap();
    let mut active = initialized.behavior;
    let actions = active
        .transition(SupervisionEvent::Behavior(ApplicationEvent::User(
            User::new(MailAddr(1), 11),
        )))
        .unwrap();
    assert_eq!(actions.creates.len(), 1);
    assert_eq!(actions.creates[0].nonce, 11);
    assert!(actions.sends.owned.child_observations.is_empty());
    assert!(actions.sends.owned.creation_observations.is_empty());
    assert_eq!(active.child_count(), 1);
}

#[test]
fn stable_and_worker_creation_facts_join_in_both_orders() {
    for worker_first in [false, true] {
        let initialized = one().initialize().unwrap();
        let mut active = initialized.behavior;
        assert!(!active.is_established(3).unwrap());
        assert!(active.is_restartable(3).unwrap());

        let stable = CreationResolved::birth(3, MailAddr(30));
        let worker = WorkerCreationResolved::new(3, 7, CreationKind::Birth, Ok(()));
        if worker_first {
            let retained = active.on(worker).unwrap();
            assert_quiet_application_owner!(retained);
            assert!(!active.is_established(3).unwrap());
            let joined = active.on(stable).unwrap();
            assert_quiet_application_owner!(joined);
        } else {
            let committed = active.on(stable).unwrap();
            assert_quiet_application_owner!(committed);
            assert!(active.is_established(3).unwrap());
            let joined = active.on(worker).unwrap();
            assert_quiet_application_owner!(joined);
        }

        assert!(active.is_established(3).unwrap());
        assert_eq!(active.base().lifecycle.len(), 1);
        assert!(matches!(
            active.base().lifecycle[0],
            SupervisionLifecycle::Ready {
                proxy: 3,
                worker: 7,
                kind: CreationKind::Birth,
            }
        ));
        assert!(matches!(
            active.on(worker),
            Err(SuperviseError::UnexpectedWorkerCreation(returned)) if returned == worker
        ));
    }
}

#[test]
fn rejected_proxy_creation_cannot_discard_an_authoritative_worker_fact() {
    let initialized = one().initialize().unwrap();
    let mut active = initialized.behavior;
    let worker = WorkerCreationResolved::new(3, 7, CreationKind::Birth, Ok(()));
    let retained = active.on(worker).unwrap();
    assert_quiet_application_owner!(retained);
    let rejected =
        CreationResolved::rejected(3, CreationKind::Birth, CreationRejection::EnvironmentFailed);

    assert!(matches!(
        active.on(rejected),
        Err(SuperviseError::ContradictoryStableAndWorkerCreation {
            proxy,
            worker: returned,
        }) if proxy == rejected && returned == worker
    ));
    assert!(active.is_restartable(3).unwrap());
}

#[test]
fn rejected_proxy_creation_cannot_discard_an_authoritative_worker_stop() {
    let initialized = one().initialize().unwrap();
    let mut active = initialized.behavior;
    let stopped = WorkerStopped::new(3, 7, Err(Crash::Failed), Instant::now());
    let retained = active.on(stopped.clone()).unwrap();
    assert_eq!(
        retained
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>(),
        [3]
    );
    assert!(retained.sends.owned.child_observations.is_empty());
    assert!(retained.sends.owned.creation_observations.is_empty());
    assert!(retained.sends.owned.schedules.is_empty());
    assert!(retained.sends.owned.failure_reports.is_empty());
    assert!(retained.sends.owned.shutdowns.is_empty());
    assert!(retained.sends.inner.is_empty());
    assert!(retained.creates.is_empty());
    assert!(matches!(retained.become_, Step::Continue));
    let rejected =
        CreationResolved::rejected(3, CreationKind::Birth, CreationRejection::EnvironmentFailed);

    assert!(matches!(
        active.on(rejected),
        Err(SuperviseError::ContradictoryStableCreationAndWorkerStop {
            proxy,
            worker: returned,
        }) if proxy == rejected && returned == stopped
    ));
}

#[test]
fn stable_stop_is_consumed_once_and_makes_establishment_false() {
    let initialized = one().initialize().unwrap();
    let mut active = initialized.behavior;
    let committed = active.on(CreationResolved::birth(3, MailAddr(30))).unwrap();
    assert_quiet_application_owner!(committed);
    let stopped = ChildStopped::new(3, Ok(Exit::Normal), Instant::now());
    let retired = active.on(stopped.clone()).unwrap();
    assert!(retired.sends.owned.child_observations.is_empty());
    assert!(retired.sends.owned.creation_observations.is_empty());
    assert!(retired.sends.owned.schedules.is_empty());
    assert!(retired.sends.owned.replacement_inputs.is_empty());
    assert_eq!(
        retired.sends.owned.failure_reports.as_slice()[0].failure,
        behavior::SupervisionFailure::stable_child_stopped(3, Ok(Exit::Normal))
    );
    assert_eq!(retired.sends.owned.failure_reports.len(), 1);
    assert!(retired.sends.owned.shutdowns.is_empty());
    assert!(retired.sends.inner.is_empty());
    assert!(retired.creates.is_empty());
    assert!(matches!(retired.become_, Step::Continue));

    assert!(!active.is_established(3).unwrap());
    assert!(!active.is_restartable(3).unwrap());
    assert!(matches!(
        active.on(stopped.clone()),
        Err(SuperviseError::UnexpectedChildStopped(returned)) if returned == stopped
    ));
}

#[test]
fn one_for_all_retains_an_initially_unresolved_member_until_it_is_ready() {
    let mut active = fleet(RestartConfiguration::new(
        Strategy::OneForAll,
        RestartPolicy::Permanent,
        8,
        Duration::MAX,
        behavior::RestartTiming::Immediate,
    ))
    .initialize()
    .unwrap()
    .behavior;
    let joined = active
        .on(WorkerCreationResolved::new(
            3,
            30,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(joined);

    let stopped = active
        .on(WorkerStopped::new(
            3,
            30,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap();
    assert_eq!(
        stopped
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>(),
        [3]
    );

    let became_ready = active
        .on(WorkerCreationResolved::new(
            5,
            50,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_eq!(
        became_ready
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>(),
        [5]
    );
}

#[test]
fn rejected_initial_member_is_not_fabricated_as_a_replacement_birth() {
    let mut active = fleet(RestartConfiguration::new(
        Strategy::OneForAll,
        RestartPolicy::Permanent,
        8,
        Duration::MAX,
        behavior::RestartTiming::Immediate,
    ))
    .initialize()
    .unwrap()
    .behavior;
    let joined = active
        .on(WorkerCreationResolved::new(
            3,
            30,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(joined);
    let stopped = active
        .on(WorkerStopped::new(
            3,
            30,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap();
    assert_eq!(stopped.sends.owned.replacement_inputs.len(), 1);
    assert!(stopped.sends.owned.child_observations.is_empty());
    assert!(stopped.sends.owned.creation_observations.is_empty());
    assert!(stopped.sends.owned.schedules.is_empty());
    assert!(stopped.sends.owned.failure_reports.is_empty());
    assert!(stopped.sends.owned.shutdowns.is_empty());
    assert!(stopped.sends.inner.is_empty());
    assert!(stopped.creates.is_empty());
    assert!(matches!(stopped.become_, Step::Continue));

    let rejected = active
        .on(WorkerCreationResolved::new(
            5,
            50,
            CreationKind::Birth,
            Err(CreationRejection::EnvironmentFailed),
        ))
        .unwrap();
    assert!(rejected.sends.owned.replacement_inputs.is_empty());
    assert_eq!(rejected.sends.owned.failure_reports.len(), 1);
    assert!(!active.is_restartable(5).unwrap());
}

#[test]
fn delayed_replacement_joins_timer_and_readiness_in_both_orders_exactly_once() {
    for timer_first in [false, true] {
        let mut active = fleet(RestartConfiguration::new(
            Strategy::OneForAll,
            RestartPolicy::Permanent,
            8,
            Duration::MAX,
            behavior::RestartTiming::Delayed(Backoff::constant(Duration::from_millis(4)).unwrap()),
        ))
        .initialize()
        .unwrap()
        .behavior;
        let joined = active
            .on(WorkerCreationResolved::new(
                3,
                30,
                CreationKind::Birth,
                Ok(()),
            ))
            .unwrap();
        assert_quiet_application_owner!(joined);
        let accepted = active
            .on(WorkerStopped::new(
                3,
                30,
                Err(Crash::Failed),
                Instant::now(),
            ))
            .unwrap();
        assert!(accepted.sends.owned.replacement_inputs.is_empty());
        let schedule = accepted.sends.owned.schedules.as_slice()[0];
        let elapsed = TimerElapsed::new(schedule.id, schedule.generation);

        let (timer_actions, ready_actions) = if timer_first {
            let timer_actions = active.on(elapsed).unwrap();
            let ready_actions = active
                .on(WorkerCreationResolved::new(
                    5,
                    50,
                    CreationKind::Birth,
                    Ok(()),
                ))
                .unwrap();
            (timer_actions, ready_actions)
        } else {
            let ready_actions = active
                .on(WorkerCreationResolved::new(
                    5,
                    50,
                    CreationKind::Birth,
                    Ok(()),
                ))
                .unwrap();
            let timer_actions = active.on(elapsed).unwrap();
            (timer_actions, ready_actions)
        };
        let timer_routes = timer_actions
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>();
        let ready_routes = ready_actions
            .sends
            .owned
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>();
        if timer_first {
            assert_eq!(timer_routes, [3]);
            assert_eq!(ready_routes, [5]);
        } else {
            assert_eq!(timer_routes, [3, 5]);
            assert!(ready_routes.is_empty());
        }
        assert!(
            active
                .on(elapsed)
                .unwrap()
                .sends
                .owned
                .replacement_inputs
                .is_empty()
        );
    }
}

#[test]
fn standalone_and_application_owners_share_the_exact_delayed_replacement_law() {
    let configuration = RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        8,
        Duration::MAX,
        behavior::RestartTiming::Delayed(Backoff::constant(Duration::from_millis(4)).unwrap()),
    );
    let mut application = Supervise::new(
        Application::default(),
        ChildTopology::new([3], child),
        configuration,
        behavior::Proxy::new,
    )
    .unwrap()
    .initialize()
    .unwrap()
    .behavior;
    let mut standalone = Supervisor::new(
        ChildTopology::new([3], child),
        configuration,
        behavior::Proxy::new,
    )
    .unwrap()
    .initialize()
    .unwrap()
    .behavior;

    let stable = CreationResolved::birth(3, MailAddr(30));
    let worker = WorkerCreationResolved::new(3, 30, CreationKind::Birth, Ok(()));
    let application_committed = application.on(stable).unwrap();
    assert_quiet_application_owner!(application_committed);
    let application_joined = application.on(worker).unwrap();
    assert_quiet_application_owner!(application_joined);
    let standalone_committed = standalone.on(stable).unwrap();
    assert_quiet_standalone_owner!(standalone_committed);
    let standalone_joined = standalone.on(worker).unwrap();
    assert_quiet_standalone_owner!(standalone_joined);

    let stopped = WorkerStopped::new(3, 30, Err(Crash::Failed), Instant::now());
    let application_stop = application.on(stopped.clone()).unwrap().sends.owned;
    let standalone_stop = standalone.on(stopped).unwrap().sends;
    assert_eq!(
        application_stop.schedules.as_slice(),
        standalone_stop.schedules.as_slice()
    );
    assert_eq!(application_stop.schedules.len(), 1);
    assert!(application_stop.replacement_inputs.is_empty());
    assert!(standalone_stop.replacement_inputs.is_empty());
    assert!(application_stop.failure_reports.is_empty());
    assert!(standalone_stop.failure_reports.is_empty());

    let schedule = application_stop.schedules.as_slice()[0];
    let elapsed = TimerElapsed::new(schedule.id, schedule.generation);
    let application_release = application.on(elapsed).unwrap().sends.owned;
    let standalone_release = standalone.on(elapsed).unwrap().sends;
    assert_eq!(
        application_release
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>(),
        [3]
    );
    assert_eq!(
        standalone_release
            .replacement_inputs
            .iter()
            .map(|input| input.nonce)
            .collect::<Vec<_>>(),
        [3]
    );
    assert!(application_release.schedules.is_empty());
    assert!(standalone_release.schedules.is_empty());

    let application_duplicate = application.on(elapsed).unwrap().sends.owned;
    let standalone_duplicate = standalone.on(elapsed).unwrap().sends;
    assert!(application_duplicate.replacement_inputs.is_empty());
    assert!(standalone_duplicate.replacement_inputs.is_empty());
    assert_eq!(application.pending_restarts(), 0);
    assert_eq!(standalone.pending_restarts(), 0);
}

#[test]
fn standalone_failure_reaction_can_select_the_advertised_terminal_result() {
    let mut supervisor = Supervisor::new(
        ChildTopology::new([3], child),
        RestartConfiguration::new(
            Strategy::OneForOne,
            RestartPolicy::Permanent,
            0,
            Duration::MAX,
            behavior::RestartTiming::Immediate,
        ),
        behavior::Proxy::new,
    )
    .unwrap()
    .with_failure_reaction(stop_standalone_on_failure)
    .initialize()
    .unwrap()
    .behavior;
    let joined = supervisor
        .on(WorkerCreationResolved::new(
            3,
            30,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_standalone_owner!(joined);

    let actions = supervisor
        .on(WorkerStopped::new(
            3,
            30,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap();

    assert!(actions.sends.replacement_inputs.is_empty());
    assert_eq!(actions.sends.failure_reports.len(), 1);
    assert!(matches!(actions.become_, Step::Stop(behavior::Stopped)));
}

#[test]
fn shutdown_cancels_a_retained_delayed_batch() {
    let mut active = fleet(RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        8,
        Duration::MAX,
        behavior::RestartTiming::Delayed(Backoff::constant(Duration::from_millis(4)).unwrap()),
    ))
    .initialize()
    .unwrap()
    .behavior;
    let joined = active
        .on(WorkerCreationResolved::new(
            3,
            30,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(joined);
    let accepted = active
        .on(WorkerStopped::new(
            3,
            30,
            Err(Crash::Failed),
            Instant::now(),
        ))
        .unwrap();
    let schedule = accepted.sends.owned.schedules.as_slice()[0];
    let shutdown = active.on(ShutdownRequested).unwrap();
    assert!(shutdown.sends.owned.child_observations.is_empty());
    assert!(shutdown.sends.owned.creation_observations.is_empty());
    assert!(shutdown.sends.owned.schedules.is_empty());
    assert!(shutdown.sends.owned.replacement_inputs.is_empty());
    assert!(shutdown.sends.owned.failure_reports.is_empty());
    assert!(shutdown.sends.owned.shutdowns.is_empty());
    assert!(shutdown.sends.inner.is_empty());
    assert!(shutdown.creates.is_empty());
    assert!(matches!(shutdown.become_, Step::Continue));
    let stale = active
        .on(TimerElapsed::new(schedule.id, schedule.generation))
        .unwrap();
    assert!(stale.sends.owned.replacement_inputs.is_empty());
    assert_eq!(active.pending_restarts(), 0);
}

#[test]
fn application_observes_replacement_start_then_exact_replacement_readiness() {
    let mut active = one().initialize().unwrap().behavior;
    let proxy_ready = active.on(CreationResolved::birth(3, MailAddr(30))).unwrap();
    assert_quiet_application_owner!(proxy_ready);
    let initially_ready = active
        .on(WorkerCreationResolved::new(
            3,
            7,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(initially_ready);
    let stopped = WorkerStopped::new(3, 7, Err(Crash::Failed), Instant::now());
    let replacement_started = active.on(stopped.clone()).unwrap();
    assert_eq!(replacement_started.sends.owned.replacement_inputs.len(), 1);
    assert!(
        replacement_started
            .sends
            .owned
            .child_observations
            .is_empty()
    );
    assert!(
        replacement_started
            .sends
            .owned
            .creation_observations
            .is_empty()
    );
    assert!(replacement_started.sends.owned.schedules.is_empty());
    assert!(replacement_started.sends.owned.failure_reports.is_empty());
    assert!(replacement_started.sends.owned.shutdowns.is_empty());
    assert!(replacement_started.sends.inner.is_empty());
    assert!(replacement_started.creates.is_empty());
    assert!(matches!(replacement_started.become_, Step::Continue));
    let replacement_ready = active
        .on(WorkerCreationResolved::new(
            3,
            8,
            CreationKind::ReplacementIncarnation { replaces: 7 },
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(replacement_ready);

    assert_eq!(active.base().lifecycle.len(), 3);
    assert!(matches!(
        &active.base().lifecycle[1],
        SupervisionLifecycle::ReplacementStarted {
            trigger,
            replacing,
            awaiting_initial,
        } if trigger == &stopped && replacing.is_empty() && awaiting_initial.is_empty()
    ));
    assert!(matches!(
        active.base().lifecycle[2],
        SupervisionLifecycle::Ready {
            proxy: 3,
            worker: 8,
            kind: CreationKind::ReplacementIncarnation { replaces: 7 },
        }
    ));
}

#[test]
fn application_observes_permanent_retirement_when_restart_is_denied() {
    let mut active = fleet(RestartConfiguration::new(
        Strategy::OneForOne,
        RestartPolicy::Permanent,
        0,
        Duration::MAX,
        behavior::RestartTiming::Immediate,
    ))
    .initialize()
    .unwrap()
    .behavior;
    let proxy_ready = active.on(CreationResolved::birth(3, MailAddr(30))).unwrap();
    assert_quiet_application_owner!(proxy_ready);
    let initially_ready = active
        .on(WorkerCreationResolved::new(
            3,
            7,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert_quiet_application_owner!(initially_ready);
    let stopped = WorkerStopped::new(3, 7, Err(Crash::Failed), Instant::now());
    let retired = active.on(stopped.clone()).unwrap();
    assert!(retired.sends.owned.child_observations.is_empty());
    assert!(retired.sends.owned.creation_observations.is_empty());
    assert!(retired.sends.owned.schedules.is_empty());
    assert!(retired.sends.owned.replacement_inputs.is_empty());
    assert_eq!(retired.sends.owned.failure_reports.len(), 1);
    assert!(retired.sends.owned.shutdowns.is_empty());
    assert!(retired.sends.inner.is_empty());
    assert!(retired.creates.is_empty());
    assert!(matches!(retired.become_, Step::Continue));

    assert_eq!(active.base().lifecycle.len(), 2);
    assert!(matches!(
        active.base().lifecycle[1],
        SupervisionLifecycle::Retired {
            failure: behavior::SupervisionFailure::RestartDenied { child: 3, .. },
        }
    ));
    assert!(!active.is_restartable(3).unwrap());
}

#[test]
fn application_observes_shutdown_once_while_installation_is_pending() {
    let mut active = one().initialize().unwrap().behavior;
    let shutdown = active.on(ShutdownRequested).unwrap();
    assert_quiet_application_owner!(shutdown);
    let duplicate = active.on(ShutdownRequested).unwrap();
    assert_quiet_application_owner!(duplicate);

    assert_eq!(active.base().lifecycle.len(), 1);
    assert!(matches!(
        active.base().lifecycle[0],
        SupervisionLifecycle::ShuttingDown { ref proxies } if proxies == &[3]
    ));
}

fn rejects_ready(lifecycle: &SupervisionLifecycle<MailAddr>) -> bool {
    matches!(lifecycle, SupervisionLifecycle::Ready { .. })
}

fn rejects_replacement(lifecycle: &SupervisionLifecycle<MailAddr>) -> bool {
    matches!(lifecycle, SupervisionLifecycle::ReplacementStarted { .. })
}

fn rejecting(
    reject: fn(&SupervisionLifecycle<MailAddr>) -> bool,
) -> Supervise<RejectingApplication, Child, fn(Child) -> behavior::Proxy<Child>> {
    Supervise::new(
        RejectingApplication {
            reject,
            accepted: Vec::new(),
        },
        ChildTopology::new([3], child),
        restart(),
        behavior::Proxy::new as fn(Child) -> behavior::Proxy<Child>,
    )
    .unwrap()
}

#[test]
fn rejected_ready_event_does_not_commit_the_join_or_consume_the_worker_fact() {
    let mut active = rejecting(rejects_ready).initialize().unwrap().behavior;
    let proxy = active.on(CreationResolved::birth(3, MailAddr(30))).unwrap();
    assert!(proxy.sends.owned.child_observations.is_empty());
    assert!(proxy.sends.owned.creation_observations.is_empty());
    assert!(proxy.sends.owned.schedules.is_empty());
    assert!(proxy.sends.owned.replacement_inputs.is_empty());
    assert!(proxy.sends.owned.failure_reports.is_empty());
    assert!(proxy.sends.owned.shutdowns.is_empty());
    assert!(proxy.sends.inner.is_empty());
    assert!(proxy.creates.is_empty());
    assert!(matches!(proxy.become_, Step::Continue));
    let worker = WorkerCreationResolved::new(3, 7, CreationKind::Birth, Ok(()));

    for _ in 0..2 {
        assert!(matches!(
            active.on(worker),
            Err(SuperviseError::Behavior(SupervisionLifecycle::Ready {
                proxy: 3,
                worker: 7,
                kind: CreationKind::Birth,
            }))
        ));
        assert_eq!(active.base().accepted.len(), 0);
        assert!(active.is_restartable(3).unwrap());
    }
}

#[test]
fn rejected_replacement_event_does_not_charge_budget_or_commit_unavailability() {
    let mut active = rejecting(rejects_replacement)
        .initialize()
        .unwrap()
        .behavior;
    let proxy = active.on(CreationResolved::birth(3, MailAddr(30))).unwrap();
    assert!(proxy.sends.owned.child_observations.is_empty());
    assert!(proxy.sends.owned.creation_observations.is_empty());
    assert!(proxy.sends.owned.schedules.is_empty());
    assert!(proxy.sends.owned.replacement_inputs.is_empty());
    assert!(proxy.sends.owned.failure_reports.is_empty());
    assert!(proxy.sends.owned.shutdowns.is_empty());
    assert!(proxy.sends.inner.is_empty());
    assert!(proxy.creates.is_empty());
    assert!(matches!(proxy.become_, Step::Continue));
    let ready = active
        .on(WorkerCreationResolved::new(
            3,
            7,
            CreationKind::Birth,
            Ok(()),
        ))
        .unwrap();
    assert!(ready.sends.owned.child_observations.is_empty());
    assert!(ready.sends.owned.creation_observations.is_empty());
    assert!(ready.sends.owned.schedules.is_empty());
    assert!(ready.sends.owned.replacement_inputs.is_empty());
    assert!(ready.sends.owned.failure_reports.is_empty());
    assert!(ready.sends.owned.shutdowns.is_empty());
    assert!(ready.sends.inner.is_empty());
    assert!(ready.creates.is_empty());
    assert!(matches!(ready.become_, Step::Continue));
    let stopped = WorkerStopped::new(3, 7, Err(Crash::Failed), Instant::now());

    for _ in 0..2 {
        assert!(matches!(
            active.on(stopped.clone()),
            Err(SuperviseError::Behavior(
                SupervisionLifecycle::ReplacementStarted { trigger, .. }
            )) if trigger == stopped
        ));
        assert_eq!(active.restarts_in_window(), 0);
        assert!(active.is_restartable(3).unwrap());
    }
}
