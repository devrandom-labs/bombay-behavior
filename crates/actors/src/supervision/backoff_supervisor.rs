//! Timer-delayed supervision as a lossless wrapper over [`Supervise`].

use std::time::Duration;

use super::{
    Backoff, BackoffError, Proxy, Supervise, SuperviseError, Supervisor, SupervisorError,
    SupervisorSends,
};
use crate::{
    ChildShutdownRejected, ChildStopped, CreationResolved, ScheduleAfter, ShutdownRequested,
    SupervisionEvent, TimerElapsed, TimerGeneration, TimerId, WorkerCreationResolved,
    WorkerStopped,
};
use behavior::{
    Actions, Address, Behavior, Births, ComposedEvent, Delivery, Here, InjectEvent,
    InterpreterRequests, Never, SendEffects, SendLayer, Step, User, UserEvent,
};

/// Complete event algebra accepted by [`BackoffSupervise`].
///
/// Timer observations, coordinated shutdown, and supervisor return facts are
/// direct members. Shutdown is therefore accepted at [`Here`], so a backoff
/// supervisor can truthfully be named by a coordinated
/// [`crate::ShutdownChild`] request. Wrapped behavior inputs remain one level
/// inside. This event structure is the exact dual of
/// [`BackoffSupervisorSends`].
#[derive(Clone, PartialEq, Eq)]
pub enum BackoffSupervisorEvent<E: UserEvent> {
    TimerElapsed(TimerElapsed),
    ShutdownRequested(ShutdownRequested),
    Supervision(SupervisionEvent<E>),
}

impl<E: UserEvent> UserEvent for BackoffSupervisorEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Supervision(SupervisionEvent::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Supervision(event) => event.into_user().map_err(Self::Supervision),
            timer => Err(timer),
        }
    }
}

impl<E: UserEvent> ComposedEvent for BackoffSupervisorEvent<E> {
    type Inner = E;

    fn from_inner(event: Self::Inner) -> Self {
        Self::Supervision(SupervisionEvent::Behavior(event))
    }
}

impl<E: UserEvent> InjectEvent<TimerElapsed, Here> for BackoffSupervisorEvent<E> {
    fn inject_at(value: TimerElapsed) -> Self {
        Self::TimerElapsed(value)
    }
}

impl<E: UserEvent> InjectEvent<ShutdownRequested, Here> for BackoffSupervisorEvent<E> {
    fn inject_at(value: ShutdownRequested) -> Self {
        Self::ShutdownRequested(value)
    }
}

macro_rules! supervision_input {
    ($input:ty, $variant:ident) => {
        impl<E: UserEvent> InjectEvent<$input, Here> for BackoffSupervisorEvent<E> {
            fn inject_at(value: $input) -> Self {
                Self::Supervision(SupervisionEvent::$variant(value))
            }
        }
    };
}

supervision_input!(ChildStopped<E::Addr>, ChildStopped);
supervision_input!(WorkerStopped<E::Addr>, WorkerStopped);
supervision_input!(CreationResolved<E::Addr>, CreationResolved);
supervision_input!(
    WorkerCreationResolved<<E::Addr as Address>::Nonce>,
    WorkerCreationResolved
);
supervision_input!(
    ChildShutdownRejected<<E::Addr as Address>::Nonce>,
    ChildShutdownRejected
);

impl<E, Input, Path> InjectEvent<Input, behavior::Inside<Path>> for BackoffSupervisorEvent<E>
where
    E: UserEvent + InjectEvent<Input, Path>,
{
    fn inject_at(input: Input) -> Self {
        Self::Supervision(SupervisionEvent::Behavior(E::inject_at(input)))
    }
}

