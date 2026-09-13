# Minimal core used by atomic supervisors and pools

This is a deletion audit, not a preservation promise. “Already in core” is not
a reason to keep a type, and “the compiler needs a name” is not a semantic law.
The atomic actors should expose their own domain protocols while depending on
the smallest common actor algebra underneath them.

There are three different questions which must not be collapsed:

1. what is irreducible actor algebra;
2. which existing interpreter carriers are still needed internally; and
3. which names an atomic-actor user must see.

Most of the current names belong only to the second category. They must not be
re-exported as supervisor/pool concepts or counted as atomic-template types.

## A. Irreducible retained algebra

These existing concepts have independent laws. The five atomic actors use
them directly and must not recreate them in `actors`.

| Existing concept | Retention law |
|---|---|
| `Protocol` | A nominal, statically known address/message identity. Equal Rust field shapes do not make two protocols interchangeable. |
| `Behavior` | Pure initialization and one-communication fold. No executor, clock, hydration, or delivery occurs inside it. |
| `Actions` | The sole successful effect boundary: typed communications, staged fresh creation, and next behavior/termination. |
| `Step` | Exhaustive continuation/become/stop verdict. `Stopped` and `Never` are supporting proof types, not new template concepts. |
| `Recipient` and `EstablishedRecipient` | Distinct logical and exact delivery capabilities. One must never be silently widened into the other. |
| `CreateChild` | One staged fresh-child request with explicit provenance. It never means replacement or successful installation. |
| `TerminalOutcome` | One authoritative terminal fact, preserving normal exit versus crash provenance. |

`BehaviorBase`, `User`, `Births<Child>`, `NoBirths`, `CreationKind`, `Exit`, and
`Crash` are current representation/support types for those seven laws. They
remain only while the existing algebra and interpreter need them. The atomic
actor modules do not wrap, alias, or re-export them merely to make their own
signatures look uniform.

This is the minimal retained semantic kernel. It is seven concepts, not the
dozens of lifecycle and topology types currently visible in the crate.

## B. Existing interpreter carriers used internally

The following values may be used to lower an atomic actor's `Actions` into the
current interpreter. They are not components from which supervisors or pools
are composed, and they are not automatically part of ordinary DevX.

| Existing family | Internal use | Decision |
|---|---|---|
| `Address`, `EndpointAddress` | Runtime-owned identity and exact endpoint families | Keep in the interpreter boundary; do not expose numeric identity in builders. |
| `ChildRoute`, `ChildDelivery`, `ChildInput`, `ChildReport` | Creator-local exact child routing and source-attributed reports | Use only inside direct owner/child folds. A route is correlation, not actor identity. |
| `InterpreterRequests`, `InterpreterRequest` | Closed typed scheduling, observation, shutdown, and structural-report lane | Keep one generic request lane; do not add a supervisor request trait and a pool request trait. |
| `ObserveCreation`, `CreationResolved` | Committed or rejected staged creation without an exact endpoint capability | Reuse internally where only that fact is required. Do not add template-specific `WorkerCreationResolved`. |
| `ObserveEstablishedCreation`, `EstablishedCreation` | Same-action committed or rejected creation with the exact installed capability | Reuse internally for stable proxies: fixed and dynamic supervisors must retain the exact committed proxy rather than reconstructing it from an address. |
| `ObserveChild`, `ChildStopped` | Exact child terminal observation | Reuse internally; do not add template-specific `WorkerStopped`. |
| `ShutdownChild`, `ChildShutdownRejected`, `ShutdownRequested` | Exact child drain request/rejection and actor shutdown ingress | Reuse internally where their current facts are complete. Deadline retirement remains `ActorDrainPolicy`. |
| `ScheduleAfter`, `TimerElapsed`, `TimerId`, `TimerGeneration` | Interpreter-clock delay and exact timer correlation | Reuse internally. IDs are actor-owned and never builder inputs. |
| `ReportToParent` | Structural lowering of a child report | Keep private to actor/interpreter integration. A worker user writes `assignment.complete(result)`, never this type. |
| `DeliveryRoute`, `DeliveryRouteFor` | Preserve a statically selected logical or exact destination | Reuse for lifecycle/customer routes; do not force every actor to support a mixed route. |

