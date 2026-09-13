use core::ops::ControlFlow;
use std::time::Instant;

use behavior_actors::{
    ChildStopped, CreationId, CreationSequence, EstablishedShutdownResolved, Exit, MailAddr,
    Protocol, ShutdownId, ShutdownRejection,
};

struct Worker;

impl Protocol for Worker {
    type Addr = MailAddr;
    type Msg = ();
}

struct DrainingWorker {
    child: CreationId,
    shutdown: ShutdownId,
    response: Option<EstablishedShutdownResolved<Worker>>,
    stopped: Option<ChildStopped<MailAddr>>,
}

impl DrainingWorker {
    const fn new(child: CreationId, shutdown: ShutdownId) -> Self {
        Self {
            child,
            shutdown,
            response: None,
            stopped: None,
        }
    }

    fn record_response(
        mut self,
        response: EstablishedShutdownResolved<Worker>,
    ) -> Result<ControlFlow<DrainedWorker, Self>, (Self, EstablishedShutdownResolved<Worker>)> {
        match self.response {
            None if response.id() == self.shutdown => {
                self.response = Some(response);
                Ok(self.retire_if_complete())
            }
            None | Some(_) => Err((self, response)),
        }
    }

    fn record_stop(
        mut self,
        stopped: ChildStopped<MailAddr>,
    ) -> Result<ControlFlow<DrainedWorker, Self>, (Self, ChildStopped<MailAddr>)> {
        match self.stopped {
            None if stopped.child == self.child => {
                self.stopped = Some(stopped);
                Ok(self.retire_if_complete())
            }
            None | Some(_) => Err((self, stopped)),
        }
    }

    fn retire_if_complete(self) -> ControlFlow<DrainedWorker, Self> {
        match (self.response, self.stopped) {
            (Some(response), Some(stopped)) => {
                ControlFlow::Break(DrainedWorker { response, stopped })
            }
            (response, stopped) => ControlFlow::Continue(Self {
                response,
                stopped,
                ..self
            }),
        }
    }
}

struct DrainedWorker {
    response: EstablishedShutdownResolved<Worker>,
    stopped: ChildStopped<MailAddr>,
}

fn creations() -> (CreationId, CreationId) {
    let mut sequence = CreationSequence::new();
    let expected = sequence.issue().expect("an initial creation ID exists");
    let foreign = sequence.issue().expect("a second creation ID exists");
    (expected, foreign)
}

fn stopped(child: CreationId) -> ChildStopped<MailAddr> {
    ChildStopped::new(child, Ok(Exit::Normal), Instant::now())
}

#[test]
fn shutdown_response_alone_keeps_worker_draining() {
    let (child, _) = creations();
    let shutdown = ShutdownId(7);
    let worker = DrainingWorker::new(child, shutdown);

    let Ok(outcome) = worker.record_response(EstablishedShutdownResolved::accepted(shutdown))
    else {
        panic!("the exact shutdown response must be retained");
    };

    assert!(matches!(outcome, ControlFlow::Continue(_)));
}

#[test]
fn worker_stop_alone_keeps_worker_draining() {
    let (child, _) = creations();
    let worker = DrainingWorker::new(child, ShutdownId(7));

    let Ok(outcome) = worker.record_stop(stopped(child)) else {
        panic!("the exact worker stop must be retained");
    };

    assert!(matches!(outcome, ControlFlow::Continue(_)));
}

