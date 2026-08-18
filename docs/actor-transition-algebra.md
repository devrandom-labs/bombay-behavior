# Actor transition algebra

This is the canonical map of Bombay's foundational types and composition laws.

## Semantic boundary

A Bombay actor definition has the typed algebra

```text
A = (P, State, Event, Effects[Event], Birth, Next, Error)
```

and one pure active transition:

```text
State × Event
    -> Result(State × Effects[Event] × Birth × Next, Error)
```

Rust represents retained `State` by `&mut self`. `Actions` contains the three
explicit actor-transition legs: effects, staged fresh creations, and the next
behavior or termination verdict.

The actor-model law is the ability to communicate to known actors, create
fresh actors, and designate subsequent behavior. Typed products,
initialization, termination, local interpreter services, creator-local routing,
and interpretation ordering are Bombay constructions and policies rather than
literal Agha syntax.

## Orthogonal, but not independent

```text
Protocol<P>       transferable established destination identity
Event<E>          closed inputs accepted by this behavior composition
SendsFor<E>      proof that send effects returning to their emitter are valid for E
```

Protocol identity is invariant under transparent wrappers. Event and effects
are coupled: an interpreter request returning a later fact to its emitter
contains a continuation into that actor's event algebra.

Write `k: R -> E` for that continuation. A wrapper adding owner `O` constructs
`E' = O + E` with inner injection `i: E -> E'`. Its inner local continuations
must be pushed forward: `k' = i . k`.

`SendsFor<E>` is the Rust proof of this relationship. The paired structural
constructor is:

```text
EventLayer<OwnedEvent, InnerEvent>
SendLayer<OwnedEffects, InnerEffects>
```

`NoSends` is the owned-effect identity. One owned interpreter-request lane
uses `InterpreterRequests<R>` directly. A named product is introduced only
when several semantic effect lanes coexist.

## Destination ownership

Reindexing applies only to a continuation returning to the actor that emitted
the effect:

| Destination | Meaning | Changed when emitter is wrapped? |
|---|---|---|
| `Recipient<P>` | established transferable identity | no |
| `ChildRecipient<P>` | emitter-local child route | no |
| emitter `Ingress<Input, Path>` | later fact for the emitter | yes |
| child ingress in `ShutdownChild<C>` | input to selected child `C` | no |
| worker lifecycle report | report through an established parent/child relationship | no |

`InterpreterRequest::ReturnToEmitter` declares only the return-to-emitter
component. It is `ReturnsToEmitter<Input, Path>` or `NoReturnToEmitter`.
Parent, child, ancestor, and established destinations must not be represented
as returns to the emitter.

## Composition laws

Every behavior wrapper must satisfy all applicable laws:

1. **Protocol invariance:** a transparent wrapper preserves `B::Protocol`.
2. **Event completeness:** events are the exhaustive sum of owned and inner inputs.
3. **Effect validity:** `Sends: SendsFor<Event>`.
4. **Emitter naturality:** inner returns to the emitter follow the same injection as inner events.
5. **Remote invariance:** established, child, and ancestor destinations do not change.
6. **Identity:** a wrapper changing neither side preserves both types literally.
7. **Associativity:** nested reindexing equals composed event injection.
8. **Monoid preservation:** empty, append, lane order, and multiplicity are preserved.
9. **Initialization naturality:** initialization uses the same effect mapping.
10. **Creation preservation:** mapping does not alter order, nonce, child, or provenance.
11. **Next preservation:** delegation retains its verdict unless documented policy changes it.
12. **Error preservation:** inner controlled failures remain unchanged unless explicitly mapped.

These are Bombay composition laws. Agha supplies the actor nucleus and
configuration composability; it does not prescribe `SendsFor`, `Ingress`, or
Rust wrapper products.

## Finding the code

```text
crates/behavior/src/transition.rs       Behavior and pure turns
crates/behavior/src/user_event.rs       EventLayer, InjectEvent, Ingress
crates/behavior/src/effects/actions.rs  Actions and effect-preserving maps
crates/behavior/src/effects/sending.rs  effect layers, interpreter requests, and traversal
crates/behavior/src/actor/              protocols, recipients, and creation
crates/actors/src/                      concrete templates and compositions
crates/behavior-testkit/                independent laws, models, and fuzzing
```

Application code constructs templates directly. It does not manually create
`SendsFor` or `ReturnsToEmitter`; those proofs belong to template
implementations and wrapper constructors retain type inference.
