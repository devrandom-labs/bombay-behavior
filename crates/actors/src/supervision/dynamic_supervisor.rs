//! Explicitly managed dynamic stable-child topology.

use crate::{
    ChildShutdownRejected, ChildStopped, CreationRejection, CreationResolved, ObserveChild,
    ObserveCreation, Own, Proxy, ProxyCommand, SendInput, ShutdownChild, WorkerCreationResolved,
    WorkerStopped,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, Births, Create, Delivery, Never, Recipient,
    SendAlgebra, ServiceSends, User, UserEvent,
};

/// One dynamically managed stable-child phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicChildPhase {
    /// The stable proxy creation has been staged but not committed.
    Installing,
    /// The stable proxy is established and accepts management commands.
    ///
    /// This describes the proxy slot, not one worker incarnation behind it.
    Available,
    /// Orderly shutdown of the stable proxy has been accepted.
    Stopping,
    /// The stable proxy is realizing a fresh worker incarnation.
    Replacing,
    /// The stable proxy slot has terminated or failed installation.
    Retired,
}

/// Admission rejection produced without consuming an existing child slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicSupervisorRejection {
    AlreadyExists,
    NotAvailable,
    NotFound,
}

/// Commands for an explicitly managed dynamic child set.
pub enum DynamicSupervisorMessage<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    Start {
        nonce: A::Nonce,
        child: C,
        reply_to: Recipient<Reply>,
    },
    Stop {
        nonce: A::Nonce,
        reply_to: Recipient<Reply>,
    },
    Replace {
        nonce: A::Nonce,
        child: C,
        reply_to: Recipient<Reply>,
    },
    Query {
        nonce: A::Nonce,
        reply_to: Recipient<Reply>,
    },
}

/// Complete command or realization outcome returned to a typed recipient.
pub enum DynamicSupervisorOutcome<N, C> {
    StartAccepted {
        nonce: N,
    },
    StartRejected {
        nonce: N,
        child: C,
        reason: DynamicSupervisorRejection,
    },
    Started {
        nonce: N,
    },
    StartFailed {
        nonce: N,
        reason: CreationRejection,
    },
    StopAccepted {
        nonce: N,
    },
    StopRejected {
        nonce: N,
        reason: DynamicSupervisorRejection,
    },
    Stopped {
        nonce: N,
    },
    ReplaceAccepted {
        nonce: N,
    },
    ReplaceRejected {
        nonce: N,
        child: C,
        reason: DynamicSupervisorRejection,
    },
    Replaced {
        nonce: N,
    },
    ReplacementFailed {
        nonce: N,
        reason: CreationRejection,
    },
    State {
        nonce: N,
        phase: Option<DynamicChildPhase>,
    },
}

enum DynamicChild<R: Behavior> {
    Installing { reply_to: Recipient<R> },
    Available,
    Stopping { reply_to: Recipient<R> },
    Replacing { reply_to: Recipient<R> },
    Retired,
}

impl<R: Behavior> DynamicChild<R> {
    const fn phase(&self) -> DynamicChildPhase {
        match self {
            Self::Installing { .. } => DynamicChildPhase::Installing,
            Self::Available => DynamicChildPhase::Available,
            Self::Stopping { .. } => DynamicChildPhase::Stopping,
            Self::Replacing { .. } => DynamicChildPhase::Replacing,
            Self::Retired => DynamicChildPhase::Retired,
        }
    }
}

/// Runtime facts and user commands accepted by [`DynamicSupervisor`].
pub enum DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    Command(User<A, DynamicSupervisorMessage<A, C, Reply>>),
    ChildStopped(ChildStopped<A>),
    CreationResolved(CreationResolved<A::Nonce>),
    WorkerCreationResolved(WorkerCreationResolved<A::Nonce>),
    WorkerStopped(WorkerStopped<A>),
    ChildShutdownRejected(ChildShutdownRejected<A::Nonce>),
}

