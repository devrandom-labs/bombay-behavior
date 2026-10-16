use core::future::{Future, pending};
use core::pin::pin;
use core::task::{Context, Poll, Waker};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use behavior_actors::worker::StartupPlan;

struct Invoice {
    number: u64,
    charges: Box<[u8; 2]>,
}

struct Reservation(Arc<AtomicUsize>);

impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn startup_closure_defers_work_and_returns_the_original_readiness() {
    let constructions = Arc::new(AtomicUsize::new(0));
    let constructed = Arc::clone(&constructions);
    let polls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&polls);
    let invoice = Invoice {
        number: 11,
        charges: Box::new([13, 17]),
    };
    let allocation = core::ptr::from_ref(invoice.charges.as_ref());
    let plan = move || {
        constructed.fetch_add(1, Ordering::SeqCst);
        async move {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok::<_, Invoice>(invoice)
        }
    };
    assert_eq!(constructions.load(Ordering::SeqCst), 0);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    let work = StartupPlan::start(plan);
    assert_eq!(constructions.load(Ordering::SeqCst), 1);
    assert_eq!(polls.load(Ordering::SeqCst), 0);
    let readiness = match work.await {
        Ok(readiness) => readiness,
        Err(_) => panic!("the actual startup accepts its invoice"),
    };
    assert_eq!(polls.load(Ordering::SeqCst), 1);
    assert_eq!(constructions.load(Ordering::SeqCst), 1);
    assert_eq!(readiness.number, 11);
    assert_eq!(*readiness.charges, [13, 17]);
    assert_eq!(core::ptr::from_ref(readiness.charges.as_ref()), allocation);
}

#[tokio::test]
async fn startup_closure_returns_the_complete_original_rejection() {
    let invoice = Invoice {
        number: 19,
        charges: Box::new([23, 29]),
    };
    let allocation = core::ptr::from_ref(invoice.charges.as_ref());
    let plan = move || async move { Err::<Invoice, _>(invoice) };
    let work = StartupPlan::start(plan);
    let rejection = match work.await {
        Ok(_) => panic!("the actual startup rejects its invoice"),
        Err(rejection) => rejection,
    };
    assert_eq!(rejection.number, 19);
    assert_eq!(*rejection.charges, [23, 29]);
    assert_eq!(core::ptr::from_ref(rejection.charges.as_ref()), allocation);
}

#[test]
fn unpolled_startup_closure_releases_its_original_reservation_once() {
    let released = Arc::new(AtomicUsize::new(0));
    let reservation = Reservation(Arc::clone(&released));
    let plan = move || async move {
        drop(reservation);
        Ok::<(), Invoice>(())
    };
    let work = StartupPlan::start(plan);
    assert_eq!(released.load(Ordering::SeqCst), 0);
    drop(work);
    assert_eq!(released.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_startup_closure_releases_its_original_reservation_once() {
    let released = Arc::new(AtomicUsize::new(0));
    let reservation = Reservation(Arc::clone(&released));
    let plan = move || async move {
        pending::<()>().await;
        drop(reservation);
        Ok::<(), Invoice>(())
    };
    {
        let work = StartupPlan::start(plan);
        let mut work = pin!(work);
        let mut context = Context::from_waker(Waker::noop());
        let polled = work.as_mut().poll(&mut context);
        assert!(matches!(polled, Poll::Pending));
        assert_eq!(released.load(Ordering::SeqCst), 0);
    }
    assert_eq!(released.load(Ordering::SeqCst), 1);
}
