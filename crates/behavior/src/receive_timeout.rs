//! Pure receive-inactivity composition. Relative scheduling is a request to
//! the emitting actor's interpreter and never observes a clock here.

use std::time::Duration;

use crate::Step;
use crate::behavior::{
    Actions, Address, Behavior, BirthMode, SendAlgebra, SendProduct, ServiceSends, UserEvent,
};
use crate::protocol::{
    ChildEvent, ChildStopped, PeerEvent, PeerStopped, ScheduleAfter, ShutdownEvent,
    ShutdownRequested, TimeEvent, TimerElapsed, TimerGeneration, TimerId, WorkerEvent,
    WorkerStopped,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveTimeoutEvent<E> {
    Inner(E),
    Elapsed(TimerElapsed),
}

impl<E> TimeEvent for ReceiveTimeoutEvent<E> {
    fn time_reached(event: TimerElapsed) -> Option<Self> {
        Some(Self::Elapsed(event))
    }
}

impl<E: UserEvent> UserEvent for ReceiveTimeoutEvent<E> {
    type Addr = E::Addr;
    type Message = E::Message;

    fn user(from: Self::Addr, message: Self::Message) -> Self {
        Self::Inner(E::user(from, message))
    }

    fn into_user(self) -> Result<crate::User<Self::Addr, Self::Message>, Self> {
        match self {
            Self::Inner(event) => event.into_user().map_err(Self::Inner),
            elapsed @ Self::Elapsed(_) => Err(elapsed),
        }
    }
}

impl<E: PeerEvent<A>, A: Address> PeerEvent<A> for ReceiveTimeoutEvent<E> {
    fn peer_stopped(event: PeerStopped<A>) -> Option<Self> {
        E::peer_stopped(event).map(Self::Inner)
    }
}

impl<E: ChildEvent<A>, A: Address> ChildEvent<A> for ReceiveTimeoutEvent<E> {
    fn child_stopped(event: ChildStopped<A>) -> Option<Self> {
        E::child_stopped(event).map(Self::Inner)
    }
}

impl<E: WorkerEvent<A>, A: Address> WorkerEvent<A> for ReceiveTimeoutEvent<E> {
    fn worker_stopped(event: WorkerStopped<A>) -> Option<Self> {
        E::worker_stopped(event).map(Self::Inner)
    }
}

impl<E: ShutdownEvent> ShutdownEvent for ReceiveTimeoutEvent<E> {
    fn shutdown_requested(event: ShutdownRequested) -> Option<Self> {
        E::shutdown_requested(event).map(Self::Inner)
    }
}

/// A controlled receive-timeout failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiveTimeoutError<E> {
    /// The inner fold or timeout reaction failed.
    Inner(E),
    /// Advancing the timer generation would make a stale delivery live again.
    GenerationExhausted,
}

pub type ReceiveTimeoutReaction<B> = fn(
    &mut B,
) -> Result<
    Actions<
        <B as Behavior>::Addr,
        <B as Behavior>::Ph,
        <B as Behavior>::Sends,
        <B as Behavior>::Birth,
    >,
    <B as Behavior>::Error,
>;

pub type ReceiveTimeoutSends<B> = SendProduct<<B as Behavior>::Sends, ServiceSends<ScheduleAfter>>;

pub type ReceiveTimeoutActions<B> = Actions<
    <B as Behavior>::Addr,
    <B as Behavior>::Ph,
    ReceiveTimeoutSends<B>,
    <B as Behavior>::Birth,
>;

/// A pure one-notification-per-idle-period receive timeout.
///
/// Only successful user communications are activity. Timer, peer, child,
/// worker, and shutdown service events compose through this wrapper but never
/// rearm it. A matching timeout consumes the live generation before invoking
/// the reaction; if that reaction continues, the timeout remains unarmed until
/// another successful continuing user communication.
pub struct ReceiveTimeout<B: Behavior> {
    inner: B,
    id: TimerId,
    after: Duration,
    live: Option<TimerGeneration>,
    last_issued: Option<TimerGeneration>,
    on_elapsed: ReceiveTimeoutReaction<B>,
}

