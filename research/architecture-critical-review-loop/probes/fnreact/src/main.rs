//! Probe: eliminate fn-pointer reactions (P-fnreact) from the candidate basis.
//!
//! Attempt A: closed enum of built-in reactions.
//! Attempt B: generic reaction seat (trait object-free, statically dispatched).

use behavior::{Behavior, Never, Step, Become, Address, MailAddr};
use behavior::{SupervisionFailure};

// ---------- Attempt A: closed enum of reactions ----------
#[derive(Clone, Copy)]
enum BuiltinReaction {
    Retire,
    Stop,
}

fn apply<B: Behavior>(
    reaction: BuiltinReaction,
    behavior: &mut B,
    failure: &SupervisionFailure<B::Addr>,
) -> Result<Become<B::Addr>, B::Error> {
    let _ = behavior;
    match reaction {
        BuiltinReaction::Retire => Ok(Step::Continue),
        BuiltinReaction::Stop => Ok(Step::Stop(failure.clone().into_exit())),
    }
}

// A user-defined escalation policy, e.g. "log-then-retire", CANNOT be added
// without editing this library enum. Demonstrated by the absence of any user
// seat: the following function cannot be given to `apply` — there is no
// variant to carry it. (Recorded as a semantic/expressiveness obstruction.)

// ---------- Attempt B: generic reaction seat ----------
trait FailureReaction<B: Behavior> {
    fn react(
        &mut self,
        behavior: &mut B,
        failure: &SupervisionFailure<B::Addr>,
    ) -> Result<Become<B::Addr>, B::Error>;
}

struct Retire;
impl<B: Behavior> FailureReaction<B> for Retire {
    fn react(&mut self, _b: &mut B, _f: &SupervisionFailure<B::Addr>) -> Result<Become<B::Addr>, B::Error> {
        Ok(Step::Continue)
    }
}

// A wrapper holding a generic reaction: the reaction seat infects the wrapper's
// type — every supervising wrapper gains one generic parameter per reaction site.
struct SupervisingWithGenericReaction<B: Behavior, R: FailureReaction<B>> {
    inner: B,
    reaction: R,
}

fn main() {
    // Attempt A compiles but forecloses user reactions (no seat for them).
    let _ = apply::<behavior::Base<behavior::FnState<(), MailAddr, (), Never, behavior::NoBirths, Never>>>;
    // Attempt B compiles; note the extra generic seat on the wrapper type.
    let _ = std::marker::PhantomData::<SupervisingWithGenericReaction<
        behavior::Base<behavior::FnState<(), MailAddr, (), Never, behavior::NoBirths, Never>>,
        Retire,
    >>;
    println!("probe compiled");
}
