//! Action-producing peer termination observation.

use core::mem;
use std::sync::Arc;

use crate::{
    CancelObservation, EstablishedObservation, ObservationAuthority, ObservationId,
    ObservationOperation, ObservationRejection, ObservationRelationship, ObserveEstablished,
    ObservePeer, PeerStopped, WatchEvent,
};
use behavior::{
    Actions, Address, Behavior, BehaviorActed, BirthMode, EndpointAddress, EventLayer,
    InterpreterRequest, InterpreterRequests, ReturnsToEmitter, SendEffects, SendLayer,
};

/// Pure reaction applied to the exact matching terminal report.
pub type TerminationReaction<B> = fn(
    &mut B,
    PeerStopped<behavior::BehaviorAddr<B>>,
) -> Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

/// Complete consumption phase of one exact terminal observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationObservation {
    /// The exact request is retained for emission or awaits its first response.
    Requested,
    /// The configured peer's terminal report has not been accepted.
    Observing,
    /// One matching terminal report was accepted and cannot be accepted again.
    Observed,
    /// The exact observation relationship was cancelled before termination.
    Cancelled,
    /// The exact observation operation was rejected.
    Rejected {
        operation: ObservationOperation,
        reason: ObservationRejection,
    },
}

/// Exact rejection from the observation wrapper.
#[derive(thiserror::Error)]
pub enum TerminationMonitorError<E, Report> {
    /// The wrapped behavior rejected its own event.
    #[error("wrapped behavior rejected its event")]
    Inner(#[source] E),
    /// A returned observation report does not belong to the current phase or
    /// configured relationship.
    #[error("observation report does not match the active relationship phase")]
    UnexpectedReport {
        observation: TerminationObservation,
        report: Report,
    },
}

impl<E: core::fmt::Debug, Report> core::fmt::Debug for TerminationMonitorError<E, Report> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inner(error) => formatter.debug_tuple("Inner").field(error).finish(),
            Self::UnexpectedReport { observation, .. } => formatter
                .debug_struct("UnexpectedReport")
                .field("observation", observation)
                .field("report", &"<retained>")
                .finish(),
        }
    }
}

pub(crate) mod sealed {
    pub trait TerminationObservationTarget<B: behavior::Behavior> {}
}

/// Static observation and reaction policy for [`TerminationMonitorWith`].
pub trait TerminationObservationTarget<B: Behavior>:
    sealed::TerminationObservationTarget<B>
{
    type Report;
    type Request: InterpreterRequest<
        ReturnToEmitter = ReturnsToEmitter<Self::Report, behavior::Here>,
    >;

    fn request(&mut self) -> Option<Self::Request>;
    fn observation(&self) -> TerminationObservation;
    fn react(
        &mut self,
        inner: &mut B,
        report: Self::Report,
    ) -> Result<Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>, Self::Report>;
}

enum LogicalTermination<A: Address> {
    Observing(A),
    Observed(A),
}

/// Address-selected legacy termination observation.
pub struct LogicalTerminationTarget<B: Behavior> {
    observation: LogicalTermination<behavior::BehaviorAddr<B>>,
    react: TerminationReaction<B>,
}

impl<B: Behavior> sealed::TerminationObservationTarget<B> for LogicalTerminationTarget<B> {}

impl<B: Behavior> TerminationObservationTarget<B> for LogicalTerminationTarget<B> {
    type Report = PeerStopped<behavior::BehaviorAddr<B>>;
    type Request = ObservePeer<behavior::BehaviorAddr<B>>;

    fn request(&mut self) -> Option<Self::Request> {
        let peer = match &self.observation {
            LogicalTermination::Observing(peer) | LogicalTermination::Observed(peer) => *peer,
        };
        Some(ObservePeer::new(peer))
    }

    fn observation(&self) -> TerminationObservation {
        match self.observation {
            LogicalTermination::Observing(_) => TerminationObservation::Observing,
            LogicalTermination::Observed(_) => TerminationObservation::Observed,
        }
    }

    fn react(
        &mut self,
        inner: &mut B,
        report: Self::Report,
    ) -> Result<Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>, Self::Report> {
        let LogicalTermination::Observing(peer) = &self.observation else {
            return Err(report);
        };
        if report.peer != *peer {
            return Err(report);
        }
        self.observation = LogicalTermination::Observed(*peer);
        Ok((self.react)(inner, report))
    }
}

