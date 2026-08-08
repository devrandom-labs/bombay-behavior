//! Pure supervision. Stable child identity is a proxy actor; replacement is a
//! message to that proxy, which creates a fresh worker incarnation.
//!
//! Supervision is a Bombay-derived construction, not a privileged actor-model
//! effect. Its transition law is the same pure fold as every other behavior:
//! one typed termination observation updates the supervisor's explicit state
//! and returns only sends, fresh creations, and become. Restart eligibility,
//! candidate selection, budget admission, and the reaction to an unsatisfied
//! topology are behavior policy. The interpreter only delivers observations
//! and interprets the resulting [`Actions`].

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, SendProduct,
    ServiceSends, User, UserEvent,
};
use crate::protocol::{
    ChildEvent, ChildStopped, ObserveChild, PeerEvent, PeerStopped, ReportWorkerStopped,
    ShutdownEvent, ShutdownRequested, TimeEvent, TimeReached, WorkerEvent, WorkerStopped,
};
use crate::verdict::{Never, Step};
use crate::{Become, Crash, Exit, RestartDenial, SupervisionFailureReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    OneForOne,
    OneForAll,
    RestForOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Permanent,
    Transient,
    Temporary,
}

#[must_use]
pub const fn restart_one() -> Strategy {
    Strategy::OneForOne
}

#[must_use]
pub const fn restart_all() -> Strategy {
    Strategy::OneForAll
}

#[must_use]
pub const fn restart_rest() -> Strategy {
    Strategy::RestForOne
}

/// A typed failure of the supervisor's child-topology contract.
///
/// The original terminal outcome is owned here so a pure reaction can inspect
/// it without consulting interpreter state. The compact `reason` is also the
/// value propagated in [`Exit::SupervisionFailed`] when the supplied reaction
/// chooses to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionFailure<A: Address> {
    /// Stable slot whose termination made the topology unsatisfiable.
    pub child: A::Nonce,
    /// Original terminal outcome, preserved without cloning or erasure.
    pub outcome: Result<Exit<A>, Crash>,
    /// Exhaustive policy reason for the failure.
    pub reason: SupervisionFailureReason,
}

impl<A: Address> SupervisionFailure<A> {
    /// Reduce the local failure observation to the terminal value propagated
    /// through ordinary child observation.
    #[must_use]
    pub const fn into_exit(self) -> Exit<A> {
        Exit::SupervisionFailed(self.reason)
    }
}

/// Pure policy applied when a supervisor cannot preserve its child topology.
///
/// A function pointer keeps the policy concrete, non-capturing, allocation
/// free, and statically dispatched. It may update the wrapped behavior as part
/// of the same serialized fold and chooses only its next behavior; it cannot
/// interpret effects or consult runtime state.
pub type SupervisionFailureReaction<B> =
    fn(
        &mut B,
        &SupervisionFailure<<B as Behavior>::Addr>,
    ) -> Result<Become<<B as Behavior>::Addr>, <B as Behavior>::Error>;

/// Retire the failed slot and keep the supervisor alive.
///
/// This is Bombay's compatibility policy and the default for a newly built
/// supervisor. It never produces a controlled behavior error.
///
/// # Errors
///
/// This supplied policy never returns a controlled behavior error.
pub fn retire_on_supervision_failure<B: Behavior>(
    _behavior: &mut B,
    _failure: &SupervisionFailure<B::Addr>,
) -> Result<Become<B::Addr>, B::Error> {
    Ok(Step::Continue)
}

/// Stop the supervisor with a typed failure outcome.
///
/// A parent observing this ordinary actor exit may apply its own supervision
/// policy. This derives escalation through the existing actor algebra rather
/// than adding an interpreter-only effect.
///
/// # Errors
///
/// This supplied policy never returns a controlled behavior error.
pub fn stop_on_supervision_failure<B: Behavior>(
    _behavior: &mut B,
    failure: &SupervisionFailure<B::Addr>,
) -> Result<Become<B::Addr>, B::Error> {
    Ok(Step::Stop(failure.into_exit()))
}

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E, A: Address> {
    Inner(E),
    ChildStopped(ChildStopped<A>),
    WorkerStopped(WorkerStopped<A>),
}

