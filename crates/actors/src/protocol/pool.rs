//! Protocol templates for the communication seams owned by worker pools.
//!
//! These zero-state types are capabilities, not actors.  They let another
//! actor retain a statically typed destination for a pool's completion lane
//! without coupling that actor to the pool behavior's state or fold.

use behavior::{Address, Protocol};

use crate::{KeyedPoolMessage, PoolMessage, PoolResponse};

/// Protocol contract required by a worker assignment.
///
/// The address and job types are projected from the completion protocol so an
/// assignment cannot repeat or disagree with either type.
pub trait PoolAssignmentProtocol: Protocol {
    type Job;
}

/// Nominal protocol implemented at a FIFO worker pool's established address.
pub struct WorkerPoolProtocol<A: Address, D, J, R>(core::marker::PhantomData<fn(A, D, J, R)>);

impl<A, D, J, R> Protocol for WorkerPoolProtocol<A, D, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Addr = A;
    type Msg = PoolMessage<A, D, J, R>;
}

impl<A, D, J, R> behavior::KeyedProtocol for WorkerPoolProtocol<A, D, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, D, J, R> PoolAssignmentProtocol for WorkerPoolProtocol<A, D, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Job = J;
}

/// Nominal protocol implemented at a keyed worker pool's established address.
pub struct KeyedWorkerPoolProtocol<A: Address, D, K, J, R>(
    core::marker::PhantomData<fn(A, D, K, J, R)>,
);

impl<A, D, K, J, R> Protocol for KeyedWorkerPoolProtocol<A, D, K, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Addr = A;
    type Msg = KeyedPoolMessage<A, D, K, J, R>;
}

impl<A, D, K, J, R> behavior::KeyedProtocol for KeyedWorkerPoolProtocol<A, D, K, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Key = behavior::NominalProtocolKey<Self>;
}

impl<A, D, K, J, R> PoolAssignmentProtocol for KeyedWorkerPoolProtocol<A, D, K, J, R>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
{
    type Job = J;
}