/// Action-producing reaction to the matching exact terminal report.
///
/// The value is always [`EstablishedObservation::Stopped`]. The complete enum
/// keeps the protocol and correlation visible in the callback type without
/// introducing a parallel terminal payload.
pub type EstablishedTerminationReaction<B, P> = fn(
    &mut B,
    EstablishedObservation<P>,
) -> Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    <B as Behavior>::Sends,
    <B as Behavior>::Birth,
>;

enum EstablishedTermination<P>
where
    P: behavior::Protocol,
    P::Addr: behavior::RecipientAddress,
{
    Unissued(ObserveEstablished<P>),
    Requested { correlation: Arc<ObservationId> },
    Observing(ObservationAuthority<P>),
    CancelPending(ObservationRelationship<P>),
    CancelRejectedWaiting(CancelObservation<P>),
    StoppedAwaitingCancel(ObservationRelationship<P>),
    ObserveRejected(ObserveEstablished<P>),
    Stopped,
    StoppedWithCancelRejected(CancelObservation<P>),
    Cancelled,
}

/// Exact-incarnation termination observation policy and its one current value.
pub struct EstablishedTerminationTarget<B: Behavior, P>
where
    P: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
    behavior::BehaviorAddr<B>: EndpointAddress,
{
    observation: EstablishedTermination<P>,
    react: EstablishedTerminationReaction<B, P>,
}

impl<B, P> sealed::TerminationObservationTarget<B> for EstablishedTerminationTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
    behavior::BehaviorAddr<B>: EndpointAddress,
{
}

impl<B, P> TerminationObservationTarget<B> for EstablishedTerminationTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
    behavior::BehaviorAddr<B>: EndpointAddress,
{
    type Report = EstablishedObservation<P>;
    type Request = ObserveEstablished<P>;

    fn request(&mut self) -> Option<Self::Request> {
        self.request_once()
    }
    fn observation(&self) -> TerminationObservation {
        self.current_observation()
    }
    fn react(
        &mut self,
        inner: &mut B,
        report: Self::Report,
    ) -> Result<Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>, Self::Report> {
        self.accept_report(inner, report)
    }
}

