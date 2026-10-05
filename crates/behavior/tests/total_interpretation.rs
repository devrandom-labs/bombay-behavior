use core::future::{Future, poll_fn};
use core::pin::pin;
use core::task::Poll;

use behavior::{
    ActionItem, Here, Inside, InterpretItem, InterpretSends, Interpretation,
    InterpretationProgress, InterpreterFault, InterpreterRequests, ItemSettlement, SendLayer,
    SettledItem, finish_item, prepare_item,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Kind {
    Watch,
    Timer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Watch(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Timer(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Plan {
    Accept,
    Reject,
    Block,
    Corrupt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Receipt {
    Watching,
    Scheduled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Rejection {
    ObserverClosed,
    TimerClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Prerequisite {
    SubjectCommit,
    ClockCommit,
}

#[derive(Default)]
struct Runtime {
    plan: BTreeMap<(Kind, u8), Plan>,
    attempts: Vec<(Kind, u8)>,
}

impl ActionItem for Watch {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = Receipt;
    type Rejection = Rejection;
    type Prerequisite = Prerequisite;
}

impl ActionItem for Timer {
    type Custody = (Option<Self>, Option<Self::Reply>);
    type Input<'a>
        = &'a mut Option<Self>
    where
        Self: 'a;
    type Reply = ItemSettlement<Self, Self::Accepted, Self::Rejection, Self::Prerequisite>;
    fn prepare_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        prepare_item::<Self>(progress);
    }
    fn interpretation_input<'a>(
        custody: &'a mut Self::Custody,
    ) -> Option<(Self::Input<'a>, &'a mut Option<Self::Reply>)>
    where
        Self: 'a,
    {
        let (input, received) = custody;
        match (&*input, &*received) {
            (Some(_), None) => Some((input, received)),
            _ => None,
        }
    }
    fn finish_interpretation(
        progress: &mut Option<InterpretationProgress<Self, Self::Custody, Self::Reply>>,
    ) {
        finish_item::<Self>(progress);
    }

    type Accepted = Receipt;
    type Rejection = Rejection;
    type Prerequisite = Prerequisite;
}

impl Runtime {
    fn with_plan(plan: impl IntoIterator<Item = ((Kind, u8), Plan)>) -> Self {
        Self {
            plan: plan.into_iter().collect(),
            attempts: Vec::new(),
        }
    }

    fn next(&mut self, kind: Kind, value: u8) -> Plan {
        self.attempts.push((kind, value));
        self.plan
            .get(&(kind, value))
            .copied()
            .unwrap_or(Plan::Accept)
    }
}

impl<RootEvent, Path> InterpretItem<Watch, RootEvent, Path> for Runtime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Watch>,
        received: &'a mut Option<<Watch as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Watch: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            let producer = {
                let plan = self.next(Kind::Watch, item.0);
                async move {
                    match plan {
                        Plan::Accept => ItemSettlement::Accepted(Receipt::Watching),
                        Plan::Reject => ItemSettlement::Rejected {
                            item,
                            reason: Rejection::ObserverClosed,
                        },
                        Plan::Block => ItemSettlement::Blocked {
                            item,
                            prerequisite: Prerequisite::SubjectCommit,
                        },
                        Plan::Corrupt => ItemSettlement::Corrupt {
                            item,
                            fault: InterpreterFault::CorruptTraversal,
                        },
                    }
                }
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

impl<RootEvent, Path> InterpretItem<Timer, RootEvent, Path> for Runtime {
    fn interpret_item<'a>(
        &'a mut self,
        input: &'a mut Option<Timer>,
        received: &'a mut Option<<Timer as ActionItem>::Reply>,
    ) -> impl Future<Output = ()> + Send + 'a
    where
        Timer: 'a,
    {
        async move {
            if received.is_some() {
                return;
            }
            let Some(item) = input.take() else {
                return;
            };
            let producer = {
                let plan = self.next(Kind::Timer, item.0);
                async move {
                    match plan {
                        Plan::Accept => ItemSettlement::Accepted(Receipt::Scheduled),
                        Plan::Reject => ItemSettlement::Rejected {
                            item,
                            reason: Rejection::TimerClosed,
                        },
                        Plan::Block => ItemSettlement::Blocked {
                            item,
                            prerequisite: Prerequisite::ClockCommit,
                        },
                        Plan::Corrupt => ItemSettlement::Corrupt {
                            item,
                            fault: InterpreterFault::CorruptTraversal,
                        },
                    }
                }
            };
            let mut producer = pin!(producer);
            poll_fn(|context| match producer.as_mut().poll(context) {
                Poll::Ready(settlement) => {
                    *received = Some(settlement);
                    Poll::Ready(())
                }
                Poll::Pending => Poll::Pending,
            })
            .await;
        }
    }
}

#[tokio::test]
async fn rejection_and_blocking_continue_through_both_wrapper_orders() {
    let effects = SendLayer::new(
        InterpreterRequests::one(Timer(2)),
        InterpreterRequests::one(Watch(1)),
    );
    let mut runtime = Runtime::with_plan([
        ((Kind::Watch, 1), Plan::Reject),
        ((Kind::Timer, 2), Plan::Block),
    ]);

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(effects));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact wrapper product must return its actual settlement");
        };
        settlement
    };
    assert_eq!(runtime.attempts, [(Kind::Watch, 1), (Kind::Timer, 2)]);
    assert_eq!(
        settlement,
        Interpretation::Complete(SendLayer::new(
            vec![SettledItem::Attempted(ItemSettlement::Blocked {
                item: Timer(2),
                prerequisite: Prerequisite::ClockCommit,
            })],
            vec![SettledItem::Attempted(ItemSettlement::Rejected {
                item: Watch(1),
                reason: Rejection::ObserverClosed,
            })],
        ))
    );

    let reversed = SendLayer::new(
        InterpreterRequests::one(Watch(4)),
        InterpreterRequests::one(Timer(3)),
    );
    let mut runtime = Runtime::with_plan([((Kind::Timer, 3), Plan::Reject)]);
    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(reversed));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact wrapper product must return its actual settlement");
        };
        settlement
    };
    assert_eq!(runtime.attempts, [(Kind::Timer, 3), (Kind::Watch, 4)]);
    assert_eq!(
        settlement,
        Interpretation::Complete(SendLayer::new(
            vec![SettledItem::Attempted(ItemSettlement::Accepted(
                Receipt::Watching,
            ))],
            vec![SettledItem::Attempted(ItemSettlement::Rejected {
                item: Timer(3),
                reason: Rejection::TimerClosed,
            })],
        ))
    );
}