/// Named effects co-owned by [`BackoffSupervise`].
///
/// Both fields are interpreted at the same structural path, matching the
/// direct timer and supervision members of [`BackoffSupervisorEvent`].
pub struct BackoffSupervisorSends<A, C, ParentPath = Here>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    /// Delayed restart schedules owned by the backoff policy.
    pub schedules: InterpreterRequests<ScheduleAfter>,
    /// Observation, replacement, failure-report, and shutdown lanes owned by
    /// the supervised proxy fleet.
    pub supervision: SupervisorSends<A, C, ParentPath>,
}

impl<A, C, ParentPath> SendEffects for BackoffSupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    fn empty() -> Self {
        Self {
            schedules: InterpreterRequests::empty(),
            supervision: SupervisorSends::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.schedules.append(other.schedules);
        self.supervision.append(other.supervision);
    }
}

impl<Event, A, C, ParentPath> behavior::SendsFor<BackoffSupervisorEvent<Event>>
    for BackoffSupervisorSends<A, C, ParentPath>
where
    A: Address,
    A::Nonce: From<u64>,
    Event: UserEvent<Addr = A>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    InterpreterRequests<ScheduleAfter>: behavior::SendsFor<BackoffSupervisorEvent<Event>>,
    InterpreterRequests<crate::ObserveChild<A>>: behavior::SendsFor<BackoffSupervisorEvent<Event>>,
    InterpreterRequests<crate::ObserveCreation<A>>:
        behavior::SendsFor<BackoffSupervisorEvent<Event>>,
    Vec<Delivery<Proxy<C>>>: behavior::SendsFor<BackoffSupervisorEvent<Event>>,
    InterpreterRequests<crate::ReportSupervisionFailure<A>>:
        behavior::SendsFor<BackoffSupervisorEvent<Event>>,
    InterpreterRequests<crate::ShutdownChild<crate::ProxyWithParent<C, ParentPath>>>:
        behavior::SendsFor<BackoffSupervisorEvent<Event>>,
{
}

impl<I, RootEvent, Path, A, C, ParentPath> behavior::InterpretSends<I, RootEvent, Path>
    for BackoffSupervisorSends<A, C, ParentPath>
where
    I: behavior::SendInterpreter,
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    InterpreterRequests<ScheduleAfter>: behavior::InterpretSends<I, RootEvent, Path>,
    SupervisorSends<A, C, ParentPath>: behavior::InterpretSends<I, RootEvent, Path>,
    Self: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.schedules, interpreter).await?;
            behavior::InterpretSends::interpret(self.supervision, interpreter).await
        }
    }
}

struct Pending<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    trigger: A::Nonce,
    id: TimerId,
    generation: TimerGeneration,
    commands: Vec<Delivery<Proxy<C>>>,
}

struct Counter<N> {
    trigger: N,
    attempt: u32,
    generation: u64,
}

struct Prepared<N> {
    trigger: N,
    id: TimerId,
    attempt: u32,
    generation: TimerGeneration,
    delay: Duration,
}

/// Controlled delayed-supervision failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackoffSupervisorError<E, N> {
    #[error("supervised ownership fold rejected the transition")]
    Supervision(#[source] E),
    #[error(transparent)]
    Backoff(#[from] BackoffError),
    #[error("restart attempt sequence exhausted")]
    AttemptExhausted { trigger: N },
    #[error("restart timer generation exhausted")]
    TimerGenerationExhausted { trigger: N },
    #[error("two pending restart batches selected the same timer id")]
    TimerCollision { id: TimerId },
}

struct BackoffState<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    policy: Backoff,
    timer: fn(A::Nonce) -> TimerId,
    counters: Vec<Counter<A::Nonce>>,
    pending: Vec<Pending<A, C>>,
}

