//! Typed send products and their accumulation contract.

use core::marker::PhantomData;

/// The lane owned by the current named send product.
pub enum Own {}

/// A lane reached through the product's composed behavior sends.
pub struct Inner<Path>(PhantomData<fn(Path)>);

/// Static evidence that a send product contains one request lane.
///
/// Implementations append the input exactly once to that lane and leave every
/// other lane unchanged. `Path` distinguishes repeated request types without
/// erasing their position or choosing a lane at runtime.
pub trait SendInput<Input, Path> {
    fn emit(&mut self, input: Input);
}

/// A product of independently typed send protocols.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendProduct<L, R> {
    pub inner: L,
    pub own: R,
}

impl<L, R> SendProduct<L, R> {
    #[must_use]
    pub const fn new(inner: L, own: R) -> Self {
        Self { inner, own }
    }

    #[must_use]
    pub fn split(self) -> (L, R) {
        (self.inner, self.own)
    }
}

impl<L, R> From<(L, R)> for SendProduct<L, R> {
    fn from((inner, own): (L, R)) -> Self {
        Self::new(inner, own)
    }
}

/// The operation required to accumulate sends across transitions.
pub trait SendAlgebra: Sized {
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

impl<T> SendAlgebra for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }

    fn append(&mut self, mut other: Self) {
        Vec::append(self, &mut other);
    }
}

impl<T> SendInput<T, Own> for Vec<T> {
    fn emit(&mut self, input: T) {
        self.push(input);
    }
}

impl<L: SendAlgebra, R: SendAlgebra> SendAlgebra for SendProduct<L, R> {
    fn empty() -> Self {
        Self::new(L::empty(), R::empty())
    }

    fn append(&mut self, other: Self) {
        let (inner, own) = other.split();
        self.inner.append(inner);
        self.own.append(own);
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

impl<M> SendInput<M, Own> for ServiceSends<M> {
    fn emit(&mut self, input: M) {
        self.requests.push(input);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_algebra_obeys_identity_and_associativity() {
        let values = vec![1, 2];
        assert_eq!(Vec::new().combine(values.clone()), values);
        assert_eq!(values.clone().combine(Vec::new()), values);

        let left = vec![1].combine(vec![2]).combine(vec![3]);
        let right = vec![1].combine(vec![2].combine(vec![3]));
        assert_eq!(left, right);
    }
}