impl<B, P> EstablishedTerminationTarget<B, P>
where
    B: Behavior,
    P: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
    behavior::BehaviorAddr<B>: EndpointAddress,
{
    /// Transfer the one current grant into a typed cancellation request.
    /// The caller must return that request in its owning Actions lane.
    #[must_use]
    pub fn take_cancellation(&mut self) -> Option<CancelObservation<P>> {
        let EstablishedTermination::Observing(authority) = &self.observation else {
            return None;
        };
        let pending = EstablishedTermination::CancelPending(authority.relationship().clone());
        match mem::replace(&mut self.observation, pending) {
            EstablishedTermination::Observing(authority) => Some(CancelObservation::new(authority)),
            _ => unreachable!("the exclusive target held its one observed grant"),
        }
    }

    /// Recover the whole rejected start or return every current target value.
    pub fn into_rejected_observe(
        self,
    ) -> Result<(ObserveEstablished<P>, ObservationRejection), Self> {
        match self {
            Self {
                observation: EstablishedTermination::ObserveRejected(request),
                ..
            } => Ok((request, ObservationRejection::IdAlreadyBound)),
            other => Err(other),
        }
    }

    /// Recover the whole rejected cancellation or return the current target.
    pub fn into_rejected_cancel(
        self,
    ) -> Result<(CancelObservation<P>, ObservationRejection), Self> {
        match self {
            Self {
                observation:
                    EstablishedTermination::CancelRejectedWaiting(request)
                    | EstablishedTermination::StoppedWithCancelRejected(request),
                ..
            } => Ok((request, ObservationRejection::NotObserved)),
            other => Err(other),
        }
    }

    fn request_once(&mut self) -> Option<ObserveEstablished<P>> {
        let EstablishedTermination::Unissued(request) = &self.observation else {
            return None;
        };
        let requested = EstablishedTermination::Requested {
            correlation: request.correlation().clone(),
        };
        match mem::replace(&mut self.observation, requested) {
            EstablishedTermination::Unissued(request) => Some(request),
            _ => unreachable!("the exclusive target held its unissued request"),
        }
    }

    fn current_observation(&self) -> TerminationObservation {
        match &self.observation {
            EstablishedTermination::Unissued(_) | EstablishedTermination::Requested { .. } => {
                TerminationObservation::Requested
            }
            EstablishedTermination::Observing(_)
            | EstablishedTermination::CancelPending(_)
            | EstablishedTermination::CancelRejectedWaiting(..) => {
                TerminationObservation::Observing
            }
            EstablishedTermination::Stopped
            | EstablishedTermination::StoppedAwaitingCancel(_)
            | EstablishedTermination::StoppedWithCancelRejected(..) => {
                TerminationObservation::Observed
            }
            EstablishedTermination::Cancelled => TerminationObservation::Cancelled,
            EstablishedTermination::ObserveRejected(_) => TerminationObservation::Rejected {
                operation: ObservationOperation::Start,
                reason: ObservationRejection::IdAlreadyBound,
            },
        }
    }

    fn accept_report(
        &mut self,
        inner: &mut B,
        report: EstablishedObservation<P>,
    ) -> Result<
        Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        EstablishedObservation<P>,
    > {
        match report {
            EstablishedObservation::Started { authority } => match &self.observation {
                EstablishedTermination::Requested { correlation }
                    if authority.matches_request(correlation) =>
                {
                    self.observation = EstablishedTermination::Observing(authority);
                    Ok(Actions::cont())
                }
                _ => Err(EstablishedObservation::Started { authority }),
            },
            EstablishedObservation::ObserveRejected { request, reason } => {
                match &self.observation {
                    EstablishedTermination::Requested { correlation }
                        if reason == ObservationRejection::IdAlreadyBound
                            && Arc::ptr_eq(request.correlation(), correlation) =>
                    {
                        self.observation = EstablishedTermination::ObserveRejected(request);
                        Ok(Actions::cont())
                    }
                    _ => Err(EstablishedObservation::ObserveRejected { request, reason }),
                }
            }
            EstablishedObservation::CancelRejected { request, reason } => match &self.observation {
                EstablishedTermination::CancelPending(expected)
                    if reason == ObservationRejection::NotObserved
                        && expected == request.relationship() =>
                {
                    self.observation = EstablishedTermination::CancelRejectedWaiting(request);
                    Ok(Actions::cont())
                }
                EstablishedTermination::StoppedAwaitingCancel(expected)
                    if reason == ObservationRejection::NotObserved
                        && expected == request.relationship() =>
                {
                    self.observation = EstablishedTermination::StoppedWithCancelRejected(request);
                    Ok(Actions::cont())
                }
                _ => Err(EstablishedObservation::CancelRejected { request, reason }),
            },
            EstablishedObservation::Cancelled { relationship } => match &self.observation {
                EstablishedTermination::CancelPending(expected) if expected == &relationship => {
                    self.observation = EstablishedTermination::Cancelled;
                    drop(relationship);
                    Ok(Actions::cont())
                }
                _ => Err(EstablishedObservation::Cancelled { relationship }),
            },
            stopped @ EstablishedObservation::Stopped { .. } => {
                let EstablishedObservation::Stopped { relationship, .. } = &stopped else {
                    unreachable!("this branch owns a complete Stopped report");
                };
                match &self.observation {
                    EstablishedTermination::Observing(authority)
                        if authority.relationship() == relationship =>
                    {
                        self.observation = EstablishedTermination::Stopped;
                    }
                    EstablishedTermination::CancelPending(expected) if expected == relationship => {
                        self.observation =
                            EstablishedTermination::StoppedAwaitingCancel(expected.clone());
                    }
                    EstablishedTermination::CancelRejectedWaiting(request)
                        if request.relationship() == relationship =>
                    {
                        let terminal =
                            mem::replace(&mut self.observation, EstablishedTermination::Stopped);
                        let EstablishedTermination::CancelRejectedWaiting(request) = terminal
                        else {
                            unreachable!("the exclusive target retained its rejected cancellation");
                        };
                        self.observation =
                            EstablishedTermination::StoppedWithCancelRejected(request);
                    }
                    _ => return Err(stopped),
                }
                // Every still-owned rejection is already in the genuine next
                // state before an application callback may consume/panic.
                Ok((self.react)(inner, stopped))
            }
        }
    }
}

