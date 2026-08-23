//! Protocol templates for the communication seams owned by worker pools.
//!
//! These zero-state types are capabilities, not actors.  They let another
//! actor retain a statically typed destination for a pool's completion lane
//! without coupling that actor to the pool behavior's state or fold.

use behavior::{Address, Protocol};

use crate::{DeliveryRoute, KeyedPoolMessage, PoolMessage, PoolResponse};

/// Protocol contract required by a worker assignment.
///
/// The address and job types are projected from the completion protocol so an
/// assignment cannot repeat or disagree with either type.
pub trait PoolAssignmentProtocol: Protocol {
    type Job;
}

/// Nominal protocol implemented at a FIFO worker pool's established address.
pub struct WorkerPoolProtocol<A: Address, D, J, R, Route>(
    core::marker::PhantomData<fn(A, D, J, R, Route)>,
);

impl<A, D, J, R, Route> Protocol for WorkerPoolProtocol<A, D, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    type Addr = A;
    type Msg = PoolMessage<A, D, J, R, Route>;
}

impl<A, D, J, R, Route> PoolAssignmentProtocol for WorkerPoolProtocol<A, D, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    type Job = J;
}

/// Nominal protocol implemented at a keyed worker pool's established address.
pub struct KeyedWorkerPoolProtocol<A: Address, D, K, J, R, Route>(
    core::marker::PhantomData<fn(A, D, K, J, R, Route)>,
);

impl<A, D, K, J, R, Route> Protocol for KeyedWorkerPoolProtocol<A, D, K, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    type Addr = A;
    type Msg = KeyedPoolMessage<A, D, K, J, R, Route>;
}

impl<A, D, K, J, R, Route> PoolAssignmentProtocol for KeyedWorkerPoolProtocol<A, D, K, J, R, Route>
where
    A: Address,
    D: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>,
    Route: DeliveryRoute<D>,
{
    type Job = J;
}