impl<B: Behavior> ReceiveTimeout<B> {
    #[must_use]
    pub fn new(
        inner: B,
        id: TimerId,
        after: Duration,
        on_elapsed: ReceiveTimeoutReaction<B>,
    ) -> Self {
        Self {
            inner,
            id,
            after,
            live: None,
            last_issued: None,
            on_elapsed,
        }
    }

    #[must_use]
    pub fn inner(&self) -> &B {
        &self.inner
    }

    fn schedule(&mut self) -> Result<ServiceSends<ScheduleAfter>, ReceiveTimeoutError<B::Error>> {
        let generation = match self.last_issued {
            None => TimerGeneration(0),
            Some(TimerGeneration(generation)) => TimerGeneration(
                generation
                    .checked_add(1)
                    .ok_or(ReceiveTimeoutError::GenerationExhausted)?,
            ),
        };
        self.last_issued = Some(generation);
        self.live = Some(generation);
        Ok(ServiceSends::one(ScheduleAfter {
            id: self.id,
            generation,
            after: self.after,
        }))
    }

    fn wrap(
        actions: Actions<B::Addr, B::Ph, B::Sends, B::Birth>,
        own: ServiceSends<ScheduleAfter>,
    ) -> ReceiveTimeoutActions<B> {
        Actions {
            sends: SendProduct {
                inner: actions.sends,
                own,
            },
            creates: actions.creates,
            become_: actions.become_,
        }
    }

    fn terminal(actions: &Actions<B::Addr, B::Ph, B::Sends, B::Birth>) -> bool {
        matches!(actions.become_, Step::Stop(_))
    }
}

impl<B, A, Ph, Sends, Br> Behavior for ReceiveTimeout<B>
where
    A: Address + Send,
    Sends: SendAlgebra,
    Br: BirthMode,
    B: Behavior<Addr = A, Ph = Ph, Sends = Sends, Birth = Br> + Send,
    B::Event: TimeEvent + Send,
    B::Msg: Send,
{
    type Addr = A;
    type Msg = B::Msg;
    type Event = ReceiveTimeoutEvent<B::Event>;
    type Sends = ReceiveTimeoutSends<B>;
    type Ph = Ph;
    type Error = ReceiveTimeoutError<B::Error>;
    type Birth = Br;

    async fn init(&mut self) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        let actions = self
            .inner
            .init()
            .await
            .map_err(ReceiveTimeoutError::Inner)?;
        let own = if Self::terminal(&actions) {
            ServiceSends::empty()
        } else {
            self.schedule()?
        };
        Ok(Self::wrap(actions, own))
    }

    async fn step(&mut self, event: Self::Event) -> Result<ReceiveTimeoutActions<B>, Self::Error> {
        match event {
            ReceiveTimeoutEvent::Elapsed(elapsed)
                if elapsed.id == self.id && self.live == Some(elapsed.generation) =>
            {
                self.live = None;
                let actions =
                    (self.on_elapsed)(&mut self.inner).map_err(ReceiveTimeoutError::Inner)?;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Elapsed(elapsed) if elapsed.id == self.id => Ok(Actions::cont()),
            ReceiveTimeoutEvent::Elapsed(elapsed) => {
                let Some(inner) = B::Event::time_reached(elapsed) else {
                    return Ok(Actions::cont());
                };
                let actions = self
                    .inner
                    .step(inner)
                    .await
                    .map_err(ReceiveTimeoutError::Inner)?;
                Ok(Self::wrap(actions, ServiceSends::empty()))
            }
            ReceiveTimeoutEvent::Inner(event) => match event.into_user() {
                Ok(user) => {
                    let event = B::Event::user(user.from, user.message);
                    let actions = self
                        .inner
                        .step(event)
                        .await
                        .map_err(ReceiveTimeoutError::Inner)?;
                    let own = if Self::terminal(&actions) {
                        ServiceSends::empty()
                    } else {
                        self.schedule()?
                    };
                    Ok(Self::wrap(actions, own))
                }
                Err(service) => {
                    let actions = self
                        .inner
                        .step(service)
                        .await
                        .map_err(ReceiveTimeoutError::Inner)?;
                    Ok(Self::wrap(actions, ServiceSends::empty()))
                }
            },
        }
    }
}
