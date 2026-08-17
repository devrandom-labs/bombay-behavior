//! Timer-delayed supervision as a lossless wrapper over [`Supervisor`].

use std::time::Duration;

use super::{Backoff, BackoffError, Proxy, Supervisor, SupervisorError, SupervisorSends};
use crate::{
    Own, ScheduleAfter, SendInput, SupervisionEvent, TimedEvent, TimerElapsed, TimerGeneration,
    TimerId,
};
use behavior::{
    Actions, Address, Behavior, Births, Delivery, Never, SendAlgebra, ServiceSends, Step,
};

struct Pending<A, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
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

type PreparedResult<B> = Result<
    Option<Prepared<<<B as Behavior>::Addr as Address>::Nonce>>,
    BackoffSupervisorError<<B as Behavior>::Error, <<B as Behavior>::Addr as Address>::Nonce>,
>;

/// Named products emitted by delayed supervision.
pub struct BackoffSupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    /// Every ordinary supervision lane, unchanged except that replacement
    /// commands are withheld until their matching timer fires.
    pub supervision: SupervisorSends<A, Sends, C>,
    /// Relative restart schedules interpreted by Bombay Timers.
    pub schedules: ServiceSends<ScheduleAfter>,
}

impl<A, Sends, C> SendAlgebra for BackoffSupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    Sends: SendAlgebra,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn empty() -> Self {
        Self {
            supervision: SupervisorSends::empty(),
            schedules: ServiceSends::empty(),
        }
    }
    fn append(&mut self, other: Self) {
        self.supervision.append(other.supervision);
        self.schedules.append(other.schedules);
    }
}
impl<A, Sends, C> SendInput<ScheduleAfter, Own> for BackoffSupervisorSends<A, Sends, C>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
{
    fn emit(&mut self, input: ScheduleAfter) {
        self.schedules.send(input);
    }
}