impl<A, C> BackoffState<A, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    const fn new(policy: Backoff, timer: fn(A::Nonce) -> TimerId) -> Self {
        Self {
            policy,
            timer,
            counters: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn prepare<E>(
        &self,
        trigger: A::Nonce,
    ) -> Result<Option<Prepared<A::Nonce>>, BackoffSupervisorError<E, A::Nonce>> {
        if self
            .pending
            .iter()
            .any(|pending| pending.trigger == trigger)
        {
            return Ok(None);
        }
        let id = (self.timer)(trigger);
        if self.pending.iter().any(|pending| pending.id == id) {
            return Err(BackoffSupervisorError::TimerCollision { id });
        }
        let current = self
            .counters
            .iter()
            .find(|counter| counter.trigger == trigger);
        let attempt = current
            .map_or(Some(1), |counter| counter.attempt.checked_add(1))
            .ok_or(BackoffSupervisorError::AttemptExhausted { trigger })?;
        let generation = current
            .map_or(Some(0), |counter| counter.generation.checked_add(1))
            .ok_or(BackoffSupervisorError::TimerGenerationExhausted { trigger })?;
        let delay = self.policy.delay(attempt)?;
        Ok(Some(Prepared {
            trigger,
            id,
            attempt,
            generation: TimerGeneration(generation),
            delay,
        }))
    }

    fn commit(&mut self, prepared: &Prepared<A::Nonce>) {
        if let Some(counter) = self
            .counters
            .iter_mut()
            .find(|counter| counter.trigger == prepared.trigger)
        {
            counter.attempt = prepared.attempt;
            counter.generation = prepared.generation.0;
        } else {
            self.counters.push(Counter {
                trigger: prepared.trigger,
                attempt: prepared.attempt,
                generation: prepared.generation.0,
            });
        }
    }
}

/// A supervisor whose accepted replacement commands are released only after
/// a matching generation-tagged timer fact.
pub struct BackoffSupervise<B, C>
where
    B: Behavior,
    crate::BehaviorAddr<B>: Address,
    <crate::BehaviorAddr<B> as Address>::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    inner: Supervise<B, C>,
    backoff: BackoffState<crate::BehaviorAddr<B>, C>,
}

impl<B, C> BackoffSupervise<B, C>
where
    B: Behavior,
    crate::BehaviorAddr<B>: Address,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    #[must_use]
    pub const fn new(
        inner: Supervise<B, C>,
        policy: Backoff,
        timer: fn(<crate::BehaviorAddr<B> as Address>::Nonce) -> TimerId,
    ) -> Self {
        Self {
            inner,
            backoff: BackoffState::new(policy, timer),
        }
    }

    #[must_use]
    pub fn pending_restarts(&self) -> usize {
        self.backoff.pending.len()
    }
}