#[tokio::test]
async fn corruption_retains_fault_item_and_every_exact_unattempted_suffix() {
    let effects = SendLayer::new(
        InterpreterRequests::new(vec![Timer(4), Timer(5)]),
        InterpreterRequests::new(vec![Watch(1), Watch(2), Watch(3)]),
    );
    let mut runtime = Runtime::with_plan([((Kind::Watch, 2), Plan::Corrupt)]);

    let settlement = {
        let mut progress = Some(InterpretationProgress::Original(effects));
        <_ as InterpretSends<_, (), Here>>::interpret(&mut progress, &mut runtime).await;
        let Some(InterpretationProgress::Completed(settlement)) = progress else {
            panic!("the exact wrapper product must return its actual settlement");
        };
        settlement
    };
    assert_eq!(runtime.attempts, [(Kind::Watch, 1), (Kind::Watch, 2)]);
    assert_eq!(
        settlement,
        Interpretation::Corrupt(SendLayer::new(
            vec![
                SettledItem::Unattempted(Timer(4)),
                SettledItem::Unattempted(Timer(5)),
            ],
            vec![
                SettledItem::Attempted(ItemSettlement::Accepted(Receipt::Watching)),
                SettledItem::Attempted(ItemSettlement::Corrupt {
                    item: Watch(2),
                    fault: InterpreterFault::CorruptTraversal,
                }),
                SettledItem::Unattempted(Watch(3)),
            ],
        ))
    );
}

fn _both_paths_are_substitution_laws()
where
    Runtime: InterpretItem<Watch, (), Here> + InterpretItem<Watch, (), Inside<Here>>,
{
}
