//! Bounded FIFO work admission over explicitly available workers.

use std::collections::VecDeque;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Never, NoBirths, Protocol,
    SendEffects, User,
};

use crate::DeliveryRoute;

/// Complete observable [`WorkQueue`] state.
pub struct WorkQueueState<WorkerRoute> {
    /// Maximum values that may wait; zero permits only immediate dispatch.
    pub capacity: usize,
    available: Vec<WorkerRoute>,
    queued: usize,
}

impl<WorkerRoute> WorkQueueState<WorkerRoute> {
    /// Workers eligible for one dispatch, in availability order.
    #[must_use]
    pub fn available(&self) -> &[WorkerRoute] {
        &self.available
    }

    /// Number of owned values waiting in FIFO order.
    #[must_use]
    pub fn queued(&self) -> usize {
        self.queued
    }
}

/// Exhaustive work-admission rejection reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkQueueRejection {
    /// No worker was available and waiting capacity was full.
    Full,
}

/// Factual result of one submission.
#[derive(Debug, PartialEq, Eq)]
pub enum WorkQueueOutcome<T> {
    /// Value is retained pending worker availability.
    Queued {
        /// Waiting depth after admission.
        depth: usize,
    },
    /// Value was assigned to an available worker.
    Dispatched {
        /// Waiting depth after dispatch.
        queued: usize,
    },
    /// Value was not admitted; ownership is returned.
    Rejected {
        /// Unaccepted value.
        value: T,
        /// Rejection reason.
        reason: WorkQueueRejection,
    },
}

/// Operations accepted by [`WorkQueue`].
pub enum WorkQueueMessage<T, WorkerRoute, ReplyRoute> {
    /// Submit one value and its typed outcome recipient.
    Submit {
        /// Work value.
        value: T,
        /// Outcome recipient retained with queued work.
        reply_to: ReplyRoute,
    },
    /// Announce one worker as eligible for exactly one dispatch.
    Available {
        /// Worker recipient.
        worker: WorkerRoute,
    },
    /// Withdraw one currently available worker, idempotently.
    Withdraw {
        /// Worker recipient.
        worker: WorkerRoute,
    },
}

/// Named effect lanes emitted by [`WorkQueue`].
pub struct WorkQueueSends<Assignments: SendEffects, OutcomeSends: SendEffects> {
    /// Work assigned to workers.
    pub assignments: Assignments,
    /// Submission admission and dispatch facts.
    pub outcomes: OutcomeSends,
}

impl<Assignments: SendEffects, OutcomeSends: SendEffects> SendEffects
    for WorkQueueSends<Assignments, OutcomeSends>
{
    fn empty() -> Self {
        Self {
            assignments: Assignments::empty(),
            outcomes: OutcomeSends::empty(),
        }
    }
    fn append(&mut self, other: Self) {
        self.assignments.append(other.assignments);
        self.outcomes.append(other.outcomes);
    }
}

impl<Event, Assignments, OutcomeSends> behavior::SendsFor<Event>
    for WorkQueueSends<Assignments, OutcomeSends>
where
    Assignments: SendEffects + behavior::SendsFor<Event>,
    OutcomeSends: SendEffects + behavior::SendsFor<Event>,
{
}

impl<I, RootEvent, Path, Assignments, OutcomeSends> behavior::InterpretSends<I, RootEvent, Path>
    for WorkQueueSends<Assignments, OutcomeSends>
where
    I: behavior::SendInterpreter,
    Assignments: SendEffects + behavior::InterpretSends<I, RootEvent, Path>,
    OutcomeSends: SendEffects,
    OutcomeSends: SendEffects + behavior::InterpretSends<I, RootEvent, Path>,
    WorkQueueSends<Assignments, OutcomeSends>: Send,
{
    fn interpret(
        self,
        interpreter: &mut I,
    ) -> impl core::future::Future<Output = Result<(), I::Error>> + Send {
        async move {
            behavior::InterpretSends::interpret(self.assignments, interpreter).await?;
            behavior::InterpretSends::interpret(self.outcomes, interpreter).await
        }
    }
}