impl<A, C, Reply> UserEvent for DynamicSupervisorEvent<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    type Addr = A;
    type Message = DynamicSupervisorMessage<A, C, Reply>;
    fn user(from: A, message: Self::Message) -> Self {
        Self::Command(User::new(from, message))
    }
    fn into_user(self) -> Result<User<A, Self::Message>, Self> {
        match self {
            Self::Command(user) => Ok(user),
            other => Err(other),
        }
    }
}

macro_rules! event_lane {
    ($ty:ty, $variant:ident) => {
        impl<A, C, Reply> crate::RouteInput<$ty> for DynamicSupervisorEvent<A, C, Reply>
        where
            A: Address,
            C: Behavior<Addr = A>,
            Reply: Behavior<Addr = A>,
        {
            fn route(value: $ty) -> Result<Self, $ty> {
                Ok(Self::$variant(value))
            }
        }
        impl<A, C, Reply> crate::EventInput<$ty> for DynamicSupervisorEvent<A, C, Reply>
        where
            A: Address,
            C: Behavior<Addr = A>,
            Reply: Behavior<Addr = A>,
        {
            fn inject(value: $ty) -> Self {
                Self::$variant(value)
            }
        }
    };
}
event_lane!(ChildStopped<A>, ChildStopped);
event_lane!(CreationResolved<A::Nonce>, CreationResolved);
event_lane!(WorkerCreationResolved<A::Nonce>, WorkerCreationResolved);
event_lane!(WorkerStopped<A>, WorkerStopped);
event_lane!(ChildShutdownRejected<A::Nonce>, ChildShutdownRejected);

/// Shutdown-capable stable proxy protocol installed by [`DynamicSupervisor`].
///
/// The proxy makes orderly subtree shutdown an explicit part of its concrete
/// event and effect products while preserving stable-address construction.
pub type DynamicProxy<C> = Proxy<C>;

/// Named effect product for dynamic topology management.
pub struct DynamicSupervisorSends<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A>,
{
    pub outcomes: Vec<Delivery<Reply>>,
    pub child_observations: ServiceSends<ObserveChild<A::Nonce>>,
    pub creation_observations: ServiceSends<ObserveCreation<A::Nonce>>,
    pub shutdowns: ServiceSends<ShutdownChild<DynamicProxy<C>>>,
    pub replacements: Vec<Delivery<DynamicProxy<C>>>,
}

impl<A, C, Reply> SendAlgebra for DynamicSupervisorSends<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A>,
{
    fn empty() -> Self {
        Self {
            outcomes: vec![],
            child_observations: ServiceSends::empty(),
            creation_observations: ServiceSends::empty(),
            shutdowns: ServiceSends::empty(),
            replacements: vec![],
        }
    }
    fn append(&mut self, other: Self) {
        self.outcomes.extend(other.outcomes);
        self.child_observations.append(other.child_observations);
        self.creation_observations
            .append(other.creation_observations);
        self.shutdowns.append(other.shutdowns);
        self.replacements.extend(other.replacements);
    }
}
impl<A, C, Reply> SendInput<ObserveChild<A::Nonce>, Own> for DynamicSupervisorSends<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A>,
{
    fn emit(&mut self, value: ObserveChild<A::Nonce>) {
        self.child_observations.send(value);
    }
}
impl<A, C, Reply> SendInput<ObserveCreation<A::Nonce>, Own> for DynamicSupervisorSends<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A>,
{
    fn emit(&mut self, value: ObserveCreation<A::Nonce>) {
        self.creation_observations.send(value);
    }
}
impl<A, C, Reply> SendInput<ShutdownChild<DynamicProxy<C>>, Own>
    for DynamicSupervisorSends<A, C, Reply>
where
    A: Address,
    A::Nonce: From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A>,
{
    fn emit(&mut self, value: ShutdownChild<DynamicProxy<C>>) {
        self.shutdowns.send(value);
    }
}