/// Observe one peer and apply its terminal report to complete behavior actions.
///
/// Unlike [`crate::Watch`], the reaction returns the wrapped behavior's full
/// [`Actions`]. Cleanup communications, lifecycle publication, fresh
/// creations, and termination therefore remain explicit behavior decisions.
/// The selected target owns either late-bound logical-name observation or
/// exact-incarnation observation; the runtime delivers the target's declared
/// typed report through the interpreter-request return path.
/// Reactions are infallible because they receive mutable access to `B`: a
/// fallible callback could change `B` and then reject the same report, violating
/// transition atomicity. Ordinary delegated `B` transitions retain `B::Error`.
///
/// ```compile_fail,E0308
/// # struct App;
/// # impl behavior::Protocol for App { type Addr = behavior::MailAddr; type Msg = (); }
/// # impl behavior::Behavior for App {
/// #   type Protocol = Self; type Event = behavior::User<behavior::MailAddr, ()>; type Sends = Vec<behavior::Never>;
/// #   type Ph = behavior::Never; type Error = behavior::Never; type Birth = behavior::NoBirths;
/// #   fn transition(&mut self, _: behavior::ActiveTurn, _: Self::Event) -> behavior::BehaviorActed<Self> { Ok(behavior::Actions::cont()) }
/// # }
/// fn fallible(_: &mut App, _: behavior_actors::PeerStopped<behavior::MailAddr>) -> behavior::BehaviorActed<App> {
///     Ok(behavior::Actions::cont())
/// }
/// let _ = behavior_actors::TerminationMonitor::new(App, behavior::MailAddr(1), fallible);
/// ```
pub struct TerminationMonitorWith<B: Behavior, Target: TerminationObservationTarget<B>> {
    inner: B,
    target: Target,
}

/// Address-selected action-producing termination monitor.
pub type TerminationMonitor<B> = TerminationMonitorWith<B, LogicalTerminationTarget<B>>;

/// Exact-incarnation action-producing termination monitor.
pub type EstablishedTerminationMonitor<B, P> =
    TerminationMonitorWith<B, EstablishedTerminationTarget<B, P>>;

type TerminationMonitorActions<B, Target> = Actions<
    behavior::BehaviorAddr<B>,
    <B as Behavior>::Ph,
    SendLayer<
        InterpreterRequests<<Target as TerminationObservationTarget<B>>::Request>,
        <B as Behavior>::Sends,
    >,
    <B as Behavior>::Birth,
>;

impl<B: Behavior> TerminationMonitorWith<B, LogicalTerminationTarget<B>> {
    /// Construct an action-producing observation definition.
    #[must_use]
    pub const fn new(
        inner: B,
        peer: behavior::BehaviorAddr<B>,
        on_stopped: TerminationReaction<B>,
    ) -> Self {
        Self {
            inner,
            target: LogicalTerminationTarget {
                observation: LogicalTermination::Observing(peer),
                react: on_stopped,
            },
        }
    }
}

impl<B, P> TerminationMonitorWith<B, EstablishedTerminationTarget<B, P>>
where
    B: Behavior,
    P: behavior::Protocol<Addr = behavior::BehaviorAddr<B>>,
    behavior::BehaviorAddr<B>: EndpointAddress,
{
    #[must_use]
    pub fn established(
        inner: B,
        request: ObserveEstablished<P>,
        react: EstablishedTerminationReaction<B, P>,
    ) -> Self {
        Self {
            inner,
            target: EstablishedTerminationTarget {
                observation: EstablishedTermination::Unissued(request),
                react,
            },
        }
    }

    /// Transfer the target's one cancellation request for an enclosing Actions lane.
    #[must_use]
    pub fn take_cancellation(&mut self) -> Option<CancelObservation<P>> {
        self.target.take_cancellation()
    }
}

