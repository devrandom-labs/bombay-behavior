//! Exact fixed-member join for StableProxy shutdown settlement and proxy exit.

use core::ops::ControlFlow;

use behavior::{
    Behavior, BehaviorAddr, CreationId, EndpointAddress, EstablishedActor, ItemSettlement,
    SettledItem,
};

use crate::atomic::RoleName;
use crate::atomic::stable_proxy::ProxyOperationWitness;
use crate::{ChildStopped, ProxyInputResult, ProxyOperation, ProxyOperationId, StableProxy};

use super::super::member::RosterOwner;
use super::super::role::{MemberRole, RosterPosition};
use super::super::{ActivationPlan, FixedSupervisorError, FixedSupervisorEvent};
use super::FixedRoster;

enum ProxyShutdown<Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    Dispatched(ProxyOperationWitness),
    Accepted(ProxyOperationId),
    Rejected(ProxyInputResult<behavior::Here, Worker, Plan>),
}

pub(in super::super) struct RetiredProxyMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    role: MemberRole<Role>,
    stopped: ChildStopped<BehaviorAddr<Worker>>,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "a rejected shutdown and its proxy remain owned until supervisor retirement"
        )
    )]
    rejected: Option<(
        EstablishedActor<StableProxy<Worker, Plan>>,
        ProxyInputResult<behavior::Here, Worker, Plan>,
    )>,
}

pub(in super::super) struct ProxyStoppingMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    role: MemberRole<Role>,
    creation: CreationId,
    proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    shutdown: ProxyShutdown<Worker, Plan>,
    stopped: Option<ChildStopped<BehaviorAddr<Worker>>>,
}

impl<Role, Worker, Plan> ProxyStoppingMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn live_proxy_role(&self, child: CreationId) -> Option<RoleName<Role>> {
        match &self.stopped {
            None if child == self.creation => Some(self.role.name()),
            None | Some(_) => None,
        }
    }

    pub(in super::super) fn position(&self) -> RosterPosition {
        self.role.position()
    }

    pub(in super::super) fn role(&self) -> &Role {
        self.role.role()
    }

    pub(in super::super) fn begin(
        role: MemberRole<Role>,
        creation: CreationId,
        proxy: EstablishedActor<StableProxy<Worker, Plan>>,
    ) -> (Self, ProxyOperation<behavior::Here, Worker, Plan>) {
        let (witness, operation) = ProxyOperation::shutdown(creation);
        (
            Self {
                role,
                creation,
                proxy,
                shutdown: ProxyShutdown::Dispatched(witness),
                stopped: None,
            },
            operation,
        )
    }

    pub(in super::super) fn accept_operation(
        self,
        settlement: ProxyInputResult<behavior::Here, Worker, Plan>,
    ) -> Result<
        ControlFlow<RetiredProxyMember<Role, Worker, Plan>, Self>,
        (Self, ProxyInputResult<behavior::Here, Worker, Plan>),
    > {
        let Self {
            role,
            creation,
            proxy,
            shutdown,
            stopped,
        } = self;
        let ProxyShutdown::Dispatched(witness) = shutdown else {
            return Err((
                Self {
                    role,
                    creation,
                    proxy,
                    shutdown,
                    stopped,
                },
                settlement,
            ));
        };
        match witness.admit(settlement) {
            Ok(SettledItem::Attempted(ItemSettlement::Accepted(receipt)))
                if receipt.creation() == creation =>
            {
                let (_, proxy, operation) = receipt.into_parts();
                Ok(Self {
                    role,
                    creation,
                    proxy,
                    shutdown: ProxyShutdown::Accepted(operation),
                    stopped,
                }
                .finish())
            }
            Ok(input) => Ok(Self {
                role,
                creation,
                proxy,
                shutdown: ProxyShutdown::Rejected(input),
                stopped,
            }
            .finish()),
            Err((witness, settlement)) => Err((
                Self {
                    role,
                    creation,
                    proxy,
                    shutdown: ProxyShutdown::Dispatched(witness),
                    stopped,
                },
                settlement,
            )),
        }
    }

    pub(in super::super) fn accepts_stop(
        &self,
        stopped: &ChildStopped<BehaviorAddr<Worker>>,
    ) -> bool {
        self.accepts_child(stopped.child)
    }

    pub(in super::super) fn accepts_child(&self, child: CreationId) -> bool {
        self.creation == child
    }

    pub(in super::super) fn accept_stop(
        self,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<
        ControlFlow<RetiredProxyMember<Role, Worker, Plan>, Self>,
        (Self, ChildStopped<BehaviorAddr<Worker>>),
    > {
        let Self {
            role,
            creation,
            proxy,
            shutdown,
            stopped: current,
        } = self;
        match current {
            None => Ok(Self {
                role,
                creation,
                proxy,
                shutdown,
                stopped: Some(stopped),
            }
            .finish()),
            Some(current) => Err((
                Self {
                    role,
                    creation,
                    proxy,
                    shutdown,
                    stopped: Some(current),
                },
                stopped,
            )),
        }
    }

    fn finish(self) -> ControlFlow<RetiredProxyMember<Role, Worker, Plan>, Self> {
        let Self {
            role,
            creation,
            proxy,
            shutdown,
            stopped,
        } = self;
        match (shutdown, stopped) {
            (ProxyShutdown::Accepted(_operation), Some(stopped)) => {
                ControlFlow::Break(RetiredProxyMember {
                    role,
                    stopped,
                    rejected: None,
                })
            }
            (ProxyShutdown::Rejected(input), Some(stopped)) => {
                ControlFlow::Break(RetiredProxyMember {
                    role,
                    stopped,
                    rejected: Some((proxy, input)),
                })
            }
            (shutdown, stopped) => ControlFlow::Continue(Self {
                role,
                creation,
                proxy,
                shutdown,
                stopped,
            }),
        }
    }
}