impl<E, A: Address> ChildEvent<A> for SupervisionEvent<E, A> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        Some(Self::ChildStopped(event))
    }
}

impl<E, A: Address> WorkerEvent<A> for SupervisionEvent<E, A> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        Some(Self::WorkerStopped(event))
    }
}

impl<E: UserEvent, A: Address> UserEvent for SupervisionEvent<E, A> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            stopped @ (Self::ChildStopped(_) | Self::WorkerStopped(_)) => Err(stopped),
        }
    }
}

impl<E: TimeEvent, A: Address> TimeEvent for SupervisionEvent<E, A> {
    fn time_reached(event: TimeReached) -> Option<Self> {
        E::time_reached(event).map(Self::Inner)
    }
}

impl<E: PeerEvent<A>, A: Address> PeerEvent<A> for SupervisionEvent<E, A> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent, A: Address> ShutdownEvent for SupervisionEvent<E, A> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        E::shutdown_requested(event).map(Self::Inner)
    }
}

/// Commands accepted by a stable proxy.
///
/// A replacement requested while the current worker is alive is held until
/// that worker's observed termination. Creation therefore never overlaps two
/// worker incarnations beneath the same proxy.
#[derive(Debug)]
pub enum ProxyCommand<C: Behavior> {
    /// Forward an application message to the current worker, if one is alive.
    Forward(C::Msg),
    /// Supply the worker behavior for the next fresh incarnation.
    Replace(C),
}

pub type SupervisorSends<A, Sends, C> = SendProduct<
    Sends,
    SendProduct<ServiceSends<ObserveChild<A>>, Vec<Delivery<A, ProxyCommand<C>>>>,
>;

pub type SupervisorActions<B, C> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    SupervisorSends<<B as Behavior>::Addr, <B as Behavior>::Sends, C>,
    Births<Proxy<C>>,
>;

/// The stable actor. Every replacement is an ordinary fresh birth beneath it.
///
/// Each emitted worker birth is paired with an [`ObserveChild`] request. A
/// matching [`ChildStopped`] leaves the proxy alive and emits a
/// [`ReportWorkerStopped`] carrying the outcome unchanged. Stale child-stop
/// observations are inert.
pub struct Proxy<C: Behavior<Ph = Never>> {
    worker: Option<C>,
    generation: u64,
    worker_alive: bool,
    pending: Option<C>,
}

impl<C: Behavior<Ph = Never>> Proxy<C> {
    #[must_use]
    pub fn new(worker: C) -> Self {
        Self {
            worker: Some(worker),
            generation: 0,
            worker_alive: false,
            pending: None,
        }
    }
}

