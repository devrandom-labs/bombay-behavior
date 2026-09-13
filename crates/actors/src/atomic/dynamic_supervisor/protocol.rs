//! Commands and immediate replies for one dynamic supervisor.

use behavior::{Behavior, BehaviorAddr, EndpointAddress, MessageProtocol, Protocol};

use crate::{ReplyRoute, WorkerSubmission};

/// Affine authority to cancel one accepted start or replacement.
///
/// ```compile_fail,E0382
/// fn duplicate<Key>(authority: behavior_actors::atomic::CancelAuthority<Key>) {
///     let first = authority;
///     let second = authority;
/// }
/// ```
///
/// Applications cannot forge an operation authority from a key and number:
///
/// ```compile_fail,E0451
/// use behavior_actors::atomic::CancelAuthority;
///
/// let _authority = CancelAuthority {
///     key: "search",
///     operation: 1,
/// };
/// ```
#[must_use = "cancellation authority must be used or deliberately retained"]
pub struct CancelAuthority<Key> {
    key: Key,
    operation: u64,
}

impl<Key> CancelAuthority<Key> {
    pub(super) const fn issued(key: Key, operation: u64) -> Self {
        Self { key, operation }
    }

    /// Inspect the application key governed by this authority.
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    pub(super) const fn operation(&self) -> u64 {
        self.operation
    }
}

/// Immediate proof that one worker change was accepted.
#[must_use = "accepted worker-change authority must be retained"]
pub struct WorkerChangeReceipt<Key> {
    /// Equal application key retained for the requesting actor.
    pub key: Key,
    /// Affine authority for this exact accepted operation.
    pub cancel: CancelAuthority<Key>,
}

/// Complete worker change returned when admission is rejected.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerChangeRejection<Key, Worker, Plan, Reason> {
    /// Submitted application key.
    pub key: Key,
    /// Submitted worker and its exact activation work.
    pub submission: WorkerSubmission<Worker, Plan>,
    /// Operation-specific rejection reason.
    pub reason: Reason,
}

/// Why a new keyed service was not admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartRejection {
    /// The key already belongs to a retained entry.
    AlreadyExists,
    /// The configured entry maximum is occupied.
    AtCapacity,
    /// Global shutdown has closed mutation.
    ShuttingDown,
    /// No fresh entry generation can be issued.
    EntryGenerationExhausted,
    /// No fresh ordered management operation can be issued.
    OperationExhausted,
    /// No fresh proxy creation ID can be issued.
    ProxyCreationExhausted,
}

/// Why a replacement was not admitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplaceRejection {
    /// The key is not retained.
    Unknown,
    /// The current entry phase cannot accept replacement.
    Unavailable,
    /// Global shutdown has closed mutation.
    ShuttingDown,
    /// No fresh ordered management operation can be issued.
    OperationExhausted,
}

/// Complete keyed rejection of an explicit stop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StopRejection<Key> {
    /// The submitted key is not retained.
    Unknown { key: Key },
    /// The entry cannot begin an explicit stop in its current phase.
    Unavailable { key: Key },
    /// The entry is already stopping.
    AlreadyStopping { key: Key },
    /// Global shutdown has closed mutation.
    ShuttingDown { key: Key },
    /// No fresh ordered management operation can be issued.
    OperationExhausted { key: Key },
}

/// Application-visible phase of one retained keyed service.
pub enum DynamicStatus<Service>
where
    Service: Protocol,
    Service::Addr: EndpointAddress,
{
    CreatingProxy,
    WaitingForActivation,
    AwaitingProxy,
    Ready {
        /// Stable service capability; never a direct worker capability.
        proxy: behavior::EstablishedRecipient<Service>,
    },
    Empty,
    Replacing,
    Stopping,
    Cancelling,
    Retiring,
    /// The supervisor has closed management and is draining this entry.
    Draining,
}

/// Immediate reply to a read-only keyed query.
pub enum QueryReply<Key, Service>
where
    Service: Protocol,
    Service::Addr: EndpointAddress,
{
    Unknown {
        key: Key,
    },
    Known {
        key: Key,
        status: DynamicStatus<Service>,
    },
}

/// Complete immediate result of one cancellation request.
pub enum CancellationReceipt<Key, Worker, Plan>
where
    Worker: Behavior,
    <Worker::Protocol as Protocol>::Addr: EndpointAddress,
{
    Returned {
        authority: CancelAuthority<Key>,
        submission: WorkerSubmission<Worker, Plan>,
    },
    Pending {
        authority: CancelAuthority<Key>,
        phase: DynamicStatus<Worker::Protocol>,
    },
    Committed {
        authority: CancelAuthority<Key>,
        resulting_phase: DynamicStatus<Worker::Protocol>,
    },
    Cancelled {
        authority: CancelAuthority<Key>,
    },
    Stale {
        authority: CancelAuthority<Key>,
    },
    /// Global shutdown owns the operation and returns its affine authority.
    Draining {
        authority: CancelAuthority<Key>,
    },
}

/// Five application operations accepted by one dynamic supervisor.
pub enum DynamicCommand<Key, Worker, Plan>
where
    Worker: Behavior,
    BehaviorAddr<Worker>: EndpointAddress,
{
    Start {
        key: Key,
        submission: WorkerSubmission<Worker, Plan>,
        reply_to: ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, StartRejection>,
                >,
            >,
        >,
    },
    Replace {
        key: Key,
        submission: WorkerSubmission<Worker, Plan>,
        reply_to: ReplyRoute<
            MessageProtocol<
                BehaviorAddr<Worker>,
                Result<
                    WorkerChangeReceipt<Key>,
                    WorkerChangeRejection<Key, Worker, Plan, ReplaceRejection>,
                >,
            >,
        >,
    },
    Stop {
        key: Key,
        reply_to:
            ReplyRoute<MessageProtocol<BehaviorAddr<Worker>, Result<Key, StopRejection<Key>>>>,
    },
    Query {
        key: Key,
        reply_to:
            ReplyRoute<MessageProtocol<BehaviorAddr<Worker>, QueryReply<Key, Worker::Protocol>>>,
    },
    Cancel {
        authority: CancelAuthority<Key>,
        reply_to: ReplyRoute<
            MessageProtocol<BehaviorAddr<Worker>, CancellationReceipt<Key, Worker, Plan>>,
        >,
    },
}

#[cfg(test)]
mod tests {
    use super::CancelAuthority;

    #[expect(
        dead_code,
        reason = "compile-only proof of one cancellation operation correlation"
    )]
    fn cancellation_authority_names_key_and_operation() {
        let _authority = CancelAuthority::issued("search", 1);
    }
}
