//! Typed send products and their accumulation contract.

/// A product of independently typed send protocols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendProduct<L, R> {
    pub inner: L,
    pub own: R,
}

/// The operation required to accumulate sends across transitions.
pub trait SendAlgebra: Sized {
    fn empty() -> Self;
    fn append(&mut self, other: Self);
}

impl<T> SendAlgebra for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }

    fn append(&mut self, mut other: Self) {
        Vec::append(self, &mut other);
    }
}

impl<L: SendAlgebra, R: SendAlgebra> SendAlgebra for SendProduct<L, R> {
    fn empty() -> Self {
        Self {
            inner: L::empty(),
            own: R::empty(),
        }
    }

    fn append(&mut self, other: Self) {
        self.inner.append(other.inner);
        self.own.append(other.own);
    }
}

/// Requests interpreted by the runtime local to the emitting actor.
///
/// Unlike [`crate::Delivery`], a service request has no actor address. Its
/// recipient is definitionally the interpreter of the actor whose transition
/// emitted it. This distinct send lane lets interpreters route ordinary
/// deliveries and local services with disjoint static implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSends<M> {
    requests: Vec<M>,
}

impl<M> ServiceSends<M> {
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

impl<M> core::ops::Index<usize> for ServiceSends<M> {
    type Output = M;
    fn index(&self, index: usize) -> &Self::Output {
        &self.requests[index]
    }
}

impl<M> IntoIterator for ServiceSends<M> {
    type Item = M;
    type IntoIter = std::vec::IntoIter<M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.into_iter()
    }
}

impl<'a, M> IntoIterator for &'a ServiceSends<M> {
    type Item = &'a M;
    type IntoIter = core::slice::Iter<'a, M>;
    fn into_iter(self) -> Self::IntoIter {
        self.requests.iter()
    }
}

impl<M> SendAlgebra for ServiceSends<M> {
    fn empty() -> Self {
        Self::new(Vec::new())
    }
    fn append(&mut self, mut other: Self) {
        self.requests.append(&mut other.requests);
    }
}