impl<B: Behavior, Target: TerminationObservationTarget<B>> TerminationMonitorWith<B, Target> {
    /// Return whether this monitor awaits or has consumed its terminal report.
    #[must_use]
    pub fn observation(&self) -> TerminationObservation {
        self.target.observation()
    }

    /// Recover the exact inner behavior and sole current observation target.
    #[must_use]
    pub fn into_parts(self) -> (B, Target) {
        (self.inner, self.target)
    }

    fn wrap(
        actions: Actions<behavior::BehaviorAddr<B>, B::Ph, B::Sends, B::Birth>,
        observations: InterpreterRequests<Target::Request>,
    ) -> TerminationMonitorActions<B, Target> {
        actions.map_sends(|inner| SendLayer::new(observations, inner))
    }
}

impl<B, Target> behavior::BehaviorBase for TerminationMonitorWith<B, Target>
where
    B: Behavior + behavior::BehaviorBase,
    Target: TerminationObservationTarget<B>,
{
    type Base = B::Base;

    fn base(&self) -> &Self::Base {
        self.inner.base()
    }
}

impl<B, Target> crate::StashStatus for TerminationMonitorWith<B, Target>
where
    B: Behavior + crate::StashStatus,
    Target: TerminationObservationTarget<B>,
{
    fn stashed_messages(&self) -> usize {
        self.inner.stashed_messages()
    }
}

