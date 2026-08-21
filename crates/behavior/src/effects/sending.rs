//! Typed send effects, their composition contract, and event ownership.

use crate::{ComposedEvent, Delivery, InjectEvent, Inside, Protocol};
use core::future::Future;

/// Error domain shared by one concrete effect interpreter.
pub trait SendInterpreter: Send {
    type Error;
}

/// Static interpretation of one complete sends value at an absolute event path.
///
/// Implementations are monomorphized over `Interpreter`; there is no erased
/// envelope, runtime lane lookup, or downcast. `RootEvent` is the event type
/// ultimately enqueued for the actor and `Path` is the current send owner's
/// absolute position in it. Composite products must visit every constituent
/// lane exactly once without discarding either index.
pub trait InterpretSends<Interpreter: SendInterpreter, RootEvent, Path>: Sized + Send {
    /// Interpret every value in this product in its defined structural order.
    /// The returned future completes each effect before beginning the next, so
    /// an asynchronous delivery can wait for bounded-mailbox capacity without
    /// reordering later effects or converting pressure into rejection.
    ///
    /// # Errors
    /// Returns the interpreter's concrete error without consuming later lanes.
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send;
}

/// Interpreter capability for one request at one exact root-event path.
pub trait InterpretRequest<Request, RootEvent, Path>: SendInterpreter {
    /// Interpret one request emitted by the actor hosted by this interpreter.
    /// The returned future remains concrete and may await runtime capacity or
    /// other interpreter-owned asynchronous work.
    /// A returning implementation can require
    /// `RootEvent: InjectEvent<ReturnedFact, Path>` and retain that exact
    /// constructor for later completion.
    ///
    /// # Errors
    /// Returns this interpreter's concrete request failure.
    fn interpret_request(
        &mut self,
        request: Request,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// Interpreter capability for deliveries to one concrete actor protocol.
///
/// `P` is preserved unchanged from [`Delivery<P>`] and is the destination's
/// canonical hosting identity. A creator-local child route changes only how
/// the address is resolved; it does not introduce a role-keyed delivery lane.
pub trait InterpretDelivery<P: Protocol>: SendInterpreter {
    /// Interpret one typed delivery, awaiting bounded-mailbox capacity when
    /// required by the concrete communication transport.
    ///
    /// # Errors
    /// Returns this interpreter's concrete delivery failure.
    fn interpret_delivery(
        &mut self,
        delivery: Delivery<P>,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

/// The lane owned by the current named send product.
pub enum Own {}

/// Static evidence that a sends type contains one request lane.
///
/// Implementations append the input exactly once to that lane and leave every
/// other lane unchanged. `Path` distinguishes repeated request types without
/// erasing their position or choosing a lane at runtime.
///
/// [`Own`] selects a named product's own semantic lane. [`SendLayer`] carries
/// wrapper-owned and inner effects as explicit named fields; request routing
/// remains a compile-time proof rather than a runtime lane lookup.
pub trait SendInput<Input, Path> {
    fn emit(&mut self, input: Input);
}

/// Send effects emitted by a pure actor transition.
///
/// Values compose without interpreting them. This keeps communications and
/// interpreter requests inside the explicit [`crate::Actions`] boundary.
pub trait SendEffects: Sized {
    fn empty() -> Self;
    fn append(&mut self, other: Self);

    #[must_use]
    fn combine(mut self, other: Self) -> Self {
        self.append(other);
        self
    }

    /// Append one request to its statically selected semantic lane.
    fn send<Input, Path>(&mut self, input: Input)
    where
        Self: SendInput<Input, Path>,
    {
        <Self as SendInput<Input, Path>>::emit(self, input);
    }

    /// Build a send product containing one request in its selected lane.
    #[must_use]
    fn sending<Input, Path>(input: Input) -> Self
    where
        Self: SendInput<Input, Path>,
    {
        let mut sends = Self::empty();
        sends.send(input);
        sends
    }
}

/// Proof that send effects are lawful for one complete event type.
///
/// Ordinary communications are independent of `Event`. Interpreter requests
/// that return a local fact are not: their continuation must select an exact
/// member of `Event`. Composite products implement this trait structurally,
/// reindexing only their wrapped behavior effects through an outer event
/// injection.
///
/// An un-reindexed return to the emitter cannot be paired with an added outer event
/// layer:
///
/// ```compile_fail
/// use behavior::{SendsFor, EventLayer, Here, MailAddr, ReturnsToEmitter,
///     InterpreterRequest, InterpreterRequests, User};
/// struct Request;
/// impl InterpreterRequest for Request {
///     type ReturnToEmitter = ReturnsToEmitter<u8, Here>;
/// }
/// fn lawful<E, F: SendsFor<E>>() {}
/// type Inner = EventLayer<u8, User<MailAddr, ()>>;
/// type Outer = EventLayer<(), Inner>;
/// lawful::<Outer, InterpreterRequests<Request>>();
/// ```
pub trait SendsFor<Event>: SendEffects {}

/// An interpreter request that produces no later fact for the emitting actor.
pub enum NoReturnToEmitter {}

/// An interpreter request whose later `Input` returns to the emitting actor at
/// `Path`, relative to the effect lane that owns the request.
pub struct ReturnsToEmitter<Input, Path>(core::marker::PhantomData<fn(Input, Path)>);

/// Local-return contract declared by one interpreter-facing request.
pub trait ReturnToEmitterFor<Event> {}

impl<Event> ReturnToEmitterFor<Event> for NoReturnToEmitter {}

impl<Event, Input, Path> ReturnToEmitterFor<Event> for ReturnsToEmitter<Input, Path> where
    Event: InjectEvent<Input, Path>
{
}

/// Declares only the continuation returning to the actor that emitted this
/// interpreter request. Destinations owned by a child, parent, ancestor, or
/// established actor are separate capabilities and are not reindexed when the
/// emitter is wrapped.
pub trait InterpreterRequest {
    type ReturnToEmitter;
}

/// Send effects containing no communications or interpreter requests.
///
/// A behavior layer that adds an event lane but emits nothing of its own uses
/// this named value rather than an ambiguous `()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoSends;

impl SendEffects for NoSends {
    fn empty() -> Self {
        Self
    }

    fn append(&mut self, _: Self) {}
}

impl<Event> SendsFor<Event> for NoSends {}

impl<Interpreter: SendInterpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path>
    for NoSends
{
    fn interpret(
        self,
        _: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async { Ok(()) }
    }
}

/// Send effects introduced by one wrapper around inner send effects.
///
/// `owned` contains effects introduced by the current behavior layer and
/// `inner` contains effects of the wrapped interaction. The structural
/// [`SendsFor`] implementation is the composition law:
///
/// ```text
/// Event'   = OwnedEvent + InnerEvent
/// Effects' = OwnedEffects × InnerEffects
/// ```
///
/// Owned return to the emitters target `Event'`; inner return to the emitters target
/// `InnerEvent` and are therefore lifted through the same `Inner` injection.
/// Established actor, child, and ancestor destinations are unaffected because
/// they are not return to the emitters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendLayer<Owned, Inner> {
    pub owned: Owned,
    pub inner: Inner,
}

impl<Owned, Inner> SendLayer<Owned, Inner> {
    #[must_use]
    pub const fn new(owned: Owned, inner: Inner) -> Self {
        Self { owned, inner }
    }
}

impl<Owned: SendEffects, Inner: SendEffects> SendEffects for SendLayer<Owned, Inner> {
    fn empty() -> Self {
        Self::new(Owned::empty(), Inner::empty())
    }

    fn append(&mut self, other: Self) {
        self.owned.append(other.owned);
        self.inner.append(other.inner);
    }
}

impl<Event, OwnedEffects, InnerEffects> SendsFor<Event> for SendLayer<OwnedEffects, InnerEffects>
where
    Event: ComposedEvent,
    OwnedEffects: SendsFor<Event>,
    InnerEffects: SendsFor<Event::Inner>,
{
}

impl<Input, Path, Owned, Inner> SendInput<Input, Path> for SendLayer<Owned, Inner>
where
    Owned: SendInput<Input, Path>,
{
    fn emit(&mut self, input: Input) {
        self.owned.emit(input);
    }
}

impl<Interpreter, RootEvent, Path, Owned, Inner> InterpretSends<Interpreter, RootEvent, Path>
    for SendLayer<Owned, Inner>
where
    Interpreter: SendInterpreter,
    Interpreter: Send,
    Owned: InterpretSends<Interpreter, RootEvent, Path> + Send,
    Inner: InterpretSends<Interpreter, RootEvent, Inside<Path>> + Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            // Initialization and delegated transition effects retain authored
            // inner-to-outer order. Failure short-circuits before later effects.
            <Inner as InterpretSends<Interpreter, RootEvent, Inside<Path>>>::interpret(
                self.inner,
                interpreter,
            )
            .await?;
            <Owned as InterpretSends<Interpreter, RootEvent, Path>>::interpret(
                self.owned,
                interpreter,
            )
            .await
        }
    }
}

