#![no_main]

use std::{num::NonZeroU32, time::Duration};

use behavior::{
    BreakerMessage, BreakerOutcome, CircuitBreaker, Compose, MailAddr, Presence, PresenceMessage,
    PresenceReply, PresenceVersion, Recipient, TimerElapsed, TimerGeneration, TimerId, Workflow,
    WorkflowDefinition, WorkflowMessage, WorkflowOutcome,
};
use bombay_behavior_fuzz::TestRecipient;
use libfuzzer_sys::fuzz_target;

type BreakerReply = TestRecipient<BreakerOutcome>;
type PresenceReplyTarget = TestRecipient<PresenceReply<Vec<u8>>>;
type WorkflowReply = TestRecipient<WorkflowOutcome<u8>>;

fn timer(key: &Vec<u8>) -> TimerId {
    TimerId(key.first().copied().map_or(0, u64::from))
}

fuzz_target!(|bytes: &[u8]| {
    let mut breaker = Compose::new(
        CircuitBreaker::<MailAddr, BreakerReply>::new(
            NonZeroU32::new(2).expect("constant is non-zero"),
            Duration::from_nanos(1),
            TimerId(1),
        )
        .expect("constant reset delay is positive"),
    )
    .initialize()
    .expect("breaker initialization is infallible")
    .behavior;
    let mut presence = Compose::new(Presence::<MailAddr, Vec<u8>, PresenceReplyTarget>::new(timer))
        .initialize()
        .expect("presence initialization is infallible")
        .behavior;
    let mut workflow = Compose::new(
        Workflow::<MailAddr, u8, WorkflowReply>::new(WorkflowDefinition {
            steps: vec![0, 1, 2],
            dependencies: vec![(0, 2), (1, 2)],
        })
        .expect("constant graph is acyclic"),
    )
    .initialize()
    .expect("workflow initialization is infallible")
    .behavior;

    let breaker_reply = Recipient::global(MailAddr(1));
    let presence_reply = Recipient::global(MailAddr(2));
    let workflow_reply = Recipient::global(MailAddr(3));
    for chunk in bytes.chunks(4) {
        let a = chunk.first().copied().unwrap_or(0);
        let b = chunk.get(1).copied().unwrap_or(0);
        let generation = TimerGeneration(u64::from(chunk.get(2).copied().unwrap_or(0)));
        let attempt = behavior::BreakerAttempt(u64::from(b));
        let breaker_message = match a % 4 {
            0 => BreakerMessage::Admit { reply_to: breaker_reply },
            1 => BreakerMessage::Succeeded { attempt },
            2 => BreakerMessage::Failed { attempt },
            _ => BreakerMessage::Elapsed(TimerElapsed::new(TimerId(1), generation)),
        };
        breaker.receive(MailAddr(0), breaker_message).expect("breaker fold is infallible");

        let participant = vec![b];
        let presence_message = if a % 3 == 0 {
            PresenceMessage::Elapsed(TimerElapsed::new(TimerId(u64::from(b)), generation))
        } else {
            PresenceMessage::Announce {
                participant,
                version: PresenceVersion(u64::from(generation.0 as u8)),
                lifetime: Duration::from_nanos(1),
                reply_to: presence_reply,
            }
        };
        presence.receive(MailAddr(0), presence_message).expect("presence fold is infallible");

        let workflow_message = match a % 4 {
            0 => WorkflowMessage::Start { reply_to: workflow_reply },
            1 => WorkflowMessage::Complete { step: b % 4 },
            2 => WorkflowMessage::Fail { step: b % 4 },
            _ => WorkflowMessage::Cancel { reply_to: workflow_reply },
        };
        workflow.receive(MailAddr(0), workflow_message).expect("workflow fold is infallible");
    }
});