impl<B, C> crate::BehaviorBase for BackoffSupervise<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::BehaviorBase,
    crate::BehaviorAddr<B>: Address,
    <crate::BehaviorAddr<B> as Address>::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = crate::BehaviorAddr<B>>,
{
    type Base = B::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C, A, Ph, Sends> Behavior for BackoffSupervise<B, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Protocol: crate::Protocol<Addr = A>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    type Protocol = B::Protocol;
    type Event = BackoffSupervisorEvent<B::Event>;
    type Sends = SendLayer<BackoffSupervisorSends<A, C>, Sends>;
    type Ph = Ph;
    type Error = BackoffSupervisorError<SuperviseError<B::Error, A::Nonce>, A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(|actions| {
                actions.map_sends(|supervision| {
                    SendLayer::new(
                        BackoffSupervisorSends {
                            schedules: InterpreterRequests::empty(),
                            supervision: supervision.owned,
                        },
                        supervision.inner,
                    )
                })
            })
            .map_err(BackoffSupervisorError::Supervision)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        match event {
            BackoffSupervisorEvent::TimerElapsed(elapsed) => {
                let Some(position) = self.backoff.pending.iter().position(|pending| {
                    pending.id == elapsed.id && pending.generation == elapsed.generation
                }) else {
                    return Ok(Actions::cont());
                };
                let pending = self.backoff.pending.remove(position);
                let mut supervision = SupervisorSends::empty();
                supervision.replacement_commands = pending.commands;
                Ok(Actions::new(
                    SendLayer::new(
                        BackoffSupervisorSends {
                            schedules: InterpreterRequests::empty(),
                            supervision,
                        },
                        B::Sends::empty(),
                    ),
                    Vec::new(),
                    Step::Continue,
                ))
            }
            BackoffSupervisorEvent::ShutdownRequested(requested) => {
                self.backoff.pending.clear();
                behavior::delegate_transition(
                    &mut self.inner,
                    SupervisionEvent::ShutdownRequested(requested),
                )
                .map(|actions| {
                    actions.map_sends(|supervision| {
                        SendLayer::new(
                            BackoffSupervisorSends {
                                schedules: InterpreterRequests::empty(),
                                supervision: supervision.owned,
                            },
                            supervision.inner,
                        )
                    })
                })
                .map_err(BackoffSupervisorError::Supervision)
            }
            BackoffSupervisorEvent::Supervision(inner) => {
                if matches!(inner, SupervisionEvent::ShutdownRequested(_)) {
                    self.backoff.pending.clear();
                }
                let trigger = match &inner {
                    SupervisionEvent::WorkerStopped(stopped) => Some(stopped.proxy),
                    _ => None,
                };
                let prepared = match trigger {
                    Some(trigger) => self.backoff.prepare(trigger)?,
                    None => None,
                };
                if trigger.is_some() && prepared.is_none() {
                    return Ok(Actions::cont());
                }
                let actions = behavior::delegate_transition(&mut self.inner, inner)
                    .map_err(BackoffSupervisorError::Supervision)?;
                let Actions {
                    sends: mut supervision,
                    creates,
                    become_,
                } = actions;
                if supervision.owned.replacement_commands.is_empty() {
                    return Ok(Actions::new(
                        SendLayer::new(
                            BackoffSupervisorSends {
                                schedules: InterpreterRequests::empty(),
                                supervision: supervision.owned,
                            },
                            supervision.inner,
                        ),
                        creates,
                        become_,
                    ));
                }
                let Some(prepared) = prepared else {
                    return Ok(Actions::new(
                        SendLayer::new(
                            BackoffSupervisorSends {
                                schedules: InterpreterRequests::empty(),
                                supervision: supervision.owned,
                            },
                            supervision.inner,
                        ),
                        creates,
                        become_,
                    ));
                };
                let commands = core::mem::take(&mut supervision.owned.replacement_commands);
                let schedule = ScheduleAfter::new(prepared.id, prepared.generation, prepared.delay);
                self.backoff.commit(&prepared);
                self.backoff.pending.push(Pending {
                    trigger: prepared.trigger,
                    id: prepared.id,
                    generation: prepared.generation,
                    commands,
                });
                Ok(Actions::new(
                    SendLayer::new(
                        BackoffSupervisorSends {
                            schedules: InterpreterRequests::one(schedule),
                            supervision: supervision.owned,
                        },
                        supervision.inner,
                    ),
                    creates,
                    become_,
                ))
            }
        }
    }
}

/// Standalone fixed-fleet supervisor with generation-safe delayed replacement.
///
/// Fixed-fleet ownership remains in [`Supervisor`]; this adapter owns only
/// checked delay attempts, timer generations, pending replacement batches, and
/// stale-timer rejection. No application behavior is required to witness child
/// creation authority.
pub struct BackoffSupervisor<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    inner: Supervisor<A, C>,
    backoff: BackoffState<A, C>,
}

impl<A, C> BackoffSupervisor<A, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    #[must_use]
    pub const fn new(
        inner: Supervisor<A, C>,
        policy: Backoff,
        timer: fn(A::Nonce) -> TimerId,
    ) -> Self {
        Self {
            inner,
            backoff: BackoffState::new(policy, timer),
        }
    }

    #[must_use]
    pub fn pending_restarts(&self) -> usize {
        self.backoff.pending.len()
    }
}