impl<T> SendEffects for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }

    fn append(&mut self, mut other: Self) {
        Vec::append(self, &mut other);
    }
}

impl<Event, T> SendsFor<Event> for Vec<T> {}

impl<Interpreter, RootEvent, Path, P> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<Delivery<P>>
where
    Interpreter: InterpretDelivery<P>,
    Interpreter: Send,
    P: Protocol,
    Delivery<P>: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            for delivery in self {
                interpreter.interpret_delivery(delivery).await?;
            }
            Ok(())
        }
    }
}

impl<Interpreter: SendInterpreter, RootEvent, Path> InterpretSends<Interpreter, RootEvent, Path>
    for Vec<crate::Never>
{
    fn interpret(
        self,
        _: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            for never in self {
                match never {}
            }
            Ok(())
        }
    }
}

impl<T> SendInput<T, Own> for Vec<T> {
    fn emit(&mut self, input: T) {
        self.push(input);
    }
}

/// Requests interpreted by the runtime local to the emitting actor.
///
/// Unlike [`crate::Delivery`], a interpreter request has no actor address. Its
/// recipient is definitionally the interpreter of the actor whose transition
/// emitted it. This distinct send lane lets interpreters route ordinary
/// deliveries and interpreter requests with disjoint static implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpreterRequests<M> {
    requests: Vec<M>,
}

