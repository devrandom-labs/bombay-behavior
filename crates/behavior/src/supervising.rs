//! Pure supervision. Stable child identity is a proxy actor; replacement is a
//! message to that proxy, which creates a fresh worker incarnation.

use std::time::Duration;

use tokio::time::Instant;

use crate::behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, SendProduct,
    ServiceSends, User, UserEvent,
};
use crate::deadlined::{TimeEvent, TimeReached};
use crate::verdict::{Never, Step};
use crate::watching::{PeerEvent, PeerStopped};
use crate::{Crash, Exit};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildStopped<A: Address> {
    pub nonce: A::Nonce,
    pub outcome: Result<Exit<A>, Crash>,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObserveChild<A: Address> {
    pub nonce: A::Nonce,
}

#[derive(Clone, PartialEq, Eq)]
pub enum SupervisionEvent<E, A: Address> {
    Inner(E),
    ChildStopped(ChildStopped<A>),
}

pub trait ChildEvent<A: Address>: Sized {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self>;
}

impl<E, A: Address> ChildEvent<A> for SupervisionEvent<E, A> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        Some(Self::ChildStopped(event))
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
            stopped @ Self::ChildStopped(_) => Err(stopped),
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

#[derive(Debug)]
pub enum ProxyCommand<C: Behavior> {
    Forward(C::Msg),
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
pub struct Proxy<C: Behavior<Ph = Never>> {
    worker: Option<C>,
    generation: u64,
}

impl<C: Behavior<Ph = Never>> Proxy<C> {
    #[must_use]
    pub fn new(worker: C) -> Self {
        Self {
            worker: Some(worker),
            generation: 0,
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
    type Event = User<C::Addr, ProxyCommand<C>>;
    type Sends = Vec<Delivery<C::Addr, C::Msg>>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<C>;
    type Effect = Actions<C::Addr, Never, Self::Sends, Births<C>>;
    type Done = Exit<C::Addr>;

    async fn init(&mut self) -> Result<Self::Effect, Never> {
        let child = self.worker.take().expect("a proxy initializes once");
        Ok(Actions {
            sends: Vec::new(),
            creates: vec![Create {
                nonce: <C::Addr as Address>::Nonce::from(self.generation),
                child,
            }],
            become_: Step::Continue,
        })
    }

    async fn step(&mut self, event: Self::Event) -> Result<Self::Effect, Never> {
        match event.message {
            ProxyCommand::Forward(message) => Ok(Actions {
                sends: vec![Delivery::new(
                    Recipient::child(<C::Addr as Address>::Nonce::from(self.generation)),
                    message,
                )],
                creates: Vec::new(),
                become_: Step::Continue,
            }),
            ProxyCommand::Replace(child) => {
                self.generation = self
                    .generation
                    .checked_add(1)
                    .expect("proxy generation exhausted");
                Ok(Actions {
                    sends: Vec::new(),
                    creates: vec![Create {
                        nonce: <C::Addr as Address>::Nonce::from(self.generation),
                        child,
                    }],
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

    fn replacements(
        &mut self,
        event: &ChildStopped<B::Addr>,
    ) -> Vec<Delivery<B::Addr, ProxyCommand<C>>> {
        let dead = self
            .position(event.nonce)
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
            return Vec::new();
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
            return Vec::new();
        }
        self.restarts
            .resize(self.restarts.len() + candidates.len(), event.at);
        candidates
            .into_iter()
            .map(|index| {
                self.slots[index].1.alive = true;
                Delivery::new(
                    Recipient::child(self.slots[index].0),
                    ProxyCommand::Replace((self.build)(index)),
                )
            })
            .collect()
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
    B: Behavior<
            Addr = A,
            Ph = Ph,
            Sends = Sends,
            Birth = Births<C>,
            Effect = Actions<A, Ph, Sends, Births<C>>,
            Done = Exit<A>,
        > + Send,
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
    type Effect = Actions<A, Ph, Self::Sends, Births<Proxy<C>>>;
    type Done = Exit<A>;

    async fn init(&mut self) -> Result<Self::Effect, B::Error> {
        let actions = self.inner.init().await?;
        let mut actions = self.wrap(actions);
        actions
            .creates
            .extend(self.slots[..self.configured_count].iter().enumerate().map(
                |(index, (nonce, _))| Create {
                    nonce: *nonce,
                    child: Proxy::new((self.build)(index)),
                },
            ));
        actions.sends.own.inner.extend(
            self.slots[..self.configured_count]
                .iter()
                .map(|(nonce, _)| ObserveChild { nonce: *nonce }),
        );
        Ok(actions)
    }

    async fn step(&mut self, event: Self::Event) -> Result<Self::Effect, B::Error> {
        match event {
            SupervisionEvent::ChildStopped(event) => Ok(Actions {
                sends: SendProduct {
                    inner: B::Sends::empty(),
                    own: SendProduct {
                        inner: ServiceSends::empty(),
                        own: self.replacements(&event),
                    },
                },
                creates: Vec::new(),
                become_: Step::Continue,
            }),
            SupervisionEvent::Inner(event) => {
                let actions = self.inner.step(event).await?;
                Ok(self.wrap(actions))
            }
        }
    }
}