/// A pure dynamic supervisor whose stable proxy set changes only through its
/// typed command protocol and committed runtime facts.
pub struct DynamicSupervisor<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    children: Vec<(A::Nonce, DynamicChild<Reply>)>,
    marker: core::marker::PhantomData<fn() -> C>,
}

impl<A, C, Reply> DynamicSupervisor<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    #[must_use]
    pub const fn new() -> Self {
        Self {
            children: vec![],
            marker: core::marker::PhantomData,
        }
    }
    #[must_use]
    pub fn phase(&self, nonce: A::Nonce) -> Option<DynamicChildPhase>
    where
        A::Nonce: Eq,
    {
        self.children
            .iter()
            .find(|(n, _)| *n == nonce)
            .map(|(_, s)| s.phase())
    }
}
impl<A, C, Reply> Default for DynamicSupervisor<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    fn default() -> Self {
        Self::new()
    }
}
impl<A, C, Reply> crate::BehaviorBase for DynamicSupervisor<A, C, Reply>
where
    A: Address,
    C: Behavior<Addr = A>,
    Reply: Behavior<Addr = A>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}

impl<A, C, Reply> Behavior for DynamicSupervisor<A, C, Reply>
where
    A: Address,
    A::Nonce: Copy + Eq + From<u64>,
    C: Behavior<Addr = A, Ph = Never>,
    Reply: Behavior<Addr = A, Msg = DynamicSupervisorOutcome<A::Nonce, C>>,
{
    type Addr = A;
    type Msg = DynamicSupervisorMessage<A, C, Reply>;
    type Event = DynamicSupervisorEvent<A, C, Reply>;
    type Sends = DynamicSupervisorSends<A, C, Reply>;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<DynamicProxy<C>>;

    #[allow(clippy::too_many_lines)]
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        let mut sends = Self::Sends::empty();
        match event {
            DynamicSupervisorEvent::Command(user) => match user.message {
                DynamicSupervisorMessage::Start {
                    nonce,
                    child,
                    reply_to,
                } => {
                    if self
                        .children
                        .iter()
                        .any(|(n, s)| *n == nonce && !matches!(s, DynamicChild::Retired))
                    {
                        sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StartRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::AlreadyExists,
                            },
                        ));
                        return Ok(Actions::send(sends));
                    }
                    if let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        *state = DynamicChild::Installing { reply_to };
                    } else {
                        self.children
                            .push((nonce, DynamicChild::Installing { reply_to }));
                    }
                    sends.outcomes.push(Delivery::new(
                        reply_to,
                        DynamicSupervisorOutcome::StartAccepted { nonce },
                    ));
                    sends.child_observations.send(ObserveChild::new(nonce));
                    sends
                        .creation_observations
                        .send(ObserveCreation::new(nonce));
                    Ok(Actions::new(
                        sends,
                        vec![Create::birth(nonce, Proxy::new(child))],
                        crate::Step::Continue,
                    ))
                }
                DynamicSupervisorMessage::Stop { nonce, reply_to } => {
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state @ DynamicChild::Available)) => {
                            *state = DynamicChild::Stopping { reply_to };
                            sends
                                .shutdowns
                                .send(ShutdownChild::<DynamicProxy<C>>::new(nonce));
                            sends.outcomes.push(Delivery::new(
                                reply_to,
                                DynamicSupervisorOutcome::StopAccepted { nonce },
                            ));
                        }
                        Some(_) => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::NotAvailable,
                            },
                        )),
                        None => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::StopRejected {
                                nonce,
                                reason: DynamicSupervisorRejection::NotFound,
                            },
                        )),
                    }
                    Ok(Actions::send(sends))
                }
                DynamicSupervisorMessage::Replace {
                    nonce,
                    child,
                    reply_to,
                } => {
                    match self.children.iter_mut().find(|(n, _)| *n == nonce) {
                        Some((_, state @ DynamicChild::Available)) => {
                            *state = DynamicChild::Replacing { reply_to };
                            sends.replacements.push(Delivery::new(
                                Recipient::child(nonce),
                                ProxyCommand::Replace(child),
                            ));
                            sends.outcomes.push(Delivery::new(
                                reply_to,
                                DynamicSupervisorOutcome::ReplaceAccepted { nonce },
                            ));
                        }
                        Some(_) => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::NotAvailable,
                            },
                        )),
                        None => sends.outcomes.push(Delivery::new(
                            reply_to,
                            DynamicSupervisorOutcome::ReplaceRejected {
                                nonce,
                                child,
                                reason: DynamicSupervisorRejection::NotFound,
                            },
                        )),
                    }
                    Ok(Actions::send(sends))
                }
                DynamicSupervisorMessage::Query { nonce, reply_to } => {
                    sends.outcomes.push(Delivery::new(
                        reply_to,
                        DynamicSupervisorOutcome::State {
                            nonce,
                            phase: self.phase(nonce),
                        },
                    ));
                    Ok(Actions::send(sends))
                }
            },
            DynamicSupervisorEvent::CreationResolved(resolved) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == resolved.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Installing { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                match resolved.result {
                    Ok(()) => {
                        *state = DynamicChild::Available;
                        sends.outcomes.push(Delivery::new(
                            reply,
                            DynamicSupervisorOutcome::Started {
                                nonce: resolved.nonce,
                            },
                        ));
                    }
                    Err(reason) => {
                        *state = DynamicChild::Retired;
                        sends.outcomes.push(Delivery::new(
                            reply,
                            DynamicSupervisorOutcome::StartFailed {
                                nonce: resolved.nonce,
                                reason,
                            },
                        ));
                    }
                }
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::ChildStopped(stopped) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == stopped.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Stopping { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                *state = DynamicChild::Retired;
                sends.outcomes.push(Delivery::new(
                    reply,
                    DynamicSupervisorOutcome::Stopped {
                        nonce: stopped.nonce,
                    },
                ));
                Ok(Actions::send(sends))
            }
            DynamicSupervisorEvent::WorkerCreationResolved(resolved) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == resolved.proxy)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Replacing { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                *state = DynamicChild::Available;
                let outcome = match resolved.result {
                    Ok(()) => DynamicSupervisorOutcome::Replaced {
                        nonce: resolved.proxy,
                    },
                    Err(reason) => DynamicSupervisorOutcome::ReplacementFailed {
                        nonce: resolved.proxy,
                        reason,
                    },
                };
                sends.outcomes.push(Delivery::new(reply, outcome));
                Ok(Actions::send(sends))
            }
            // The phase describes availability of the stable proxy slot, not
            // liveness of one worker incarnation behind it. Replacement
            // realization remains the separate `WorkerCreationResolved` fact.
            DynamicSupervisorEvent::WorkerStopped(_) => Ok(Actions::cont()),
            DynamicSupervisorEvent::ChildShutdownRejected(rejected) => {
                let Some((_, state)) = self.children.iter_mut().find(|(n, _)| *n == rejected.nonce)
                else {
                    return Ok(Actions::cont());
                };
                let DynamicChild::Stopping { reply_to } = state else {
                    return Ok(Actions::cont());
                };
                let reply = *reply_to;
                *state = DynamicChild::Available;
                sends.outcomes.push(Delivery::new(
                    reply,
                    DynamicSupervisorOutcome::StopRejected {
                        nonce: rejected.nonce,
                        reason: DynamicSupervisorRejection::NotAvailable,
                    },
                ));
                Ok(Actions::send(sends))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{Activate as _, CreationKind, Exit};
    use behavior::{MailAddr, NoBirths};

    struct Worker;
    impl Behavior for Worker {
        type Addr = MailAddr;
        type Msg = u8;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    struct Reply;
    impl Behavior for Reply {
        type Addr = MailAddr;
        type Msg = DynamicSupervisorOutcome<u64, Worker>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }

    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(99))
    }

    #[test]
    fn start_is_distinct_from_committed_installation_and_duplicate_returns_child() {
        let initialized = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        let accepted = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Installing));
        assert_eq!(accepted.creates.len(), 1);
        assert_eq!(
            accepted.sends.child_observations.as_slice(),
            [ObserveChild::new(7)]
        );
        assert_eq!(
            accepted.sends.creation_observations.as_slice(),
            [ObserveCreation::new(7)]
        );
        assert!(matches!(
            accepted.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartAccepted { nonce: 7 }
        ));

        let duplicate = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 7,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(duplicate.creates.is_empty());
        assert!(duplicate.sends.child_observations.is_empty());
        assert!(duplicate.sends.creation_observations.is_empty());
        assert!(matches!(
            duplicate.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartRejected {
                nonce: 7,
                reason: DynamicSupervisorRejection::AlreadyExists,
                ..
            }
        ));

        let installed = active
            .on(CreationResolved::installed(7, CreationKind::Birth))
            .unwrap();
        assert_eq!(active.phase(7), Some(DynamicChildPhase::Available));
        assert!(matches!(
            installed.sends.outcomes[0].message,
            DynamicSupervisorOutcome::Started { nonce: 7 }
        ));
    }

    #[test]
    fn rejected_start_resolution_retires_the_slot_without_losing_the_observation_request() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        let start = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 5,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(
            start.sends.child_observations.as_slice(),
            [ObserveChild::new(5)]
        );
        assert_eq!(
            start.sends.creation_observations.as_slice(),
            [ObserveCreation::new(5)]
        );

        let rejected = active
            .on(CreationResolved::rejected(
                5,
                CreationKind::Birth,
                CreationRejection::EnvironmentFailed,
            ))
            .unwrap();
        assert_eq!(active.phase(5), Some(DynamicChildPhase::Retired));
        assert!(matches!(
            rejected.sends.outcomes[0].message,
            DynamicSupervisorOutcome::StartFailed {
                nonce: 5,
                reason: CreationRejection::EnvironmentFailed,
            }
        ));
    }

    #[test]
    fn stop_and_replace_wait_for_their_exact_runtime_fact() {
        let mut active = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap()
            .behavior;
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        active.on(CreationResolved::birth(3)).unwrap();

        let replacing = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Replace {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(replacing.sends.replacements.len(), 1);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Replacing));
        active
            .on(WorkerCreationResolved::new(
                3,
                4,
                CreationKind::replacement_of(2),
                Ok(()),
            ))
            .unwrap();
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));

        let stopping = active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Stop {
                    nonce: 3,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert_eq!(stopping.sends.shutdowns.as_slice(), [ShutdownChild::new(3)]);
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Stopping));
        active
            .on(ChildStopped::new(3, Ok(Exit::Normal), Instant::now()))
            .unwrap();
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Retired));
    }

    #[test]
    fn worker_stop_is_accepted_without_reinterpreting_proxy_slot_state() {
        let initialized = DynamicSupervisor::<MailAddr, Worker, Reply>::new()
            .initialize()
            .unwrap();
        let mut active = initialized.behavior;
        active
            .receive(
                MailAddr(1),
                DynamicSupervisorMessage::Start {
                    nonce: 3,
                    child: Worker,
                    reply_to: reply(),
                },
            )
            .unwrap();
        active.on(CreationResolved::birth(3)).unwrap();
        let actions = active
            .on(WorkerStopped::new(3, 0, Ok(Exit::Normal), Instant::now()))
            .unwrap();

        assert!(actions.sends.outcomes.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, crate::Step::Continue));
        assert_eq!(active.phase(3), Some(DynamicChildPhase::Available));
    }
}