impl<M> InterpreterRequests<M> {
    #[must_use]
    pub fn new(requests: Vec<M>) -> Self {
        Self { requests }
    }
    #[must_use]
    pub fn one(request: M) -> Self {
        Self::new(vec![request])
    }
    #[must_use]
    pub fn as_slice(&self) -> &[M] {
        &self.requests
    }
    pub fn iter(&self) -> core::slice::Iter<'_, M> {
        self.requests.iter()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.requests.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
    pub fn extend(&mut self, requests: impl IntoIterator<Item = M>) {
        self.requests.extend(requests);
    }
    #[must_use]
    pub fn into_requests(self) -> Vec<M> {
        self.requests
    }
}

impl<M> core::ops::Index<usize> for InterpreterRequests<M> {
    type Output = M;
    fn index(&self, index: usize) -> &Self::Output {
        &self.requests[index]
    }
}

impl<M> IntoIterator for InterpreterRequests<M> {
    type Item = M;
    type IntoIter = std::vec::IntoIter<M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.into_iter()
    }
}

impl<'a, M> IntoIterator for &'a InterpreterRequests<M> {
    type Item = &'a M;
    type IntoIter = core::slice::Iter<'a, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.iter()
    }
}

impl<M> SendEffects for InterpreterRequests<M> {
    fn empty() -> Self {
        Self::new(Vec::new())
    }
    fn append(&mut self, mut other: Self) {
        self.requests.append(&mut other.requests);
    }
}

impl<Event, M> SendsFor<Event> for InterpreterRequests<M>
where
    M: InterpreterRequest,
    M::ReturnToEmitter: ReturnToEmitterFor<Event>,
{
}

impl<M> SendInput<M, Own> for InterpreterRequests<M> {
    fn emit(&mut self, input: M) {
        self.requests.push(input);
    }
}

impl<Interpreter, RootEvent, Path, Request> InterpretSends<Interpreter, RootEvent, Path>
    for InterpreterRequests<Request>
