# Bombay Behavior

Bombay Behavior is the statically typed functional core of the Bombay actor
stack. `bombay-behavior` defines the pure `Behavior` fold and its explicit
`Actions`; `bombay-behavior-actors` provides reusable behaviors and typed
interpreter requests built on that algebra.

One accepted event produces exactly one result:

```text
(current behavior, event)
        │
        ▼
Actions { sends, fresh creations, next behavior or termination }
```

Evaluation is synchronous and deterministic. Mailboxes, scheduling, clocks,
fresh address allocation, endpoint installation, transport, and effect
interpretation belong to a runtime. The core contains no `dyn`, `Any`,
`TypeId`, runtime protocol key, erased envelope, or hidden effect channel.

## Actor algebra

The foundational roles are deliberately separate:

| Type | Meaning |
|---|---|
| `Protocol` | Canonical public destination identity: `Addr` and `Msg` |
| `Behavior::Event` | The complete closed event algebra accepted by one concrete behavior |
| `Behavior` | State plus the pure transition fold |
| `Actions` | Typed communications, staged fresh creations, and the next-state verdict |

`Behavior::Protocol` is the canonical identity of the actor exposed by a
behavior. Transparent wrappers preserve it. Structural event paths and child
occurrences are compile-time navigation evidence; they never become a second
identity or runtime key.

`Actions` is Bombay's typed realization of actor transition effects. Its Rust
shape includes derived Bombay constructions—typed effect products, phases,
termination, initialization effects, creator-local child routes, and defined
interpretation ordering—so it is not claimed to be a literal transcription of
Agha's formal notation.

See [Actor transition algebra](docs/actor-transition-algebra.md) for the core
laws, [Behavior layer laws](docs/behavior-layer-laws.md) for same-actor
composition, and [Behavior adapter contract](docs/adapter-contract.md) for
runtime obligations.

## Destination capabilities

The API now distinguishes four facts that were previously easy to conflate:

```text
Recipient<P>                 logical protocol address
ChildRoute<C, O>             staged child route in one creator namespace
EstablishedRecipient<P>      exact installed protocol endpoint
EstablishedActor<B>          exact installed endpoint plus concrete-behavior proof
```

`Recipient<P>` remains correct at transport, discovery, and stable-name
boundaries. It requires runtime address resolution. An
`EstablishedRecipient<P>` instead carries the runtime's exact monomorphized
endpoint and can be delivered to, observed, or transferred without a
protocol-wide lookup table.

`ChildRoute<C, O>` carries a creator-local nonce. The nonce is correlation and
routing evidence only: it is not an actor address, identity, or freshness
proof. Same-action local communication uses `ChildDelivery<P, O>` and is
resolved through the creator instance's committed `(occurrence, nonce)`
binding. No address is derived from a nonce.

Fresh allocation is an interpreter law. A successful
`EstablishedCreation<P, O>` is produced only after allocation,
initialization, installation, and binding commit. Rejection is an exhaustive
typed result and contains no endpoint capability.

The runtime owns the endpoint family once per address namespace by
implementing `EndpointAddress`. Ordinary protocols declare no keys and domain
types acquire no endpoint boilerplate. Public interpretation traits are an
intentional power-user transfer boundary; endpoint values have no direct
accessor or ambient send operation. Exact endpoints are cloneable but not
intrinsically `Send`; concrete asynchronous interpretation requires `Send`
only for the values it actually transports.

The full capability and lifecycle contract is in
[Established capabilities](docs/established-capabilities.md).

The [actor-template composition audit](docs/template-composition-audit.md)
classifies the complete current catalogue by distinct transition law. The
[Behavior Actors template law audit](docs/template-law-audit.md) records which
retained templates intentionally use logical names, exact installed
capabilities, or creator-local child routes, and the hard-coding checks applied
across the catalogue.

The [external actor-system interface](docs/external-actor-interface.md) records
the intended common boundary for HTTP, CLI, tests, embedded clients, and future
transports: a statically typed receptionist product plus real external actor
endpoints, with discovery remaining an optional actor protocol rather than a
global runtime service.

## Authoring a behavior

Use `#[behavior::behavior(...)]` for a nominal user-message behavior. The
attribute emits the same concrete protocol, event, sends, births, and role
types that can be written manually. It does not perform creation, routing, or
runtime lookup.

```rust
use behavior::{Actions, MailAddr, Never, NoBirths};

struct Counter(u64);

#[behavior::behavior(
    addr = MailAddr,
    message = u64,
    sends = Vec<Never>,
    births = NoBirths,
    error = Never,
)]
impl Counter {
    fn receive(
        &mut self,
        _from: MailAddr,
        message: u64,
    ) -> behavior::BehaviorActed<Self> {
        self.0 += message;
        Ok(Actions::cont())
    }
}
```

