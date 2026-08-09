//! Probe: eliminate P-birthmode by encoding creation authority as marker
//! traits instead of an associated-type type function.
//!
//! Expected outcome: the composite wrapper cannot name the child type for its
//! creates seat without an associated type; reintroducing one is BirthMode.

use behavior::{Actions, Behavior, Births, Create, MailAddr, Never, NoBirths};
use core::marker::PhantomData;

// Attempt 1: pure marker traits.
trait MarkerNoBirths {}
trait MarkerMayBirth {}

struct Composite<B>(PhantomData<B>);

// Goal: the composite's step returns Actions whose creates seat has child type
// equal to the inner behavior's child type. With markers only, there is no
// type to name:
//
//   fn step(...) -> Acted<MailAddr, Never, Sends, ???>
//
// ??? cannot be written: MarkerMayBirth carries no Child type. Uncommenting
// the next line is a compile error because the child type is unnameable:
//
// fn demo<B: Behavior>(_: &Composite<B>) -> Vec<Create<MailAddr, ???>> {}

// Attempt 2: name the child type. The minimal fix is an associated type:
trait BirthCapability {
    type Child;
}
struct NoCap;
impl BirthCapability for NoCap {
    type Child = Never;
}
struct Cap<C>(PhantomData<C>);
impl<C> BirthCapability for Cap<C> {
    type Child = C;
}
// This compiles — and it IS BirthMode reintroduced, proving the reduction.

fn main() {
    // Sanity: the real basis composes authority through the associated type.
    let _: Actions<MailAddr, Never, Vec<behavior::Delivery<MailAddr, ()>>, NoBirths> = Actions::cont();
    let _: Actions<MailAddr, Never, Vec<behavior::Delivery<MailAddr, ()>>, Births<()>> = Actions::cont();
    println!("marker-only encoding cannot name the child type; associated type reintroduced");
}
