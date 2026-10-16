//! Named and occurrence-owned timers retain their exact request products.

use behavior::{ActionItem, InterpretationProgress};
use behavior_actors::{
    OwnedScheduleAfter, OwnedScheduleAt, ScheduleAfter, ScheduleAt, TimerElapsed, TimerGeneration,
    TimerId, TimerScheduled,
};
use std::time::{Duration, Instant};

#[test]
fn named_timer_products_preserve_constructors_fields_patterns_and_tuples() {
    let id = TimerId(17);
    let generation = TimerGeneration(23);
    let at = Instant::now();
    let after = Duration::from_secs(5);
    let absolute = ScheduleAt::new(id, generation, at);
    assert_eq!(absolute, ScheduleAt::from((id, generation, at)));
    let ScheduleAt {
        id: named,
        generation: returned,
        at: deadline,
    } = absolute;
    assert_eq!((named, returned, deadline), (id, generation, at));
    let relative = ScheduleAfter::new(id, generation, after);
    assert_eq!(
        relative,
        ScheduleAfter {
            id,
            generation,
            after
        }
    );
    assert_eq!(relative, ScheduleAfter::from((id, generation, after)));
    let elapsed = TimerElapsed::new(id, generation);
    assert_eq!(elapsed, TimerElapsed::from((id, generation)));
    let TimerElapsed {
        id: elapsed_id,
        generation: elapsed_generation,
    } = elapsed;
    assert_eq!(
        TimerElapsed {
            id: elapsed_id,
            generation: elapsed_generation
        },
        TimerElapsed { id, generation }
    );
    let TimerScheduled {
        id: accepted,
        generation: committed,
    } = TimerScheduled { id, generation };
    assert_eq!((accepted, committed), (id, generation));
}

#[test]
fn an_unsettled_owned_absolute_timer_keeps_its_exact_original_request() {
    let at = Instant::now();
    let original = OwnedScheduleAt::new(TimerGeneration(29), at);
    let mut progress = Some(InterpretationProgress::Original(original));
    OwnedScheduleAt::prepare_interpretation(&mut progress);
    OwnedScheduleAt::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Interpreting((Some(retained), None))) = progress else {
        panic!("a timer without a receipt retains its original request");
    };
    assert_eq!(retained, original);
    assert_eq!(
        (retained.generation, retained.at),
        (TimerGeneration(29), at)
    );
}

#[test]
fn an_unsettled_owned_relative_timer_keeps_its_exact_original_request() {
    let after = Duration::from_secs(7);
    let original = OwnedScheduleAfter::new(TimerGeneration(31), after);
    let mut progress = Some(InterpretationProgress::Original(original));
    OwnedScheduleAfter::prepare_interpretation(&mut progress);
    OwnedScheduleAfter::finish_interpretation(&mut progress);
    let Some(InterpretationProgress::Interpreting((Some(retained), None))) = progress else {
        panic!("a timer without a receipt retains its original request");
    };
    assert_eq!(retained, original);
    assert_eq!(
        (retained.generation, retained.after),
        (TimerGeneration(31), after)
    );
}
