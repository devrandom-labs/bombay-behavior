//! Lifecycle messages published by one fixed supervisor.

use core::convert::Infallible;
use core::num::NonZeroU64;

use behavior::{
    Behavior, BehaviorAddr, Delivery, EndpointAddress, EstablishedActor, EstablishedDelivery,
    EstablishedRecipient, NoSends, Protocol, Recipient, SendEffects,
};

use crate::atomic::RoleName;
use crate::{ChildStopped, DeliveryRoute, ProxyPhase, ReplyDeliveries, ReplyRoute, StableProxy};

use super::ActivationPlan;
use super::WorkerUnavailable;

mod sealed {
    pub trait FixedLifecycleRoute<Role, Worker, Plan> {}
}

enum FixedLifecycleChange<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Started {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    },
    Restarted {
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        recovery: NonZeroU64,
    },
    WorkerStoppedIneligible {
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    },
    WorkerStoppedAfterAdmission {
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        recovery: NonZeroU64,
    },
    Unavailable {
        sender: BehaviorAddr<Worker>,
        phase: ProxyPhase,
        command: <Worker::Protocol as Protocol>::Msg,
    },
    MemberRetired,
}

/// One lifecycle message published by a fixed supervisor.
///
/// The role is stored once even though the lifecycle change is exhaustive.
/// [`FixedLifecycle::event`] exposes a borrowed application view without
/// requiring `Role: Clone`.
pub struct FixedLifecycle<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    role: RoleName<Role>,
    change: FixedLifecycleChange<Worker, Plan>,
}

/// Borrowed exhaustive view of one fixed-supervisor lifecycle message.
pub enum FixedLifecycleEvent<'a, Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    /// One fixed role's first worker became ready.
    Started {
        role: &'a Role,
        proxy: &'a EstablishedActor<StableProxy<Worker, Plan>>,
    },
    /// A replacement worker became ready after its predecessor stopped.
    Restarted {
        role: &'a Role,
        proxy: &'a EstablishedActor<StableProxy<Worker, Plan>>,
        recovery: NonZeroU64,
    },
    /// A worker stopped without automatic recovery.
    WorkerStoppedIneligible {
        role: &'a Role,
        stopped: &'a ChildStopped<BehaviorAddr<Worker>>,
    },
    /// A worker stop entered one admitted recovery.
    WorkerStoppedAfterAdmission {
        role: &'a Role,
        stopped: &'a ChildStopped<BehaviorAddr<Worker>>,
        recovery: NonZeroU64,
    },
    /// A service command arrived while the role had no ready worker.
    Unavailable {
        role: &'a Role,
        sender: &'a BehaviorAddr<Worker>,
        phase: ProxyPhase,
        command: &'a <Worker::Protocol as Protocol>::Msg,
    },
    /// The role no longer belongs to the live supervisor topology.
    MemberRetired { role: &'a Role },
}