#[test]
fn response_and_stop_retire_worker_in_either_order() {
    let (child, _) = creations();
    let shutdown = ShutdownId(7);
    let worker = DrainingWorker::new(child, shutdown);
    let Ok(ControlFlow::Continue(worker)) =
        worker.record_response(EstablishedShutdownResolved::accepted(shutdown))
    else {
        panic!("the first response must leave the stop outstanding");
    };
    let Ok(ControlFlow::Break(response_first)) = worker.record_stop(stopped(child)) else {
        panic!("the exact stop must complete retirement");
    };

    let worker = DrainingWorker::new(child, shutdown);
    let Ok(ControlFlow::Continue(worker)) = worker.record_stop(stopped(child)) else {
        panic!("the first stop must leave the response outstanding");
    };
    let Ok(ControlFlow::Break(stop_first)) =
        worker.record_response(EstablishedShutdownResolved::accepted(shutdown))
    else {
        panic!("the exact response must complete retirement");
    };

    assert_eq!(response_first.response.id(), shutdown);
    assert_eq!(response_first.stopped.child, child);
    assert_eq!(stop_first.response.id(), shutdown);
    assert_eq!(stop_first.stopped.child, child);
}

#[test]
fn shutdown_rejection_and_stop_are_both_retained() {
    let (child, _) = creations();
    let shutdown = ShutdownId(7);
    let worker = DrainingWorker::new(child, shutdown);
    let rejection =
        EstablishedShutdownResolved::rejected(shutdown, ShutdownRejection::AlreadyStopping);
    let Ok(ControlFlow::Continue(worker)) = worker.record_response(rejection) else {
        panic!("the exact rejection must leave the stop outstanding");
    };
    let Ok(ControlFlow::Break(retired)) = worker.record_stop(stopped(child)) else {
        panic!("the exact stop must complete retirement");
    };

    match retired.response {
        EstablishedShutdownResolved::Rejected { id, reason, .. } => {
            assert_eq!(id, shutdown);
            assert_eq!(reason, ShutdownRejection::AlreadyStopping);
        }
        EstablishedShutdownResolved::Accepted { .. } => {
            panic!("the rejection must not be rewritten as acceptance");
        }
    }
    assert_eq!(retired.stopped.child, child);
}

#[test]
fn foreign_and_duplicate_inputs_are_returned() {
    let (child, foreign_child) = creations();
    let shutdown = ShutdownId(7);
    let foreign_shutdown = ShutdownId(8);
    let worker = DrainingWorker::new(child, shutdown);
    let foreign_response = EstablishedShutdownResolved::accepted(foreign_shutdown);
    let Err((worker, returned_response)) = worker.record_response(foreign_response) else {
        panic!("a foreign shutdown response must be returned");
    };
    let Ok(ControlFlow::Continue(worker)) =
        worker.record_response(EstablishedShutdownResolved::accepted(shutdown))
    else {
        panic!("the exact shutdown response must be retained");
    };
    let duplicate_response =
        EstablishedShutdownResolved::rejected(shutdown, ShutdownRejection::AlreadyStopped);
    let Err((worker, returned_duplicate_response)) = worker.record_response(duplicate_response)
    else {
        panic!("a duplicate shutdown response must be returned");
    };
    let retained_response = worker
        .response
        .as_ref()
        .map(EstablishedShutdownResolved::id);

    let worker = DrainingWorker::new(child, shutdown);
    let Err((worker, returned_stop)) = worker.record_stop(stopped(foreign_child)) else {
        panic!("a foreign worker stop must be returned");
    };
    let Ok(ControlFlow::Continue(worker)) = worker.record_stop(stopped(child)) else {
        panic!("the exact stop must be retained");
    };
    let duplicate_stop = stopped(child);
    let duplicate_at = duplicate_stop.at;
    let Err((worker, returned_duplicate)) = worker.record_stop(duplicate_stop) else {
        panic!("a duplicate worker stop must be returned");
    };

    assert_eq!(returned_response.id(), foreign_shutdown);
    assert_eq!(returned_duplicate_response.id(), shutdown);
    assert_eq!(retained_response, Some(shutdown));
    assert_eq!(returned_stop.child, foreign_child);
    assert_eq!(returned_duplicate.child, child);
    assert_eq!(returned_duplicate.at, duplicate_at);
    assert_eq!(
        worker.stopped.expect("the exact stop remains owned").child,
        child
    );
}
