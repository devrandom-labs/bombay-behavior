# Runtime-backed actor capability record

This record identifies actor-system roles that cannot be completed by a
deterministic `bombay-behavior-actors` fold alone. It complements the
[behavior-template catalogue](actor-catalogue.md): the catalogue names the
reusable actor vocabulary, while this document records the non-deterministic
capabilities required to make that vocabulary run as a complete Bombay
framework.

A runtime-backed actor is one construction with two separate halves:

```text
application domain slot
        |
        v
pure actor template: state + event -> Actions
        |
        v
typed capability protocol
        |
        v
Bombay environment interpreter:
mailbox / clock / address / task / store / network / OS
```

The template owns deterministic policy, protocol phases, typed requests,
typed observations, and rejection handling. The runtime owner performs effects
and returns facts. A catalogue entry is not a usable framework actor until both
halves exist through the universal Driver.

## Classification rule

| Class | Meaning | Implementation consequence |
|---|---|---|
| Pure template | Its complete law is a deterministic fold over supplied events | Implement and test in `bombay-behavior-actors` |
| Capability-backed actor | It has a deterministic policy half but needs runtime facts or effects | Implement the fold in Actors and the typed interpreter in the owning runtime or integration crate |
| Runtime facility | Its defining invariant is execution, allocation, storage, transport, or OS interaction | Implement outside Actors; expose only a concrete typed capability to templates |
| Application actor | Its protocol and decisions are specific to the user's domain | The application supplies it and may compose the templates above |

"Can be represented by messages" does not make an effect a pure template.
Sleeping, spawning, registration, durable commit, network publication, and
resource access remain effects even when a behavior can request them.

## Runtime-backed actor families

Status is **realized** when the current Bombay path interprets the capability
end to end, **partial** when a primitive or adapter exists without the complete
actor path, **gap** when Bombay lacks the subsystem, and **external boundary**
when an application-selected adapter must supply the mechanism.

| Actor role or family | Pure Actors responsibility | Required runtime responsibility | Owner / current mechanism | Status |
|---|---|---|---|---|
| Ordinary actor / state machine | Domain fold, phase, typed messages, sends, creations, termination | Serialized mailbox turns, task ownership, failure classification, effect interpretation | Engine, Bombay, Communication | realized at component level; façade remains separate |
| Guardian / application root | Typed child topology and bootstrap policy | Transactional activation, address publication, child retention and retirement | Bombay `System` and incarnation lifecycle | partial as a reusable application actor |
| Watch / link | Peer selection and reaction to an exact terminal fact | Exact-incarnation observation, cancellation, retained terminal publication | Observe and Bombay | watch realized; general link topology partial |
| Task / lifecycle monitor / reaper | Typed task request, result and reaction policy | Async execution, cancellation, panic capture, completion publication, external child ownership | Tokio and Observe through Bombay | `Task` realized; general external-task roles partial |
| Supervisor / stable proxy | Strategy, restart eligibility, budget, fleet state, replacement provenance | Child installation, exact terminal observation, creation results, retirement | Bombay, Address, Observe | realized for current local paths |
| Backoff supervisor / retrying child | Restart decision and checked delay policy | Timers, child installation and terminal observation | Timers, Observe, Bombay | partial |
| Worker pool | Admission, assignment, affinity and interruption policy | Worker creation, delivery, observation, shutdown and draining | Bombay, Communication, Observe | realized locally |
| Addressed router | Membership and deterministic selection policy | Endpoint resolution and physical delivery with payload recovery | Address, Communication, Bombay | realized locally |
| Random router | Selection from supplied entropy | Entropy acquisition | Future Bombay RNG capability | gap |
| Group router / receptionist | Membership, subscription and empty-group policy | Live registration, lifecycle removal and listing updates | Address locally; future discovery/cluster adapter | partial locally; distributed gap |
| Registry / resolver / presence | Binding, conflict, version, expiry, lookup and subscription policy | Endpoint authority, lifecycle facts and external discovery | Address, Observe, Timers, discovery adapters | templates exist; adapters partial |
| Entity host / directory | Activation policy, identity routing and passivation decisions | Single-flight activation, one live generation, bounded admission, draining | `bombay-entity` plus Bombay | mechanism exists; façade partial |
| Deadline / one-shot / periodic / receive timeout | Identity, generation, stale-event and reaction policy | Clock, sleeping, replacement/cancellation and due delivery | Timers plus Bombay | realized except general cancellation lane |
| Retry / heartbeat / debounce / throttle / idle passivation | Schedule and acceptance policy | Timer scheduling/cancellation and lifecycle delivery | Timers, Observe, Bombay | partial |
| Durable reminder | Reminder state, generation and delivery policy | Durable schedule commit, recovery, wakeup and clock execution | Mnesis plus future host | gap |
| Mailbox / bounded ingress / bulkhead | Application-visible admission or overflow policy | Queues, producer suspension, control priority, fairness and draining | Communication | mailbox realized; higher roles partial |
| Dead letters / delivery diagnostics | Rejection classification and reporting policy | Unknown/retired destination and enqueue-rejection detection, retention/export | Address, Communication, Bombay | partial |
| Reliable delivery | Sequence, acknowledgement, retry and deduplication policy | Transport acknowledgement, redelivery timers and optional durable state | Communication, Timers, future transport, Mnesis | local pieces partial; remote gap |
| Event-sourced entity | Command decision, event application, recovery phases and outcomes | Append, concurrency, hydration, snapshots and single-writer hosting | Mnesis, `mnesis-bombay`, Entity | partial integration |
| Durable-state entity | Versioned transition and conflict policy | Durable read/write, concurrency control, recovery and ownership | Mnesis and `mnesis-bombay` | partial integration |
| Projection / checkpoint / committed-event relay | Projection fold, checkpoint and delivery policy | Durable subscription, wakeups, storage and runner lifecycle | Mnesis plus `mnesis-bombay` host | complete actor host gap |
| Durable saga / process manager | Correlation, phase, compensation and decisions | Durable state, committed-event subscription, wakeups and effect delivery | Mnesis and `mnesis-bombay` | substrate partial; actor host gap |
| Durable inbox / outbox | Deduplication, dispatch and acknowledgement phases | Atomic persistence, committed-log relay and retry execution | Mnesis and `mnesis-bombay` | gap |
| Stream source / processor / sink | Demand, transformation, buffering, completion and cancellation policy | Materialization, upstream wakeups, I/O, scheduling and physical backpressure | Future stream integration; Communication locally | gap as a subsystem |
| Gateway / ingress / egress | Typed translation policy and application destination | Sockets, protocol serving, codecs, TLS, admission and shutdown | Bombay integration over selected stacks | external boundary / gap |
| Remote actor transport | Typed delivery request and factual outcome | Connections, framing, authentication, serialization, ordering and flow control | Future transport; CESR/KERI where selected | gap |
| Cluster membership / failure detector | Membership and suspicion policy over observations | Probes, clocks, dissemination and authenticated node identity | Future cluster subsystem; `foca` candidate | gap |
| Downing / split-brain resolver | Decision policy from reachability and quorum evidence | Membership authority, fencing, ownership and enforcement | Future cluster/coordination subsystem | gap |
| Cluster singleton | Handoff policy around leadership evidence | Consensus/lease, fencing, recovery and remote placement | Future coordination subsystem | gap |
| Distributed sharding / rebalance | Placement, activation, handoff and routing policy | Cluster directory, transport, coordinator state, fencing and entity movement | Future cluster plus Entity and transport | gap |
| Replicator | Merge/delta policy and peer protocol | Discovery, transport, local durability and anti-entropy scheduling | Future cluster subsystem | gap |
| Metrics / tracing / audit | Collection, aggregation, filtering and export policy | Instrumentation observations, exporter I/O and durable audit sink | Bombay operations adapters | external boundary / gap |
| Health / readiness | Aggregation over explicit component evidence | Probes, lifecycle/configuration observations and protocol export | Capability-specific Bombay adapters | templates exist; adapters partial |
| Configuration / features | Versioning, conflict and rollout policy | Source watching, authenticated updates and optional persistence | Application/Bombay adapters | external boundary |
| Resource manager | Acquisition/release phases and reactions | Files, sockets, devices, processes, blocking work and cleanup | Capability-specific adapter | external boundary |
| Security / identity gateway | Authorization policy over verified evidence | Secret custody, verification and authenticated transport | Application adapter; CESR/KERI where selected | external boundary |
| Coordinated shutdown | Dependency graph and acknowledgement policy | Stop ingress, phases, deadlines, task cancellation and resource retirement | Bombay `System`, Timers and adapters | per-actor shutdown exists; system-wide partial |

