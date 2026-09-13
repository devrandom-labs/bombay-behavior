//! One semantic role, its direct worker, and its own admission queue.

use std::collections::BTreeMap;

use behavior::{Behavior, BehaviorAddr, EndpointAddress};

use super::super::pool::assignment::{AdmissionOrdinal, CustomerJob};
use super::super::pool::worker::{Member, MemberState, Worker, WorkerPhase};
use super::super::{ActivationPlan, BacklogCapacity};
use super::job::KeyedCustomer;

pub(super) enum AdmissionTarget {
    Assign,
    Queue,
    QueueFull,
    Unavailable,
}

pub(super) enum ManagementTarget {
    Bindable,
    Unavailable,
}

pub(super) struct RoleCell<Role, W, P, Job, WorkerResult, Route>
where
    W: Behavior,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(super) member: Member<Role, W, P, Job, WorkerResult, KeyedCustomer<Role, Route>>,
    pub(super) queue: BTreeMap<AdmissionOrdinal, CustomerJob<Job, KeyedCustomer<Role, Route>>>,
    pub(super) capacity: BacklogCapacity,
}

impl<Role, W, P, Job, WorkerResult, Route> RoleCell<Role, W, P, Job, WorkerResult, Route>
where
    W: Behavior,
    P: ActivationPlan,
    BehaviorAddr<W>: EndpointAddress,
{
    pub(super) fn new(
        member: Member<Role, W, P, Job, WorkerResult, KeyedCustomer<Role, Route>>,
        capacity: BacklogCapacity,
    ) -> Self {
        Self {
            member,
            queue: BTreeMap::new(),
            capacity,
        }
    }

    pub(super) fn role(&self) -> &Role {
        self.member.role.role()
    }

    pub(super) fn admission_target(&self) -> AdmissionTarget {
        match &self.member.state {
            MemberState::Creating(_) | MemberState::Recovering(_) => self.queue_target(),
            MemberState::Worker(Worker { phase, .. }) => match phase {
                WorkerPhase::Idle => AdmissionTarget::Assign,
                WorkerPhase::Initializing { stopped: None, .. }
                | WorkerPhase::WaitingForActivation { .. }
                | WorkerPhase::ActivationDispatched { stopped: None, .. }
                | WorkerPhase::Activating { stopped: None, .. }
                | WorkerPhase::Busy(_) => self.queue_target(),
                WorkerPhase::Initializing {
                    stopped: Some(_), ..
                }
                | WorkerPhase::ActivationDispatched {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Activating {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Stopping(_) => AdmissionTarget::Unavailable,
            },
            MemberState::Retired => AdmissionTarget::Unavailable,
        }
    }

    pub(super) fn management_target(&self) -> ManagementTarget {
        match &self.member.state {
            MemberState::Creating(_) | MemberState::Recovering(_) => ManagementTarget::Bindable,
            MemberState::Worker(Worker { phase, .. }) => match phase {
                WorkerPhase::Initializing { stopped: None, .. }
                | WorkerPhase::WaitingForActivation { .. }
                | WorkerPhase::ActivationDispatched { stopped: None, .. }
                | WorkerPhase::Activating { stopped: None, .. }
                | WorkerPhase::Idle
                | WorkerPhase::Busy(_) => ManagementTarget::Bindable,
                WorkerPhase::Initializing {
                    stopped: Some(_), ..
                }
                | WorkerPhase::ActivationDispatched {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Activating {
                    stopped: Some(_), ..
                }
                | WorkerPhase::Stopping(_) => ManagementTarget::Unavailable,
            },
            MemberState::Retired => ManagementTarget::Unavailable,
        }
    }

    fn queue_target(&self) -> AdmissionTarget {
        match self.queue.len().cmp(&self.capacity.maximum()) {
            core::cmp::Ordering::Less => AdmissionTarget::Queue,
            core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => AdmissionTarget::QueueFull,
        }
    }
}