None of these rows proves that every current helper trait around the value must
survive. `SendEffects`, `SendsFor`, `InterpretSends`, `InterpretDelivery`,
`InterpretChildDelivery`, `InterpretChildInput`, and `InterpretRequest` are
implementation machinery to audit as one interpreter equation. Keep only the
smallest closed set used by the actual named effect product. Do not mirror it
with template-local traits.

### Required interpreter-contract replacement

The current `InterpretSends` contract returns one `Interpreter::Error` and
documents that later lanes remain unconsumed after failure. That is
incompatible with the selected ownership law: the source fold has already
committed, so every later value still needs a surviving owner.

The replacement must interpret the named effect product into the equally
named, ordered per-item settlement product from the solution. Every item is
`Accepted`, `Rejected`, or `NotAttempted` after an exact dependency failure.
Independent later items are interpreted; dependent later items are returned
untouched. All current same-action occurrence-dependent operations must be
grouped with their creation prerequisite: `ObserveCreation`,
`ObserveEstablishedCreation`, `ObserveChild`, `ChildDelivery`, `ChildInput`,
and `ShutdownChild`. The staged sum distinguishes direct dependence on
creation from dependence on required-observation acceptance. All required
observations are attempted in named order; if several reject, their rejections
remain independently owned and later dependent effects cite the first
rejection's correlation. No lane index, path, lookup table, or general graph
is authorized.
`ReturnsToEmitter`/`NoReturnToEmitter` cannot be the general answer
because the emitter may stop in the same action. They are replaced or narrowed
to the live-primary-admission optimization only after typed host settlement is
proven. The Driver gives the transitive settlement chain priority before the
source's next ordinary user communication, so unresolved batches do not
accumulate across separate user turns. This supplies no decreasing measure:
the iterative queue alone proves neither bounded memory, termination,
fairness, nor eventual return to ordinary traffic.

## C. Conditional machinery, not selected core

| Current family | Keep only if |
|---|---|
| `EstablishedChild` | A focused use proves that a combined local-route/exact-actor helper product removes real ownership plumbing beyond the retained `EstablishedCreation` fact. |
| `ObserveEstablished`, `ShutdownEstablished` | The operation genuinely starts from a transferable exact capability rather than the owner's child namespace. |
| `ReplyRoute`, `ReplyDelivery`, `ReplyDeliveries` | One real API accepts both logical and exact reply routes. Exact-only callers must not pay for a mixed sum. |
| `EventIngress`, `InjectEvent`, `Here`, `Inside`, `EventLayer`, `SendLayer` | The current closed wrapper/interpreter composition truly needs them. Ordinary actor and template APIs never expose positions or paths. |
| `ChildRole`, `ChildOccurrence`, `ResolveChildOccurrence`, `ChildHead`, `ChildTail`, `Children`, `ChildChoice` | A closed authored birth product needs exact structural occurrence. Runtime supervisor roles and pool workers are values, not one type per role. |
| `ReportTerminalOutcome` | A fold must publish a terminal override independently of its own `Step::Stop`. |
| `Activate`, `Initialized`, `Active` | Pure one-time `Behavior` initialization. They do not prove asynchronous worker readiness. |
| `operations::Readiness` | Its existing fixed-dependency/version law is independently needed. It is not worker activation. |
| shutdown coordinators and wrappers | An application explicitly composes that outer lifecycle policy. They are not internal pieces of an atomic supervisor or pool. |

## C.1 Complete disposition of current public core families

This table closes the audit over the families currently re-exported by
`crates/behavior/src/lib.rs`. It is a family-level disposition rather than a
promise to preserve every helper name.