impl<C> Behavior for Proxy<C>
where
    C: Behavior<Ph = Never> + Send,
    C::Addr: Send,
    <C::Addr as Address>::Nonce: From<u64> + Send,
    C::Msg: Send,
    C: Send,
{
    type Addr = C::Addr;
    type Msg = ProxyCommand<C>;
    type Event = SupervisionEvent<User<C::Addr, ProxyCommand<C>>, C::Addr>;
    type Sends = SendProduct<
        Vec<Delivery<C::Addr, C::Msg>>,
        SendProduct<
            ServiceSends<ObserveChild<C::Addr>>,
            ServiceSends<ReportWorkerStopped<C::Addr>>,
        >,
    >;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<C>;

    async fn init(&mut self) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        let child = self.worker.take().expect("a proxy initializes once");
        self.worker_alive = true;
        Ok(Actions {
            sends: SendProduct {
                inner: Vec::new(),
                own: SendProduct {
                    inner: ServiceSends::one(ObserveChild {
                        nonce: <C::Addr as Address>::Nonce::from(self.generation),
                    }),
                    own: ServiceSends::empty(),
                },
            },
            creates: vec![Create::birth(
                <C::Addr as Address>::Nonce::from(self.generation),
                child,
            )],
            become_: Step::Continue,
        })
    }

    async fn step(
        &mut self,
        event: Self::Event,
    ) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        let SupervisionEvent::Inner(event) = event else {
            return match event {
                SupervisionEvent::ChildStopped(event)
                    if event.nonce == <C::Addr as Address>::Nonce::from(self.generation) =>
                {
                    self.worker_alive = false;
                    let report = ReportWorkerStopped {
                        outcome: event.outcome,
                        at: event.at,
                    };
                    let creates = self.pending.take().map_or_else(Vec::new, |child| {
                        self.generation = self
                            .generation
                            .checked_add(1)
                            .expect("proxy generation exhausted");
                        self.worker_alive = true;
                        vec![Create::replacement_incarnation(
                            <C::Addr as Address>::Nonce::from(self.generation),
                            child,
                        )]
                    });
                    let observes = creates
                        .iter()
                        .map(|create| ObserveChild {
                            nonce: create.nonce,
                        })
                        .collect();
                    Ok(Actions {
                        sends: SendProduct {
                            inner: Vec::new(),
                            own: SendProduct {
                                inner: ServiceSends::new(observes),
                                own: ServiceSends::one(report),
                            },
                        },
                        creates,
                        become_: Step::Continue,
                    })
                }
                SupervisionEvent::ChildStopped(_) | SupervisionEvent::WorkerStopped(_) => {
                    Ok(Actions::cont())
                }
                SupervisionEvent::Inner(_) => unreachable!(),
            };
        };
        match event.message {
            ProxyCommand::Forward(message) => Ok(Actions {
                sends: SendProduct {
                    inner: self
                        .worker_alive
                        .then(|| {
                            Delivery::new(
                                Recipient::child(<C::Addr as Address>::Nonce::from(
                                    self.generation,
                                )),
                                message,
                            )
                        })
                        .into_iter()
                        .collect(),
                    own: SendProduct {
                        inner: ServiceSends::empty(),
                        own: ServiceSends::empty(),
                    },
                },
                creates: Vec::new(),
                become_: Step::Continue,
            }),
            ProxyCommand::Replace(child) => {
                if self.worker_alive {
                    self.pending = Some(child);
                    return Ok(Actions::cont());
                }
                self.generation = self
                    .generation
                    .checked_add(1)
                    .expect("proxy generation exhausted");
                self.worker_alive = true;
                let nonce = <C::Addr as Address>::Nonce::from(self.generation);
                Ok(Actions {
                    sends: SendProduct {
                        inner: Vec::new(),
                        own: SendProduct {
                            inner: ServiceSends::one(ObserveChild { nonce }),
                            own: ServiceSends::empty(),
                        },
                    },
                    creates: vec![Create::replacement_incarnation(nonce, child)],
                    become_: Step::Continue,
                })
            }
        }
    }
}

struct Slot {
    alive: bool,
    sequence: u64,
}

enum ReplacementDecision<A: Address, C: Behavior<Addr = A>> {
    Retire,
    Replace(Vec<Delivery<A, ProxyCommand<C>>>),
    Failed(SupervisionFailure<A>),
}

pub struct Supervising<B: Behavior, C: Behavior<Ph = Never, Addr = B::Addr>> {
    inner: B,
    slots: Vec<(<B::Addr as Address>::Nonce, Slot)>,
    configured_count: usize,
    next_sequence: u64,
    build: fn(usize) -> C,
    strategy: Strategy,
    policy: RestartPolicy,
    max_restarts: u32,
    window: Duration,
    restarts: Vec<Instant>,
    on_failure: SupervisionFailureReaction<B>,
}

