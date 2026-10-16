//! Exact contradictions and runtime rejections from one dynamic supervisor.

use behavior::{Behavior, BehaviorAddr, ChildReport, EndpointAddress};

use crate::proxy::ProxyDiagnostic;
use crate::templates::proxy_creation::StableProxyCreationSettlement;
use crate::{
    ChildStopped, ProxyInputResult, ProxyOutcome, StableProxy, StartupPlan, WorkerSubmission,
};

/// Diagnostic value requiring one routed attempt or terminal custody.
pub enum DynamicDiagnostic<Key, Worker, Plan>
where
    Worker: Behavior,
    Plan: StartupPlan,
    BehaviorAddr<Worker>: EndpointAddress,
    StableProxy<Worker, Plan>: Behavior<Protocol = Worker::Protocol>,
{
    RejectedInput {
        key: Key,
        submission: WorkerSubmission<Worker, Plan>,
    },
    RejectedProxyOutcome {
        report: ChildReport<ProxyOutcome<Worker, Plan>>,
    },
    ProxyDiagnosticReported {
        report: ChildReport<ProxyDiagnostic<Worker, Plan>>,
    },
    ProxyInputRejected {
        key: Key,
        generation: u64,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    },
    ProxyCreationRejected {
        key: Key,
        generation: u64,
        creation: StableProxyCreationSettlement<Worker, Plan>,
    },
    ProxyShutdownRejected {
        key: Key,
        generation: u64,
        input: ProxyInputResult<behavior::Here, Worker, Plan>,
    },
    RejectedProxyStop {
        stopped: ChildStopped<BehaviorAddr<Worker>>,
    },
}