| Current public family | Atomic-actor decision |
|---|---|
| `Address`, `MailAddr`, `EndpointAddress` | Keep as protocol/interpreter identity support. Builders never accept raw addresses. |
| `Protocol`, `MessageProtocol` | Keep nominal `Protocol`. Keep `MessageProtocol` only as an explicitly structural helper; it must not collapse two domain protocols merely because address/message shapes match. |
| `Behavior`, `BehaviorActed`, `BehaviorAddr`, `BehaviorMessage`, `InitializationTurn`, `ActiveTurn` | Keep as the pure fold contract and truthful aliases/turn witnesses. Atomic actors implement this contract directly. |
| `BehaviorLayer`, `BehaviorBase`, `initialize`, `delegate_transition` | Keep for genuine transparent wrappers and initialization composition. Supervisors/pools do not use them as a behavioral decomposition. |
| `Actions`, `Step`, `Stopped`, `Never`, `Become`, `Acted`, `AppendSend` | Keep the explicit transition result. Convenience aliases remain only while they preserve all three action legs and the exact verdict. |
| `Effect` | Keep as optional no-birth/infallible shorthand. Atomic templates use full named `Actions`; `Effect` is not a second algebra. |
| `SendEffects`, `SendsFor`, `NoSends`, `SendLayer` and generated named products | Keep only the product/append laws required by concrete behaviors. Ordinary atomic-actor source sees semantic lane names, never `SendLayer` nesting. |
| `SendInterpreter`, `InterpretSends`, `InterpretDelivery`, `InterpretEstablishedDelivery`, `InterpretChildDelivery`, `InterpretChildInput`, `InterpretRequest` | Replace the success/one-error short-circuit equation with complete per-item settlement; retain only the static concrete dispatch pieces that realize it. |
| `InterpreterRequest`, `InterpreterRequests`, `ReportToParent`, `Own`, `SendInput` | Keep interpreter-facing and hidden from ordinary users. `ReportToParent` must compose behind `assignment.complete`, not appear in worker code. |
| `NoReturnToEmitter`, `ReturnsToEmitter` | Remove as a universal rejection law. A live emitter may be a primary consumer, but host settlement is the required surviving owner. |
| `Recipient`, `EstablishedRecipient`, `Delivery`, `EstablishedDelivery`, `EstablishedActor`, `InterpretEstablished` | Keep logical and exact capability families distinct. Atomic protocols preserve the concrete selected route. |
| `CreateChild`, `CreationKind`, `CreationRejection`, `AllocationRejection`, `BirthMode`, `NoBirths`, `Births` | Keep staged fresh creation and its typed rejection. Do not create template-specific duplicates. |
| `ChildRoute`, `ChildDelivery`, `ChildInput`, `ChildReport`, `EstablishedCreation` | Keep as exact creator/child carriers where their facts remain complete; never expose structural routing in ordinary actor DevX. |
| `ChildChoice`, role/occurrence/position traits, fold/mapping traits, `Children`, and birth protocol products | Keep the closed authored-child algebra for macros/interpreters and other templates. Atomic runtime roles are private values, not one public type-level role per member. |
| `User`, `UserEvent`, `Ingress`, `EventIngress`, `ChildInputIngress`, `InjectEvent`, `EventLayer`, `ComposedEvent`, `Here`, `Inside` | Keep typed event composition for real wrappers/interpreters. Paths and nesting markers remain generated/internal to ordinary atomic-actor source. |
| `LogicalHostRequirements`, `LogicalDeliveryProtocols`, birth logical-host projections | Keep static evidence for intentional logical routes. Exact/child/interpreter routes must not acquire fake logical-host obligations. |
| the `#[behavior]` owning macro | Keep syntax generation for the same concrete algebra. Do not add supervisor/pool or completion macros until ordinary composition is proven impossible. |

Behavior intentionally exposes no finite mailbox reducer. One-turn
initialization and event transitions are the algebraic boundary; Bombay owns
runtime sequencing. The unpublished testkit alone owns finite test sequencing
through `drive` and reports `DriveDisposition::{MailboxDrained,
BehaviorStopped(Stopped)}` without a semantic boolean.

## D. Delete or replace with the legacy templates

These names encode the architecture being replaced or incomplete policy
shapes. Widespread use does not make them core.

- old `Proxy`, `Supervisor`, `DynamicSupervisor`, `WorkerPool`, and
  `KeyedWorkerPool` implementations and their aliases;