struct Waiting<T, Route> {
    value: T,
    reply_to: Route,
}

/// Bounded FIFO admission and worker-availability behavior.
///
/// Availability is a one-dispatch capability. It immediately consumes the
/// oldest waiting value or joins a unique FIFO of available workers. Submission
/// consumes the oldest worker or enters the bounded FIFO; at capacity it
/// returns the owned value. Duplicate availability and withdrawal are
/// idempotent. Initialization is empty, no actors are created, and the host
/// never terminates by policy. FIFO selection and bounded admission are Bombay
/// policy. Worker execution, mailbox admission, and physical backpressure are
/// runtime responsibilities. No transition has a semantic panic condition.
pub struct WorkQueue<
    A: Address,
    T,
    WorkerRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>> + Clone + PartialEq,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = WorkQueueOutcome<T>>>,
> {
    capacity: usize,
    available: VecDeque<WorkerRoute>,
    waiting: VecDeque<Waiting<T, ReplyRoute>>,
    marker: core::marker::PhantomData<fn() -> A>,
}

type QueueActions<A, Assignments, OutcomeSends> =
    Actions<A, Never, WorkQueueSends<Assignments, OutcomeSends>, NoBirths>;

impl<A, T, WorkerRoute, ReplyRoute> WorkQueue<A, T, WorkerRoute, ReplyRoute>
where
    A: Address,
    WorkerRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>> + Clone + PartialEq,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = WorkQueueOutcome<T>>> + Clone,
{
    /// Construct an empty queue with explicit waiting capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            available: VecDeque::new(),
            waiting: VecDeque::with_capacity(capacity),
            marker: core::marker::PhantomData,
        }
    }
    /// Return complete observable queue state.
    #[must_use]
    pub fn state(&self) -> WorkQueueState<WorkerRoute> {
        WorkQueueState {
            capacity: self.capacity,
            available: self.available.iter().cloned().collect(),
            queued: self.waiting.len(),
        }
    }
    fn sends(
        assignments: WorkerRoute::Sends,
        outcomes: ReplyRoute::Sends,
    ) -> QueueActions<A, WorkerRoute::Sends, ReplyRoute::Sends> {
        Actions::send(WorkQueueSends {
            assignments,
            outcomes,
        })
    }
    fn submit(
        &mut self,
        value: T,
        reply_to: ReplyRoute,
    ) -> QueueActions<A, WorkerRoute::Sends, ReplyRoute::Sends> {
        if let Some(worker) = self.available.pop_front() {
            return Self::sends(
                worker.deliver(value),
                reply_to.deliver(WorkQueueOutcome::Dispatched {
                    queued: self.waiting.len(),
                }),
            );
        }
        if self.waiting.len() == self.capacity {
            return Self::sends(
                WorkerRoute::Sends::empty(),
                reply_to.deliver(WorkQueueOutcome::Rejected {
                    value,
                    reason: WorkQueueRejection::Full,
                }),
            );
        }
        self.waiting.push_back(Waiting {
            value,
            reply_to: reply_to.clone(),
        });
        Self::sends(
            WorkerRoute::Sends::empty(),
            reply_to.deliver(WorkQueueOutcome::Queued {
                depth: self.waiting.len(),
            }),
        )
    }
    fn announce(
        &mut self,
        worker: WorkerRoute,
    ) -> QueueActions<A, WorkerRoute::Sends, ReplyRoute::Sends> {
        if let Some(waiting) = self.waiting.pop_front() {
            return Self::sends(
                worker.deliver(waiting.value),
                waiting.reply_to.deliver(WorkQueueOutcome::Dispatched {
                    queued: self.waiting.len(),
                }),
            );
        }
        if !self.available.contains(&worker) {
            self.available.push_back(worker);
        }
        Actions::cont()
    }
}

