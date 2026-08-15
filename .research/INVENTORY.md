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

## Dependencies and custom machinery

Record each dependency and every local mechanism that duplicates a standard or
owned Bombay capability. External mechanisms must remain behind Bombay-owned
semantic types.

## Invariants and risk boundaries

Use `AGENTS.md`, `docs/actor-catalogue.md`, `docs/driver.md`,
`docs/ecosystem.md`, and `README.md` as the initial local authority. Reconcile
semantic changes against cited primary research and actual interpreter
boundaries.

## Existing verification and gaps

Discover and record unit/example, composition, independent-model, exhaustive,
property, fuzz, compile-fail, documentation, architecture, mutation, and
dependency gates. No coverage claim is established by this scaffold.

## Nix gates and platform coverage

`nix flake check` is authoritative. Inventory its checks and supported systems
from the flake before changing them.

## Rust state modeling and public API

Audit sums, products, newtypes, aliases, generics, wrapper protocols, error
composition, effect lanes, and construction dependencies. Specifically look
for duplicated semantic authority, correlated `Option`/boolean state, sentinel
values, inferred provenance, positional lane access, needless wrapper depth,
and repeated concrete products that may justify a named reusable type.
