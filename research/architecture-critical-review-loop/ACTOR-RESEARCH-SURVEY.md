# Actor research survey protocol

## Purpose

Build a reviewable research corpus before accepting or rejecting any primitive
in the Bombay behavior calculus. The corpus must cover Gul Agha's actor work,
the foundational actor-semantics lineage, later formal actor calculi, and
representative actor languages/frameworks.

“All research” cannot honestly mean proving that every publication ever written
has been found. For this campaign it means a broad, repeatable search with
author and citation chasing, an explicit stopping rule, and a bibliography that
records every considered source and every exclusion.

## Evidence hierarchy

Use sources in this order:

1. Original books, dissertations, peer-reviewed papers, formal specifications,
   and author-hosted manuscripts.
2. Official project papers, design documents, manuals, and source repositories
   for framework-specific behavior.
3. Surveys and secondary histories only to discover primary sources or compare
   terminology.

Framework APIs and documentation may establish what a framework implements.
They may not establish an actor-model law. Blogs, tutorials, and folklore may
suggest search terms but are not semantic authority.

## Agha coverage

Start from both the University of Illinois Open Systems Laboratory publication
catalogue and DBLP's Gul A. Agha author record. Search the complete records by
title, coauthor, venue, citation graph, and references. Do not limit the search
to papers already cited in this repository.

At minimum, classify and inspect the actor-relevant works in these lines:

- *Actors: A Model of Concurrent Computation in Distributed Systems*;
- actor-language structure, syntax, semantics, translation, and equivalence;
- concurrent object-oriented programming and actor architectures;
- *A Foundation for Actor Computation* and its functional/lambda foundation;
- actor algebra, algebraic theories, testing equivalence, and open systems;
- ActorSpaces, SALSA, mobility, location, and dynamic reconfiguration;
- synchronization constraints, coordination abstractions, modularity, and
  composition;
- actor implementation, parallel/distributed execution, real-time behavior,
  QoS, fault tolerance, and security; and
- later surveys or retrospectives coauthored by Agha that point to additional
  primary actor work.

For every Agha publication found, record one of: `included-semantic`,
`included-capability`, `included-framework-comparison`, or `excluded`, with a
specific reason. An exclusion such as “not relevant” is too vague.

Seed discovery indexes:

- OSL publications: <https://osl.cs.illinois.edu/publications/>
- OSL Agha profile: <https://osl.cs.illinois.edu/members/agha.html>
- DBLP author record: <https://dblp.org/pid/a/GulAAgha>
- OSL record for *A Foundation for Actor Computation*:
  <https://osl.cs.illinois.edu/publications/journals/jfp/AghaMST97.html>

These are discovery indexes, not substitutes for reading the primary works.

## Foundational and formal actor lineage

Search backward and forward from Agha's references. At minimum cover:

- Hewitt, Bishop, and Steiger's original actor formalism and Hewitt's later
  actor-model semantics;
- Greif's behavioral/operational semantics;
- Baker and Hewitt's actor ordering and event work;
- Clinger's denotational foundations;
- Agha, Mason, Smith, and Talcott's operational and equivalence foundation;
- Talcott's actor algebra;
- Agha, Thati, Ziaei, and collaborators on algebraic reasoning, open systems,
  locality, and testing; and
- actor-related process calculi, typed actor calculi, session/behavioral types,
  and formal verification where they make a distinct primitive or composition
  claim.

For each formalism, extract its primitive syntax, operational rules,
observational equivalence, fairness/ordering assumptions, creation/name laws,
and composition results. State whether each result transfers to Bombay, needs a
derived typed realization, or depends on a different computational model.

## Post-2000 algebraic and formal actor research

This is not a survey of production actor frameworks. Do not research Akka,
Erlang/OTP, Orleans, Actix, CAF, Pony, Dapr, or similar implementation systems
unless a specific primary formal paper is later shown to define an algebra,
calculus, equivalence, or theorem directly relevant to primitive derivation.

Prioritize Agha's post-2000 formal line and its citation graph:

- open distributed actor systems and compositional reasoning;
- may testing for actors;
- asynchronous calculi with locality and no name matching;
- algebraic theories of actors and their object-language application;
- probabilistic rewrite theories and rewrite-based actor specifications;
- formal modeling, model checking, and actor-system verification;
- later actor semantics/reference chapters that revise or clarify the basis;
- formal termination detection where it changes lifecycle assumptions; and
- later work that explicitly proposes actor primitives, equivalences,
  composition operators, or soundness/completeness results.

For each formal work, record its syntax, transition rules, structural
congruence, observational/testing equivalence, theorems, model assumptions, and
the exact relationship to Bombay's candidate basis. Implementation or testing
papers are included only when they supply a semantic counterexample, model, or
law relevant to the algebra.

## Required artifacts

Begin from the local source pack at
`research/architecture-critical-review-loop/RESEARCH-SOURCES.md`. It contains
the initial bibliography, stable locators, extraction template, formal-research
queue, and sequential search log. Expand it in place or link a completed
bibliography from it.

Add these sections to `REPORT.md`:

- `## Comprehensive actor research method`
- `## Agha bibliography and disposition`
- `## Foundational semantics comparison`
- `## Post-2000 actor algebra and formalism comparison`
- `## Research-to-primitive claim map`

The bibliography may live directly in the report or in a separate Markdown,
BibTeX, or CSL-JSON file linked from it. Markdown is preferred for review. A PDF
may be added as a rendered convenience, but never as the only artifact because
the checker and reviewers need searchable text.

Each bibliography entry must contain authors, exact title, venue/publisher,
year, DOI or stable URL when available, source tier, inclusion disposition,
claims supported, and notes/limitations.

## Repeatability and stopping rule

Record exact queries, indexes, dates, forward/backward citation searches, and
duplicate resolution. Stop only when all of the following hold:

- the OSL and DBLP Agha records have been completely dispositioned for
  actor-relevance;
- backward and forward citation chasing for the semantic nucleus reaches no
  new primitive or distinct semantic family in two consecutive passes;
- every post-2000 formal research line above has an authoritative source and
  calculus/capability map;
- newly discovered distinct frameworks/formalisms have been added; and
- every primitive claim in the calculus report links to at least one primary
  semantic source and concrete Bombay evidence.

Remaining inaccessible papers, ambiguous claims, contradictory semantics, and
coverage limits must be listed explicitly. The checker establishes artifact
presence and status only; reviewers must still judge source entailment.
