//! `Phased` — a `Behavior`: the gated-buffer engine with a phase-aware policy.
//! It gates each message by the current phase, holds the deferred ones, and
//! releases them when the inner handler's `Goto` moves the phase.

use bombay::capability::{Deferred, Disposition};

use crate::behavior::Behavior;
use crate::gated::{Admit, Gate, Gated};

/// The phase-aware policy: a gate function of the current phase, plus the phase
/// state it transitions on `Goto`.
pub struct PhaseGate<B: Behavior> {
    pub(crate) phase: B::Ph,
    gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>,
}

impl<B: Behavior> Gate for PhaseGate<B>
where
    B::Ph: Copy + PartialEq,
{
    type Msg = B::Msg;
    type Ph = B::Ph;
    fn admit(&self, msg: &B::Msg) -> Admit {
        match (self.gate)(self.phase, msg) {
            Disposition::Deliver => Admit::Deliver,
            Disposition::Defer(Deferred) => Admit::Defer,
            Disposition::Ignore => Admit::Ignore,
        }
    }
    fn advance(&mut self, to: B::Ph) -> bool {
        let changed = to != self.phase;
        self.phase = to;
        changed
    }
}

/// A phased behavior: the gated buffer with a [`PhaseGate`].
pub type Phased<B> = Gated<B, PhaseGate<B>>;

impl<B: Behavior> Gated<B, PhaseGate<B>>
where
    B::Ph: Copy + PartialEq,
{
    /// Builds a phased behavior in `initial` with a per-phase gate.
    pub fn new(
        inner: B,
        initial: B::Ph,
        gate: fn(B::Ph, &B::Msg) -> Disposition<Deferred>,
    ) -> Self {
        Gated::build(inner, PhaseGate { phase: initial, gate })
    }

    /// The committed phase (test observability).
    pub fn phase(&self) -> B::Ph {
        self.gate.phase
    }
}

#[cfg(test)]
mod tests {
    use crate::behavior::{Behavior, Envelope};
    use crate::{Base, Exit, Phased};
    use bombay::capability::{Deferred, Disposition, Step};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ph {
        Loading,
        Ready,
    }

    enum Msg {
        Work(u64),
        Promote,
        Quit,
    }

    /// Work defers in Loading; the promotion releases the deferred batch FIFO
    /// within the goto step, ahead of the backlog.
    #[tokio::test]
    async fn phased_releases_the_deferred_batch_fifo_on_goto() {
        let inner = Base::new(Vec::<u64>::new(), |seen: &mut Vec<u64>, msg: Msg| match msg {
            Msg::Work(id) => {
                seen.push(id);
                Ok::<Step<Ph, Exit>, &'static str>(Step::Continue)
            }
            Msg::Promote => Ok(Step::Goto(Ph::Ready)),
            Msg::Quit => Ok(Step::Stop(Exit::Normal)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |ph, msg| match (ph, msg) {
            (Ph::Loading, Msg::Work(_)) => Disposition::Defer(Deferred),
            _ => Disposition::Deliver,
        });
        for m in [Msg::Work(1), Msg::Work(2), Msg::Promote, Msg::Work(3), Msg::Quit] {
            let _ = p.step(Envelope::User(m)).await;
        }
        assert_eq!(p.inner().state(), &vec![1, 2, 3], "batch replays FIFO inside the goto");
        assert_eq!(p.phase(), Ph::Ready);
    }

    /// D3: a failing handler never half-switches the phase.
    #[tokio::test]
    async fn phased_never_commits_a_failed_handlers_goto() {
        let inner = Base::new((), |(): &mut (), msg: Msg| match msg {
            Msg::Work(_) => Err("bang"),
            _ => Ok::<Step<Ph, Exit>, &'static str>(Step::Goto(Ph::Ready)),
        });
        let mut p = Phased::new(inner, Ph::Loading, |_, _| Disposition::Deliver);
        assert_eq!(p.step(Envelope::User(Msg::Work(1))).await, Err("bang"));
        assert_eq!(p.phase(), Ph::Loading, "an Err never half-switches the phase (D3)");
    }
}
