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

mod delivery_custody {
    use core::future::Future;
    use std::collections::VecDeque;

    use behavior::{
        ActionItem, ActionItemResult, Address, Delivery, EstablishedDelivery, EstablishedRecipient,
        ExactDeliveryReason, Here, InterpretEstablished, InterpretItem, InterpretSends,
        Interpretation, InterpretationProgress, InterpreterFault, ItemSettlement,
        LogicalDeliveryReason, Never, NoSends, ParentReportReason, Protocol, Recipient,
        RecipientAddress, ReportToParent, SendLayer, SendSettlements, SettledItem, SourceCustody,
        SourceProgress, SourceSettlementCustody,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct DeliveryAddress(u8);

    impl Address for DeliveryAddress {
        type Nonce = u8;
    }

    impl RecipientAddress for DeliveryAddress {
        type Established<P>
            = u8
        where
            P: Protocol<Addr = Self>;
    }

    struct ParcelProtocol;

    impl Protocol for ParcelProtocol {
        type Addr = DeliveryAddress;
        type Msg = Parcel;
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Parcel {
        name: &'static str,
        bytes: Box<[u8]>,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ParcelTrace {
        name: &'static str,
        bytes: Vec<u8>,
        allocation: usize,
    }

    impl ParcelTrace {
        fn observe(parcel: &Parcel) -> Self {
            Self {
                name: parcel.name,
                bytes: parcel.bytes.to_vec(),
                allocation: parcel.bytes.as_ptr() as usize,
            }
        }
    }

    #[derive(Clone, Copy)]
    enum DeliveryDisposition {
        Accepted,
        Rejected,
        Corrupt,
    }

    struct DeliveryInterpreter {
        dispositions: VecDeque<DeliveryDisposition>,
        attempts: Vec<(DeliveryAddress, ParcelTrace)>,
    }

    impl InterpretItem<Delivery<ParcelProtocol>, (), Here> for DeliveryInterpreter {
        fn interpret_item<'a>(
            &'a mut self,
            input: &'a mut Option<Delivery<ParcelProtocol>>,
            received: &'a mut Option<<Delivery<ParcelProtocol> as ActionItem>::Reply>,
        ) -> impl Future<Output = ()> + Send + 'a
        where
            Delivery<ParcelProtocol>: 'a,
        {
            async move {
                if received.is_some() {
                    return;
                }
                let Some(item) = input.take() else {
                    return;
                };
                self.attempts
                    .push((item.to.address(), ParcelTrace::observe(&item.message)));
                let disposition = self
                    .dispositions
                    .pop_front()
                    .expect("one authored disposition per attempt");
                *received = Some(match disposition {
                    DeliveryDisposition::Accepted => ItemSettlement::Accepted(()),
                    DeliveryDisposition::Rejected => ItemSettlement::Rejected {
                        item,
                        reason: LogicalDeliveryReason::ClosedRecipient,
                    },
                    DeliveryDisposition::Corrupt => ItemSettlement::Corrupt {
                        item,
                        fault: InterpreterFault::CorruptTraversal,
                    },
                });
            }
        }
    }

    struct EndpointObservation;

    impl InterpretEstablished<ParcelProtocol> for EndpointObservation {
        type Output = u8;
        fn interpret_established(&mut self, endpoint: u8) -> u8 {
            endpoint
        }
    }

    fn parcel(name: &'static str, bytes: &[u8]) -> Parcel {
        Parcel {
            name,
            bytes: bytes.into(),
        }
    }

    fn delivery(address: u8, message: Parcel) -> Delivery<ParcelProtocol> {
        Delivery::new(Recipient::global(DeliveryAddress(address)), message)
    }

    // This is one public loan law exercised with three real owning ActionItems.
    // It does not implement an ActionItem or reproduce an interpreter traversal.
    fn unanswered_request_has_one_loan<Item>(
        request: Item,
        reply: Item::Reply,
    ) -> (Item, Item::Reply)
    where
        Item: ActionItem<Custody = (Option<Item>, Option<<Item as ActionItem>::Reply>)>,
    {
        let mut custody = (Some(request), Some(reply));
        let loan = Item::interpretation_input(&mut custody);
        assert!(
            loan.is_none(),
            "an acquired reply forbids another request loan"
        );
        drop(loan);
        let request = custody
            .0
            .take()
            .expect("denial preserves the original request");
        let loan = Item::interpretation_input(&mut custody);
        assert!(
            loan.is_none(),
            "an acquired reply without original input cannot be offered"
        );
        drop(loan);
        let reply = custody
            .1
            .take()
            .expect("denial preserves the complete reply");
        let loan = Item::interpretation_input(&mut custody);
        assert!(loan.is_none(), "absent original input cannot be offered");
        drop(loan);
        custody.0 = Some(request);
        let loan = Item::interpretation_input(&mut custody);
        assert!(
            loan.is_some(),
            "an unanswered original must remain loanable"
        );
        drop(loan);
        let request = custody
            .0
            .take()
            .expect("constructing a loan transfers no request");
        assert!(custody.1.is_none(), "constructing a loan invents no reply");
        (request, reply)
    }

    fn rejected_request_finishes_once<Item, Reason>(request: Item, reason: Reason) -> Item::Reply
    where
        Item: ActionItem<
                Accepted = (),
                Rejection = Reason,
                Prerequisite = Never,
                Reply = ItemSettlement<Item, (), Reason, Never>,
                Custody = (
                    Option<Item>,
                    Option<ItemSettlement<Item, (), Reason, Never>>,
                ),
            >,
        Reason: Send,
    {
        let mut progress = Some(InterpretationProgress::Original(request));
        Item::prepare_interpretation(&mut progress);
        assert!(
            matches!(&progress, Some(InterpretationProgress::Interpreting(_))),
            "preparation must establish outside custody before replay"
        );
        Item::prepare_interpretation(&mut progress);
        let Some(InterpretationProgress::Interpreting(custody)) = &mut progress else {
            panic!("cold preparation must retain a request and its independent reply slot");
        };
        let loan = Item::interpretation_input(custody);
        assert!(
            loan.is_some(),
            "cold preparation must permit the exact original request"
        );
        drop(loan);
        let request = custody
            .0
            .take()
            .expect("producer acquires the actual request");
        custody.1 = Some(ItemSettlement::Rejected {
            item: request,
            reason,
        });
        let loan = Item::interpretation_input(custody);
        assert!(
            loan.is_none(),
            "published reply forbids a second producer loan"
        );
        drop(loan);
        Item::finish_interpretation(&mut progress);
        assert!(
            matches!(
                &progress,
                Some(InterpretationProgress::Completed(Interpretation::Complete(
                    ItemSettlement::Rejected { .. }
                )))
            ),
            "the acquired rejection must complete before any later operation"
        );
        Item::finish_interpretation(&mut progress);
        Item::prepare_interpretation(&mut progress);
        Item::finish_interpretation(&mut progress);
        let Some(InterpretationProgress::Completed(Interpretation::Complete(reply))) = progress
        else {
            panic!("finalization and replay must preserve one complete lawful rejection");
        };
        reply
    }

    #[test]
    fn built_in_delivery_and_parent_report_loans_preserve_both_owned_values() {
        let first = parcel("original", &[3, 5]);
        let second = parcel("already replied", &[7, 11]);
        let first_trace = ParcelTrace::observe(&first);
        let second_trace = ParcelTrace::observe(&second);
        let (original, reply) = unanswered_request_has_one_loan(
            delivery(13, first),
            ItemSettlement::Rejected {
                item: delivery(17, second),
                reason: LogicalDeliveryReason::UnknownAddress,
            },
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain exact rejection");
        };
        assert_eq!(
            (
                original.to.address(),
                ParcelTrace::observe(&original.message)
            ),
            (DeliveryAddress(13), first_trace)
        );
        assert_eq!(
            (
                item.to.address(),
                ParcelTrace::observe(&item.message),
                reason
            ),
            (
                DeliveryAddress(17),
                second_trace,
                LogicalDeliveryReason::UnknownAddress
            )
        );

        let first = parcel("exact original", &[19, 23]);
        let second = parcel("exact replied", &[29, 31]);
        let first_trace = ParcelTrace::observe(&first);
        let second_trace = ParcelTrace::observe(&second);
        let (original, reply) = unanswered_request_has_one_loan(
            EstablishedDelivery::<ParcelProtocol>::new(EstablishedRecipient::issued(37), first),
            ItemSettlement::Rejected {
                item: EstablishedDelivery::<ParcelProtocol>::new(
                    EstablishedRecipient::issued(41),
                    second,
                ),
                reason: ExactDeliveryReason::ClosedRecipient,
            },
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain exact rejection");
        };
        let original_endpoint = original.to.interpret(&mut EndpointObservation);
        let replied_endpoint = item.to.interpret(&mut EndpointObservation);
        assert_eq!(
            (original_endpoint, ParcelTrace::observe(&original.message)),
            (37, first_trace)
        );
        assert_eq!(
            (
                replied_endpoint,
                ParcelTrace::observe(&item.message),
                reason
            ),
            (41, second_trace, ExactDeliveryReason::ClosedRecipient)
        );

        let first = parcel("original report", &[43, 47]);
        let second = parcel("replied report", &[53, 59]);
        let first_trace = ParcelTrace::observe(&first);
        let second_trace = ParcelTrace::observe(&second);
        let (original, reply) = unanswered_request_has_one_loan(
            ReportToParent::new(first),
            ItemSettlement::Rejected {
                item: ReportToParent::new(second),
                reason: ParentReportReason::ClosedParentControlLane,
            },
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain exact rejection");
        };
        let original_report = original.into_inner();
        assert_eq!(ParcelTrace::observe(&original_report), first_trace);
        let report = item.into_inner();
        assert_eq!(
            (ParcelTrace::observe(&report), reason),
            (second_trace, ParentReportReason::ClosedParentControlLane)
        );
    }

    #[test]
    fn built_in_delivery_and_parent_report_rejections_finish_once() {
        let message = parcel("logical rejection", &[61, 67]);
        let expected = ParcelTrace::observe(&message);
        let reply = rejected_request_finishes_once(
            delivery(71, message),
            LogicalDeliveryReason::ClosedRecipient,
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain complete logical rejection");
        };
        assert_eq!(
            (
                item.to.address(),
                ParcelTrace::observe(&item.message),
                reason
            ),
            (
                DeliveryAddress(71),
                expected,
                LogicalDeliveryReason::ClosedRecipient
            )
        );

        let message = parcel("exact rejection", &[73, 79]);
        let expected = ParcelTrace::observe(&message);
        let reply = rejected_request_finishes_once(
            EstablishedDelivery::<ParcelProtocol>::new(EstablishedRecipient::issued(83), message),
            ExactDeliveryReason::ClosedRecipient,
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain complete exact rejection");
        };
        let endpoint = item.to.interpret(&mut EndpointObservation);
        assert_eq!(
            (endpoint, ParcelTrace::observe(&item.message), reason),
            (83, expected, ExactDeliveryReason::ClosedRecipient)
        );

        let report = parcel("parent rejection", &[89, 97]);
        let expected = ParcelTrace::observe(&report);
        let reply = rejected_request_finishes_once(
            ReportToParent::new(report),
            ParentReportReason::ClosedParentControlLane,
        );
        let ItemSettlement::Rejected { item, reason } = reply else {
            panic!("retain complete parent rejection");
        };
        let report = item.into_inner();
        assert_eq!(
            (ParcelTrace::observe(&report), reason),
            (expected, ParentReportReason::ClosedParentControlLane)
        );
    }

    #[tokio::test]
    async fn ordinary_delivery_vector_continues_after_acceptance_and_rejection_then_replays_once() {
        let messages = [
            parcel("first", &[2, 3]),
            parcel("rejected", &[5, 7]),
            parcel("last", &[11, 13]),
        ];
        let observed = messages.each_ref().map(ParcelTrace::observe);
        let [first, rejected, last] = messages;
        let mut progress = Some(InterpretationProgress::Original(vec![
            delivery(17, first),
            delivery(19, rejected),
            delivery(23, last),
        ]));
        let mut interpreter = DeliveryInterpreter {
            dispositions: [
                DeliveryDisposition::Accepted,
                DeliveryDisposition::Rejected,
                DeliveryDisposition::Accepted,
            ]
            .into(),
            attempts: Vec::new(),
        };
        <Vec<Delivery<ParcelProtocol>> as InterpretSends<_, (), Here>>::interpret(
            &mut progress,
            &mut interpreter,
        )
        .await;
        assert!(
            matches!(
                &progress,
                Some(InterpretationProgress::Completed(Interpretation::Complete(
                    _
                )))
            ),
            "the first traversal must establish its full verdict before replay"
        );
        <Vec<Delivery<ParcelProtocol>> as InterpretSends<_, (), Here>>::interpret(
            &mut progress,
            &mut interpreter,
        )
        .await;
        let Some(InterpretationProgress::Completed(Interpretation::Complete(settled))) = progress
        else {
            panic!("all independent actual deliveries must complete");
        };
        let [
            SettledItem::Attempted(ItemSettlement::Accepted(())),
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
            SettledItem::Attempted(ItemSettlement::Accepted(())),
        ] = settled.as_slice()
        else {
            panic!("retain the entire ordered typed settlement lane");
        };
        let rejected_trace = ParcelTrace::observe(&item.message);
        assert_eq!(
            (item.to.address(), rejected_trace, *reason),
            (
                DeliveryAddress(19),
                ParcelTrace {
                    name: observed[1].name,
                    bytes: observed[1].bytes.clone(),
                    allocation: observed[1].allocation
                },
                LogicalDeliveryReason::ClosedRecipient
            )
        );
        let [first, rejected, last] = observed;
        assert_eq!(
            interpreter.attempts,
            [
                (DeliveryAddress(17), first),
                (DeliveryAddress(19), rejected),
                (DeliveryAddress(23), last)
            ]
        );
        assert!(interpreter.dispositions.is_empty());
    }

    #[tokio::test]
    async fn ordinary_delivery_vector_corruption_preserves_exact_unattempted_suffix_on_replay() {
        let messages = [
            parcel("prefix", &[29, 31]),
            parcel("corrupt", &[37, 41]),
            parcel("suffix", &[43, 47]),
        ];
        let [prefix_trace, corrupt_trace, suffix_trace] =
            messages.each_ref().map(ParcelTrace::observe);
        let [prefix, corrupt, suffix] = messages;
        let mut progress = Some(InterpretationProgress::Original(vec![
            delivery(53, prefix),
            delivery(59, corrupt),
            delivery(61, suffix),
        ]));
        let mut interpreter = DeliveryInterpreter {
            dispositions: [DeliveryDisposition::Accepted, DeliveryDisposition::Corrupt].into(),
            attempts: Vec::new(),
        };
        <Vec<Delivery<ParcelProtocol>> as InterpretSends<_, (), Here>>::interpret(
            &mut progress,
            &mut interpreter,
        )
        .await;
        assert!(
            matches!(
                &progress,
                Some(InterpretationProgress::Completed(Interpretation::Corrupt(
                    _
                )))
            ),
            "the first traversal must establish its full verdict before replay"
        );
        <Vec<Delivery<ParcelProtocol>> as InterpretSends<_, (), Here>>::interpret(
            &mut progress,
            &mut interpreter,
        )
        .await;
        let Some(InterpretationProgress::Completed(Interpretation::Corrupt(settled))) = progress
        else {
            panic!("retain the exact corruption verdict");
        };
        let [
            SettledItem::Attempted(ItemSettlement::Accepted(())),
            SettledItem::Attempted(ItemSettlement::Corrupt { item, fault }),
            SettledItem::Unattempted(suffix),
        ] = settled.as_slice()
        else {
            panic!("retain complete prefix, fault and suffix");
        };
        assert_eq!(
            (
                item.to.address(),
                ParcelTrace::observe(&item.message),
                *fault
            ),
            (
                DeliveryAddress(59),
                ParcelTrace {
                    name: corrupt_trace.name,
                    bytes: corrupt_trace.bytes.clone(),
                    allocation: corrupt_trace.allocation
                },
                InterpreterFault::CorruptTraversal
            )
        );
        assert_eq!(
            (suffix.to.address(), ParcelTrace::observe(&suffix.message)),
            (DeliveryAddress(61), suffix_trace)
        );
        assert_eq!(
            interpreter.attempts,
            [
                (DeliveryAddress(53), prefix_trace),
                (DeliveryAddress(59), corrupt_trace)
            ]
        );
        assert!(interpreter.dispositions.is_empty());
    }

    #[derive(Clone, Copy)]
    enum EmptySendOperation {
        Prepare,
        Finish,
        Unattempted,
    }

    #[test]
    fn empty_send_lane_operations_complete_cold_inputs_and_preserve_completed_replay() {
        for operation in [
            EmptySendOperation::Prepare,
            EmptySendOperation::Finish,
            EmptySendOperation::Unattempted,
        ] {
            let mut no_sends = Some(InterpretationProgress::Original(NoSends));
            let mut impossible_sends = Some(InterpretationProgress::Original(Vec::<Never>::new()));
            match operation {
                EmptySendOperation::Prepare => {
                    NoSends::prepare_interpretation(&mut no_sends);
                    Vec::<Never>::prepare_interpretation(&mut impossible_sends);
                }
                EmptySendOperation::Finish => {
                    NoSends::finish_interpretation(&mut no_sends);
                    Vec::<Never>::finish_interpretation(&mut impossible_sends);
                }
                EmptySendOperation::Unattempted => {
                    NoSends::unattempted(&mut no_sends);
                    Vec::<Never>::unattempted(&mut impossible_sends);
                }
            }
            let Some(InterpretationProgress::Completed(Interpretation::Complete(NoSends))) =
                &no_sends
            else {
                panic!("an empty send lane completes with its exact original product");
            };
            let Some(InterpretationProgress::Completed(Interpretation::Complete(empty))) =
                &impossible_sends
            else {
                panic!("an impossible item lane completes with its exact empty product");
            };
            assert!(empty.is_empty());
            NoSends::prepare_interpretation(&mut no_sends);
            NoSends::finish_interpretation(&mut no_sends);
            NoSends::unattempted(&mut no_sends);
            Vec::<Never>::prepare_interpretation(&mut impossible_sends);
            Vec::<Never>::finish_interpretation(&mut impossible_sends);
            Vec::<Never>::unattempted(&mut impossible_sends);
            let Some(InterpretationProgress::Completed(Interpretation::Complete(NoSends))) =
                no_sends
            else {
                panic!("replay preserves empty send completion");
            };
            let Some(InterpretationProgress::Completed(Interpretation::Complete(empty))) =
                impossible_sends
            else {
                panic!("replay preserves impossible send completion");
            };
            assert!(empty.is_empty());
        }
    }

    #[test]
    fn unattempted_send_layer_retains_both_exact_delivery_lanes_on_replay() {
        let inner = parcel("inner untouched", &[67, 71]);
        let owned = parcel("owned untouched", &[73, 79]);
        let inner_trace = ParcelTrace::observe(&inner);
        let owned_trace = ParcelTrace::observe(&owned);
        let mut progress = Some(InterpretationProgress::Original(SendLayer::new(
            vec![delivery(83, owned)],
            vec![delivery(89, inner)],
        )));
        <SendLayer<Vec<Delivery<ParcelProtocol>>, Vec<Delivery<ParcelProtocol>>> as SendSettlements>::unattempted(&mut progress);
        assert!(
            matches!(
                &progress,
                Some(InterpretationProgress::Completed(Interpretation::Complete(
                    _
                )))
            ),
            "the first unattempted projection must complete both original send lanes"
        );
        <SendLayer<Vec<Delivery<ParcelProtocol>>, Vec<Delivery<ParcelProtocol>>> as SendSettlements>::unattempted(&mut progress);
        let Some(InterpretationProgress::Completed(Interpretation::Complete(layer))) = progress
        else {
            panic!("retain complete untouched send product");
        };
        let [SettledItem::Unattempted(inner)] = layer.inner.as_slice() else {
            panic!("retain exact inner item");
        };
        let [SettledItem::Unattempted(owned)] = layer.owned.as_slice() else {
            panic!("retain exact owned item");
        };
        assert_eq!(
            (inner.to.address(), ParcelTrace::observe(&inner.message)),
            (DeliveryAddress(89), inner_trace)
        );
        assert_eq!(
            (owned.to.address(), ParcelTrace::observe(&owned.message)),
            (DeliveryAddress(83), owned_trace)
        );
    }

    #[test]
    fn empty_source_lanes_finish_original_and_offering_custody_then_preserve_replay() {
        let mut no_sends = Some(SourceProgress::Original(NoSends));
        <NoSends as SourceSettlementCustody<(), ()>>::finish_source(&mut no_sends);
        assert!(
            matches!(
                &no_sends,
                Some(SourceProgress::Completed(SourceCustody::Exhausted(NoSends)))
            ),
            "the first finish must exhaust the original empty source lane"
        );
        <NoSends as SourceSettlementCustody<(), ()>>::finish_source(&mut no_sends);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(NoSends))) = no_sends else {
            panic!("finish conserves and exhausts the original empty lane");
        };
        let mut no_sends = Some(SourceProgress::Offering(NoSends));
        <NoSends as SourceSettlementCustody<(), ()>>::finish_source(&mut no_sends);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(NoSends))) = no_sends else {
            panic!("finish conserves and exhausts the offering empty lane");
        };
        let mut impossible = Some(SourceProgress::Original(Vec::<Never>::new()));
        <Vec<Never> as SourceSettlementCustody<(), ()>>::prepare_source(&mut impossible);
        assert!(
            matches!(&impossible, Some(SourceProgress::Completed(SourceCustody::Exhausted(empty))) if empty.is_empty()),
            "the first preparation must exhaust the original impossible source lane"
        );
        <Vec<Never> as SourceSettlementCustody<(), ()>>::finish_source(&mut impossible);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(empty))) = impossible else {
            panic!("prepare exhausts the impossible source lane");
        };
        assert!(empty.is_empty());
        let mut impossible = Some(SourceProgress::Offering(Vec::<Never>::new()));
        <Vec<Never> as SourceSettlementCustody<(), ()>>::finish_source(&mut impossible);
        assert!(
            matches!(&impossible, Some(SourceProgress::Completed(SourceCustody::Exhausted(empty))) if empty.is_empty()),
            "the first finish must exhaust the offering impossible source lane"
        );
        <Vec<Never> as SourceSettlementCustody<(), ()>>::finish_source(&mut impossible);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(empty))) = impossible else {
            panic!("finish exhausts the offering impossible source lane");
        };
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn source_delivery_results_cannot_finish_before_all_owned_residuals_are_visited() {
        let first = parcel("rejected source", &[97, 101]);
        let second = parcel("untouched source", &[103, 107]);
        let first_trace = ParcelTrace::observe(&first);
        let second_trace = ParcelTrace::observe(&second);
        let results: Vec<ActionItemResult<Delivery<ParcelProtocol>>> = vec![
            SettledItem::Attempted(ItemSettlement::Rejected {
                item: delivery(109, first),
                reason: LogicalDeliveryReason::UnknownAddress,
            }),
            SettledItem::Unattempted(delivery(113, second)),
        ];
        let mut progress = Some(SourceProgress::Original(results));
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::prepare_source(&mut progress);
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::finish_source(&mut progress);
        let Some(SourceProgress::Offering(custody)) = &mut progress else {
            panic!("premature finish cannot erase unvisited authoritative results");
        };
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::offer_next_to_source(custody, &mut ()).await;
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::finish_source(&mut progress);
        assert!(
            matches!(
                &progress,
                Some(SourceProgress::Completed(SourceCustody::Retained(_)))
            ),
            "the first finish after complete traversal must retain every authoritative residual before replay"
        );
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::finish_source(&mut progress);
        let Some(SourceProgress::Completed(SourceCustody::Retained(results))) = progress else {
            panic!("full residual custody remains retained on replay");
        };
        let [
            SettledItem::Attempted(ItemSettlement::Rejected { item, reason }),
            SettledItem::Unattempted(second),
        ] = results.as_slice()
        else {
            panic!("retain every complete typed source result");
        };
        assert_eq!(
            (
                item.to.address(),
                ParcelTrace::observe(&item.message),
                *reason
            ),
            (
                DeliveryAddress(109),
                first_trace,
                LogicalDeliveryReason::UnknownAddress
            )
        );
        assert_eq!(
            (second.to.address(), ParcelTrace::observe(&second.message)),
            (DeliveryAddress(113), second_trace)
        );

        let mut empty = Some(SourceProgress::Original(Vec::<
            ActionItemResult<Delivery<ParcelProtocol>>,
        >::new()));
        <Vec<ActionItemResult<Delivery<ParcelProtocol>>> as SourceSettlementCustody<(), ()>>::prepare_source(&mut empty);
        let Some(SourceProgress::Completed(SourceCustody::Exhausted(empty))) = empty else {
            panic!("empty result preparation establishes exhaustion without an offer");
        };
        assert!(empty.is_empty());
    }
    #[tokio::test]
    async fn delivery_vector_preparation_preserves_every_original_before_any_interpretation() {
        let messages = [
            parcel("prepared first", &[127, 131]),
            parcel("prepared second", &[137, 139]),
        ];
        let [first_trace, second_trace] = messages.each_ref().map(ParcelTrace::observe);
        let [first, second] = messages;
        let mut progress = Some(InterpretationProgress::Original(vec![
            delivery(149, first),
            delivery(151, second),
        ]));
        Vec::<Delivery<ParcelProtocol>>::prepare_interpretation(&mut progress);
        assert!(
            matches!(&progress, Some(InterpretationProgress::Interpreting(_))),
            "the first vector preparation must establish outside custody before replay"
        );
        Vec::<Delivery<ParcelProtocol>>::prepare_interpretation(&mut progress);
        let Some(InterpretationProgress::Interpreting(rows)) = &mut progress else {
            panic!("vector preparation must expose every original in outside custody");
        };
        let mut prepared = Vec::new();
        for row in rows {
            Delivery::<ParcelProtocol>::prepare_interpretation(row);
            let Some(InterpretationProgress::Interpreting(custody)) = row else {
                panic!("each prepared original remains loanable without interpretation");
            };
            let Some((input, received)) = Delivery::<ParcelProtocol>::interpretation_input(custody)
            else {
                panic!("every unanswered original has its own available loan");
            };
            let original = input.as_ref().expect("the caller retains the original");
            prepared.push((
                original.to.address(),
                ParcelTrace::observe(&original.message),
            ));
            assert!(
                received.is_none(),
                "preparation cannot publish an invented reply"
            );
        }
        assert_eq!(
            prepared,
            [
                (DeliveryAddress(149), first_trace),
                (DeliveryAddress(151), second_trace)
            ]
        );
        let mut interpreter = DeliveryInterpreter {
            dispositions: [DeliveryDisposition::Accepted, DeliveryDisposition::Accepted].into(),
            attempts: Vec::new(),
        };
        <Vec<Delivery<ParcelProtocol>> as InterpretSends<_, (), Here>>::interpret(
            &mut progress,
            &mut interpreter,
        )
        .await;
        let Some(InterpretationProgress::Completed(Interpretation::Complete(settled))) = progress
        else {
            panic!("all prepared delivery originals must complete");
        };
        let [
            SettledItem::Attempted(ItemSettlement::Accepted(())),
            SettledItem::Attempted(ItemSettlement::Accepted(())),
        ] = settled.as_slice()
        else {
            panic!("preserve the entire exact delivery settlement lane");
        };
        assert_eq!(interpreter.attempts, prepared);
        assert!(interpreter.dispositions.is_empty());
    }
}
