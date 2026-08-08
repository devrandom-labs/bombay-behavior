# Primitive derivation protocol

## Question

Find the smallest defensible statically typed basis from which Bombay actor
behaviors and reusable behavior capabilities can be composed. The intended
analogy is a calculus: a small syntax of primitive forms, explicit operational
meaning, laws, and derived forms. A catalogue of convenient framework features
is not a basis.

The campaign must answer three different questions without conflating them:

1. **Soundness:** does every well-typed primitive and composition preserve the
   actor transition boundary and the invariants claimed for it?
2. **Expressiveness:** can every capability in the documented survey be
   constructed from the candidate basis without hidden effects or weakened
   types?
3. **Minimality:** is every candidate primitive necessary, or can it be derived
   from the others without losing semantics, static guarantees, or lawful
   composition?

Passing examples establishes none of these by itself.

## Semantic nucleus

Start from the actor transition law, not the current Rust API:

```text
(behavior state, one typed communication)
    -> (typed communications, fresh creation requests, next behavior or stop)
```

Bombay's concrete `Actions` is a typed realization of these effects and adds
derived constructions and policy: typed send products, termination, phases,
initialization effects, creator-local child routing, lifecycle provenance, and
interpretation ordering. The campaign must classify every such addition as
LAW, BOMBAY-DERIVED, or BOMBAY-POLICY.

## Candidate-basis worksheet

Before claiming closure, list every candidate primitive in a table with:

- its public formation rule and concrete Rust type;
- its input and output/effect types;
- its deterministic fold or transformation semantics;
- actor-model law, Bombay derivation, or Bombay policy status;
- identity, associativity, preservation, and ordering laws that apply;
- at least one capability it helps derive;
- an eliminability experiment using the rest of the basis; and
- exact evidence for retaining it as primitive or demoting it to a derived
  form.

No item belongs in the basis merely because it already exists publicly.

## Soundness obligations

For the nucleus and every retained primitive or composition operator, verify:

1. one input communication causes one deterministic, inspectable fold;
2. all sends, creations, and become/termination decisions remain explicit in
   `Actions`;
3. no event or effect lane is dropped, duplicated, relabeled, or reordered;
4. initialization composes in a stated order before mailbox events;
5. errors and termination preserve every unaffected effect seat;
6. fresh creation remains staged and cannot overwrite an existing binding;
7. illegal protocols and capabilities fail to type-check where the invariant
   is static; and
8. interpretation-dependent behavior is identified as boundary policy rather
   than smuggled into the pure calculus.

Use unit, mixed-composition, independent-model, exhaustive, property,
compile-fail, and relevant fuzz evidence. Tests are evidence about an
implementation; the report must separately state why each test corresponds to
the claimed law.

## Minimality and independence

For each candidate primitive `p`, temporarily remove `p` from the conceptual
basis and attempt to derive its witnesses from the remaining forms.

A failed derivation counts only when the report gives:

- the exact desired type signature;
- at least one concrete implementation attempt;
- the compiler error, semantic counterexample, or violated law;
- why aliases, sums, products, ordinary generics, or higher-order transforms do
  not resolve it; and
- why the obstruction is reusable algebra rather than application or
  interpreter work.

If the derivation succeeds, `p` is a derived form even if retaining a named
wrapper is ergonomically useful. If it fails only because the current public
API is awkward, that is not yet proof of a new semantic primitive.

## Capability closure

For every capability-matrix row, provide a derivation tree:

```text
capability
  <- concrete sums/products/wrappers
  <- retained primitive basis
```

Each tree must state initialization, event lanes, effect lanes, error and
termination behavior, composition order, and static limitations. Interpreter
and application rows are boundary classifications, not failed pure
derivations.

Focused campaigns such as phase-indexed/session protocols are adversarial
falsification probes. They return one of four results to this campaign:

- derived from the current basis;
- application-local construction;
- interpreter responsibility; or
- a concrete minimal-core-gap candidate.

They do not own or redefine the basis.

## Honest closure criterion

The campaign may claim closure only relative to the repeatable literature
survey and explicit capability taxonomy. It cannot prove enumeration of an
infinite behavior-program space. The final claim must therefore have this
shape:

> No surveyed pure actor-behavior capability currently demonstrates a need for
> another primitive; every resolved pure row has a checked derivation from the
> retained basis, and every non-pure row is explicitly assigned to the
> interpreter or application boundary.

Any unresolved row, missing independence experiment, or partially represented
static guarantee makes the conclusion qualified rather than complete.
