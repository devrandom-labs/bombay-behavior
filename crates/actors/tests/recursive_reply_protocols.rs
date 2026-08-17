//! Compile-time regression matrix for reply adapters that target their sender's root.

use behavior_actors::*;

struct Target;

impl Protocol for Target {
    type Addr = MailAddr;
    type Msg = u8;
}

impl Behavior for Target {
    type Event = User<MailAddr, u8>;
    type Sends = Vec<Never>;
    type Ph = Never;
    type Error = Never;
    type Birth = NoBirths;

    fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
        Ok(Actions::cont())
    }
}

macro_rules! recursive_reply_case {
    ($module:ident, $input:ty, $subject:ty) => {
        mod $module {
            use super::*;

            struct Root;
            type Reply = MessageAdapter<$input, Guardian<Root>>;
            type Subject = $subject;

            impl Protocol for Root {
                type Addr = MailAddr;
                type Msg = ();
            }

            impl Behavior for Root {
                type Event = User<MailAddr, ()>;
                type Sends = Vec<Delivery<Subject>>;
                type Ph = Never;
                type Error = Never;
                type Birth = NoBirths;

                fn transition(&mut self, _: ActiveTurn, _: Self::Event) -> BehaviorActed<Self> {
                    Ok(Actions::cont())
                }
            }

            fn adapt(_: $input) {}

            #[test]
            fn root_and_reply_template_form_a_finite_trait_proof() {
                fn assert_behavior<B: Behavior>() {}
                assert_behavior::<Guardian<Root>>();
                assert_behavior::<Subject>();
                let root = Recipient::<Guardian<Root>>::global(MailAddr(1));
                let _: Reply = MessageAdapter::new(root, adapt);
            }
        }
    };
}

recursive_reply_case!(
    acknowledgements,
    AcknowledgementOutcome<u8, u16>,
    Acknowledgements<MailAddr, u8, u16, Reply>
);
recursive_reply_case!(
    buffer,
    BufferOutcome<u8>,
    Buffer<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(
    cache,
    CacheResult<u8, u16>,
    Cache<MailAddr, u8, u16, Reply>
);
recursive_reply_case!(
    circuit_breaker,
    BreakerOutcome,
    CircuitBreaker<MailAddr, Reply>
);
recursive_reply_case!(
    configuration,
    ConfigurationState<u8>,
    Configuration<MailAddr, u8, Reply>
);
recursive_reply_case!(
    correlator,
    CorrelationResult<u8, u16>,
    Correlator<MailAddr, u8, u16, Reply>
);
recursive_reply_case!(
    deduplicator,
    DeduplicatorOutcome<u8, u8>,
    Deduplicator<MailAddr, u8, u8, Target, Reply>
);
recursive_reply_case!(health, HealthReport<u8>, Health<MailAddr, u8, Reply>);
recursive_reply_case!(lease, LeaseOutcome<u8>, Lease<MailAddr, u8, Reply>);
recursive_reply_case!(
    order_gate,
    OrderGateOutcome<u8, u8>,
    OrderGate<MailAddr, u8, u8, Target, Reply>
);
recursive_reply_case!(
    presence,
    PresenceReply<u8>,
    Presence<MailAddr, u8, Reply>
);
recursive_reply_case!(
    priority_queue,
    PriorityQueueOutcome<u8>,
    PriorityQueue<MailAddr, u8, u8, Target, Reply>
);
recursive_reply_case!(
    rate_limiter,
    RateLimiterOutcome<u8>,
    RateLimiter<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(
    readiness,
    ReadinessReport<u8>,
    Readiness<MailAddr, u8, Reply>
);
recursive_reply_case!(
    registry,
    RegistryResult<u8, Target>,
    Registry<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(
    resolver,
    Resolution<u8, Target>,
    Resolver<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(
    sequencer,
    SequencerOutcome<u8>,
    Sequencer<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(task, TaskResult<u8>, Task<MailAddr, u8, Reply>);
recursive_reply_case!(
    work_queue,
    WorkQueueOutcome<u8>,
    WorkQueue<MailAddr, u8, Target, Reply>
);
recursive_reply_case!(
    workflow,
    WorkflowOutcome<u8>,
    Workflow<MailAddr, u8, Reply>
);
