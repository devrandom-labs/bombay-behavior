# Nominal Behavior Attribute

`#[behavior::behavior(...)]` removes only the mechanical implementation that
adapts an ordinary user-message method to `Behavior`. It is an attribute on a
normal inherent impl, so state, constructors, helper methods, generics,
where-clauses, visibility, documentation, and `&mut self` behavior methods
remain ordinary Rust:

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
        value: u64,
    ) -> behavior::Acted<MailAddr, Never, Vec<Never>, NoBirths, Never> {
        self.0 += value;
        Ok(Actions::cont())
    }
}
```

The expansion preserves that inherent impl unchanged and adds an ordinary
`Behavior` implementation with these exact associated types:

```text
Addr  = declared addr       Msg   = declared message
Event = User<Addr, Msg>     Sends = declared sends
Ph    = Never               Error = declared error
Birth = declared births
```

When present, `Behavior::init` calls the inherent `init` method during the
single consuming `initialize` transition.
When omitted, it returns the explicit empty initialization transition.
`Behavior::transition` destructures the concrete `User` event and calls the
inherent `receive` method exactly once. Rust checks both returned values against
`BehaviorActed<Self>`; the macro does not parse or infer semantic types from a
return-type alias.

The seven associated types are deliberately explicit and ordered. The macro
does not provide defaults, accept unknown options, infer capabilities, create
constructors, add state, generate messages, register protocols, erase types,
box futures, interpret effects, or introduce another transition path.

The attribute is intentionally limited to `User`/`Never` behaviors. A behavior
that owns service-event variants, phases, routing transformations, or wrapper
semantics implements `Behavior` explicitly so its complete sum and product
types remain visible. This boundary keeps the attribute an authoring aid rather
than a second behavior language.

The construction is Bombay policy, not an actor-model primitive. Its generated
implementation is the same pure fold a user could write manually.

## Why an attribute macro

Rust defines attribute macros as transformations of an attributed item: they
receive the attribute input and the item, then replace that item with generated
items. That shape fits this API precisely—the inherent impl remains ordinary
Rust input and the generated `Behavior` impl is an adjacent item. See the
[Rust Reference on attribute macros][attribute-macros].

A derive macro cannot implement this contract because derives receive only the
annotated item and add generated items; they have no syntax for supplying the
`init` and `receive` method bodies. A function-like macro would instead require
a separate invocation and a new mini-language around code that Rust already
expresses. The Rust Book distinguishes these three procedural-macro forms in
its [procedural macros overview][procedural-macros].

This rationale concerns Rust authoring syntax. It does not change the actor
transition law or grant the macro any interpreter capability.

## Separate macro boundaries

`#[behavior::actor]` is the inferred shorthand for the common synchronous,
infallible, no-birth subset:

```rust,ignore
#[behavior::actor]
impl Printer {
    fn receive(&mut self, from: MailAddr, message: String) -> Effect<Delivery<Sink>> {
        Effect::send(Delivery::new(Recipient::global(from), message))
    }
}
```

It infers `Addr` and `Msg` from the explicit parameters and the vector send
element from `Effect<Send>`. Its remaining associated types are truthfully
fixed to `Never` phase/error and `NoBirths`. `Effect` is only a pure shorthand
for `Actions<A, Never, Vec<Send>, NoBirths>`; it has no runtime dependency or
interpretation authority. Any behavior requiring initialization effects,
named send products, controlled errors, births, phases, or service-event sums
uses the complete `#[behavior(...)]` contract.

`#[behavior]` generates only nominal user-message fold wiring.
`#[behavior::births]` applies to a closed enum whose variants each contain one
concrete child behavior. It generates only exhaustive `DispatchBirth` wiring:
the enum is a creation product and never a `Behavior`, message union, factory,
registry, or protocol adapter. Wrapper stacks remain ordinary Rust values built
through public owning-type constructors and inferred at local, generic spawn,
and adapter boundaries.

```rust,ignore
#[behavior::births]
enum ApplicationChildren {
    DeviceGroups(DynamicSupervisor<...>),
    Queries(WorkerPool<...>),
}

type Birth = behavior::Births<ApplicationChildren>;
```

The generated implementation requires the selected installer to implement
`InstallBirth` for both concrete variants. Its exhaustive match forwards the
original nonce and `CreationKind`; it does not install `ApplicationChildren`
as an actor. This keeps each child recipient indexed by its concrete behavior
protocol and makes incomplete runtime support a compile-time error.

Both attributes are governed by the [Universal Behavior Driver](driver.md).
They generate only concrete static implementations a user could write
manually.

Applications may depend on `bombay-behavior` directly or use its re-export
through the `bombay-rs` façade. Generated paths first select the direct package
when present, then fall back to `bombay-rs::behavior`; Cargo dependency aliases
are preserved in both cases. If neither package is a direct dependency, macro
expansion fails with a compile-time resolution error.

[attribute-macros]: https://doc.rust-lang.org/reference/procedural-macros.html#attribute-macros
[procedural-macros]: https://doc.rust-lang.org/book/ch20-05-macros.html
