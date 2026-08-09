//! Stable proxy lifecycle and fresh worker incarnation replacement.

use super::protocol::{ProxyCommand, SupervisionEvent};
use crate::behavior::{
    Actions, Address, Behavior, Births, Create, Delivery, Recipient, SendAlgebra, SendProduct,
    ServiceSends, User,
};
use crate::protocol::{ObserveChild, ReportWorkerStopped};
use crate::verdict::{Never, Step};

/// The stable actor. Every replacement is an ordinary fresh birth beneath it.
///
/// Each emitted worker birth is paired with an [`ObserveChild`] request. A
/// matching [`ChildStopped`] leaves the proxy alive and emits a
/// [`ReportWorkerStopped`] carrying the outcome unchanged. Stale child-stop
/// observations are inert.
pub struct Proxy<C: Behavior<Ph = Never>> {
    worker: Option<C>,
    generation: u64,
    worker_alive: bool,
    pending: Option<C>,
}

impl<C: Behavior<Ph = Never>> Proxy<C> {
    #[must_use]
    pub fn new(worker: C) -> Self {
        Self {
            worker: Some(worker),
            generation: 0,
            worker_alive: false,
            pending: None,
        }
    }
}

impl<C> Behavior for Proxy<C>
where
    C: Behavior<Ph = Never> + Send,
    C::Addr: Send,
    <C::Addr as Address>::Nonce: From<u64> + Send,
    C::Msg: Send,
    C: Send,
{
    type Addr = C::Addr;
    type Msg = ProxyCommand<C>;
    type Event = SupervisionEvent<User<C::Addr, ProxyCommand<C>>, C::Addr>;
    type Sends = SendProduct<
        Vec<Delivery<C::Addr, C::Msg>>,
        SendProduct<
            ServiceSends<ObserveChild<C::Addr>>,
            ServiceSends<ReportWorkerStopped<C::Addr>>,
        >,
    >;
    type Ph = Never;
    type Error = Never;
    type Birth = Births<C>;

    async fn init(&mut self) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        let child = self.worker.take().expect("a proxy initializes once");
        self.worker_alive = true;
        Ok(Actions {
            sends: SendProduct {
                inner: Vec::new(),
                own: SendProduct {
                    inner: ServiceSends::one(ObserveChild {
                        nonce: <C::Addr as Address>::Nonce::from(self.generation),
                    }),
                    own: ServiceSends::empty(),
                },
            },
            creates: vec![Create::birth(
                <C::Addr as Address>::Nonce::from(self.generation),
                child,
            )],
            become_: Step::Continue,
        })
    }

    async fn step(
        &mut self,
        event: Self::Event,
    ) -> Result<Actions<C::Addr, Never, Self::Sends, Births<C>>, Never> {
        let SupervisionEvent::Inner(event) = event else {
            return match event {
                SupervisionEvent::ChildStopped(event)
                    if event.nonce == <C::Addr as Address>::Nonce::from(self.generation) =>
                {
                    self.worker_alive = false;
                    let report = ReportWorkerStopped {
                        outcome: event.outcome,
                        at: event.at,
                    };
                    let creates = self.pending.take().map_or_else(Vec::new, |child| {
                        self.generation = self
                            .generation
                            .checked_add(1)
                            .expect("proxy generation exhausted");
                        self.worker_alive = true;
                        vec![Create::replacement_incarnation(
                            <C::Addr as Address>::Nonce::from(self.generation),
                            child,
                        )]
                    });
                    let observes = creates
                        .iter()
                        .map(|create| ObserveChild {
                            nonce: create.nonce,
                        })
                        .collect();
                    Ok(Actions {
                        sends: SendProduct {
                            inner: Vec::new(),
                            own: SendProduct {
                                inner: ServiceSends::new(observes),
                                own: ServiceSends::one(report),
                            },
                        },
                        creates,
                        become_: Step::Continue,
                    })
                }
                SupervisionEvent::ChildStopped(_) | SupervisionEvent::WorkerStopped(_) => {
                    Ok(Actions::cont())
                }
                SupervisionEvent::Inner(_) => unreachable!(),
            };
        };
        match event.message {
            ProxyCommand::Forward(message) => Ok(Actions {
                sends: SendProduct {
                    inner: self
                        .worker_alive
                        .then(|| {
                            Delivery::new(
                                Recipient::child(<C::Addr as Address>::Nonce::from(
                                    self.generation,
                                )),
                                message,
                            )
                        })
                        .into_iter()
                        .collect(),
                    own: SendProduct {
                        inner: ServiceSends::empty(),
                        own: ServiceSends::empty(),
                    },
                },
                creates: Vec::new(),
                become_: Step::Continue,
            }),
            ProxyCommand::Replace(child) => {
                if self.worker_alive {
                    self.pending = Some(child);
                    return Ok(Actions::cont());
                }
                self.generation = self
                    .generation
                    .checked_add(1)
                    .expect("proxy generation exhausted");
                self.worker_alive = true;
                let nonce = <C::Addr as Address>::Nonce::from(self.generation);
                Ok(Actions {
                    sends: SendProduct {
                        inner: Vec::new(),
                        own: SendProduct {
                            inner: ServiceSends::one(ObserveChild { nonce }),
                            own: ServiceSends::empty(),
                        },
                    },
                    creates: vec![Create::replacement_incarnation(nonce, child)],
                    become_: Step::Continue,
                })
            }
        }
    }
}
