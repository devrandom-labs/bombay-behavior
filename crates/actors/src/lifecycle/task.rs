//! One-result terminal child behavior.

use behavior::{
    Actions, Address, Behavior, BehaviorActed, BehaviorBase, Delivery, Never, NoBirths, Recipient,
    Step, User,
};
use thiserror::Error;

type TaskProtocol<A, R, Reply> = core::marker::PhantomData<fn() -> (A, R, Reply)>;

/// Complete semantic state of a [`Task`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskState {
    /// One completion or cancellation may still be accepted.
    Pending,
    /// A result was reported and the behavior requested normal termination.
    Completed,
    /// Cancellation was reported and the behavior requested normal termination.
    Cancelled,
}

/// Terminal fact emitted by a [`Task`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskResult<R> {
    /// The task accepted exactly one owned result.
    Completed(R),
    /// The task accepted cancellation before a result.
    Cancelled,
}

/// Commands accepted by a [`Task`].
pub enum TaskMessage<R, Reply: behavior::Protocol> {
    /// Complete with one owned result and its typed recipient.
    Complete {
        /// Owned terminal result.
        result: R,
        /// Recipient for the terminal fact.
        reply_to: Recipient<Reply>,
    },
    /// Cancel before completion and report to a typed recipient.
    Cancel {
        /// Recipient for the cancellation fact.
        reply_to: Recipient<Reply>,
    },
}

/// Rejected terminal transition when a test or nonconforming interpreter
/// invokes a task after its stopping action.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TaskError<R> {
    /// A second result followed successful completion; ownership is returned.
    #[error("task result arrived after completion")]
    ResultAfterCompletion(R),
    /// A result followed cancellation; ownership is returned.
    #[error("task result arrived after cancellation")]
    ResultAfterCancellation(R),
    /// Cancellation followed successful completion.
    #[error("task cancellation arrived after completion")]
    CancellationAfterCompletion,
    /// Cancellation was repeated.
    #[error("task cancellation arrived after cancellation")]
    CancellationAfterCancellation,
}

/// One-result terminal behavior template.
///
/// State is exactly [`TaskState::Pending`], `Completed`, or `Cancelled`.
/// Pending completion commits `Completed`, emits one typed owned result, and
/// requests normal stop in the same `Actions`. Pending cancellation commits
/// `Cancelled`, emits one cancellation fact, and requests normal stop. A
/// universal Driver therefore accepts no later mailbox turn; direct misuse of
/// the pure fold still produces a typed [`TaskError`] and returns any result
/// ownership. Initialization is empty. Completion is an application/Bombay
/// construction over actor communication and behavior-requested termination,
/// not a new actor-model primitive. Typed delivery and terminal publication
/// are interpreted by Bombay Communication and Observe. No method has a
/// semantic panic condition.
pub struct Task<A: Address, R, Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>> {
    state: TaskState,
    protocol: TaskProtocol<A, R, Reply>,
}

impl<A: Address, R, Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>> Task<A, R, Reply> {
    /// Construct a pending task definition.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: TaskState::Pending,
            protocol: core::marker::PhantomData,
        }
    }

    /// Current exhaustive terminal state.
    #[must_use]
    pub const fn state(&self) -> TaskState {
        self.state
    }
}

impl<A: Address, R, Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>> Default
    for Task<A, R, Reply>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Address, R, Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>> BehaviorBase
    for Task<A, R, Reply>
{
    type Base = Self;

    fn base(&self) -> &Self {
        self
    }
}

impl<A, R, Reply> behavior::Protocol for Task<A, R, Reply>
where
    A: Address,
    Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>,
{
    type Addr = A;
    type Msg = TaskMessage<R, Reply>;
}

impl<A, R, Reply> behavior::KeyedProtocol for Task<A, R, Reply>
where
    A: Address,
    Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, R, Reply> Behavior for Task<A, R, Reply>
where
    A: Address,
    Reply: behavior::Protocol<Addr = A, Msg = TaskResult<R>>,
{
    type Protocol = Self;
    type Event = User<A, crate::BehaviorMessage<Self>>;
    type Sends = Vec<Delivery<Reply>>;
    type Ph = Never;
    type Error = TaskError<R>;
    type Birth = NoBirths;

    fn transition(&mut self, _: crate::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match (self.state, event.message) {
            (TaskState::Pending, TaskMessage::Complete { result, reply_to }) => {
                self.state = TaskState::Completed;
                Ok(Actions::new(
                    vec![Delivery::new(reply_to, TaskResult::Completed(result))],
                    Vec::new(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (TaskState::Pending, TaskMessage::Cancel { reply_to }) => {
                self.state = TaskState::Cancelled;
                Ok(Actions::new(
                    vec![Delivery::new(reply_to, TaskResult::Cancelled)],
                    Vec::new(),
                    Step::Stop(behavior::Stopped),
                ))
            }
            (TaskState::Completed, TaskMessage::Complete { result, .. }) => {
                Err(TaskError::ResultAfterCompletion(result))
            }
            (TaskState::Cancelled, TaskMessage::Complete { result, .. }) => {
                Err(TaskError::ResultAfterCancellation(result))
            }
            (TaskState::Completed, TaskMessage::Cancel { .. }) => {
                Err(TaskError::CancellationAfterCompletion)
            }
            (TaskState::Cancelled, TaskMessage::Cancel { .. }) => {
                Err(TaskError::CancellationAfterCancellation)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use behavior::MailAddr;

    struct Reply;

    impl behavior::Protocol for Reply {
        type Addr = MailAddr;
        type Msg = TaskResult<u8>;
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

    type TestTask = Task<MailAddr, u8, Reply>;

    #[test]
    fn completion_reports_owned_result_and_stops_atomically() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut task = (TestTask::new()).initialize().unwrap().behavior;
        let completed = task
            .receive(
                MailAddr(9),
                TaskMessage::Complete {
                    result: 7,
                    reply_to: reply,
                },
            )
            .unwrap();
        assert!(completed.sends == vec![Delivery::new(reply, TaskResult::Completed(7))]);
        assert!(completed.creates.is_empty());
        assert!(matches!(completed.become_, Step::Stop(_)));
        assert_eq!(task.state(), TaskState::Completed);
        assert!(matches!(
            task.receive(
                MailAddr(9),
                TaskMessage::Complete {
                    result: 8,
                    reply_to: reply,
                },
            ),
            Err(TaskError::ResultAfterCompletion(8))
        ));
    }

    #[test]
    fn cancellation_is_a_distinct_terminal_fact() {
        let reply = Recipient::<Reply>::global(MailAddr(8));
        let mut task = (TestTask::new()).initialize().unwrap().behavior;
        let cancelled = task
            .receive(MailAddr(9), TaskMessage::Cancel { reply_to: reply })
            .unwrap();
        assert!(cancelled.sends == vec![Delivery::new(reply, TaskResult::Cancelled)]);
        assert!(matches!(cancelled.become_, Step::Stop(_)));
        assert_eq!(task.state(), TaskState::Cancelled);
        assert!(matches!(
            task.receive(
                MailAddr(9),
                TaskMessage::Complete {
                    result: 9,
                    reply_to: reply,
                },
            ),
            Err(TaskError::ResultAfterCancellation(9))
        ));
    }
}
