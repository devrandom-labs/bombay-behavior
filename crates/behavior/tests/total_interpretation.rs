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

mod normal_progress_receipts {
    use behavior::{
        ActionItem, Actions, Creations, Here, InterpretItem, Interpretation,
        InterpretationProgress, InterpreterRequest, InterpreterRequests, ItemSettlement, MailAddr,
        Never, NoBirthProtocols, NoBirths, NoReturnToEmitter, SendLayer, SettledItem, Step,
        Stopped, finish_item, prepare_item,
    };
    use core::future::Future;
    use core::pin::{Pin, pin};
    use core::task::{Context, Poll, Waker};
    use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
    use std::sync::{Arc, Weak};
    use std::vec;

    struct ReceiptCommand {
        entry: u64,
        values: Arc<Vec<u8>>,
    }
    impl InterpreterRequest for ReceiptCommand {
        type ReturnToEmitter = NoReturnToEmitter;
        type LogicalProtocols = NoBirthProtocols;
    }
    impl ActionItem for ReceiptCommand {
        type Accepted = (u64, Arc<Vec<u8>>);
        type Rejection = Never;
        type Prerequisite = Never;
        type Custody = (Option<Self>, Option<Self::Reply>);
        type Input<'a>
            = &'a mut Option<Self>
        where
            Self: 'a;
        type Reply = ItemSettlement<Self, Self::Accepted, Never, Never>;
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
    }
    enum ReceiptDisposal {
        Ordinary,
        Panicked(Arc<Vec<u8>>),
    }
    struct ReceiptInterpreter {
        disposals: vec::IntoIter<ReceiptDisposal>,
        attempts: Vec<u64>,
    }
    struct ReceiptAttempt<'a> {
        input: &'a mut Option<ReceiptCommand>,
        received: &'a mut Option<<ReceiptCommand as ActionItem>::Reply>,
        attempts: &'a mut Vec<u64>,
        disposal: Option<ReceiptDisposal>,
    }
    impl Future for ReceiptAttempt<'_> {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            if self.received.is_none() {
                if let Some(command) = self.input.take() {
                    self.attempts.push(command.entry);
                    *self.received =
                        Some(ItemSettlement::Accepted((command.entry, command.values)));
                }
            }
            Poll::Ready(())
        }
    }
    impl Drop for ReceiptAttempt<'_> {
        fn drop(&mut self) {
            match self
                .disposal
                .take()
                .expect("each actual producer is disposed once")
            {
                ReceiptDisposal::Ordinary => {}
                ReceiptDisposal::Panicked(cause) => panic_any(cause),
            }
        }
    }
    impl<RootEvent, Path> InterpretItem<ReceiptCommand, RootEvent, Path> for ReceiptInterpreter {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<ReceiptCommand>,
            received: &'a mut Option<<ReceiptCommand as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            ReceiptCommand: 'a,
        {
            ReceiptAttempt {
                input,
                received,
                attempts: &mut self.attempts,
                disposal: Some(
                    self.disposals
                        .next()
                        .expect("one actual disposition per original request"),
                ),
            }
        }
    }
    fn receipt_fact(receipt: &<ReceiptCommand as ActionItem>::Reply) -> (u64, usize, Vec<u8>) {
        let ItemSettlement::Accepted((entry, values)) = receipt else {
            panic!("the actual host supplies accepted receipt facts")
        };
        (
            *entry,
            Arc::as_ptr(values) as usize,
            values.as_slice().to_vec(),
        )
    }
    fn input_fact(input: &ReceiptCommand) -> (u64, usize, Vec<u8>) {
        (
            input.entry,
            Arc::as_ptr(&input.values) as usize,
            input.values.as_slice().to_vec(),
        )
    }
    fn normal_product_receipts(disposal: ReceiptDisposal) {
        let originals = [
            Arc::new(vec![61_u8, 67]),
            Arc::new(vec![71, 73]),
            Arc::new(vec![79, 83]),
            Arc::new(vec![89, 97]),
        ];
        let allocations = originals.each_ref().map(|v| Arc::as_ptr(v) as usize);
        let retained = originals.each_ref().map(Arc::downgrade);
        let [first, current, tail, owned] = originals;
        let cause = match &disposal {
            ReceiptDisposal::Ordinary => None,
            ReceiptDisposal::Panicked(value) => {
                Some((Arc::downgrade(value), Arc::as_ptr(value) as usize))
            }
        };
        let mut interpreter = ReceiptInterpreter {
            disposals: vec![
                ReceiptDisposal::Ordinary,
                disposal,
                ReceiptDisposal::Ordinary,
                ReceiptDisposal::Ordinary,
            ]
            .into_iter(),
            attempts: Vec::new(),
        };
        let sends = SendLayer::new(
            InterpreterRequests::one(ReceiptCommand {
                entry: 4,
                values: owned,
            }),
            InterpreterRequests::new(vec![
                ReceiptCommand {
                    entry: 1,
                    values: first,
                },
                ReceiptCommand {
                    entry: 2,
                    values: current,
                },
                ReceiptCommand {
                    entry: 3,
                    values: tail,
                },
            ]),
        );
        let actions = Actions::<MailAddr, u64, _, NoBirths>::new(
            sends,
            Creations::empty(),
            Step::Goto::<u64, Stopped>(19),
        );
        let mut progress = Some(InterpretationProgress::Original(actions));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let mut execution = pin!(Actions::interpret::<_, (), Here>(
                &mut progress,
                &mut interpreter
            ));
            let mut context = Context::from_waker(Waker::noop());
            let polled = execution.as_mut().poll(&mut context);
            match polled {
                Poll::Ready(()) => {}
                Poll::Pending => panic!("the actual fixture producers all return Ready"),
            }
        }));
        // All execution futures have been disposed before acquiring observations.
        let snapshot = match &progress {
            Some(InterpretationProgress::Completed(Interpretation::Complete(settlement))) => {
                let inner = settlement
                    .sends
                    .inner
                    .iter()
                    .map(|row| {
                        let SettledItem::Attempted(receipt) = row else {
                            panic!("healthy inner request must be attempted")
                        };
                        receipt_fact(receipt)
                    })
                    .collect::<Vec<_>>();
                let owned = settlement
                    .sends
                    .owned
                    .iter()
                    .map(|row| {
                        let SettledItem::Attempted(receipt) = row else {
                            panic!("healthy owned request must be attempted")
                        };
                        receipt_fact(receipt)
                    })
                    .collect::<Vec<_>>();
                Some((
                    settlement.creations.len(),
                    inner,
                    Vec::new(),
                    owned,
                    settlement.become_,
                ))
            }
            Some(InterpretationProgress::Interpreting((
                (
                    Some(InterpretationProgress::Completed(Interpretation::Complete(creations))),
                    Some(InterpretationProgress::Interpreting(layer)),
                ),
                become_,
            ))) => {
                let Some(InterpretationProgress::Interpreting(rows)) = &layer.inner else {
                    panic!("the inner vector remains its real current progress")
                };
                let [
                    Some(InterpretationProgress::Completed(Interpretation::Complete(first))),
                    Some(InterpretationProgress::Interpreting((None, Some(current)))),
                    Some(InterpretationProgress::Original(tail)),
                ] = rows.as_slice()
                else {
                    panic!("the exact accepted prefix, received current and untouched tail coexist")
                };
                let Some(InterpretationProgress::Original(owned)) = &layer.owned else {
                    panic!("the owned sibling has never been offered")
                };
                Some((
                    creations.len(),
                    vec![receipt_fact(first), receipt_fact(current)],
                    vec![input_fact(tail)],
                    owned.as_slice().iter().map(input_fact).collect(),
                    *become_,
                ))
            }
            None => None,
            _ => panic!("a new classification is not inferred from incomplete progress"),
        };
        let visits = interpreter.attempts.clone();
        let counts = retained.each_ref().map(Weak::strong_count);
        let cause_fact = cause.as_ref().map(|(weak, _)| {
            let original = weak
                .upgrade()
                .expect("original opaque native cause remains owned");
            let fact = (
                weak.strong_count() - 1,
                Arc::as_ptr(&original) as usize,
                original.as_slice().to_vec(),
            );
            drop(original);
            fact
        });
        let interrupted = caught.is_err();
        drop(progress);
        drop(interpreter);
        drop(caught);
        let released = retained.each_ref().map(Weak::strong_count);
        let cause_released = cause.as_ref().map(|(weak, _)| weak.strong_count());
        let expected_cause = cause
            .as_ref()
            .map(|(_, allocation)| (1, *allocation, vec![47, 53]));
        let first = (1, allocations[0], vec![61, 67]);
        let current = (2, allocations[1], vec![71, 73]);
        let tail = (3, allocations[2], vec![79, 83]);
        let owned = (4, allocations[3], vec![89, 97]);
        let expected = match cause {
            Some(_) => Some((
                0,
                vec![first, current],
                vec![tail],
                vec![owned],
                Step::Goto(19),
            )),
            None => Some((
                0,
                vec![first, current, tail],
                Vec::new(),
                vec![owned],
                Step::Goto(19),
            )),
        };
        eprintln!(
            "outside complete normal custody={snapshot:?}; counts={counts:?}; released={released:?}"
        );
        assert_eq!(cause_fact, expected_cause);
        assert_eq!(cause_released, expected_cause.as_ref().map(|_| 0));
        assert_eq!(interrupted, expected_cause.is_some());
        assert_eq!(
            visits,
            if interrupted {
                vec![1, 2]
            } else {
                vec![1, 2, 3, 4]
            }
        );
        assert_eq!(
            snapshot, expected,
            "library acquired prefix/current and untouched inner tail/owned sibling/verdict must remain outside the disposed producer"
        );
        assert_eq!(counts, [1, 1, 1, 1]);
        assert_eq!(released, [0, 0, 0, 0]);
    }
    #[test]
    fn normal_progress_keeps_complete_receipts_and_verdict() {
        normal_product_receipts(ReceiptDisposal::Ordinary);
    }
    #[test]
    fn normal_progress_keeps_prefix_current_tail_and_sibling_after_ready_drop() {
        normal_product_receipts(ReceiptDisposal::Panicked(Arc::new(vec![47, 53])));
    }
}
