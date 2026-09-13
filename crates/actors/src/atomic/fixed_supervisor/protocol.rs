//! FixedSupervisor commands and read-only replies.

use behavior::{Address, EndpointAddress, EstablishedRecipient, MessageProtocol, Protocol};

use crate::ReplyRoute;

/// Read-only phase of one fixed roster member.
pub enum MemberStatus<Service>
where
    Service: Protocol,
    Service::Addr: EndpointAddress,
{
    /// The stable proxy is not committed yet.
    CreatingProxy,
    /// The proxy is waiting for global activation capacity.
    WaitingForActivation,
    /// Initial or replacement input is awaiting its atomic proxy outcome.
    AwaitingProxy,
    /// The stable proxy is ready to accept service messages.
    Ready {
        /// Exact capability of the ready stable proxy, never its worker.
        proxy: EstablishedRecipient<Service>,
    },
    /// The stable proxy has no current worker.
    Empty,
    /// The role belongs to one coordinated recovery.
    Recovering,
    /// The stable proxy is retiring.
    Stopping,
    /// The role no longer owns a stable proxy.
    Retired,
}

/// Fixed roster status in the original declaration order.
pub struct FixedSnapshot<Service>
where
    Service: Protocol,
    Service::Addr: EndpointAddress,
{
    /// One status per declared role, in semantic roster order.
    pub members: Vec<MemberStatus<Service>>,
}

/// Non-ready phase returned by an exact capability query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnavailablePhase {
    /// The stable proxy is not committed yet.
    CreatingProxy,
    /// The proxy is waiting for global activation capacity.
    WaitingForActivation,
    /// Initial or replacement input is awaiting its atomic proxy outcome.
    AwaitingProxy,
    /// The stable proxy has no current worker.
    Empty,
    /// The role belongs to one coordinated recovery.
    Recovering,
    /// The stable proxy is retiring.
    Stopping,
    /// The role no longer owns a stable proxy.
    Retired,
}

impl UnavailablePhase {
    pub(super) fn status<Service>(self) -> MemberStatus<Service>
    where
        Service: Protocol,
        Service::Addr: EndpointAddress,
    {
        match self {
            Self::CreatingProxy => MemberStatus::CreatingProxy,
            Self::WaitingForActivation => MemberStatus::WaitingForActivation,
            Self::AwaitingProxy => MemberStatus::AwaitingProxy,
            Self::Empty => MemberStatus::Empty,
            Self::Recovering => MemberStatus::Recovering,
            Self::Stopping => MemberStatus::Stopping,
            Self::Retired => MemberStatus::Retired,
        }
    }
}

/// Result of querying one submitted semantic role.
pub enum CapabilityResult<Role, Service>
where
    Service: Protocol,
    Service::Addr: EndpointAddress,
{
    /// The role is ready through its stable proxy.
    Ready {
        /// Exact submitted role value.
        role: Role,
        /// Exact stable proxy capability.
        proxy: EstablishedRecipient<Service>,
    },
    /// The role exists but is not ready.
    Unavailable {
        /// Exact submitted role value.
        role: Role,
        /// Current non-ready phase.
        phase: UnavailablePhase,
    },
    /// No declared role equals the submitted value.
    UnknownRole {
        /// Exact submitted role returned unchanged.
        submitted: Role,
    },
}

/// Management input accepted by one fixed supervisor.
///
/// A recipient for another reply protocol cannot be substituted:
///
/// ```compile_fail,E0277
/// #[derive(Clone, Copy, Eq, PartialEq)]
/// struct RuntimeAddress;
/// impl behavior::Address for RuntimeAddress { type Nonce = u64; }
/// #[derive(Clone)]
/// struct Endpoint;
/// impl behavior::EndpointAddress for RuntimeAddress {
///     type Established<P> = Endpoint where P: behavior::Protocol<Addr = Self>;
/// }
/// struct Service;
/// impl behavior::Protocol for Service {
///     type Addr = RuntimeAddress;
///     type Msg = u8;
/// }
/// let wrong = behavior::Recipient::<
///     behavior::MessageProtocol<RuntimeAddress, u8>
/// >::global(RuntimeAddress);
/// let _: behavior_actors::atomic::FixedCommand<RuntimeAddress, u16, Service> =
///     behavior_actors::atomic::FixedCommand::status(wrong);
/// ```
pub enum FixedCommand<A, Role, Service>
where
    A: Address + EndpointAddress,
    Service: Protocol<Addr = A>,
{
    /// Query every member status in declaration order.
    Status {
        /// Temporary logical or exact status recipient.
        reply_to: ReplyRoute<MessageProtocol<A, FixedSnapshot<Service>>>,
    },
    /// Query the stable service capability for one semantic role.
    Capability {
        /// Submitted role value, returned in every capability result.
        role: Role,
        /// Temporary logical or exact capability-result recipient.
        reply_to: ReplyRoute<MessageProtocol<A, CapabilityResult<Role, Service>>>,
    },
    /// Close recovery admission and retire the complete owned actor graph.
    Shutdown,
}

impl<A, Role, Service> FixedCommand<A, Role, Service>
where
    A: Address + EndpointAddress,
    Service: Protocol<Addr = A>,
{
    /// Construct a declaration-ordered status query.
    #[must_use]
    pub fn status<Route>(reply_to: Route) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, FixedSnapshot<Service>>>>,
    {
        Self::Status {
            reply_to: reply_to.into(),
        }
    }

    /// Construct one semantic-role capability query.
    #[must_use]
    pub fn capability<Route>(role: Role, reply_to: Route) -> Self
    where
        Route: Into<ReplyRoute<MessageProtocol<A, CapabilityResult<Role, Service>>>>,
    {
        Self::Capability {
            role,
            reply_to: reply_to.into(),
        }
    }

    /// Construct shutdown without a reply placeholder.
    #[must_use]
    pub const fn shutdown() -> Self {
        Self::Shutdown
    }
}