impl<B, C> Supervising<B, C>
where
    B: Behavior<Birth = Births<C>>,
    C: Behavior<Ph = Never, Addr = B::Addr>,
{
    #[allow(clippy::too_many_arguments, reason = "hidden by Spec")]
    /// Construct the concrete supervisor behavior hidden by `Spec`.
    ///
    /// # Panics
    /// Panics only if a fleet index cannot be represented by `u64`.
    #[must_use]
    pub fn new(
        inner: B,
        nonces: fn(usize) -> <B::Addr as Address>::Nonce,
        count: usize,
        build: fn(usize) -> C,
        strategy: Strategy,
        policy: RestartPolicy,
        max_restarts: u32,
        window: Duration,
    ) -> Self {
        let slots = (0..count)
            .map(|index| {
                (
                    nonces(index),
                    Slot {
                        alive: true,
                        sequence: u64::try_from(index).expect("fleet index fits u64"),
                    },
                )
            })
            .collect();
        Self {
            inner,
            slots,
            configured_count: count,
            next_sequence: u64::try_from(count).expect("fleet size fits u64"),
            build,
            strategy,
            policy,
            max_restarts,
            window,
            restarts: Vec::new(),
            on_failure: retire_on_supervision_failure::<B>,
        }
    }

    #[must_use]
    pub fn with_strategy(mut self, strategy: Strategy) -> Self {
        self.strategy = strategy;
        self
    }

    #[must_use]
    pub fn with_policy(mut self, policy: RestartPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn with_budget(mut self, max: u32, window: Duration) -> Self {
        self.max_restarts = max;
        self.window = window;
        self
    }

    #[must_use]
    /// Replace the pure reaction used for typed supervision failures.
    pub fn with_failure_reaction(mut self, reaction: SupervisionFailureReaction<B>) -> Self {
        self.on_failure = reaction;
        self
    }

    fn position(&self, nonce: <B::Addr as Address>::Nonce) -> Option<usize> {
        self.slots.iter().position(|(known, _)| *known == nonce)
    }

    #[must_use]
    /// Report whether a known supervised proxy is alive.
    ///
    /// # Panics
    /// Panics when `nonce` is not part of this supervisor topology.
    pub fn is_alive(&self, nonce: <B::Addr as Address>::Nonce) -> bool {
        self.slots[self.position(nonce).expect("unknown supervised nonce")]
            .1
            .alive
    }

    #[must_use]
    pub fn child_count(&self) -> usize {
        self.slots.len()
    }

    #[must_use]
    pub fn restarts_in_window(&self) -> usize {
        self.restarts.len()
    }

    fn replacement_decision(
        &mut self,
        event: &WorkerStopped<B::Addr>,
    ) -> ReplacementDecision<B::Addr, C> {
        let dead = self
            .position(event.proxy)
            .expect("unknown supervised nonce");
        let eligible = match self.policy {
            RestartPolicy::Permanent => true,
            RestartPolicy::Transient => {
                !matches!(&event.outcome, Ok(Exit::Normal | Exit::Collected))
            }
            RestartPolicy::Temporary => false,
        };
        if !eligible {
            self.slots[dead].1.alive = false;
            return ReplacementDecision::Retire;
        }
        if self.window != Duration::MAX {
            self.restarts.retain(|stamp| {
                event
                    .at
                    .checked_duration_since(*stamp)
                    .is_none_or(|age| age <= self.window)
            });
        }
        let sequence = self.slots[dead].1.sequence;
        let candidates: Vec<usize> = match self.strategy {
            Strategy::OneForOne => vec![dead],
            Strategy::OneForAll => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, (_, slot))| slot.alive.then_some(index))
                .collect(),
            Strategy::RestForOne => self
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, (_, slot))| {
                    (slot.alive && slot.sequence >= sequence).then_some(index)
                })
                .collect(),
        };
        if self.restarts.len() + candidates.len() > self.max_restarts as usize {
            self.slots[dead].1.alive = false;
            return ReplacementDecision::Failed(SupervisionFailure {
                child: event.proxy,
                outcome: event.outcome,
                reason: SupervisionFailureReason::RestartDenied(RestartDenial::BudgetExceeded {
                    restarts_in_window: self.restarts.len(),
                    replacements_requested: candidates.len(),
                    maximum_restarts: self.max_restarts,
                }),
            });
        }
        self.restarts
            .resize(self.restarts.len() + candidates.len(), event.at);
        ReplacementDecision::Replace(
            candidates
                .into_iter()
                .map(|index| {
                    self.slots[index].1.alive = true;
                    Delivery::new(
                        Recipient::child(self.slots[index].0),
                        ProxyCommand::Replace((self.build)(index)),
                    )
                })
                .collect(),
        )
    }

    fn react_to_failure(
        &mut self,
        failure: &SupervisionFailure<B::Addr>,
    ) -> Result<Become<B::Addr, B::Ph>, B::Error> {
        Ok(match (self.on_failure)(&mut self.inner, failure)? {
            Step::Continue => Step::Continue,
            Step::Goto(never) => match never {},
            Step::Stop(exit) => Step::Stop(exit),
        })
    }

    fn wrap(
        &mut self,
        actions: Actions<B::Addr, B::Ph, B::Sends, Births<C>>,
    ) -> SupervisorActions<B, C> {
        let born: Vec<_> = actions.creates.iter().map(|create| create.nonce).collect();
        for create in &actions.creates {
            assert!(
                self.position(create.nonce).is_none(),
                "a child birth nonce must be fresh"
            );
            self.slots.push((
                create.nonce,
                Slot {
                    alive: true,
                    sequence: self.next_sequence,
                },
            ));
            self.next_sequence = self
                .next_sequence
                .checked_add(1)
                .expect("birth sequence exhausted");
        }
        Actions {
            sends: SendProduct {
                inner: actions.sends,
                own: SendProduct {
                    inner: ServiceSends::new(
                        born.into_iter()
                            .map(|nonce| ObserveChild { nonce })
                            .collect(),
                    ),
                    own: Vec::new(),
                },
            },
            creates: actions
                .creates
                .into_iter()
                .map(|create| Create {
                    nonce: create.nonce,
                    child: Proxy::new(create.child),
                    kind: create.kind,
                })
                .collect(),
            become_: actions.become_,
        }
    }
}