impl<A, C> crate::BehaviorBase for BackoffSupervisor<A, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
{
    type Base = <Supervisor<A, C> as crate::BehaviorBase>::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<A, C> Behavior for BackoffSupervisor<A, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Ph = Never>,
    C::Protocol: crate::Protocol<Addr = A>,
    SupervisorSends<A, C>: behavior::SendsFor<SupervisionEvent<User<A, Never>>>,
    BackoffSupervisorSends<A, C>: behavior::SendsFor<BackoffSupervisorEvent<User<A, Never>>>,
{
    type Protocol = <Supervisor<A, C> as Behavior>::Protocol;
    type Event = BackoffSupervisorEvent<User<A, Never>>;
    type Sends = BackoffSupervisorSends<A, C>;
    type Ph = Never;
    type Error = BackoffSupervisorError<SupervisorError<A::Nonce>, A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(|actions| {
                actions.map_sends(|supervision| BackoffSupervisorSends {
                    schedules: InterpreterRequests::empty(),
                    supervision,
                })
            })
            .map_err(BackoffSupervisorError::Supervision)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        match event {
            BackoffSupervisorEvent::TimerElapsed(elapsed) => {
                let Some(position) = self.backoff.pending.iter().position(|pending| {
                    pending.id == elapsed.id && pending.generation == elapsed.generation
                }) else {
                    return Ok(Actions::cont());
                };
                let pending = self.backoff.pending.remove(position);
                let mut supervision = SupervisorSends::empty();
                supervision.replacement_commands = pending.commands;
                Ok(Actions::send(BackoffSupervisorSends {
                    schedules: InterpreterRequests::empty(),
                    supervision,
                }))
            }
            BackoffSupervisorEvent::ShutdownRequested(requested) => {
                self.backoff.pending.clear();
                behavior::delegate_transition(
                    &mut self.inner,
                    SupervisionEvent::ShutdownRequested(requested),
                )
                .map(|actions| {
                    actions.map_sends(|supervision| BackoffSupervisorSends {
                        schedules: InterpreterRequests::empty(),
                        supervision,
                    })
                })
                .map_err(BackoffSupervisorError::Supervision)
            }
            BackoffSupervisorEvent::Supervision(inner) => {
                if matches!(inner, SupervisionEvent::ShutdownRequested(_)) {
                    self.backoff.pending.clear();
                }
                let trigger = match &inner {
                    SupervisionEvent::WorkerStopped(stopped) => Some(stopped.proxy),
                    _ => None,
                };
                let prepared = match trigger {
                    Some(trigger) => self.backoff.prepare(trigger)?,
                    None => None,
                };
                if trigger.is_some() && prepared.is_none() {
                    return Ok(Actions::cont());
                }
                let actions = behavior::delegate_transition(&mut self.inner, inner)
                    .map_err(BackoffSupervisorError::Supervision)?;
                let Actions {
                    sends: mut supervision,
                    creates,
                    become_,
                } = actions;
                if supervision.replacement_commands.is_empty() {
                    return Ok(Actions::new(
                        BackoffSupervisorSends {
                            schedules: InterpreterRequests::empty(),
                            supervision,
                        },
                        creates,
                        become_,
                    ));
                }
                let Some(prepared) = prepared else {
                    return Ok(Actions::new(
                        BackoffSupervisorSends {
                            schedules: InterpreterRequests::empty(),
                            supervision,
                        },
                        creates,
                        become_,
                    ));
                };
                let commands = core::mem::take(&mut supervision.replacement_commands);
                let schedule = ScheduleAfter::new(prepared.id, prepared.generation, prepared.delay);
                self.backoff.commit(&prepared);
                self.backoff.pending.push(Pending {
                    trigger: prepared.trigger,
                    id: prepared.id,
                    generation: prepared.generation,
                    commands,
                });
                Ok(Actions::new(
                    BackoffSupervisorSends {
                        schedules: InterpreterRequests::one(schedule),
                        supervision,
                    },
                    creates,
                    become_,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{
        Activate as _, ChildTopology, Crash, CreationResolved, RestartConfiguration, RestartPolicy,
        ShutdownRequested, Strategy, TimerElapsed, WorkerStopped,
    };
    use behavior::{Actions, MailAddr, NoBirths, User};

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
        ) -> behavior::BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    struct Parent;
    impl behavior::Protocol for Parent {
        type Addr = MailAddr;
        type Msg = ();
    }

    impl Behavior for Parent {
        type Protocol = Self;
        type Event = User<MailAddr, ()>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<Child>;
        fn transition(
            &mut self,
            _: crate::ActiveTurn,
            _: Self::Event,
        ) -> behavior::BehaviorActed<Self> {
            Ok(Actions::create(vec![behavior::Create::birth(99, Child)]))
        }
    }
    #[allow(clippy::unnecessary_wraps)]
    fn child(_: usize) -> Option<Child> {
        Some(Child)
    }
    fn timer(nonce: u64) -> TimerId {
        TimerId(nonce + 10)
    }
    fn same_timer(_: u64) -> TimerId {
        TimerId(1)
    }
    fn subject(timer: fn(u64) -> TimerId) -> BackoffSupervise<Parent, Child> {
        let supervisor = Supervise::new(
            Parent,
            ChildTopology::new([1, 2], child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                10,
                Duration::MAX,
            ),
        )
        .unwrap();
        BackoffSupervise::new(
            supervisor,
            Backoff::exponential(Duration::from_secs(2), Duration::from_secs(10)).unwrap(),
            timer,
        )
    }
    fn standalone(timer: fn(u64) -> TimerId) -> BackoffSupervisor<MailAddr, Child> {
        let supervisor = Supervisor::new(
            ChildTopology::new([1, 2], child),
            RestartConfiguration::new(
                Strategy::OneForOne,
                RestartPolicy::Permanent,
                10,
                Duration::MAX,
            ),
        )
        .unwrap();
        BackoffSupervisor::new(
            supervisor,
            Backoff::exponential(Duration::from_secs(2), Duration::from_secs(10)).unwrap(),
            timer,
        )
    }
    fn stopped(proxy: u64) -> WorkerStopped<MailAddr> {
        WorkerStopped::new(proxy, 100 + proxy, Err(Crash::Panicked), Instant::now())
    }

    #[test]
    fn accepted_replacement_is_scheduled_then_released_by_exact_timer() {
        let initialized = subject(timer).initialize().unwrap();
        assert_eq!(initialized.actions.creates.len(), 2);
        assert_eq!(
            initialized
                .actions
                .sends
                .owned
                .supervision
                .creation_observations
                .len(),
            2
        );
        for (creation, observation) in initialized.actions.creates.iter().zip(
            initialized
                .actions
                .sends
                .owned
                .supervision
                .creation_observations
                .iter(),
        ) {
            assert_eq!(creation.nonce, observation.nonce);
        }
        assert!(initialized.actions.sends.owned.schedules.is_empty());
        let mut active = initialized.behavior;
        let delayed = active.on_path(stopped(1)).unwrap();
        assert!(
            delayed
                .sends
                .owned
                .supervision
                .replacement_commands
                .is_empty()
        );
        assert_eq!(
            delayed.sends.owned.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(11),
                TimerGeneration(0),
                Duration::from_secs(2)
            )]
        );
        assert_eq!(active.pending_restarts(), 1);

        let stale = active
            .on_path(TimerElapsed::new(TimerId(11), TimerGeneration(9)))
            .unwrap();
        assert!(
            stale
                .sends
                .owned
                .supervision
                .replacement_commands
                .is_empty()
        );
        assert!(stale.sends.owned.schedules.is_empty());
        assert_eq!(active.pending_restarts(), 1);

        let released = active
            .on_path(TimerElapsed::new(TimerId(11), TimerGeneration(0)))
            .unwrap();
        assert_eq!(
            released.sends.owned.supervision.replacement_commands.len(),
            1
        );
        assert!(released.sends.owned.schedules.is_empty());
        assert_eq!(active.pending_restarts(), 0);
    }

    #[test]
    fn repeated_failure_uses_next_generation_and_attempt_delay() {
        let mut active = subject(timer).initialize().unwrap().behavior;
        active.on_path(stopped(1)).unwrap();
        let duplicate = active.on_path(stopped(1)).unwrap();
        assert!(
            duplicate
                .sends
                .owned
                .supervision
                .replacement_commands
                .is_empty()
        );
        assert!(duplicate.sends.owned.schedules.is_empty());
        active
            .on_path(TimerElapsed::new(TimerId(11), TimerGeneration(0)))
            .unwrap();
        active
            .on_path(WorkerCreationResolved::new(
                1,
                201,
                behavior::CreationKind::replacement_of(101),
                Ok(()),
            ))
            .unwrap();
        let second = active
            .on_path(WorkerStopped::new(
                1,
                201,
                Err(Crash::Panicked),
                Instant::now(),
            ))
            .unwrap();
        assert_eq!(
            second.sends.owned.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(11),
                TimerGeneration(1),
                Duration::from_secs(4)
            )]
        );
    }

