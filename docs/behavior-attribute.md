# Nominal Behavior Attribute

`#[behavior::behavior(...)]` is the single generated authoring path. It
preserves an ordinary inherent impl and emits the adjacent nominal `Protocol`,
pure `Behavior`, optional named send product, and optional closed child product
that a careful user could write by hand.

```rust,ignore
use behavior::{Actions, BehaviorActed, Delivery, MailAddr};

struct QueryPoolProtocol;
struct QueryStarter;

#[behavior::behavior(
    addr = MailAddr,
    message = behavior::Never,
    sends = {
        query_pool: Vec<Delivery<QueryPoolProtocol>>,
    },
)]
impl QueryStarter {
    fn init(&mut self) -> BehaviorActed<Self> {
        Ok(Actions::cont().send_query_pool(/* typed delivery */))
    }

    fn receive(&mut self, _: MailAddr, message: behavior::Never) -> BehaviorActed<Self> {
        match message {}
    }
}
```

`addr` and `message` are required because they establish public protocol
identity. The other declarations are capability-denying defaults:

- omitted `sends` means `NoSends`;
- omitted `births` means `NoBirths`; and
- omitted `error` means `Never`.

Those defaults do not infer or grant an effect. Code that tries to send, create,
or return an error absent from the declaration fails to type-check.
An advanced author may still supply an existing concrete sends or birth type
instead of a generated `{ ... }` product; this preserves handwritten catalogue
products without adding another macro path.

## Named send products

For an actor `System`, `sends = { workers: Vec<Delivery<Workers>> }` generates:

- the nominal `SystemSends` product with a named `workers` field;
- the uninhabited lane selector `SystemSendsWorkers`;
- the `SystemActions` extension trait, with a fluent `send_workers` method;
- `SendEffects`, preserving each field independently in declaration order;
- `SendInput<_, SystemSendsWorkers>`, used through `SendEffects::send`;
- `SendsFor<Event>`, requiring every field to be lawful for the complete event;
  and
- `InterpretSends`, requiring and visiting every field in declaration order.

Two fields with identical product and payload types still have different lane
selector types. No recursive position, type-name search, runtime registry, or
catch-all effect value is generated. An interpreter missing any constituent
delivery or request implementation fails its `InterpretSends` bound.

Generated fluent methods reuse the same `Actions` value and `SendInput` proof:
they append exactly once to their named lane while preserving existing sends,
creation order, and `Continue`, `Goto`, or `Stop`. They do not interpret an
effect or introduce a second effect representation. Existing concrete send
products remain directly constructible for advanced composition code.

## Closed birth products

For an actor `System`, this declaration:

```rust,ignore
births = {
    workers: Workers,
    query_reply: ManagedQueryReply,
    query_starter: ManagedQueryStarter,
}
```

generates `SystemChildren` as the exact recursive `ChildChoice` produced by
calling `Children::child` in that declaration order. The generated behavior's
birth capability is `Births<SystemChildren>`. The field labels record semantic
roles. It also generates `SystemChildrenRoutes`, with one nominally distinct
typed route per role. The same route stages that role's creation and
produces its creator-local `ChildRecipient`, so creation and routing do not
repeat a raw nonce. Values and nonces remain explicitly authored.

These names extend the macro's existing generated namespace: `SystemChild`,
`SystemChildren`, `SystemChildrenRoutes`, and one `SystemChildrenRole` marker
for each field (for example, `SystemChildrenWorkers`). As with `SystemSends`
and its lane selectors, callers must leave those actor-prefixed names to the
macro. Using the same child behavior in two fields still produces two
incompatible role types; sharing a protocol cannot accidentally exchange their
routes.

The same declaration also generates the `SystemChild` selector namespace. Its
inhabited values, such as `SystemChild::Workers`, implement
`ChildRole<System, Child = Workers>`. Each role also carries a sealed
`ChildHead`/`ChildTail<_>` position proving where that exact behavior occurs in
`SystemChildren`. This is the Behavior-owned proof consumed by static
application topology and existing heterogeneous effect products:

```rust,ignore
Application::new(System::new())
    .child(SystemChild::Workers, workers)
    .child(SystemChild::Queries, queries)
```

The selector contains no nonce and performs no creation. `SystemChildrenRoutes`
remains the separate creator-local routing product used when the parent stages
and addresses actual child creations.

A route is routing intent only. It is not an actor identity, proof of
freshness, or successful installation fact.

This is Bombay's staged creation policy: `Children` builds typed `Create`
requests, and the interpreter must commit them before dependent same-action
sends. The macro does not allocate, install, replace, choose nonces, or infer
lifecycle provenance. `DispatchBirth` still requires one concrete
`InstallBirth` implementation for every alternative.

## Exact initialization and transition results

An optional inherent `init(&mut self) -> BehaviorActed<Self>` becomes the
initialization fold. If it is absent, initialization returns the explicit empty
`Actions::cont()`. The required
`receive(&mut self, from, message) -> BehaviorActed<Self>` becomes the user
transition fold. Rust checks both method results against the generated complete
`Actions<Addr, Never, Sends, Birth>` and declared error type.

The macro generates no runtime context, constructor, scheduler, mailbox,
executor, interpreter, or template policy. Initialization and receive bodies
remain application-authored pure folds.

## Why one attribute

Rust attribute macros transform an attributed item and their output is
unhygienic. The implementation therefore resolves absolute paths through the
direct `bombay-behavior` dependency or the `bombay-rs` facade, including Cargo
renames. See the [Rust Reference on procedural macros][procedural-macros].

Serde-style stacked helper attributes are formally supported for derive
macros, which can register inert helpers. An impl-level attribute macro has no
equivalent helper-registration mechanism. Keeping sends, births, and errors as
named sections of the one owning `#[behavior]` invocation avoids expansion
order as an API constraint and keeps one coherent generated fold.

This construction is Bombay policy, not an actor-model primitive. `Actions`
remains Bombay's typed realization of communications, fresh actor creation,
and the next behavior or termination.

[procedural-macros]: https://doc.rust-lang.org/reference/procedural-macros.html