Omitted `sends`, `births`, and `error` select `NoSends`, `NoBirths`, and
`Never`. Named send and birth products preserve semantic field names and
duplicate child roles. Implement `Behavior` manually when the behavior owns
service events, phases, or wrapper semantics. There is deliberately no macro
for actor spaces or composed wrapper stacks.

See [Nominal Behavior attribute](docs/behavior-attribute.md).

## Installation and composition

```toml
[dependencies]
bombay-behavior = "0.14"
bombay-behavior-actors = "0.14"
```

Composition has two axes. `Behavior::layer` invokes a statically typed
constructor while preserving the fully concrete output type. The constructor
itself proves no operational law; its output `Behavior` must satisfy the
[Behavior layer laws](docs/behavior-layer-laws.md) for same-mailbox event,
effect, initialization, error, birth, and lifecycle composition.
`DeliveryRoute` connects independent actors through logical or established
capabilities; `DeliveryRouteFor<Owner>` additionally admits only a direct
`ChildRoute` proven by that owner's birth algebra. Topology owners such as
`Supervisor` and `Proxy` retain lifecycle correlation, while `Router`, queues,
workflows, and domain actors retain their own independent laws.

For supervised workers, construct the domain behavior and all of its
same-mailbox layers first, pass those inferred values to `ChildTopology`, and
give the topology owner the reusable stable-incarnation layer. The resulting
hierarchy is `Supervisor → Proxy → worker layers → domain behavior`.
Construction names no composed output type; only real protocol roles and
destinations remain explicit.

The [Actor composition map](docs/composition-recipes.md#the-composition-map)
shows the hierarchy, selection rules, and an executable
`Router → Supervisor-owned Proxy → PriorityQueue → Target` trace.

`BirthProtocols` folds a behavior's transitive birth algebra into
an occurrence-preserving protocol product. It excludes protocols mentioned
only by delivery lanes. Duplicate protocol occurrences remain distinct; the
product is structural installation evidence, not a protocol registry.

`LogicalHostRequirements` structurally projects every intentional logical
`Delivery<P>` in a behavior's concrete sends and in every transitive birth.
Core structural and handwritten catalogue send products append their lane
projections in interpretation order. Repeated occurrences remain repeated, while exact
established deliveries, direct-child effects, and interpreter requests add no
logical host. A framework consumes the resulting proof product using its own
static `Hosts<P>` implementation; the projection performs no installation or
runtime lookup.

`shutdown_after_children(app)` remains the single child-derived shutdown-plan
builder. Direct callers declare phases and finish normally. Generic frameworks
use `DeclareShutdownPhase` and `FinishShutdownPhases`; their associated output
types carry the hidden availability and phase proofs without copying the
builder typestate.

`Children` stages heterogeneous creations. `DispatchBirth` recursively proves
that an interpreter implements `InstallBirth<Position, Child, ...>` for every
alternative. The structural position prevents duplicate child alternatives
from silently sharing an installer obligation while avoiding false `Behavior`
bounds on unrelated wrapper and domain types.

`FoldBirthNode` exposes that same closed direct-child structure to static
runtimes. A runtime implements `BirthNodeMapper` to choose its own empty and
per-child storage types; Behavior supplies every concrete leaf and its existing
`ChildHead`/`ChildTail<_>` position. The fold creates no values and adds no
creation semantics, registry, protocol key, or runtime dependency. Nested
children are folded in the namespace of the concrete actor that creates them,
not flattened into root-owned storage.

`ResolveChildOccurrence<Occurrence>` statically joins an emitted effect back to
that storage. Generated roles resolve through `BehaviorBase` only across
wrappers that preserve the exact protocol and birth algebra. Raw `ChildHead`
and `ChildTail<_>` positions resolve against the running emitter's own direct
births. A topology-changing wrapper therefore exposes its new structural child
instead of silently inheriting a stale base role. Resolution is type-only: it
adds no registry, key, value, or runtime lookup.

Initialization is a consuming typestate transition. Wrapper initialization
effects compose in defined order and must be interpreted before mailbox
events. Send products likewise retain named lanes and structural ordering;
they are never flattened into a dynamic envelope.

The reusable worker-pool contract is documented in
[Worker pool semantics](docs/worker-pool.md). Correctness-sensitive actor
arrangements are documented in [Actor composition](docs/composition-recipes.md).
The catalogue classification is recorded in the
[actor-template composition audit](docs/template-composition-audit.md), and the
repository-wide law and adversarial-test audit is recorded in
[Behavior Actors template-law audit](docs/template-law-audit.md).

## Development

Use the pinned environment and repository gates:

```sh
nix develop
cargo nextest run --workspace
nix flake check
```

`nix flake check` is authoritative and includes build, tests, rustdoc,
formatting, dependency audit, and dependency policy. Benchmarks and fuzz
targets are run explicitly when their surfaces change.

## License

Licensed under either Apache-2.0 or MIT at your option.