impl<B, C, A, Ph, Sends> Behavior for Supervising<B, C>
where
    A: Address + Send,
    Sends: SendAlgebra,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Births<C>> + Send,
    B::Event: ChildEvent<B::Addr> + Send,
    A::Nonce: From<u64> + Send,
    B::Msg: Send,
    C: Behavior<Ph = Never, Addr = B::Addr> + Send,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = SupervisionEvent<B::Event, B::Addr>;
    type Sends = SupervisorSends<A, Sends, C>;
    type Ph = Ph;
    type Error = B::Error;
    type Birth = Births<Proxy<C>>;

    async fn init(&mut self) -> Result<SupervisorActions<B, C>, B::Error> {
        let actions = self.inner.init().await?;
        let mut actions = self.wrap(actions);
        actions.creates.extend(
            self.slots[..self.configured_count]
                .iter()
                .enumerate()
                .map(|(index, (nonce, _))| Create::birth(*nonce, Proxy::new((self.build)(index)))),
        );
        actions.sends.own.inner.extend(
            self.slots[..self.configured_count]
                .iter()
                .map(|(nonce, _)| ObserveChild { nonce: *nonce }),
        );
        Ok(actions)
    }

    async fn step(&mut self, event: Self::Event) -> Result<SupervisorActions<B, C>, B::Error> {
        match event {
            SupervisionEvent::WorkerStopped(event) => {
                let decision = self.replacement_decision(&event);
                match decision {
                    ReplacementDecision::Retire => Ok(Actions::cont()),
                    ReplacementDecision::Replace(replacements) => Ok(Actions {
                        sends: SendProduct {
                            inner: B::Sends::empty(),
                            own: SendProduct {
                                inner: ServiceSends::empty(),
                                own: replacements,
                            },
                        },
                        creates: Vec::new(),
                        become_: Step::Continue,
                    }),
                    ReplacementDecision::Failed(failure) => {
                        Ok(Actions::just(self.react_to_failure(&failure)?))
                    }
                }
            }
            SupervisionEvent::ChildStopped(event) => {
                let dead = self
                    .position(event.nonce)
                    .expect("unknown supervised nonce");
                self.slots[dead].1.alive = false;
                let failure = SupervisionFailure {
                    child: event.nonce,
                    outcome: event.outcome,
                    reason: SupervisionFailureReason::StableChildStopped,
                };
                Ok(Actions::just(self.react_to_failure(&failure)?))
            }
            SupervisionEvent::Inner(event) => {
                let actions = self.inner.step(event).await?;
                Ok(self.wrap(actions))
            }
        }
    }
}