/// Controlled delayed-supervision failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackoffSupervisorError<E, N> {
    #[error(transparent)]
    Supervisor(#[from] SupervisorError<E, N>),
    #[error(transparent)]
    Backoff(#[from] BackoffError),
    #[error("restart attempt sequence exhausted")]
    AttemptExhausted { trigger: N },
    #[error("restart timer generation exhausted")]
    TimerGenerationExhausted { trigger: N },
    #[error("two pending restart batches selected the same timer id")]
    TimerCollision { id: TimerId },
}

/// A supervisor whose accepted replacement commands are released only after
/// a matching generation-tagged timer fact.
pub struct BackoffSupervisor<B, C>
where
    B: Behavior,
    B::Addr: Address,
    <B::Addr as Address>::Nonce: From<u64>,
    C: Behavior<Addr = B::Addr, Ph = Never>,
{
    inner: Supervisor<B, C>,
    policy: Backoff,
    timer: fn(<B::Addr as Address>::Nonce) -> TimerId,
    counters: Vec<Counter<<B::Addr as Address>::Nonce>>,
    pending: Vec<Pending<B::Addr, C>>,
}

impl<B, C> BackoffSupervisor<B, C>
where
    B: Behavior,
    B::Addr: Address,
    <B::Addr as Address>::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Addr = B::Addr, Ph = Never>,
{
    #[must_use]
    pub const fn new(
        inner: Supervisor<B, C>,
        policy: Backoff,
        timer: fn(<B::Addr as Address>::Nonce) -> TimerId,
    ) -> Self {
        Self {
            inner,
            policy,
            timer,
            counters: Vec::new(),
            pending: Vec::new(),
        }
    }

    #[must_use]
    pub fn pending_restarts(&self) -> usize {
        self.pending.len()
    }

    fn prepare(&self, trigger: <B::Addr as Address>::Nonce) -> PreparedResult<B> {
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

    fn commit_counter(&mut self, prepared: &Prepared<<B::Addr as Address>::Nonce>) {
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

impl<B, C> crate::BehaviorBase for BackoffSupervisor<B, C>
where
    B: Behavior<Birth = Births<C>> + crate::BehaviorBase,
    B::Addr: Address,
    <B::Addr as Address>::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Addr = B::Addr, Ph = Never>,
{
    type Base = B::Base;
    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, C, A, Ph, Sends> Behavior for BackoffSupervisor<B, C>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    Sends: SendAlgebra,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Births<C>>,
    B::Event: crate::RouteInput<crate::ChildStopped<A>>
        + crate::RouteInput<crate::CreationResolved<A::Nonce>>
        + crate::RouteInput<crate::WorkerCreationResolved<A::Nonce>>
        + crate::RouteInput<TimerElapsed>,
    C: Behavior<Addr = A, Ph = Never>,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = TimedEvent<SupervisionEvent<B::Event>>;
    type Sends = BackoffSupervisorSends<A, Sends, C>;
    type Ph = Ph;
    type Error = BackoffSupervisorError<B::Error, A::Nonce>;
    type Birth = Births<Proxy<C>>;

    fn init(&mut self, _: crate::InitializationTurn) -> behavior::BehaviorActed<Self> {
        behavior::initialize(&mut self.inner)
            .map(|actions| {
                actions.map_sends(|supervision| BackoffSupervisorSends {
                    supervision,
                    schedules: ServiceSends::empty(),
                })
            })
            .map_err(BackoffSupervisorError::Supervisor)
    }

    fn transition(
        &mut self,
        _: crate::ActiveTurn,
        event: Self::Event,
    ) -> behavior::BehaviorActed<Self> {
        match event {
            TimedEvent::Elapsed(elapsed) => {
                let Some(position) = self.pending.iter().position(|pending| {
                    pending.id == elapsed.id && pending.generation == elapsed.generation
                }) else {
                    return match <SupervisionEvent<B::Event> as crate::RouteInput<TimerElapsed>>::route(elapsed) {
                        Ok(inner) => behavior::delegate_transition(&mut self.inner, inner)
                            .map(|actions| actions.map_sends(|supervision| BackoffSupervisorSends { supervision, schedules: ServiceSends::empty() }))
                            .map_err(BackoffSupervisorError::Supervisor),
                        Err(_) => Ok(Actions::cont()),
                    };
                };
                let pending = self.pending.remove(position);
                let mut supervision = SupervisorSends::empty();
                supervision.replacement_commands = pending.commands;
                Ok(Actions::new(
                    BackoffSupervisorSends {
                        supervision,
                        schedules: ServiceSends::empty(),
                    },
                    Vec::new(),
                    Step::Continue,
                ))
            }
            TimedEvent::Behavior(inner) => {
                let trigger = match &inner {
                    SupervisionEvent::WorkerStopped(stopped) => Some(stopped.proxy),
                    _ => None,
                };
                let prepared = match trigger {
                    Some(trigger) => self.prepare(trigger)?,
                    None => None,
                };
                if trigger.is_some() && prepared.is_none() {
                    return Ok(Actions::cont());
                }
                let actions = behavior::delegate_transition(&mut self.inner, inner)
                    .map_err(BackoffSupervisorError::Supervisor)?;
                let Actions {
                    sends: mut supervision,
                    creates,
                    become_,
                } = actions;
                if supervision.replacement_commands.is_empty() {
                    return Ok(Actions::new(
                        BackoffSupervisorSends {
                            supervision,
                            schedules: ServiceSends::empty(),
                        },
                        creates,
                        become_,
                    ));
                }
                let Some(prepared) = prepared else {
                    return Ok(Actions::new(
                        BackoffSupervisorSends {
                            supervision,
                            schedules: ServiceSends::empty(),
                        },
                        creates,
                        become_,
                    ));
                };
                let commands = core::mem::take(&mut supervision.replacement_commands);
                let schedule = ScheduleAfter::new(prepared.id, prepared.generation, prepared.delay);
                self.commit_counter(&prepared);
                self.pending.push(Pending {
                    trigger: prepared.trigger,
                    id: prepared.id,
                    generation: prepared.generation,
                    commands,
                });
                Ok(Actions::new(
                    BackoffSupervisorSends {
                        supervision,
                        schedules: ServiceSends::one(schedule),
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
        Activate as _, ChildTopology, Crash, RestartConfiguration, RestartPolicy, Strategy,
        WorkerStopped,
    };
    use behavior::{Actions, MailAddr, NoBirths, User};

    struct Child;
    impl Behavior for Child {
        type Addr = MailAddr;
        type Msg = ();
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
    impl Behavior for Parent {
        type Addr = MailAddr;
        type Msg = ();
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
            Ok(Actions::cont())
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
    fn subject(timer: fn(u64) -> TimerId) -> BackoffSupervisor<Parent, Child> {
        let supervisor = Supervisor::new(
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
                .supervision
                .creation_observations
                .len(),
            2
        );
        for (creation, observation) in initialized.actions.creates.iter().zip(
            initialized
                .actions
                .sends
                .supervision
                .creation_observations
                .iter(),
        ) {
            assert_eq!(creation.nonce, observation.nonce);
        }
        assert!(initialized.actions.sends.schedules.is_empty());
        let mut active = initialized.behavior;
        let delayed = active.on(stopped(1)).unwrap();
        assert!(delayed.sends.supervision.replacement_commands.is_empty());
        assert_eq!(
            delayed.sends.schedules.as_slice(),
            [ScheduleAfter::new(
                TimerId(11),
                TimerGeneration(0),
                Duration::from_secs(2)
            )]
        );
        assert_eq!(active.pending_restarts(), 1);

        let stale = active
            .on(TimerElapsed::new(TimerId(11), TimerGeneration(9)))
            .unwrap();
        assert!(stale.sends.supervision.replacement_commands.is_empty());
        assert!(stale.sends.schedules.is_empty());
        assert_eq!(active.pending_restarts(), 1);

        let released = active
            .on(TimerElapsed::new(TimerId(11), TimerGeneration(0)))
            .unwrap();
        assert_eq!(released.sends.supervision.replacement_commands.len(), 1);
        assert!(released.sends.schedules.is_empty());
        assert_eq!(active.pending_restarts(), 0);
    }

    #[test]
    fn repeated_failure_uses_next_generation_and_attempt_delay() {
        let mut active = subject(timer).initialize().unwrap().behavior;
        active.on(stopped(1)).unwrap();
        let duplicate = active.on(stopped(1)).unwrap();
        assert!(duplicate.sends.supervision.replacement_commands.is_empty());
        assert!(duplicate.sends.schedules.is_empty());
        active
            .on(TimerElapsed::new(TimerId(11), TimerGeneration(0)))
            .unwrap();
        let second = active.on(stopped(1)).unwrap();
        assert_eq!(
            second.sends.schedules.as_slice(),
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
        active.on(stopped(1)).unwrap();
        assert!(matches!(
            active.on(stopped(2)),
            Err(BackoffSupervisorError::TimerCollision { id: TimerId(1) })
        ));
        assert_eq!(active.pending_restarts(), 1);
    }
}
