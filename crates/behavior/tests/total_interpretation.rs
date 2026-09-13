use behavior::{
    ActionItem, Here, Inside, InterpretItem, InterpretSends, Interpretation, InterpreterFault,
    InterpreterRequests, ItemSettlement, SendLayer, SettledItem,
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
    type Accepted = Receipt;
    type Rejection = Rejection;
    type Prerequisite = Prerequisite;
}

impl ActionItem for Timer {
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
    fn interpret_item(
        &mut self,
        item: Watch,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Watch,
            <Watch as ActionItem>::Accepted,
            <Watch as ActionItem>::Rejection,
            <Watch as ActionItem>::Prerequisite,
        >,
    > + Send {
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
    }
}

impl<RootEvent, Path> InterpretItem<Timer, RootEvent, Path> for Runtime {
    fn interpret_item(
        &mut self,
        item: Timer,
    ) -> impl core::future::Future<
        Output = ItemSettlement<
            Timer,
            <Timer as ActionItem>::Accepted,
            <Timer as ActionItem>::Rejection,
            <Timer as ActionItem>::Prerequisite,
        >,
    > + Send {
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

    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(effects, &mut runtime).await;
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
    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(reversed, &mut runtime).await;
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

    let settlement = <_ as InterpretSends<_, (), Here>>::interpret(effects, &mut runtime).await;
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
