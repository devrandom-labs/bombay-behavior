# Repository inventory

## Architecture and data flow

- `bombay-behavior`: foundational pure behavior algebra.
- `bombay-behavior-actors`: reusable typed actor-template catalogue built on
  `bombay-behavior`.
- Governing flow: typed event -> concrete behavior fold -> typed `Actions` ->
  generic Driver -> statically selected Environment interpreters.

## Public surface and consumers

Inventory every public item in both scoped crates, its owning module, semantic
role, consumers, and intended Bombay façade exposure. Do not treat technical
public visibility as evidence that ordinary users should see an item.

The loop counted 622 public items. The authoritative item-by-family and façade
mapping remains `docs/actor-catalogue.md`; this audit found no evidence that its
component-vs-Bombay exposure classifications should change. Root aliases with
no repository consumers were retained when they provide truthful names for
verbose concrete protocol types.

## Dependencies and custom machinery

Record each dependency and every local mechanism that duplicates a standard or
owned Bombay capability. External mechanisms must remain behind Bombay-owned
semantic types.

No new dependency was introduced. The stale `ring` license clarification and
unused license allowances in `deny.toml` were removed only after confirming the
packages/licenses were absent from `Cargo.lock` and `cargo deny` passed.

## Invariants and risk boundaries

Use `AGENTS.md`, `docs/actor-catalogue.md`, `docs/driver.md`,
`docs/ecosystem.md`, and `README.md` as the initial local authority. Reconcile
semantic changes against cited primary research and actual interpreter
boundaries.

No retained experiment changes actor creation, ordering, supervision policy,
observation delivery, timers, persistence, or interpreter ownership. E01/E04
tighten the existing proxy incarnation representation without changing its
fresh-installation or lifecycle-provenance law.

## Existing verification and gaps

Discover and record unit/example, composition, independent-model, exhaustive,
property, fuzz, compile-fail, documentation, architecture, mutation, and
dependency gates. No coverage claim is established by this scaffold.

Observed coverage: 282 nextest cases and 10 doctests/compile-fail examples.
E10 adds the two previously missing circuit-breaker exhaustion boundaries.
The independent audit found no failing all-target build or doctest.

## Nix gates and platform coverage

`nix flake check` is authoritative. Inventory its checks and supported systems
from the flake before changing them.

The live flake gate passed all seven exposed checks on aarch64-darwin. No Nix
source or platform matrix was changed by the loop.

## Rust state modeling and public API

Audit sums, products, newtypes, aliases, generics, wrapper protocols, error
composition, effect lanes, and construction dependencies. Specifically look
for duplicated semantic authority, correlated `Option`/boolean state, sentinel
values, inferred provenance, positional lane access, needless wrapper depth,
and repeated concrete products that may justify a named reusable type.

Retained findings: one duplicated private/public error authority (E01), one
boolean phase split (E02), one dead sentinel conversion (E03), one exclusive
effect sum (E04), six forgeable derived snapshots (E06/E06b), and one
circuit-breaker dummy-generation tuple (E09). Similar versions, generations,
IDs, evidence types, errors, phases, and aliases were retained when their
authority or laws differ. E05's broad primitive conversions were reverted as
unused convenience surface rather than semantic composition.
