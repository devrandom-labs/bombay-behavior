//! Probe: eliminate P-never by substituting the inhabited unit type () for
//! the uninhabited Never in the error and phase seats.
//!
//! Expected outcome: the substitution COMPILES, and the values it makes
//! constructible (Err(()), Goto(())) demonstrate the lost static guarantee.

use behavior::{Acted, Actions, MailAddr, Never, NoBirths, Step};

fn main() {
    // With the real basis (Error = Never, Ph = Never): the following seats
    // have no inhabitants, so a behavior cannot fail and cannot go to a phase.
    let ok: Acted<MailAddr, Never, Vec<behavior::Delivery<MailAddr, ()>>, NoBirths, Never> =
        Ok(Actions::cont());
    let _ = ok;

    // Substituting () for Never type-checks — and admits failure/phase values
    // that were previously unrepresentable:
    let hole: Acted<MailAddr, (), Vec<behavior::Delivery<MailAddr, ()>>, NoBirths, ()> =
        Err(()); // <- constructible only because () is inhabited
    let _ = hole;
    let phase_hole: Step<(), behavior::Exit<MailAddr>> = Step::Goto(());
    let _ = phase_hole;

    // A function claiming "this behavior never fails" cannot prove it with ():
    // nothing stops a caller from constructing Err(()) above. With Never the
    // claim is compile-time-true because Err has no value to carry.
    println!("unit-seat substitution compiles; the hole is constructible");
}