    #[test]
    fn timer_collision_is_typed_before_second_supervisor_transition() {
        let mut active = subject(same_timer).initialize().unwrap().behavior;
        active.on_path(stopped(1)).unwrap();
        assert!(matches!(
            active.on_path(stopped(2)),
            Err(BackoffSupervisorError::TimerCollision { id: TimerId(1) })
        ));
        assert_eq!(active.pending_restarts(), 1);
    }

    #[test]
    fn shutdown_cancels_delayed_restarts_and_drains_the_fixed_proxy_fleet() {
        let mut active = subject(timer).initialize().unwrap().behavior;
        for nonce in [1, 2] {
            active
                .on_path(CreationResolved::birth(nonce, MailAddr(20 + nonce)))
                .unwrap();
        }
        active.on_path(stopped(1)).unwrap();
        assert_eq!(active.pending_restarts(), 1);

        let shutdown = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(active.pending_restarts(), 0);
        assert_eq!(shutdown.sends.owned.supervision.shutdowns.len(), 2);
        assert!(shutdown.sends.owned.schedules.is_empty());
    }

    #[test]
    fn standalone_backoff_preserves_generation_and_shutdown_laws_without_carrier() {
        let initialized = standalone(timer).initialize().unwrap();
        assert_eq!(initialized.actions.creates.len(), 2);
        assert_eq!(
            initialized
                .actions
                .sends
                .supervision
                .creation_observations
                .len(),
            2
        );
        let mut active = initialized.behavior;

        let delayed = active.on_path(stopped(1)).unwrap();
        assert!(delayed.sends.supervision.replacement_commands.is_empty());
        assert_eq!(
            delayed.sends.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(11),
                TimerGeneration(0),
                Duration::from_secs(2)
            )]
        );
        let stale = active
            .on_path(TimerElapsed::new(TimerId(11), TimerGeneration(9)))
            .unwrap();
        assert!(stale.sends.supervision.replacement_commands.is_empty());
        assert_eq!(active.pending_restarts(), 1);

        let released = active
            .on_path(TimerElapsed::new(TimerId(11), TimerGeneration(0)))
            .unwrap();
        assert_eq!(released.sends.supervision.replacement_commands.len(), 1);
        active.on_path(stopped(2)).unwrap();
        assert_eq!(active.pending_restarts(), 1);
        let shutdown = active.on_path(ShutdownRequested).unwrap();
        assert_eq!(active.pending_restarts(), 0);
        assert!(shutdown.sends.schedules.is_empty());
    }
}
