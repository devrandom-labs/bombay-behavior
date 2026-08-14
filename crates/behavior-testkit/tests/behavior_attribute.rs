use std::time::Duration;

use behavior::{
    Actions, Behavior, Births, CreationKind, Delivery, InterruptionPolicy, JobId, MailAddr, Never,
    NoBirths, PoolAssignment, PoolMessage, Recipient, RestartPolicy, Step, WorkerCreationResolved,
    WorkerPool,
};

struct Counter {
    total: u64,
}

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
    births = NoBirths,
    error = Never,
)]
impl Counter {
    fn init(
        &mut self,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        NoBirths,
        Never,
    > {
        self.total = 1;
        Ok(Actions::cont())
    }

    fn receive(
        &mut self,
        from: MailAddr,
        message: u64,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<u64>>>,
        NoBirths,
        Never,
    > {
        self.total += message;
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), self.total)],
            Vec::new(),
            Step::Continue,
        ))
    }
}

struct Worker;

#[behavior::behavior(
    addr = MailAddr,
    message = PoolAssignment<u8>,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Worker {
    fn init(&mut self) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }

    fn receive(
        &mut self,
        _from: MailAddr,
        _assignment: PoolAssignment<u8>,
    ) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        Ok(Actions::cont())
    }
}

struct Generic<T> {
    last: Option<T>,
}

#[behavior::behavior(
    addr = MailAddr,
    message = T,
    sends = Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
    births = NoBirths,
    error = Never,
)]
impl<T> Generic<T>
where
    T: Clone,
{
    fn init(
        &mut self,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
        NoBirths,
        Never,
    > {
        Ok(Actions::cont())
    }

    fn receive(
        &mut self,
        from: MailAddr,
        message: T,
    ) -> behavior::Acted<
        MailAddr,
        Never,
        Vec<Delivery<behavior_testkit::TestRecipient<T>>>,
        NoBirths,
        Never,
    > {
        self.last = Some(message.clone());
        Ok(Actions::new(
            vec![Delivery::new(Recipient::global(from), message)],
            Vec::new(),
            Step::Continue,
        ))
    }
}

fn nonce(index: usize) -> u64 {
    u64::try_from(index).unwrap()
}

fn worker(_index: usize) -> Worker {
    Worker
}

#[test]
fn attribute_preserves_normal_methods_and_exact_actions() {
    let mut counter = Counter { total: 0 };
    counter.init().unwrap();
    let actions = counter.receive(MailAddr(7), 4).unwrap();
    assert_eq!(counter.total, 5);
    assert_eq!(actions.sends[0].message, 5);
    assert_eq!(actions.sends[0].to, Recipient::global(MailAddr(7)));
}

#[test]
fn generated_behavior_is_nominal_in_pool_and_supervision_positions() {
    let mut pool: WorkerPool<
        MailAddr,
        behavior_testkit::TestRecipient<behavior::PoolResponse<u8, (), MailAddr>>,
        u8,
        (),
        Worker,
    > = WorkerPool::new(
        nonce,
        1,
        worker,
        0,
        InterruptionPolicy::Fail,
        RestartPolicy::Permanent,
        1,
        Duration::from_secs(1),
    )
    .unwrap();
    let initial = pool.init().unwrap();
    assert_eq!(initial.creates.len(), 1);
    pool.on(WorkerCreationResolved::new(
        0,
        0,
        CreationKind::Birth,
        Ok(()),
    ))
    .unwrap();
    let actions = pool
        .receive(
            MailAddr(1),
            PoolMessage::Submit {
                job: JobId(0),
                payload: 9,
                reply_to: Recipient::global(MailAddr(2)),
            },
        )
        .unwrap();
    assert_eq!(actions.sends.behavior.assignments.len(), 1);
}

#[test]
fn attribute_preserves_impl_generics_and_where_clause() {
    let mut generic = Generic::<u16> { last: None };
    generic.init().unwrap();
    let actions = generic.receive(MailAddr(3), 11).unwrap();
    assert_eq!(generic.last, Some(11));
    assert_eq!(actions.sends[0].message, 11);
}

#[allow(dead_code)]
type NameableBirth = Births<Counter>;
