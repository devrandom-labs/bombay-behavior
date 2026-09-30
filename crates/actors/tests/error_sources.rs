use std::error::Error;

use behavior::{MailAddr, User};
use behavior_actors::{
    MachineError, TerminationMonitorError, TerminationObservation, TerminationPropagationError,
};

#[derive(Debug)]
struct SourceFault;

impl std::fmt::Display for SourceFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("original fault")
    }
}

impl Error for SourceFault {}

#[test]
fn machine_error_exposes_source_and_keeps_the_owned_event() {
    let error = MachineError {
        event: User::new(MailAddr(7), String::from("owned input")),
        cause: SourceFault,
    };
    assert_eq!(error.source().unwrap().to_string(), "original fault");
    assert_eq!(error.event.from, MailAddr(7));
    assert_eq!(error.event.message, "owned input");
}

#[test]
fn observation_error_exposes_only_the_inner_fault() {
    let inner = TerminationMonitorError::<SourceFault, String>::Inner(SourceFault);
    assert_eq!(inner.source().unwrap().to_string(), "original fault");

    let rejection = TerminationMonitorError::<SourceFault, String>::UnexpectedReport {
        observation: TerminationObservation::Observed,
        report: String::from("owned report"),
    };
    assert!(rejection.source().is_none());
    match rejection {
        TerminationMonitorError::UnexpectedReport { report, .. } => {
            assert_eq!(report, "owned report");
        }
        TerminationMonitorError::Inner(_) => panic!("unexpected inner fault"),
    }
}

#[test]
fn propagation_error_exposes_only_the_inner_fault() {
    let inner = TerminationPropagationError::<SourceFault, String>::Inner(SourceFault);
    assert_eq!(inner.source().unwrap().to_string(), "original fault");

    let rejection = TerminationPropagationError::<SourceFault, String>::UnexpectedReport {
        state: behavior_actors::TerminalPropagationState::Observing,
        report: String::from("owned report"),
    };
    assert!(rejection.source().is_none());
    match rejection {
        TerminationPropagationError::UnexpectedReport { report, .. } => {
            assert_eq!(report, "owned report");
        }
        TerminationPropagationError::Inner(_) => panic!("unexpected inner fault"),
    }
}