- `ChildTopology`, `FixedFleetOwnership`, slot/fleet ownership state, and
  wrapper/path-specific `WithParent` forms;
- `SupervisionEvent`, `SupervisionSends`, `SupervisorSends`, `PoolSends`, and
  products whose fields exist only because the old templates are nested;
- `WorkerCreationResolved`, `WorkerStopped`, `ReplacementRequested`,
  `ReplacementResolution`, `ReportWorkerCreationResolved`,
  `ReportWorkerStopped`, and `ReportProxyUnavailable` in their legacy shapes;
- the existing `RestartConfiguration`/`RestartPolicy` split if it permits
  contradictory or meaningless combinations;
- the current `RestartDenial` if it cannot retain overlap, clock-order,
  activation, or forced-retirement reasons required by the catalogue; and
- assignment/completion types that expose customer routes or require workers
  to echo correlation fields.

Deletion happens only after the five replacements have feature and
interpreter witnesses and all callers are migrated. No compatibility layer
keeps the old concepts alive under new names.

## E. Core gaps exposed by the laws

Five requirements are not solved by the retained kernel:

1. **Authoritative installation and initialization settlement.** The pure init
   fold must precede host commitment, but initialization `Actions` must follow
   commitment of an installed-but-not-ready incarnation. Their partial success
   cannot be rolled back. The interpreter needs distinct pre-commit init/host
   rejection and post-commit initialization-effect settlement.
2. **Incarnation activation.** The selected semantic equation splits committed
   installation into an actor-retained exact incarnation and a one-shot
   activation permit. `BeginActivation` consumes the permit and concrete plan;
   exact resolved/cancelled/late facts settle readiness. Hydration/I/O remains
   outside `Behavior`. The retained algebra and interpreter do not yet realize
   this equation.
3. **Creation-scoped dependency lowering.** One creation and the complete
   current set of same-action occurrence operations—`ObserveCreation`,
   `ObserveEstablishedCreation`, `ObserveChild`, `ChildDelivery`, `ChildInput`,
   and `ShutdownChild`—must lower as one named staged bundle, with direct and
   after-required-observation alternatives. Independent effects stay outside.
   Existing `Actions` does not yet prove that grouping without structural
   paths or runtime lookup.
4. **Rejected delivery and terminal projection.** The selected semantic equation projects
   ordered per-item settlement from each named effect-product lane. Every item
   is accepted, rejected, or not attempted after an exact dependency failure;
   several failures remain distinct. A not-attempted item references the one
   authoritative prerequisite settlement rather than cloning its payload. A
   statically known host owns settlement after source commit or termination
   and transfers unresolved values outward to a root runner. Current
   interpretation does not realize that equation, and a committed fold cannot
   be rolled back. Its root boundary needs total heterogeneous static lifts,
   duplicate-role provenance, compositional wrapper transfer, and
   wrapper-depth-independent compiler diagnostics.
5. **Residual root lifetime.** Actor-side forced retirement may transfer an
   uncancellable activation or future late incarnation to the root. The run
   future must remain a live typed owner until those facts resolve and the
   exact incarnation drains; a finished error value is not an owner. The
   current runner has no such residual state.

These are foundational interpreter equations, not permission to add a broad
“lifecycle engine,” callback, ambient query, or template-local interpreter
trait. Their semantic ownership is selected; the smallest concrete association
with the retained core still requires a focused end-to-end regression and
compile/interpreter witness.

## F. Visibility rule

An ordinary user of a supervisor or pool should normally see only:

- the actor's builder and public command/outcome protocol;
- their domain role/key/job/result/worker types;
- selected recovery, actor-specific capacity bounds, shutdown, and diagnostic
  policies; and
- logical or exact recipients they deliberately provide.

They must not see child routes, occurrence positions, interpreter requests,
parent-report plumbing, effect-lane products, nonce/timer issuers, activation
joins, completion correlations, or typestate proof markers. If Rust makes an
internal proof type appear in diagnostics, that is an implementation problem
to measure—not a semantic reason to promote the type into the public model.
