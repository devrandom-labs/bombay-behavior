//! Bounded FIFO work admission over explicitly available workers.

use std::collections::VecDeque;

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    SendAlgebra, User,
};

/// Complete observable [`WorkQueue`] state.
pub struct WorkQueueState<W: Behavior> {
    /// Maximum values that may wait; zero permits only immediate dispatch.
    pub capacity: usize,
    /// Workers eligible for one dispatch, in availability order.
    pub available: Vec<Recipient<W>>,
    /// Number of owned values waiting in FIFO order.
    pub queued: usize,
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
pub enum WorkQueueMessage<T, W: Behavior, Reply: Behavior> {
    /// Submit one value and its typed outcome recipient.
    Submit {
        /// Work value.
        value: T,
        /// Outcome recipient retained with queued work.
        reply_to: Recipient<Reply>,
    },
    /// Announce one worker as eligible for exactly one dispatch.
    Available {
        /// Worker recipient.
        worker: Recipient<W>,
    },
    /// Withdraw one currently available worker, idempotently.
    Withdraw {
        /// Worker recipient.
        worker: Recipient<W>,
    },
}

/// Named effect lanes emitted by [`WorkQueue`].
pub struct WorkQueueSends<W: Behavior, Reply: Behavior> {
    /// Work assigned to workers.
    pub assignments: Vec<Delivery<W>>,
    /// Submission admission and dispatch facts.
    pub outcomes: Vec<Delivery<Reply>>,
}

impl<W: Behavior, Reply: Behavior> SendAlgebra for WorkQueueSends<W, Reply> {
    fn empty() -> Self {
        Self {
            assignments: Vec::new(),
            outcomes: Vec::new(),
        }
    }
    fn append(&mut self, mut other: Self) {
        self.assignments.append(&mut other.assignments);
        self.outcomes.append(&mut other.outcomes);
    }
}

struct Waiting<T, Reply: Behavior> {
    value: T,
    reply_to: Recipient<Reply>,
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
    W: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = WorkQueueOutcome<T>>,
> {
    capacity: usize,
    available: VecDeque<Recipient<W>>,
    waiting: VecDeque<Waiting<T, Reply>>,
    marker: core::marker::PhantomData<fn() -> A>,
}

type QueueActions<A, W, Reply> = Actions<A, Never, WorkQueueSends<W, Reply>, NoBirths>;

impl<A, T, W, Reply> WorkQueue<A, T, W, Reply>
where
    A: Address,
    W: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = WorkQueueOutcome<T>>,
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
    pub fn state(&self) -> WorkQueueState<W> {
        WorkQueueState {
            capacity: self.capacity,
            available: self.available.iter().copied().collect(),
            queued: self.waiting.len(),
        }
    }
    fn sends(
        assignments: Vec<Delivery<W>>,
        outcomes: Vec<Delivery<Reply>>,
    ) -> QueueActions<A, W, Reply> {
        Actions::send(WorkQueueSends {
            assignments,
            outcomes,
        })
    }
    fn submit(&mut self, value: T, reply_to: Recipient<Reply>) -> QueueActions<A, W, Reply> {
        if let Some(worker) = self.available.pop_front() {
            return Self::sends(
                vec![Delivery::new(worker, value)],
                vec![Delivery::new(
                    reply_to,
                    WorkQueueOutcome::Dispatched {
                        queued: self.waiting.len(),
                    },
                )],
            );
        }
        if self.waiting.len() == self.capacity {
            return Self::sends(
                Vec::new(),
                vec![Delivery::new(
                    reply_to,
                    WorkQueueOutcome::Rejected {
                        value,
                        reason: WorkQueueRejection::Full,
                    },
                )],
            );
        }
        self.waiting.push_back(Waiting { value, reply_to });
        Self::sends(
            Vec::new(),
            vec![Delivery::new(
                reply_to,
                WorkQueueOutcome::Queued {
                    depth: self.waiting.len(),
                },
            )],
        )
    }
    fn announce(&mut self, worker: Recipient<W>) -> QueueActions<A, W, Reply> {
        if let Some(waiting) = self.waiting.pop_front() {
            return Self::sends(
                vec![Delivery::new(worker, waiting.value)],
                vec![Delivery::new(
                    waiting.reply_to,
                    WorkQueueOutcome::Dispatched {
                        queued: self.waiting.len(),
                    },
                )],
            );
        }
        if !self.available.contains(&worker) {
            self.available.push_back(worker);
        }
        Actions::cont()
    }
}

impl<A, T, W, Reply> BehaviorBase for WorkQueue<A, T, W, Reply>
where
    A: Address,
    W: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = WorkQueueOutcome<T>>,
{
    type Base = Self;
    fn base(&self) -> &Self {
        self
    }
}
impl<A, T, W, Reply> Behavior for WorkQueue<A, T, W, Reply>
where
    A: Address,
    W: Behavior<Addr = A, Msg = T>,
    Reply: Behavior<Addr = A, Msg = WorkQueueOutcome<T>>,
{
    type Addr = A;
    type Msg = WorkQueueMessage<T, W, Reply>;
    type Event = User<A, Self::Msg>;
    type Sends = WorkQueueSends<W, Reply>;
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
                    .position(|candidate| *candidate == worker)
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
    use behavior::MailAddr;
    struct Worker;
    struct Reply;
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
    impl Behavior for Reply {
        type Addr = MailAddr;
        type Msg = WorkQueueOutcome<u8>;
        type Event = User<MailAddr, Self::Msg>;
        type Sends = Vec<Never>;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;
        fn transition(&mut self, _: crate::ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
            Ok(Actions::cont())
        }
    }
    type Subject = WorkQueue<MailAddr, u8, Worker, Reply>;
    fn worker(n: u64) -> Recipient<Worker> {
        Recipient::global(MailAddr(n))
    }
    fn reply() -> Recipient<Reply> {
        Recipient::global(MailAddr(9))
    }
    #[test]
    fn availability_and_waiting_are_fifo() {
        let mut s = crate::Compose::new(Subject::new(2))
            .initialize()
            .unwrap()
            .behavior;
        for w in [worker(1), worker(2)] {
            s.receive(MailAddr(0), WorkQueueMessage::Available { worker: w })
                .unwrap();
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
        assert_eq!(s.state().queued, 1);
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
        let mut s = crate::Compose::new(Subject::new(0))
            .initialize()
            .unwrap()
            .behavior;
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
        assert_eq!(s.state().queued, 0);
    }
}