impl<Role, Worker, Plan> FixedLifecycle<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(super) const fn started(
        role: RoleName<Role>,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    ) -> Self {
        Self {
            role,
            change: FixedLifecycleChange::Started { proxy },
        }
    }

    pub(super) const fn restarted(
        role: RoleName<Role>,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
        recovery: NonZeroU64,
    ) -> Self {
        Self {
            role,
            change: FixedLifecycleChange::Restarted { proxy, recovery },
        }
    }

    pub(super) const fn worker_stopped_ineligible(
        role: RoleName<Role>,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Self {
        Self {
            role,
            change: FixedLifecycleChange::WorkerStoppedIneligible { stopped },
        }
    }

    pub(super) const fn worker_stopped_after_admission(
        role: RoleName<Role>,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
        recovery: NonZeroU64,
    ) -> Self {
        Self {
            role,
            change: FixedLifecycleChange::WorkerStoppedAfterAdmission { stopped, recovery },
        }
    }

    pub(super) fn unavailable(unavailable: WorkerUnavailable<Role, Worker>) -> Self {
        let (role, sender, phase, command) = unavailable.into_parts();
        Self {
            role,
            change: FixedLifecycleChange::Unavailable {
                sender,
                phase,
                command,
            },
        }
    }

    pub(super) const fn member_retired(role: RoleName<Role>) -> Self {
        Self {
            role,
            change: FixedLifecycleChange::MemberRetired,
        }
    }

    /// Inspect the complete lifecycle change while borrowing its role and
    /// affine payloads.
    #[must_use]
    pub fn event(&self) -> FixedLifecycleEvent<'_, Role, Worker, Plan> {
        let role = self.role.role();
        match &self.change {
            FixedLifecycleChange::Started { proxy } => FixedLifecycleEvent::Started { role, proxy },
            FixedLifecycleChange::Restarted { proxy, recovery } => FixedLifecycleEvent::Restarted {
                role,
                proxy,
                recovery: *recovery,
            },
            FixedLifecycleChange::WorkerStoppedIneligible { stopped } => {
                FixedLifecycleEvent::WorkerStoppedIneligible { role, stopped }
            }
            FixedLifecycleChange::WorkerStoppedAfterAdmission { stopped, recovery } => {
                FixedLifecycleEvent::WorkerStoppedAfterAdmission {
                    role,
                    stopped,
                    recovery: *recovery,
                }
            }
            FixedLifecycleChange::Unavailable {
                sender,
                phase,
                command,
            } => FixedLifecycleEvent::Unavailable {
                role,
                sender,
                phase: *phase,
                command,
            },
            FixedLifecycleChange::MemberRetired => FixedLifecycleEvent::MemberRetired { role },
        }
    }
}

/// Static lifecycle delivery product selected by the configured capability.
#[doc(hidden)]
pub trait FixedLifecycleRoute<Role, Worker, Plan>:
    sealed::FixedLifecycleRoute<Role, Worker, Plan> + Sized
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Sends: SendEffects;

    fn deliver(self, lifecycle: FixedLifecycle<Role, Worker, Plan>) -> Self::Sends;
}

impl<Role, Worker, Plan> sealed::FixedLifecycleRoute<Role, Worker, Plan> for Infallible
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
}

impl<Role, Worker, Plan> FixedLifecycleRoute<Role, Worker, Plan> for Infallible
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    type Sends = NoSends;

    fn deliver(self, _: FixedLifecycle<Role, Worker, Plan>) -> Self::Sends {
        match self {}
    }
}

impl<Role, Worker, Plan, P> sealed::FixedLifecycleRoute<Role, Worker, Plan> for Recipient<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
}

impl<Role, Worker, Plan, P> FixedLifecycleRoute<Role, Worker, Plan> for Recipient<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
    type Sends = Vec<Delivery<P>>;

    fn deliver(self, lifecycle: FixedLifecycle<Role, Worker, Plan>) -> Self::Sends {
        DeliveryRoute::deliver(self, lifecycle)
    }
}

impl<Role, Worker, Plan, P> sealed::FixedLifecycleRoute<Role, Worker, Plan>
    for EstablishedRecipient<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
}

impl<Role, Worker, Plan, P> FixedLifecycleRoute<Role, Worker, Plan> for EstablishedRecipient<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
    type Sends = Vec<EstablishedDelivery<P>>;

    fn deliver(self, lifecycle: FixedLifecycle<Role, Worker, Plan>) -> Self::Sends {
        DeliveryRoute::deliver(self, lifecycle)
    }
}

impl<Role, Worker, Plan, P> sealed::FixedLifecycleRoute<Role, Worker, Plan> for ReplyRoute<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
}

impl<Role, Worker, Plan, P> FixedLifecycleRoute<Role, Worker, Plan> for ReplyRoute<P>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
    P: Protocol<Addr = BehaviorAddr<Worker>, Msg = FixedLifecycle<Role, Worker, Plan>>,
{
    type Sends = ReplyDeliveries<Delivery<P>, EstablishedDelivery<P>>;

    fn deliver(self, lifecycle: FixedLifecycle<Role, Worker, Plan>) -> Self::Sends {
        DeliveryRoute::deliver(self, lifecycle)
    }
}
