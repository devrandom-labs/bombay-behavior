//! One-attempt diagnostic delivery or route-free terminal custody.

use core::convert::Infallible;

use behavior::{
    ActionItem, EndpointAddress, EstablishedRecipient, ExactDeliveryReason, InterpreterRequest,
    LogicalDeliveryReason, Never, NoReturnToEmitter, Protocol, Recipient,
};

mod sealed {
    pub trait DiagnosticRoute<Diagnostic> {}
}

/// One-attempt diagnostic delivery or route-free terminal custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticDisposition<Route> {
    /// Attempt each diagnostic once through this typed route.
    DeliverTo(Route),
    /// Transfer each diagnostic directly to terminal custody.
    Terminate,
}

impl<Route> DiagnosticDisposition<Route> {
    /// Attempt each diagnostic once through this typed route.
    #[must_use]
    pub const fn deliver_to(route: Route) -> Self {
        Self::DeliverTo(route)
    }

    pub(crate) fn action<Diagnostic>(
        &self,
        diagnostic: Diagnostic,
    ) -> DiagnosticAction<Route, Diagnostic>
    where
        Route: Clone,
    {
        match self {
            Self::DeliverTo(route) => DiagnosticAction::deliver(route.clone(), diagnostic),
            Self::Terminate => DiagnosticAction::terminal(diagnostic),
        }
    }
}

impl DiagnosticDisposition<Infallible> {
    /// Transfer diagnostics directly to terminal custody without a route.
    #[must_use]
    pub const fn terminate() -> Self {
        Self::Terminate
    }
}

/// Static rejection vocabulary selected by one diagnostic route family.
#[doc(hidden)]
pub trait DiagnosticRoute<Diagnostic>: sealed::DiagnosticRoute<Diagnostic> + Send + Sized {
    type Rejection: Send;
}

impl<P> sealed::DiagnosticRoute<P::Msg> for Recipient<P>
where
    P: Protocol,
    P::Addr: Send,
    P::Msg: Send,
{
}

impl<P> DiagnosticRoute<P::Msg> for Recipient<P>
where
    P: Protocol,
    P::Addr: Send,
    P::Msg: Send,
{
    type Rejection = LogicalDeliveryReason;
}

impl<P> sealed::DiagnosticRoute<P::Msg> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    P::Msg: Send,
{
}

impl<P> DiagnosticRoute<P::Msg> for EstablishedRecipient<P>
where
    P: Protocol,
    P::Addr: EndpointAddress,
    <P::Addr as EndpointAddress>::Established<P>: Send,
    P::Msg: Send,
{
    type Rejection = ExactDeliveryReason;
}

impl<Diagnostic> sealed::DiagnosticRoute<Diagnostic> for Infallible where Diagnostic: Send {}

impl<Diagnostic> DiagnosticRoute<Diagnostic> for Infallible
where
    Diagnostic: Send,
{
    type Rejection = Never;
}

/// One complete diagnostic selected for delivery or terminal custody.
///
/// A route for another diagnostic protocol cannot become an action item:
///
/// ```compile_fail,E0271
/// use behavior::{ActionItem, MailAddr, MessageProtocol, Recipient};
/// use behavior_actors::atomic::DiagnosticAction;
/// fn require_action<Item: ActionItem>(_: Item) {}
/// let route = Recipient::<MessageProtocol<MailAddr, u8>>::global(MailAddr(1));
/// require_action(DiagnosticAction::deliver(route, String::from("failed")));
/// ```
#[doc(hidden)]
#[must_use = "a diagnostic action must be interpreted or retained"]
pub enum DiagnosticAction<Route, Diagnostic> {
    Deliver {
        route: Route,
        diagnostic: Diagnostic,
    },
    Terminal {
        diagnostic: Diagnostic,
    },
}

impl<Route, Diagnostic> DiagnosticAction<Route, Diagnostic> {
    /// Construct one routed diagnostic without cloning its payload.
    #[doc(hidden)]
    #[must_use]
    pub const fn deliver(route: Route, diagnostic: Diagnostic) -> Self {
        Self::Deliver { route, diagnostic }
    }

    /// Construct one route-free terminal diagnostic.
    #[doc(hidden)]
    #[must_use]
    pub const fn terminal(diagnostic: Diagnostic) -> Self {
        Self::Terminal { diagnostic }
    }
}

impl<Route, Diagnostic> InterpreterRequest for DiagnosticAction<Route, Diagnostic>
where
    Route: DiagnosticRoute<Diagnostic>,
{
    type ReturnToEmitter = NoReturnToEmitter;
}

impl<Route, Diagnostic> ActionItem for DiagnosticAction<Route, Diagnostic>
where
    Route: DiagnosticRoute<Diagnostic>,
    Diagnostic: Send,
{
    type Accepted = DiagnosticAccepted<Diagnostic>;
    type Rejection = Route::Rejection;
    type Prerequisite = Never;
}

/// Complete accepted disposition of one diagnostic action.
#[doc(hidden)]
#[must_use = "terminal diagnostic custody must be retained"]
pub enum DiagnosticAccepted<Diagnostic> {
    Delivered,
    Terminal(Diagnostic),
}

impl<Diagnostic> DiagnosticAccepted<Diagnostic> {
    /// Record one accepted routed delivery.
    #[doc(hidden)]
    #[must_use]
    pub const fn delivered() -> Self {
        Self::Delivered
    }

    /// Transfer one complete diagnostic into terminal custody.
    #[doc(hidden)]
    #[must_use]
    pub const fn terminal(diagnostic: Diagnostic) -> Self {
        Self::Terminal(diagnostic)
    }
}
