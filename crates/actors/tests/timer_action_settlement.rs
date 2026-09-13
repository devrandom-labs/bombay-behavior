use std::time::{Duration, Instant};

use behavior_actors::{
    ActionItem, ActionItemResult, Behavior, Deadline, InterpreterRequests, ItemSettlement,
    MailAddr, MessageProtocol, Never, NoBirths, NoSends, ReceiveTimeout, ScheduleAfter,
    ScheduleAfterRejection, ScheduleAt, ScheduleAtRejection, SendLayer, SendSettlements,
    SettledItem, TimerGeneration, TimerId, TimerScheduled, User,
};

struct Probe;

impl Behavior for Probe {
    type Protocol = MessageProtocol<MailAddr, Never>;
    type Event = User<MailAddr, Never>;
    type Sends = NoSends;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(
        &mut self,
        _: behavior_actors::ActiveTurn,
        event: Self::Event,
    ) -> behavior_actors::BehaviorActed<Self> {
        match event.message {}
    }
}

fn has_total_settlement<Sends>()
where
    Sends: SendSettlements,
{
}

fn has_exact_timer_contract<Request>()
where
    Request: ActionItem<Accepted = TimerScheduled, Prerequisite = Never>,
{
}

#[test]
fn real_timer_wrappers_preserve_both_lanes_in_both_orders() {
    has_total_settlement::<<behavior_actors::ReceiveTimeout<Probe> as Behavior>::Sends>();
    has_total_settlement::<<Deadline<Probe> as Behavior>::Sends>();
    has_total_settlement::<<ReceiveTimeout<Deadline<Probe>> as Behavior>::Sends>();
    has_total_settlement::<<Deadline<ReceiveTimeout<Probe>> as Behavior>::Sends>();

    let _: Option<
        SendLayer<
            InterpreterRequests<ScheduleAfter>,
            SendLayer<InterpreterRequests<ScheduleAt>, NoSends>,
        >,
    > = None::<<ReceiveTimeout<Deadline<Probe>> as Behavior>::Sends>;
    let _: Option<
        SendLayer<
            InterpreterRequests<ScheduleAt>,
            SendLayer<InterpreterRequests<ScheduleAfter>, NoSends>,
        >,
    > = None::<<Deadline<ReceiveTimeout<Probe>> as Behavior>::Sends>;
}

#[test]
fn accepted_receipts_and_rejections_retain_exact_timer_values() {
    let id = TimerId(7);
    let generation = TimerGeneration(11);
    let scheduled = TimerScheduled { id, generation };
    let accepted: ItemSettlement<ScheduleAfter, TimerScheduled, ScheduleAfterRejection, Never> =
        ItemSettlement::Accepted(scheduled);
    assert_eq!(accepted, ItemSettlement::Accepted(scheduled));

    let relative = ScheduleAfter::new(id, generation, Duration::from_secs(3));
    let relative_rejected: ItemSettlement<
        ScheduleAfter,
        TimerScheduled,
        ScheduleAfterRejection,
        Never,
    > = ItemSettlement::Rejected {
        item: relative,
        reason: ScheduleAfterRejection::DeadlineOverflow,
    };
    assert_eq!(
        relative_rejected,
        ItemSettlement::Rejected {
            item: relative,
            reason: ScheduleAfterRejection::DeadlineOverflow,
        }
    );

    let absolute = ScheduleAt::new(id, generation, Instant::now());
    let absolute_rejected: ItemSettlement<ScheduleAt, TimerScheduled, ScheduleAtRejection, Never> =
        ItemSettlement::Rejected {
            item: absolute,
            reason: ScheduleAtRejection::QueueSequenceExhausted,
        };
    assert_eq!(
        absolute_rejected,
        ItemSettlement::Rejected {
            item: absolute,
            reason: ScheduleAtRejection::QueueSequenceExhausted,
        }
    );
}

#[test]
fn every_dependency_rejection_is_explicit_and_requests_remain_unattempted() {
    let relative_rejections = [
        ScheduleAfterRejection::DeadlineOverflow,
        ScheduleAfterRejection::QueueGenerationExhausted,
        ScheduleAfterRejection::QueueSequenceExhausted,
    ];
    let absolute_rejections = [
        ScheduleAtRejection::QueueGenerationExhausted,
        ScheduleAtRejection::QueueSequenceExhausted,
    ];
    assert_eq!(relative_rejections.len(), 3);
    assert_eq!(absolute_rejections.len(), 2);

    let request = ScheduleAfter::new(TimerId(2), TimerGeneration(5), Duration::ZERO);
    let unattempted = InterpreterRequests::one(request).unattempted();
    assert_eq!(
        unattempted,
        vec![behavior_actors::SettledItem::Unattempted(request)]
    );

    has_exact_timer_contract::<ScheduleAt>();
    has_exact_timer_contract::<ScheduleAfter>();
}

#[test]
fn timer_actions_return_their_generic_settlement_without_an_actor_adapter() {
    let relative = ScheduleAfter::new(TimerId(13), TimerGeneration(17), Duration::from_secs(5));
    let relative_result: ActionItemResult<ScheduleAfter> = SettledItem::Unattempted(relative);
    assert_eq!(relative_result, SettledItem::Unattempted(relative));

    let absolute = ScheduleAt::new(TimerId(19), TimerGeneration(23), Instant::now());
    let absolute_result: ActionItemResult<ScheduleAt> = SettledItem::Unattempted(absolute);
    assert_eq!(absolute_result, SettledItem::Unattempted(absolute));
}
