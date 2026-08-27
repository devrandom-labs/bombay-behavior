//! Protocol templates for the communication seams owned by worker pools.
//!
//! These zero-state types are capabilities, not actors. They describe only a
//! pool's customer-facing command protocol; worker completion travels through
//! the established child/parent structure instead of a second destination.

use behavior::{Address, Protocol};

use crate::{DeliveryRoute, KeyedPoolMessage, PoolMessage, PoolResponse};

/// Nominal protocol implemented at a FIFO worker pool's established address.
pub struct WorkerPoolProtocol<A: Address, J, R, Route>(
    core::marker::PhantomData<fn(A, J, R, Route)>,
);

impl<A, J, R, Route> Protocol for WorkerPoolProtocol<A, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    type Addr = A;
    type Msg = PoolMessage<A, J, R, Route>;
}

/// Nominal protocol implemented at a keyed worker pool's established address.
pub struct KeyedWorkerPoolProtocol<A: Address, K, J, R, Route>(
    core::marker::PhantomData<fn(A, K, J, R, Route)>,
);

impl<A, K, J, R, Route> Protocol for KeyedWorkerPoolProtocol<A, K, J, R, Route>
where
    A: Address,
    Route: DeliveryRoute<Protocol: Protocol<Addr = A, Msg = PoolResponse<J, R, A>>>,
{
    type Addr = A;
    type Msg = KeyedPoolMessage<A, K, J, R, Route>;
}