impl<A, T, WorkerRoute, ReplyRoute> BehaviorBase for WorkQueue<A, T, WorkerRoute, ReplyRoute>
where
    A: Address,
    WorkerRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>> + Clone + PartialEq,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = WorkQueueOutcome<T>>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, T, WorkerRoute, ReplyRoute> behavior::Protocol for WorkQueue<A, T, WorkerRoute, ReplyRoute>
where
    A: Address,
    WorkerRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>> + Clone + PartialEq,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = WorkQueueOutcome<T>>>,
{
    type Addr = A;
    type Msg = WorkQueueMessage<T, WorkerRoute, ReplyRoute>;
}

impl<A, T, WorkerRoute, ReplyRoute> Behavior for WorkQueue<A, T, WorkerRoute, ReplyRoute>
where
    A: Address,
    WorkerRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = T>> + Clone + PartialEq,
    ReplyRoute: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = WorkQueueOutcome<T>>> + Clone,
    WorkerRoute::Sends: behavior::SendsFor<User<A, WorkQueueMessage<T, WorkerRoute, ReplyRoute>>>,
    ReplyRoute::Sends: behavior::SendsFor<User<A, WorkQueueMessage<T, WorkerRoute, ReplyRoute>>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = WorkQueueSends<WorkerRoute::Sends, ReplyRoute::Sends>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;
    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        Ok(match event.message {
            WorkQueueMessage::Submit { value, reply_to } => self.submit(value, reply_to),
            WorkQueueMessage::Available { worker } => self.announce(worker),
            WorkQueueMessage::Withdraw { worker } => {
                if let Some(index) = self
                    .available
                    .iter()
                    .position(|candidate| candidate == &worker)
                {
                    self.available.remove(index);
                }
                Actions::cont()
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Activate as _, Recipient};
    use behavior::MailAddr;
    struct Worker;
    struct Reply;
    impl behavior::Protocol for Worker {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Worker {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = WorkQueueOutcome<u8>;
    }

    impl Behavior for Reply {
        type Protocol = Self;
        type Event = User<MailAddr, crate::BehaviorMessage<Self>>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = WorkQueue<MailAddr, u8, Recipient<Worker>, Recipient<Reply>>;
    fn worker(n: u64) -> Recipient<Worker> {
        Recipient::global(MailAddr(n))
    }
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(9))
    }
    #[test]
    fn availability_and_waiting_are_fifo() {
        let mut s = (Subject::new(2)).initialize().unwrap().behavior;
        for w in [worker(1), worker(2)] {
            let available = s
                .receive(MailAddr(0), WorkQueueMessage::Available { worker: w })
                .unwrap();
            assert!(available.sends.assignments.is_empty());
            assert!(available.sends.outcomes.is_empty());
            assert!(available.creates.is_empty());
            assert_eq!(available.become_, crate::Step::Continue);
        }
        for value in [10, 20, 30] {
            let a = s
                .receive(
                    MailAddr(0),
                    WorkQueueMessage::Submit {
                        value,
                        reply_to: reply(),
                    },
                )
                .unwrap();
            if value < 30 {
                assert_eq!(a.sends.assignments[0].message, value);
            }
        }
        assert_eq!(s.state().queued(), 1);
        let a = s
            .receive(
                MailAddr(0),
                WorkQueueMessage::Available { worker: worker(1) },
            )
            .unwrap();
        assert_eq!(a.sends.assignments[0].message, 30);
    }
    #[test]
    fn zero_capacity_returns_unaccepted_value() {
        let mut s = (Subject::new(0)).initialize().unwrap().behavior;
        let a = s
            .receive(
                MailAddr(0),
                WorkQueueMessage::Submit {
                    value: 7,
                    reply_to: reply(),
                },
            )
            .unwrap();
        assert!(matches!(
            a.sends.outcomes[0].message,
            WorkQueueOutcome::Rejected {
                value: 7,
                reason: WorkQueueRejection::Full
            }
        ));
        assert_eq!(s.state().queued(), 0);
    }
}