impl<B, Target, A, Ph, Sends, Br> Behavior for TerminationMonitorWith<B, Target>
where
    A: Address,
    Sends: SendEffects + behavior::SendsFor<B::Event>,
    Br: BirthMode,
    B: Behavior<Ph = Ph, Sends = Sends, Birth = Br>,
    B::Protocol: behavior::Protocol<Addr = A>,
    Target: TerminationObservationTarget<B>,
{
    type Protocol = B::Protocol;
    type Event = WatchEvent<B::Event, Target::Report>;
    type Sends = SendLayer<InterpreterRequests<Target::Request>, Sends>;
    type Ph = Ph;
    type Error = TerminationMonitorError<B::Error, Target::Report>;
    type Birth = Br;

    fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
        let actions =
            behavior::initialize(&mut self.inner).map_err(TerminationMonitorError::Inner)?;
        Ok(Self::wrap(
            actions,
            match self.target.request() {
                Some(request) => InterpreterRequests::one(request),
                None => InterpreterRequests::empty(),
            },
        ))
    }

    fn transition(&mut self, _: behavior::ActiveTurn, event: Self::Event) -> BehaviorActed<Self> {
        match event {
            EventLayer::Owned(report) => {
                let actions = self
                    .target
                    .react(&mut self.inner, report)
                    .map_err(|report| TerminationMonitorError::UnexpectedReport {
                        observation: self.target.observation(),
                        report,
                    })?;
                Ok(Self::wrap(actions, InterpreterRequests::empty()))
            }
            EventLayer::Inner(event) => behavior::delegate_transition(&mut self.inner, event)
                .map(|actions| Self::wrap(actions, InterpreterRequests::empty()))
                .map_err(TerminationMonitorError::Inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Activate as _;
    use crate::{Crash, Exit};
    use behavior::{
        Births, CreateChild, CreationId, CreationSequence, Creations, MailAddr, Never, Step, User,
    };

    fn worker_creation() -> CreationId {
        CreationSequence::new()
            .issue()
            .expect("the first worker creation ID exists")
    }

    struct Probe;

    impl behavior::BehaviorBase for Probe {
        type Base = Self;

        fn base(&self) -> &Self {
            self
        }
    }

    impl behavior::Protocol for Probe {
        type Addr = MailAddr;
        type Msg = u8;
    }

    impl Behavior for Probe {
        type Protocol = Self;
        type Event = User<MailAddr, u8>;
        type Sends = Vec<u8>;
        type Ph = Never;
        type Error = Never;
        type Birth = Births<()>;

        fn init(&mut self, _: behavior::InitializationTurn) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![1]))
        }

        fn transition(
            &mut self,
            _: behavior::ActiveTurn,
            event: Self::Event,
        ) -> BehaviorActed<Self> {
            Ok(Actions::send(vec![event.message]))
        }
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the reaction contract transfers ownership of the complete terminal report"
    )]
    fn reap(
        _: &mut Probe,
        stopped: PeerStopped<MailAddr>,
    ) -> Actions<MailAddr, Never, Vec<u8>, Births<()>> {
        assert_eq!(stopped.peer, MailAddr(4));
        assert_eq!(stopped.outcome, Err(Crash::Panicked));
        Actions::new(
            vec![9],
            Creations::one(CreateChild::birth(worker_creation(), ())),
            Step::Continue,
        )
    }

    #[test]
    fn matching_terminal_report_preserves_complete_reaction_actions() {
        let initialized = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap();
        assert_eq!(initialized.actions.sends.inner, [1]);
        assert_eq!(
            initialized.actions.sends.owned,
            InterpreterRequests::one(crate::ObservePeer::new(MailAddr(4)))
        );

        let mut active = initialized.behavior;
        let actions = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::Panicked)))
            .unwrap();
        assert_eq!(actions.sends.inner, [9]);
        assert!(actions.sends.owned.is_empty());
        assert_eq!(
            actions.creates,
            Creations::one(CreateChild::birth(worker_creation(), ()))
        );
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(active.observation(), TerminationObservation::Observed);

        let duplicate = PeerStopped::new(MailAddr(4), Err(Crash::Panicked));
        assert!(matches!(
            active.on_path(duplicate.clone()),
            Err(TerminationMonitorError::UnexpectedReport {
                observation: TerminationObservation::Observed,
                report,
            }) if report == duplicate
        ));
    }

    #[allow(
        clippy::needless_pass_by_value,
        reason = "the reaction explicitly consumes the complete terminal report"
    )]
    fn acknowledge_capability_failure(
        _: &mut Probe,
        stopped: PeerStopped<MailAddr>,
    ) -> Actions<MailAddr, Never, Vec<u8>, Births<()>> {
        assert_eq!(stopped.peer, MailAddr(4));
        assert_eq!(stopped.outcome, Err(Crash::CapabilityFailed));
        Actions::send(vec![9])
    }

    #[test]
    fn capability_failure_reaches_the_reaction_once_without_reclassification() {
        let initialized =
            crate::TerminationMonitor::new(Probe, MailAddr(4), acknowledge_capability_failure)
                .initialize()
                .unwrap();
        assert_eq!(initialized.actions.sends.inner, [1]);
        assert_eq!(
            initialized.actions.sends.owned,
            InterpreterRequests::one(crate::ObservePeer::new(MailAddr(4)))
        );
        assert!(initialized.actions.creates.is_empty());
        assert!(matches!(initialized.actions.become_, Step::Continue));
        let mut active = initialized.behavior;
        let actions = active
            .on_path(PeerStopped::new(MailAddr(4), Err(Crash::CapabilityFailed)))
            .unwrap();
        assert_eq!(actions.sends.inner, [9]);
        assert!(actions.sends.owned.is_empty());
        assert!(actions.creates.is_empty());
        assert!(matches!(actions.become_, Step::Continue));
        assert_eq!(active.observation(), TerminationObservation::Observed);

        let duplicate = PeerStopped::new(MailAddr(4), Err(Crash::CapabilityFailed));
        let rejected = active.on_path(duplicate.clone());
        assert!(matches!(
            rejected,
            Err(TerminationMonitorError::UnexpectedReport {
                observation: TerminationObservation::Observed,
                report,
            }) if report == duplicate
        ));
    }

    #[test]
    fn unmatched_terminal_report_is_returned_complete_and_user_actions_still_delegate() {
        let mut active = crate::TerminationMonitor::new(Probe, MailAddr(4), reap)
            .initialize()
            .unwrap()
            .behavior;
        let unmatched = PeerStopped::new(MailAddr(5), Ok(Exit::Normal));
        assert!(matches!(
            active.on_path(unmatched.clone()),
            Err(TerminationMonitorError::UnexpectedReport {
                observation: TerminationObservation::Observing,
                report,
            }) if report == unmatched
        ));

        let delegated = active.receive(MailAddr(0), 7).unwrap();
        assert_eq!(delegated.sends.inner, [7]);
        assert!(delegated.sends.owned.is_empty());
    }
}