where
    Interpreter: InterpretRequest<Request, RootEvent, Path>,
    Interpreter: Send,
    Request: Send,
{
    fn interpret(
        self,
        interpreter: &mut Interpreter,
    ) -> impl Future<Output = Result<(), Interpreter::Error>> + Send {
        async move {
            for request in self {
                <Interpreter as InterpretRequest<Request, RootEvent, Path>>::interpret_request(
                    interpreter,
                    request,
                )
                .await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_accumulation_obeys_identity_and_associativity() {
        let values = vec![1, 2];
        assert_eq!(Vec::new().combine(values.clone()), values);
        assert_eq!(values.clone().combine(Vec::new()), values);

        let left = vec![1].combine(vec![2]).combine(vec![3]);
        let right = vec![1].combine(vec![2].combine(vec![3]));
        assert_eq!(left, right);
    }

    #[test]
    fn vector_and_service_lanes_emit_and_iterate_in_order() {
        assert!(<Vec<u8> as SendEffects>::empty().is_empty());
        let mut vector = Vec::new();
        <Vec<u8> as SendInput<u8, Own>>::emit(&mut vector, 1);
        assert_eq!(vector, [1]);

        let mut services = InterpreterRequests::one(2);
        services.extend([4, 5]);
        <InterpreterRequests<u8> as SendInput<u8, Own>>::emit(&mut services, 3);
        assert!(!services.is_empty());
        assert_eq!(services.as_slice(), [2, 4, 5, 3]);
        assert_eq!(
            (&services).into_iter().copied().collect::<Vec<_>>(),
            [2, 4, 5, 3]
        );
        assert_eq!(services.into_iter().collect::<Vec<_>>(), [2, 4, 5, 3]);

        let requests = InterpreterRequests::new(vec![4, 5]).into_requests();
        assert_eq!(requests, [4, 5]);
    }

    struct Returning;

    impl InterpreterRequest for Returning {
        type ReturnToEmitter = ReturnsToEmitter<u8, crate::Here>;
    }

    fn lawful<Event, Effects: SendsFor<Event>>() {}

    #[test]
    fn local_return_proofs_compose_through_exact_event_layers() {
        type Inner = crate::EventLayer<u8, crate::User<crate::MailAddr, ()>>;
        type Outer = crate::EventLayer<(), Inner>;

        lawful::<Inner, InterpreterRequests<Returning>>();
        lawful::<Outer, SendLayer<NoSends, InterpreterRequests<Returning>>>();
    }

    #[test]
    fn send_layer_emits_into_its_designated_owned_lane() {
        let mut effects = SendLayer::new(Vec::<u8>::new(), Vec::<u16>::new());
        <SendLayer<Vec<u8>, Vec<u16>> as SendInput<u8, Own>>::emit(&mut effects, 7);
        assert_eq!(effects.owned, [7]);
        assert!(effects.inner.is_empty());
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Seen {
        Inner(u8),
        Outer(u16),
    }

    struct Trace(Vec<Seen>);

    type TraceEvent =
        crate::EventLayer<u16, crate::EventLayer<u8, crate::User<crate::MailAddr, ()>>>;

    impl SendInterpreter for Trace {
        type Error = core::convert::Infallible;
    }

    impl InterpretRequest<u8, TraceEvent, crate::Inside<crate::Here>> for Trace {
        fn interpret_request(
            &mut self,
            request: u8,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Inner(request));
                Ok(())
            }
        }
    }

    impl InterpretRequest<u16, TraceEvent, crate::Here> for Trace {
        fn interpret_request(
            &mut self,
            request: u16,
        ) -> impl Future<Output = Result<(), Self::Error>> + Send {
            async move {
                self.0.push(Seen::Outer(request));
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn structural_interpretation_visits_every_lane_inner_to_outer() {
        let effects = SendLayer::new(
            InterpreterRequests::new(vec![3_u16, 5]),
            SendLayer::new(InterpreterRequests::one(2_u8), NoSends),
        );
        let mut trace = Trace(Vec::new());

        <_ as InterpretSends<_, TraceEvent, crate::Here>>::interpret(effects, &mut trace)
            .await
            .unwrap();

        assert_eq!(trace.0, [Seen::Inner(2), Seen::Outer(3), Seen::Outer(5)]);
    }
}