impl<Role, Worker, Plan> RetiredProxyMember<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn role_name(&self) -> RoleName<Role> {
        self.role.name()
    }

    pub(in super::super) fn position(&self) -> RosterPosition {
        self.role.position()
    }

    pub(in super::super) fn role(&self) -> &Role {
        self.role.role()
    }

    pub(in super::super) fn accepts_child(&self, child: CreationId) -> bool {
        self.stopped.child == child
    }
}

impl<Role, Worker, Plan> FixedRoster<Role, Worker, Plan>
where
    Worker: Behavior,
    Plan: ActivationPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    pub(in super::super) fn accept_proxy_stop<Preparation>(
        self,
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    ) -> Result<
        (Self, Option<RoleName<Role>>),
        (Self, FixedSupervisorError<Role, Worker, Plan, Preparation>),
    > {
        match self {
            Self::Operating(mut members) => {
                let declaration = members.iter().position(|member| match member {
                    RosterOwner::Stopping(stopping) => stopping.accepts_stop(&stopped),
                    RosterOwner::Unrecovered(member) => member.accepts_stop(&stopped),
                    RosterOwner::Starting(_)
                    | RosterOwner::Online(_)
                    | RosterOwner::Empty(_)
                    | RosterOwner::Recovery(_)
                    | RosterOwner::Retired(_) => false,
                });
                let declaration = match declaration {
                    Some(declaration) => declaration,
                    None => {
                        let roster = Self::Operating(members);
                        let input = FixedSupervisorEvent::ProxyStopped(stopped);
                        return Err((roster, FixedSupervisorError::InputRejected { input }));
                    }
                };
                let member = members.remove(declaration);
                match member {
                    RosterOwner::Stopping(stopping) => match stopping.accept_stop(stopped) {
                        Ok(ControlFlow::Continue(stopping)) => {
                            members.insert(declaration, RosterOwner::Stopping(stopping));
                            Ok((Self::Operating(members), None))
                        }
                        Ok(ControlFlow::Break(retired)) => {
                            let role = retired.role_name();
                            members.insert(declaration, RosterOwner::Retired(retired));
                            Ok((Self::Operating(members), Some(role)))
                        }
                        Err((stopping, stopped)) => {
                            members.insert(declaration, RosterOwner::Stopping(stopping));
                            let roster = Self::Operating(members);
                            let input = FixedSupervisorEvent::ProxyStopped(stopped);
                            Err((roster, FixedSupervisorError::InputRejected { input }))
                        }
                    },
                    RosterOwner::Unrecovered(member) => match member.accept_stop(stopped) {
                        Ok(ControlFlow::Continue(member)) => {
                            members.insert(declaration, RosterOwner::Unrecovered(member));
                            Ok((Self::Operating(members), None))
                        }
                        Ok(ControlFlow::Break(retired)) => {
                            let role = retired.role_name();
                            members.insert(declaration, RosterOwner::Retired(retired));
                            Ok((Self::Operating(members), Some(role)))
                        }
                        Err((member, stopped)) => {
                            members.insert(declaration, RosterOwner::Unrecovered(member));
                            let roster = Self::Operating(members);
                            let input = FixedSupervisorEvent::ProxyStopped(stopped);
                            Err((roster, FixedSupervisorError::InputRejected { input }))
                        }
                    },
                    member => {
                        members.insert(declaration, member);
                        let roster = Self::Operating(members);
                        let input = FixedSupervisorEvent::ProxyStopped(stopped);
                        Err((roster, FixedSupervisorError::InputRejected { input }))
                    }
                }
            }
            roster => {
                let input = FixedSupervisorEvent::ProxyStopped(stopped);
                Err((roster, FixedSupervisorError::InputRejected { input }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use core::ops::ControlFlow;
    use std::time::Instant;

    use behavior::{
        ActiveTurn, Address, Behavior, BehaviorActed, CreationId, EndpointAddress,
        EstablishedActor, ItemSettlement, MessageProtocol, Never, NoBirths, NoSends, Protocol,
        SettledItem, User,
    };

    use crate::atomic::ImmediateActivation;
    use crate::{Exit, ProxyInputReceipt, StableProxy};

    use super::super::super::role::{MemberRole, RosterPosition};
    use super::{ProxyStoppingMember, RetiredProxyMember};

    #[derive(Clone, Copy, Eq, PartialEq)]
    struct TestAddr;

    impl Address for TestAddr {
        type Nonce = u64;
    }

    #[derive(Clone, Copy)]
    struct TestEndpoint;

    impl EndpointAddress for TestAddr {
        type Established<P>
            = TestEndpoint
        where
            P: Protocol<Addr = Self>;
    }

    struct Worker;

    impl Behavior for Worker {
        type Protocol = MessageProtocol<TestAddr, Never>;
        type Event = User<TestAddr, Never>;
        type Sends = NoSends;
        type Ph = Never;
        type Error = Never;
        type Birth = NoBirths;

        fn transition(&mut self, _: ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
            match event.message {}
        }
    }

    struct SearchRole;

    fn creation(id: u64) -> CreationId {
        let mut sequence = behavior::CreationSequence::new();
        (0..id)
            .map(|_| sequence.issue())
            .last()
            .flatten()
            .unwrap_or_else(|| panic!("test creation ID is issued"))
    }

    #[test]
    fn proxy_exit_before_shutdown_acceptance_closes_only_after_acceptance() {
        let creation = creation(17);
        let proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint);
        let (stopping, shutdown) = ProxyStoppingMember::begin(
            MemberRole::declared(RosterPosition::new(0), SearchRole),
            creation,
            proxy,
        );
        let stopped = crate::ChildStopped::new(creation, Ok(Exit::Normal), Instant::now());
        let member = stopping
            .accept_stop(stopped)
            .unwrap_or_else(|_| panic!("the exact early exit is retained"));
        let stopping = match member {
            ControlFlow::Continue(stopping) => stopping,
            ControlFlow::Break(_) => {
                panic!("exit alone cannot retire before shutdown acceptance")
            }
        };

        let (creation, _, operation) = shutdown.into_parts();
        let accepted = SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
            creation,
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint),
            operation,
        )));
        let member = stopping.accept_operation(accepted);

        match member {
            Ok(ControlFlow::Break(_)) => {}
            Ok(ControlFlow::Continue(_)) | Err(_) => {
                panic!("the exact acceptance closes the retained early exit")
            }
        }
    }

    #[test]
    fn shutdown_receipt_requires_the_retained_proxy_creation() {
        let exact_creation = creation(19);
        let exact_proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint);
        let (exact_stopping, exact_shutdown) = ProxyStoppingMember::begin(
            MemberRole::declared(RosterPosition::new(0), SearchRole),
            exact_creation,
            exact_proxy,
        );
        let exact_stopped =
            crate::ChildStopped::new(exact_creation, Ok(Exit::Normal), Instant::now());
        let exact_stopping = match exact_stopping
            .accept_stop(exact_stopped)
            .unwrap_or_else(|_| panic!("the exact early exit is retained"))
        {
            ControlFlow::Continue(stopping) => stopping,
            ControlFlow::Break(_) => panic!("the shutdown receipt is still required"),
        };
        let (_, _, exact_operation) = exact_shutdown.into_parts();
        let exact_receipt =
            SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
                exact_creation,
                EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint),
                exact_operation,
            )));
        match exact_stopping.accept_operation(exact_receipt) {
            Ok(ControlFlow::Break(RetiredProxyMember {
                rejected: None,
                stopped,
                ..
            })) => assert_eq!(stopped.child, exact_creation),
            Ok(ControlFlow::Continue(_)) | Ok(ControlFlow::Break(_)) | Err(_) => {
                panic!("the exact receipt commits shutdown without rejected custody")
            }
        }

        let retained_creation = creation(31);
        let foreign_creation = creation(32);
        let retained_proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint);
        let (foreign_stopping, foreign_shutdown) = ProxyStoppingMember::begin(
            MemberRole::declared(RosterPosition::new(0), SearchRole),
            retained_creation,
            retained_proxy,
        );
        let retained_stop =
            crate::ChildStopped::new(retained_creation, Ok(Exit::Normal), Instant::now());
        let foreign_stopping = match foreign_stopping
            .accept_stop(retained_stop)
            .unwrap_or_else(|_| panic!("the retained proxy exit is exact"))
        {
            ControlFlow::Continue(stopping) => stopping,
            ControlFlow::Break(_) => panic!("the shutdown receipt is still required"),
        };
        let (_, _, foreign_operation) = foreign_shutdown.into_parts();
        let foreign_receipt =
            SettledItem::Attempted(ItemSettlement::Accepted(ProxyInputReceipt::new(
                foreign_creation,
                EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint),
                foreign_operation,
            )));
        match foreign_stopping.accept_operation(foreign_receipt) {
            Ok(ControlFlow::Break(RetiredProxyMember {
                rejected: Some((_, SettledItem::Attempted(ItemSettlement::Accepted(receipt)))),
                stopped,
                ..
            })) => {
                assert_eq!(receipt.creation(), foreign_creation);
                assert_eq!(stopped.child, retained_creation);
            }
            Ok(ControlFlow::Continue(_)) | Ok(ControlFlow::Break(_)) | Err(_) => {
                panic!("the foreign receipt remains complete in rejected custody")
            }
        }
    }

    #[test]
    fn rejected_shutdown_after_exit_retains_both_complete_inputs() {
        let creation = creation(23);
        let proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint);
        let (stopping, shutdown) = ProxyStoppingMember::begin(
            MemberRole::declared(RosterPosition::new(0), SearchRole),
            creation,
            proxy,
        );
        let stopped = crate::ChildStopped::new(creation, Ok(Exit::Normal), Instant::now());
        let member = stopping
            .accept_stop(stopped)
            .unwrap_or_else(|_| panic!("the exact early exit is retained"));
        let stopping = match member {
            ControlFlow::Continue(stopping) => stopping,
            ControlFlow::Break(_) => {
                panic!("the shutdown result is still required")
            }
        };
        let member = stopping.accept_operation(SettledItem::Unattempted(shutdown));

        match member {
            Ok(ControlFlow::Break(RetiredProxyMember {
                rejected: Some((_, SettledItem::Unattempted(shutdown))),
                stopped,
                ..
            })) => {
                assert_eq!(shutdown.creation().get(), 23);
                assert_eq!(stopped.child.get(), 23);
            }
            Ok(ControlFlow::Continue(_)) | Ok(ControlFlow::Break(_)) | Err(_) => {
                panic!("rejected shutdown and early exit remain in one exact state")
            }
        }
    }

    #[test]
    fn proxy_exit_after_rejected_shutdown_retires_with_both_complete_inputs() {
        let creation = creation(29);
        let proxy =
            EstablishedActor::<StableProxy<Worker, ImmediateActivation>>::issued(TestEndpoint);
        let (stopping, shutdown) = ProxyStoppingMember::begin(
            MemberRole::declared(RosterPosition::new(0), SearchRole),
            creation,
            proxy,
        );
        let waiting = match stopping.accept_operation(SettledItem::Unattempted(shutdown)) {
            Ok(ControlFlow::Continue(waiting)) => waiting,
            Ok(ControlFlow::Break(_)) | Err(_) => {
                panic!("rejected shutdown alone cannot prove proxy retirement")
            }
        };
        let stopped = crate::ChildStopped::new(creation, Ok(Exit::Normal), Instant::now());
        let retired = waiting
            .accept_stop(stopped)
            .unwrap_or_else(|_| panic!("the exact proxy exit closes the join"));

        match retired {
            ControlFlow::Break(RetiredProxyMember {
                rejected: Some((_, SettledItem::Unattempted(shutdown))),
                stopped,
                ..
            }) => {
                assert_eq!(shutdown.creation().get(), 29);
                assert_eq!(stopped.child.get(), 29);
            }
            ControlFlow::Continue(_) | ControlFlow::Break(_) => {
                panic!("the retired result retains rejection and exact exit")
            }
        }
    }
}
