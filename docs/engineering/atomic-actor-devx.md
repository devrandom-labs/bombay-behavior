# Atomic actor component authoring

Use the owning component constructors and ordinary concrete Behavior workers.
This page identifies the component contract; the
[developer experience PRD](developer-experience-prd.md) owns proposed Bombay
application simplification. Its target inferred assembly is not a claim that
current unannotated applications compile.

## Current source entry points

The owning modules and generated rustdoc define the selected signatures.
Supervisors own stable proxies; FIFO and keyed pools directly own workers.
Their distinct ownership laws remain separate.

| Operation | Owning entry |
| --- | --- |
| Stable service across replacement | `proxy::StableProxy::new`; `immediate` selects a no-work startup plan |
| Fixed roster supervision | `supervisor::roster::new(factory, roles, roster::Config { … }).build()` |
| Dynamic membership supervision | `supervisor::directory::new(directory::Config { … })` |
| FIFO worker distribution | `pool::queue::new(factory, roles, queue::Config { … })` |
| Key-affine worker distribution | `pool::router::new(factory, roles, selector, router::Config { … })` |
| Worker definition and readiness | `worker::WorkerSubmission::immediate` or `with_startup` |
| Assignment worker authoring | `pool::pool_worker`, `Assignment::payload`, `Assignment::complete` |

Config names simultaneous choices and consumes them into flat actor storage.
Automatic fixed recovery takes `roster::RecoveryPolicy { strategy, limit, delay }`;
automatic pool recovery takes `pool::RecoveryPolicy { limit, delay, failure }`.
Keep the source separate: `Recovery::permanent(source, policy)` or
`PoolRecovery::permanent(source, policy)`; transient has the same shape.
Temporary recovery owns no unused source. Preparation refusal returns the
original factory, prepared prefix, refused role/reason, untouched suffix and
configuration; keyed refusal also returns its original selector.

Ordinary source selects domain values and policy: roles/keys, jobs/results,
roster/capacity, startup, recovery, restart budget/release, interruption,
drain and diagnostic disposition. No-op sources, unused routes and guessed
nonces are not policy choices. Temporary recovery has no unused automatic
source; immediate startup has no dummy asynchronous plan.

Ordinary factories let Rust infer the composed worker type. Pass startup work
as `WorkerSubmission::with_startup(worker, move || async move { ... })`; its
actual `Result` determines readiness and rejection. This avoids naming the
wrapped worker, a startup struct or its associated types. Construction stores
the closure; the existing initialization-authorized startup operation calls it
once. Do not invoke it while constructing the submission. Named `StartupPlan`
implementations remain available when a separate domain plan is useful.
A closure covered by this implementation cannot also implement the trait itself.

## Ownership obligations

A worker completes the assignment it receives; it does not choose the pool's
customer route or mint completion authority. A runtime request receipt is not
the later operation outcome. Initial creation, replacement creation,
initialization settlement and readiness retain their distinct provenance.

Worker-source rejection before any submission is different from rejection of
a later role while prepared workers remain owned. Shutdown during preparation
must retain that prefix and all late values. A constructor error returns its
complete available inputs. Do not infer readiness from successful construction.

Logical, exact and mixed routes preserve their actual capability. Diagnostic
delivery and terminal retention are explicit different dispositions; no
mandatory unused recipient may be introduced to unify their spelling.

## Application use and evidence

Bombay supplies the runtime and ordinary `ActorExt` composition. Its live
[supervision](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/supervision/src/main.rs)
and [pool](https://github.com/devrandom-labs/bombay/blob/c3afb3011090ac4c9112fd39e4a76980db856df0/examples/worker-pool/src/main.rs)
examples show the current complete caller syntax, including the assembly
ceremony targeted by the PRD. Do not copy hypothetical shorter syntax into
current usage docs until the complete program compiles and executes.

The [proxy](../actor-laws/proxy.md),
[fixed](../actor-laws/fixed-supervisor.md),
[dynamic](../actor-laws/dynamic-supervisor.md),
[FIFO](../actor-laws/fifo-pool.md) and [keyed](../actor-laws/keyed-pool.md)
laws retain family-specific transitions.
[Verification requirements](atomic-actor-verification.md) distinguish pure,
static and live evidence. Historical construction experiments are available
in Git; they do not define additional supported APIs.
