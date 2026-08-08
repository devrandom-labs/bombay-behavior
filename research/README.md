# Research campaigns

The research tree contains two separate campaigns. They share the repository's
actor-model laws, but they answer different questions and must not be treated as
one continuous loop.

## 1. Architecture critical review

Path: `architecture-critical-review-loop/`

Scope: the complete pure actor-behavior algebra and its interpreter boundary.
This campaign inventories capabilities, classifies each as actor-model law,
Bombay derivation, Bombay policy, interpreter responsibility, or application
concern, and verifies the public type/effect surface.

The architecture campaign asks only this session-related question:

> Does the current algebra already provide session/phase guarantees, or is the
> capability partial and in need of a focused derivation?

Its answer is a classification and a handoff. It does not own the concrete
phase-indexed protocol experiments.

## 2. Session protocol derivation

Paths:

- `.session-protocol-derivation-loop-DONE/` — first completed campaign,
  using the Supervised Worker Lifecycle case.
- `session-protocol-derivation-loop/` — independent validation campaign,
  using the Resource Pool case.

Scope: the narrow question of whether Bombay needs phase-indexed protocol
typing beyond `Fsm`. This campaign owns concrete protocols, valid and invalid
traces, compile-time probes, typestate attempts, session-duality comparison,
wrapper-composition checks, and the final core-vs-application decision.

The second campaign repeats the derivation with a materially different actor
protocol to test whether the first campaign's obstruction generalizes. It is
not a continuation or reopening of the architecture audit.

## Dependency between the campaigns

```text
architecture behavior calculus
        |
        | exports a candidate primitive basis
        v
focused session falsification probe
        |
        | tries to derive a difficult capability
        v
architecture report retains, demotes, or extends the basis
```

Information flows across that boundary; loop state does not. Each campaign has
its own goal, evidence ledger, progress log, report, runbook, and checker.

The architecture campaign is authoritative for the primitive basis. Its method
is specified in
`architecture-critical-review-loop/PRIMITIVE-DERIVATION.md`: prove soundness,
derive the surveyed capabilities, and challenge every candidate primitive for
eliminability. The session campaigns are inputs to that process, not competing
architectures.

The architecture campaign's local research starting point is
`architecture-critical-review-loop/RESEARCH-SOURCES.md`. It is deliberately
stored as searchable Markdown so a research loop can proceed without beginning
with multiple simultaneous web searches.

## Reading order

1. Read `architecture-critical-review-loop/REPORT.md` for the system-wide
   architecture result.
2. Read `.session-protocol-derivation-loop-DONE/REPORT.md` for the original
   focused derivation.
3. Read `session-protocol-derivation-loop/REPORT.md` only for the independent
   Resource Pool validation.

Do not interleave the architecture obligation list with the session attempt
numbers. Architecture IDs describe audit coverage; session attempt numbers
describe competing concrete encodings.
