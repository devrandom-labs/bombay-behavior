# Public surface inventory

This inventory supports [A13](repository-quality-audit.md). `#[doc(hidden)]`
changes Rustdoc display, not Rust visibility. The counts below are annotation
sites in source, so a grouped re-export and its original declaration each
count once. The 2026-09-28 branch has 22 sites in `crates/behavior/src` and 86
in `crates/actors/src`; the earlier audit counted 25 and 89 before six
documentation markers were removed. A private constructor is included in the
count and is not part of the public surface.

## Hidden core declarations

| Owner | Annotated declarations | Contract owner | Visibility decision to review |
|---|---|---|---|
| `actor/addressing.rs` | `Recipient::new` | Private representation | Already private; only `Recipient::global` and the lawful `From` conversion are public. |
| `actor/creation.rs`, identity and correlation | `CreationId::get`, `CreateChild::into_parts`, `CreationCorrelation` | Runtime port | Interpreter code needs exact identity and owned request parts; document custody before displaying these ports. |
| `actor/creation.rs`, occurrence proof | `ChildOccurrence::Resolution`, `DeclaredChildOccurrence`, `StructuralChildOccurrence`, `ChildCreationProduct`, `ChildOccurrenceResolution`, `ResolveChildOccurrenceDescriptor`, `BirthNodeAt`, `ChildOccurrenceProductAt` | Generated code obligation | The macro and structural child products implement these proofs. A visibility change needs compile-pass and forged-occurrence compile-fail witnesses. |
| `actor/creation.rs`, protocol projection | `BirthProtocolProduct`, `BirthModeProtocols`, `BirthNodeProtocols`, `BirthNodeLogicalHosts` | Generated code obligation | These traits project closed birth and logical-host products; consumers can name the resulting associated types without constructing the proof nodes. |
| `actor/creation.rs`, creation staging | `ChildProduct::stage` | Runtime port | The interpreter consumes ordered staged child requests; it must retain every owned child on rejection. |
| `effects/actions.rs` | `CreationSettlement`, `CreationSettlements`, `CreationsSettled`, `InterpretCreations` | Runtime port | These settle creations in order and preserve exact rejected requests. |
| `effects/sending.rs` | `settle_in_order` | Runtime port | Shared ordered interpretation used by concrete send products. |

## Hidden actor declarations

| Owner | Annotated declarations | Contract owner | Visibility decision to review |
|---|---|---|---|
| `atomic/diagnostic.rs` | `DiagnosticRoute`, `DiagnosticAction`, `DiagnosticAction::{deliver,terminal}`, `DiagnosticAccepted`, `DiagnosticAccepted::{delivered,terminal}` | Runtime port | Diagnostic admission and terminal return are typed operation outcomes. `DiagnosticRoute` is sealed, so third parties cannot add a new route form. |
| `atomic/{dynamic_supervisor,fifo_pool,fixed_supervisor,keyed_pool}/{event,requests}.rs` | `DynamicSupervisorEvent`, `DynamicSupervisorRequests`, `FifoEvent`, `FifoRequests`, `FixedSupervisorEvent`, `FixedSupervisorRequests`, `KeyedEvent`, `KeyedRequests` | Runtime port | Public associated event and send products let an interpreter carry complete typed lanes; the aggregate owns their transition semantics. |
| `atomic/fixed_supervisor/lifecycle.rs` | `FixedLifecycleRoute` | Generated code obligation | This sealed route proof is implemented for the finite fixed-supervision lifecycle forms. |
| `atomic/pool/assignment.rs` and `atomic/pool/customer.rs` | `AssignmentReceipt`, `AssignWorker`, `AssignWorker::{target,receipt,into_parts}`, `CustomerDelivery` | Runtime port | The interpreter acknowledges the exact worker assignment and preserves customer custody. |
| `atomic/pool/mod.rs` | `CompletesAssignments` | Generated code obligation | The sealed completion capability belongs to generated pool workers and declared completion products. |
| `atomic/stable_proxy/{effects,operation,protocol}.rs` and `atomic/mod.rs` | `ProxyEffects`, `ProxyOperationId`, `ProxyOperation`, `ProxyInputResult`, `ProxyOperation::{creation,into_parts}`, `ProxyInputReceipt`, `ProxyInputReceipt::new`, `ProxyDrain`, `ProxyEvent`, plus re-export-only `ProxyControl` and `WorkerStartResult` | Runtime port | The host and typed proxy effects must keep operation identity, rejection custody, and every ordered lane. |
| `atomic/worker/activation.rs` | `ActivationStartRejection`, `BeginActivation`, `BeginActivation::{new,target,worker,initialization,started,start_rejected,activate}`, `WorkerActivation`, `WorkerActivation::{worker,into_ready,into_rejection}` | Runtime port | The host settles activation only after the exact worker and initialization attempt are known. |
| `atomic/worker/initialization.rs` | `InitializationAttempt`, `ActivationPermit`, `ActivationPermit::{worker,initialization,target}`, `InitializeWorker`, `InitializeWorker::{target,worker,initialization,resolve}`, `WorkerInitializationOutcome`, `WorkerInitializationReport`, `WorkerInitializationFailure` | Runtime port | Initialization settlement carries exact worker custody and can authorize or reject later activation. |
| `atomic/worker/preparation.rs` | `PrepareWorkers`, `PrepareWorkers::{source_and_role,accept,reject}`, `PendingWorkerPreparation`, `PendingWorkerPreparation::{source_and_role,accept,reject}`, `WorkerPreparation` | Runtime port | A lawful application `WorkerSource` implementation supplies preparation and receives its complete rejection. The pool owns sequencing. |
| `atomic/worker/mod.rs` | `HostedInitialization`, `WorkerRecovery`, `WorkerRecovery::into_retirement`, `WorkerAttempt`, `WorkerAttempt::creation` | Runtime port | The host retains the attempt and complete initialization effects through retirement. |
| `lifecycle/shutdown_coordinator.rs` | `HeterogeneousShutdownItem`, `ChoiceSettlements`, `HeterogeneousShutdownChoiceSettlement` | Generated code obligation | The closed heterogeneous choice product supplies the typed settlement shape. |
| `protocol/mod.rs` | `ObserveCreation` | Runtime port | Observation must refer to the exact staged child creation and return its request on rejection. |
| `atomic/mod.rs`, `atomic/pool/mod.rs`, and `lifecycle/shutdown_coordinator.rs` | Grouped re-exports of the declarations above | Same as original declaration | The re-export annotations add no second capability; each name remains publicly reachable through its parent module. `FixedBuilder`, `FifoError`, and `KeyedError` are visible re-exports because applications name the inferred builder and aggregate errors. |

This table classifies ownership but does not by itself justify retaining each
public spelling. In particular, an associated type that mentions one of these
values is not proof that applications must name it. Closing A13 still requires
caller-facing compile witnesses, a trait-implementor inventory, and a
repeatable compile-cost comparison before changing visibility or bounds.