## Runtime capability backlog by dependency order

This inventory must not be implemented from top to bottom:

1. **Finish existing local seams.** Align the Actors dependency in Bombay,
   consume every current named effect lane, add timer cancellation, and prove
   supervisor, pool, task, registry, and operations through the Driver.
2. **Complete local entity hosting.** Compose Entity, Address, Communication,
   Observe and Timers into an ordinary typed `EntityHost` exposed by Bombay.
3. **Complete durable actor hosting.** Use Mnesis and `mnesis-bombay` for
   hydration, committed-event relay, projection, saga/process manager,
   checkpoint, inbox, outbox and effect-delivery actors.
4. **Complete external boundaries.** Add typed gateway, discovery,
   configuration, telemetry, resource and security adapters.
5. **Define transport before cluster actors.** Remote delivery needs explicit
   framing, identity, authentication, ordering, acknowledgement, rejection,
   backpressure and lifecycle contracts.
6. **Define membership and fencing before placement.** Membership precedes
   downing; fencing/leadership precedes singleton and durable shard ownership;
   transport plus membership plus fencing precede sharding and rebalancing.
7. **Treat streams as a subsystem.** Define demand, cancellation,
   materialization, failure and physical-backpressure laws before exposing
   source/processor/sink as complete framework actors.

## End-to-end admission gate

A runtime-backed actor is implemented only when this complete trace exists:

```text
domain input -> concrete actor event -> deterministic Behavior fold
 -> named capability request -> Driver -> runtime interpreter
 -> factual success/rejection/timeout/cancellation/failure
 -> concrete actor event -> deterministic next fold
```

For every role, record the domain slot, complete state/event/action types,
runtime owner for every lane, interpretation ordering, every factual outcome,
cancellation and retirement ownership, exact Driver path, deterministic and
integration tests, and whether the Bombay façade hides interpreter plumbing.
If any part is missing, the role is not yet a complete Bombay actor capability.
