//! The shared gated-buffer ENGINE — this is NOT a behavior, it is the mechanism
//! two behaviors are built on. `Gated<B, G>` = an inner behavior + a held
//! buffer + a gate policy `G`. `Phased` (`phased.rs`) and `Stashing`
//! (`stashing.rs`) are the two behaviors; each is `Gated` with a different
//! `Gate`. A third-party gate is a third `Gate` impl. (The #298 decomposition
//! finding: bombay's `Phased` and `Stashing` are one thing.)

use std::collections::VecDeque;

use bombay::capability::{Never, Step};
use tokio::time::Instant;

use crate::Exit;
use crate::behavior::{Behavior, Envelope};

/// What a gate policy decides for one user message.
pub enum Admit {
    /// Hand it to the inner behavior now.
    Deliver,
    /// Hold it in the buffer; re-classify on the next drain.
    Defer,
    /// Drop it by declaration.
    Ignore,
    /// Deliver now, then drain the whole held buffer (message-triggered
    /// release — the `Stashing` shape).
    Release,
}

/// A gated-buffer policy: classify a message against its own state, and react
/// to the inner handler's phase transition.
pub trait Gate {
    /// The user-message type classified.
    type Msg;
    /// The phase menu the inner handler transitions over (`Never` = stateless).
    type Ph;
    /// Classify a message against current policy state.
    fn admit(&self, msg: &Self::Msg) -> Admit;
    /// The inner handler returned `Goto(to)`. Update policy state; return `true`
    /// if the transition should release + re-gate the held buffer. A stateless
    /// policy has `Ph = Never`, so this is never reached.
    fn advance(&mut self, to: Self::Ph) -> bool;
}

/// The engine: an inner behavior + a held buffer + a gate policy. Erases the
/// inner become-menu (`Ph = Never` upward — the gate consumes it as buffer
/// releases). Not constructed directly — see `Phased::new` / `Stashing::new`.
pub struct Gated<B: Behavior, G> {
    pub(crate) inner: B,
    pub(crate) gate: G,
    held: VecDeque<B::Msg>,
}

impl<B: Behavior, G> Gated<B, G>
where
    G: Gate<Msg = B::Msg, Ph = B::Ph>,
{
    /// Wraps `inner` in `gate` with an empty buffer (crate-internal; the two
    /// behaviors' `new` constructors call this).
    pub(crate) fn build(inner: B, gate: G) -> Self {
        Self { inner, gate, held: VecDeque::new() }
    }

    /// The wrapped behavior (test observability).
    pub fn inner(&self) -> &B {
        &self.inner
    }

    /// How many messages are currently held (test observability).
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Run one event through the inner behavior; a `Goto` drives the gate's
    /// `advance` (D3: the commit is inside the `Ok`, so an `Err` can never
    /// half-transition). Returns whether the transition asks for a drain.
    async fn run_inner(
        &mut self,
        ev: Envelope<B::Msg>,
    ) -> Result<(Step<Never, Exit>, bool), B::Error> {
        Ok(match self.inner.step(ev).await? {
            Step::Continue => (Step::Continue, false),
            Step::Stop(exit) => (Step::Stop(exit), false),
            Step::Goto(to) => (Step::Continue, self.gate.advance(to)),
        })
    }

    /// Drain a snapshot of the held buffer, re-gating each message in the
    /// current policy state: `Ignore` drops, `Defer` re-holds (the snapshot
    /// bound), `Deliver`/`Release` fold. A mid-drain transition folds the
    /// freshly-held messages back in; a `Stop` abandons the rest.
    async fn drain(&mut self) -> Result<Step<Never, Exit>, B::Error> {
        let mut batch: VecDeque<B::Msg> = self.held.drain(..).collect();
        while let Some(m) = batch.pop_front() {
            match self.gate.admit(&m) {
                Admit::Ignore => {}
                Admit::Defer => self.held.push_back(m),
                Admit::Deliver | Admit::Release => {
                    let (verdict, transitioned) = self.run_inner(Envelope::User(m)).await?;
                    if let Step::Stop(exit) = verdict {
                        self.held.extend(batch);
                        return Ok(Step::Stop(exit));
                    }
                    if transitioned {
                        batch.extend(self.held.drain(..));
                    }
                }
            }
        }
        Ok(Step::Continue)
    }
}

impl<B, G> Behavior for Gated<B, G>
where
    B: Behavior + Send,
    B::Msg: Send,
    B::Ph: Send,
    G: Gate<Msg = B::Msg, Ph = B::Ph> + Send,
{
    type Msg = B::Msg;
    type Ph = Never;
    type Error = B::Error;
    async fn step(&mut self, ev: Envelope<B::Msg>) -> Result<Step<Never, Exit>, B::Error> {
        let Envelope::User(m) = ev else {
            // Framework events are not gated; a reaction that transitions still
            // releases the buffer via `advance`.
            let (verdict, transitioned) = self.run_inner(ev).await?;
            return match verdict {
                Step::Stop(exit) => Ok(Step::Stop(exit)),
                _ if transitioned => self.drain().await,
                _ => Ok(Step::Continue),
            };
        };
        match self.gate.admit(&m) {
            Admit::Ignore => Ok(Step::Continue),
            Admit::Defer => {
                self.held.push_back(m);
                Ok(Step::Continue)
            }
            Admit::Deliver => {
                let (verdict, transitioned) = self.run_inner(Envelope::User(m)).await?;
                match verdict {
                    Step::Stop(exit) => Ok(Step::Stop(exit)),
                    _ if transitioned => self.drain().await,
                    _ => Ok(Step::Continue),
                }
            }
            Admit::Release => {
                if let (Step::Stop(exit), _) = self.run_inner(Envelope::User(m)).await? {
                    return Ok(Step::Stop(exit));
                }
                self.drain().await
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.inner.next_deadline()
    }
}
