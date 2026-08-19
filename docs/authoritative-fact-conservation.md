# Authoritative fact conservation

## Law and classification

The actor-model law is one isolated transition per accepted communication,
with communication, fresh creation, and the next behavior represented as the
actor's explicit effects. Agha's actor calculus does not require child failure
to terminate a parent, define lifecycle observation, or prescribe a root
application result.

Bombay derives the following composition law for its typed observation and
capability-result protocols:

> A reusable composition may consume an authoritative fact only through a
> total, named typed policy that explicitly preserves it, transforms it, or
> discharges it. A transparent layer must preserve the fact. A policy layer
> may transform or discharge only the semantic dimensions named by its
> contract.

This is **Bombay-derived**, not an Agha guarantee. The concrete choice to
propagate, restart, retire, retry, reject, or ignore is **Bombay policy** owned
by the selected template or application composition.

An authoritative fact is a typed result minted by the interpreter or an actor
that owns the underlying transition: exact-incarnation termination, committed
creation resolution, worker lifecycle reporting, timer expiry, child-shutdown
rejection, or another capability result. An ordinary application command is
not an authoritative fact merely because its payload is called a result.

## Required proof for a fact consumer

For every event variant carrying an authoritative fact, review all of these
dimensions:

1. **Identity:** address, creator-local nonce, stable proxy, worker
   incarnation, timer identifier, and timer generation remain distinct.
2. **Provenance:** birth versus replacement, request versus acceptance versus
   completion, and observed exit versus execution crash are never inferred.
3. **Payload:** exact rejection, failure, result, and owned rejected values are
   retained wherever the consumer's declared result represents them.
4. **Multiplicity:** independent structural consumers each receive their
   selected fact; a later observation cannot silently cancel an earlier one.
5. **Order:** a fact is folded once in source order and any same-action
   publication is interpreted before the terminal verdict.
6. **Disposition:** stale, unrelated, duplicate, discharged, transformed, and
   propagated cases are exhaustive and independently tested.
7. **Composition:** every supported wrapper order preserves the same fact and
   does not require positional access to an inner effect lane.

Stopping alone is not propagation. `Stopped` deliberately contains no
lifecycle provenance. Logging is not propagation. A diagnostic next to an
ordinary stop is not a terminal result unless an explicit typed publication
effect joins the two in one `Actions` value.

## Terminal publication construction

`TerminalOutcome<A> = Result<Exit<A>, Crash>` is the complete terminal sum for
one exact incarnation. `ReportTerminalOutcome<A>` asks an interpreter to
publish that value unchanged for the emitter before applying the same action's
stop verdict.

`PropagateTermination<B, Target>` is the pure composition:

```text
target observation
    -> exact ChildStopped or PeerStopped
    -> TerminalDisposition::Discharge
       | TerminalDisposition::Propagate
           -> ReportTerminalOutcome(original outcome) + Stop
```

`ChildTermination<A>` and `PeerTermination<A>` are concrete, statically
dispatched targets. No runtime target enum, structural child handle, global
lookup, or erased capability is introduced. `propagate_all` and
`propagate_abnormal` are deliberate policies; neither is an actor-model law.

## Production consumer inventory

| Fact | Consumers | Required disposition |
|---|---|---|
| `TimerElapsed { id, generation }` | deadline, one-shot, periodic, receive timeout, lease, backoff, presence, circuit breaker | Matching generations transform into the documented reaction; stale and unrelated generations are explicitly inert. No wall-clock fact is reconstructed in behavior code. |
| `PeerStopped { peer, outcome }` | `Watch`, `TerminationMonitor`, `PropagateTermination<PeerTermination<_>>` | `Watch` applies a named verdict-only link policy; `TerminationMonitor` supplies the complete fact to an action-producing reaction; propagation preserves the complete outcome. Multiple structural observations must remain independent in the interpreter. |
| `ChildStopped { nonce, outcome, at }` | stable proxy, fixed/dynamic supervision, pools, shutdown coordinators, `PropagateTermination<ChildTermination<_>>` | Proxy reporting preserves outcome and time. Supervision transforms it through restart policy. Shutdown coordination deliberately discharges outcome/time and owns only phase settlement. Terminal propagation preserves outcome. Independent coordinator and propagation observations must both complete. |
| `CreationResolved { nonce, kind, result }` | stable proxy, fixed/dynamic supervision, pools | Proxy validates attempt and provenance. Dynamic start exposes committed success or exact rejection. Fixed ownership must not accept mismatched provenance or silently retire a rejected configured slot without a typed topology result. |
| `WorkerStopped { proxy, worker, outcome, at }` | fixed/backoff supervision, dynamic supervision, pools | Fixed supervision preserves the complete fact through eligibility and budget decisions. Pools additionally transform it into assignment interruption outcomes. Dynamic supervision deliberately discharges it because its public phase describes the stable proxy, while replacement completion remains a separate fact. |
| `WorkerCreationResolved { proxy, worker, kind, result }` | fixed/backoff supervision, dynamic supervision, pools | Dynamic replace and pools expose exact rejection. Fixed/backoff supervision must explicitly handle rejected replacement realization rather than leave a configured slot silently vacant. |
| `ChildShutdownRejected { nonce, reason }` | proxy, fixed/dynamic supervision, pools, shutdown coordinators | Matching requests return exact typed failures or replies. Dynamic explicit-stop replies retain the capability rejection rather than relabeling it as availability. Stale/unrelated rejections are inert. |
| `ReportSupervisionFailure` | Bombay interpreter | Publishes the supervisor's typed terminal classification before a same-action stop; it is not a general parent-escalation request. |
| `ReportTerminalOutcome` | Bombay interpreter | Publishes the complete supplied terminal sum unchanged before a same-action stop. |

