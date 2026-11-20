//! Construction choices for statically typed customer replies.

use behavior::{EndpointAddress, EstablishedRecipient, Protocol};

use super::{DeliveryRoute, ReplyRoute};

mod sealed {
    pub trait ReplyRoutes {}
}

/// The destination capabilities accepted by one actor's reply protocols.
///
/// Owning constructors select exact endpoints for ordinary clients. Explicitly
/// enabling replies by address selects the mixed family instead. This choice fixes
/// the command and send types; it performs no resolution or delivery. Those
/// operations remain owned by [`DeliveryRoute`] and the interpreter.
pub trait ReplyRoutes: sealed::ReplyRoutes {
    /// One concrete capability for this reply protocol.
    type Route<P>: DeliveryRoute<Protocol = P>
    where
        P: Protocol,
        P::Addr: EndpointAddress;
}

/// Replies to direct recipients, without resolving actor addresses.
pub struct ExactReplies;

/// Replies accepting both actor addresses and direct recipients.
pub struct MixedReplies;

impl sealed::ReplyRoutes for ExactReplies {}
impl ReplyRoutes for ExactReplies {
    type Route<P>
        = EstablishedRecipient<P>
    where
        P: Protocol,
        P::Addr: EndpointAddress;
}

impl sealed::ReplyRoutes for MixedReplies {}
impl ReplyRoutes for MixedReplies {
    type Route<P>
        = ReplyRoute<P>
    where
        P: Protocol,
        P::Addr: EndpointAddress;
}