## Audit status and proof ledger

The first audit repaired peer/child observation multiplicity, exact
fixed/backoff creation-rejection reporting, stable-proxy and worker-
incarnation creation-provenance validation, and active shutdown of live
OneForAll/RestForOne replacement candidates. These remain subject to the full
repository gates.

The conservation obligation is closed by evidence at each owning layer:

| Proof obligation | Owning layer and evidence |
|---|---|
| Independent model | `behavior-testkit/tests/terminal_fact_model.rs` uses an independent waiting/quiet/published oracle over arbitrary mixed selected, unrelated, and domain-event sequences. It compares state, exact publication, stop coupling, and transparent inner sends after every operation. |
| Exhaustive outcomes | `lifecycle::termination_propagation::tests::every_terminal_variant_is_propagated_without_reclassification` enumerates every `Exit` and `Crash` variant, including every nested supervision failure and creation-rejection reason. |
| Property sequences | `terminal_fact_model::arbitrary_fact_sequences_match_an_independent_single_consumption_model` covers both public policies and arbitrary terminal payload fields; the focused in-module property independently checks exact once-only payload retention. |
| Compile-time target identity | The `PropagateTermination` compile-fail rustdoc rejects a child selector whose nonce does not belong to the selected address family. |
| Composition and initialization | The focused initialization/delegation tests preserve inner sends, births, verdicts, and the exact structural observation lane. The workspace algebra and cross-lane suites cover the surrounding wrapper products. |
| Interpreter multiplicity | Bombay's `observation::tests::structural_observations_of_one_peer_are_multiplicity_preserving` proves two structural paths receive the same exact peer terminal fact. `actor_system::shutdown_and_terminal_propagation_consume_the_same_child_fact_independently` proves the shutdown and propagation consumers independently receive one child fact. |
| Report interpretation | Bombay's report tests publish every `TerminalOutcome` unchanged, and the application capability interprets the report lane before the Driver applies the same action's stop verdict. |
| End-to-end reference application | Bombay's `supervision` example runs the same heterogeneous topology through graceful coordinated shutdown and restart-budget exhaustion. It accepts only the exact expected terminal classification and uses the public `App` boundary. |

The cross-repository runtime evidence belongs to Bombay rather than this
template crate. It is cited here as an end-to-end contract proof; no runtime,
observation registry, address mechanism, mailbox, or executor implementation
is copied into Actors.

The workspace-wide public outcome review covered these complete families:

| Family | Public result sums reviewed | Conservation evidence |
|---|---|---|
| routing | buffer, priority/work queue, sequencer, order gate, deduplicator, correlator, acknowledgements, rate limiter, circuit breaker, router | Rejection variants retain owned inputs where ownership can be refused; empty, stale, duplicate, cancelled, exhausted, and accepted states remain disjoint. Unit, boundary, catalogue-model, property, and fuzz surfaces exercise them. |
| discovery | registry, resolver, topic/pub-sub, presence | Lookup absence, duplicate/conflicting definition, stale evidence, expiry generation, and rejected publication retain their distinct keys, protocols, versions, or payloads. |
| persistence and operations | cache, configuration, health, readiness, features | Displaced values, stale/conflicting candidates, tombstones, versions, and missing dependency evidence are returned or retained explicitly. |
| workflow and lifecycle | workflow, latch, barrier, task, shutdown coordination | Blocked/failed/completed, generation exhaustion, task cancellation/completion, shutdown rejection, stale child facts, and phase completion are exhaustive named states. |
| supervision and pools | proxy, fixed/backoff/dynamic supervision, worker pools | Termination, creation, replacement provenance, shutdown rejection, interruption, restart denial, and terminal publication now preserve their complete authoritative facts through named sums. |

No additional payload-collapse defect was found in the non-supervision business
outcome sums. Their existing focused tests are part of the 407-test workspace
gate; this audit does not claim that an interpreter outside this workspace
implements every declared request.
