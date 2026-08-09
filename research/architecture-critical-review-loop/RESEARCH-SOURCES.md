# Local actor research source pack

## Status

This file is a local seed corpus for the architecture loop. It prevents the
research agent from needing to begin with several simultaneous web searches.
It is not the final bibliography and does not establish that a cited source
entails a Bombay semantic claim.

Process sources sequentially. Open one index or primary work, record its
disposition and extracted claims, checkpoint progress, and only then continue
to the next source. Do not launch a batch of web searches.

## Local research question

Determine the smallest sound and defensible set of typed actor-behavior
primitives from which the surveyed reusable capabilities can be derived.

For every source, extract only information relevant to:

- primitive actor operations;
- transition and configuration semantics;
- message ordering and fairness;
- creation, freshness, naming, and acquaintance;
- behavior replacement;
- open-system composition and equivalence;
- typing and protocol constraints;
- supervision, failure, lifecycle, and observation;
- distribution, location, mobility, persistence, and security boundaries; and
- whether a capability is pure behavior, interpreter work, or application
  policy.

## Start here: Agha publication indexes

### Open Systems Laboratory publication catalogue

- URL: <https://osl.cs.illinois.edu/publications/>
- Role: primary discovery index for Agha and OSL publications.
- Required action: walk every entry associated with Gul Agha and disposition
  every actor-relevant item. Search within the page by author, title, coauthor,
  and actor terminology.
- Important limitation: an index entry proves bibliographic existence, not the
  semantic content of the linked work.

### Open Systems Laboratory Agha profile

- URL: <https://osl.cs.illinois.edu/members/agha.html>
- Role: author-maintained publication discovery page.
- Required action: reconcile its actor-related entries with the OSL catalogue
  and DBLP; record missing, duplicate, and inaccessible works.

### DBLP Gul A. Agha record

- URL: <https://dblp.org/pid/a/GulAAgha>
- Role: independent bibliographic index and export source.
- Required action: compare the complete record with OSL, then classify every
  actor-relevant work as semantic, capability, framework-comparison, or
  excluded with a precise reason.

## Foundational actor corpus

### Hewitt, Bishop, and Steiger — original actor formalism

- Citation: Carl Hewitt, Peter Bishop, and Richard Steiger, “A Universal Modular
  ACTOR Formalism for Artificial Intelligence,” IJCAI, 1973, pp. 235–245.
- Search locator: title plus `IJCAI 1973 PDF`.
- Inspect for: actor primitives, locality/acquaintance, creation, communication,
  and the distinction between the actor model and implementation mechanisms.
- Current Bombay use: semantic origin only; do not infer later Agha operational
  details from this paper without checking them.

### Hewitt — message-passing control structures

- Citation: Carl Hewitt, “Viewing Control Structures as Patterns of Passing
  Messages,” *Artificial Intelligence* 8(3), 1977, pp. 323–364.
- DOI: <https://doi.org/10.1016/0004-3702(77)90033-9>
- Inspect for: actor computation structure, messaging, control, concurrency,
  and ordering claims.

### Greif — behavioral actor semantics

- Citation: Irene Greif, *Semantics of Communicating Parallel Processes*, MIT
  MAC-TR-146, 1975, doctoral dissertation.
- Search locator: exact title plus `MIT MAC-TR-146`.
- Inspect for: event diagrams, behavioral semantics, communication events,
  observational meaning, and historical assumptions later changed by Clinger
  or Agha.

### Clinger — denotational foundations

- Citation: William D. Clinger, *Foundations of Actor Semantics*, MIT AI-TR-633,
  1981, doctoral dissertation.
- Stable locator:
  <https://dspace.mit.edu/bitstream/handle/1721.1/6935/AITR-633.pdf>
- Inspect for: event ordering, unbounded nondeterminism, fairness, denotational
  semantics, newly created addresses, and limitations of earlier accounts.

### Agha — core actor model

- Citation: Gul A. Agha, *Actors: A Model of Concurrent Computation in
  Distributed Systems*, MIT Press, 1986.
- Search locator: exact title plus `MIT Agha 1986 PDF`.
- Inspect completely, especially the operational task semantics and chapters
  defining communication, creation, acquaintances, and replacement behavior.
- Claims requiring exact page/section evidence:
  - an actor processes one communication at a time;
  - a transition may send communications, create actors, and designate its
    replacement behavior;
  - creation selects a fresh address;
  - acquaintances constrain communication; and
  - behavior replacement is distinct from actor creation.
- Bombay warning: `Actions` is a typed realization with additional Bombay
  constructions and policy. Do not call its concrete Rust shape exactly Agha's
  triple.

### Agha — concurrent object-oriented programming

- Citation: Gul A. Agha, “Concurrent Object-Oriented Programming,”
  *Communications of the ACM* 33(9), 1990, pp. 125–141.
- DOI: <https://doi.org/10.1145/83880.84585>
- Inspect for: actor-language structure, encapsulation, concurrency, behavior
  change, coordination, and what is model law versus language design.

### Agha — structure and semantics of actor languages

- Citation: Gul A. Agha, “The Structure and Semantics of Actor Languages,” REX
  Workshop, LNCS 489, Springer, 1990, pp. 1–59.
- Discovery record: the OSL publication catalogue lists this work under 1990.
- Inspect for: language constructs, semantic basis, derived syntax, composition,
  and equivalence.

### Agha, Mason, Smith, and Talcott — functional foundation

- Citation: Gul A. Agha, Ian A. Mason, Scott F. Smith, and Carolyn L. Talcott,
  “A Foundation for Actor Computation,” *Journal of Functional Programming*
  7(1), 1997, pp. 1–72.
- OSL record:
  <https://osl.cs.illinois.edu/publications/journals/jfp/AghaMST97.html>
- Author-hosted PDF:
  <https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf>
- DOI: <https://doi.org/10.1017/S095679689700261X>
- Inspect completely for: functional-language extension, actor configurations,
  labeled operational semantics, open distributed systems, composability,
  fairness, fresh names, and testing equivalence.
- Calculus relevance: compare its primitive syntax and derived forms directly
  against Bombay's proposed nucleus.

### Talcott — actor algebra

- Citation: Carolyn Talcott, “An Actor Algebra,” Fourth International School on
  Advanced Functional Programming, Springer, 1996.
- Search locator: exact title, author, and venue.
- Inspect for: algebraic operators, equivalences, congruence/composition laws,
  and which forms are primitive versus derived.

### Agha and Thati — algebraic theory

- Citation: Gul Agha and Prasanna Thati, “An Algebraic Theory of Actors and Its
  Application to a Simple Object-Based Language,” LNCS 2941, Springer, 2004.
- DOI: <https://doi.org/10.1007/978-3-540-39993-3_4>
- Inspect for: algebraic syntax, operational rules, equivalence, composition,
  locality, and the relationship between actor and object-language constructs.

### Agha, Thati, and Ziaei — open distributed systems

- Citation: Gul Agha, Prasanna Thati, and Reza Ziaei, “Actors: A Model for
  Reasoning About Open Distributed Systems,” Open Systems Laboratory report.
- Stable locator:
  <https://osl.cs.illinois.edu/media/papers/agha-2001-actors.pdf>
- Inspect for: open-system interfaces, observational reasoning, locality,
  composition, testing, and which guarantees require environment assumptions.

### Agha and Kim — parallel and distributed computing

- Citation: Gul A. Agha and WooYoung Kim, “Actors: A Unifying Model for
  Parallel and Distributed Computing,” *Journal of Systems Architecture*
  45(15), 1999, pp. 1263–1277.
- OSL record:
  <https://osl.cs.illinois.edu/publications/journals/jsa/AghaK99.html>
- DOI: <https://doi.org/10.1016/S1383-7621(98)00067-8>
- Inspect for: actor-language and implementation survey, expressiveness,
  portability, efficiency, predictability, middleware, agents, and separation
  of semantic model from runtime realization.

## Agha-related capability lines to expand locally

The following are mandatory citation-chasing categories. Add exact records from
OSL/DBLP before resolving the bibliography obligation:

- ActorSpaces and open coordination spaces;
- SALSA and dynamically reconfigurable/mobile actors;
- Rosette and actor runtime architecture;
- synchronization constraints and modular coordination;
- testing equivalences, locality, and asynchronous calculi;
- real-time actors, timing constraints, and QoS;
- fault tolerance and distributed artificial intelligence;
- security, mobility, and location-aware actor systems; and
- implementation and performance work that distinguishes calculus primitives
  from interpreter optimizations.

## Formal alternatives and adjacent calculi

These sources do not automatically govern Bombay. Use them to challenge the
candidate basis and document semantic mismatches.

### Honda — session types

- Citation: Kohei Honda, “Types for Dyadic Interaction,” CONCUR 1993, LNCS 715,
  pp. 509–523/528 depending on bibliographic record.
- Search locator: exact title plus `CONCUR 1993`.
- Inspect for: dual endpoints, send/receive sequencing, continuation types, and
  closed two-party assumptions.
- Bombay relevance: session duality is not an Agha actor-model law. Compare it
  carefully with dynamic actor acquaintance and per-actor phase validity.

### Typed actor and behavioral-type work

- Search sequentially for: `typed actor calculus`, `actor behavioral types`,
  `actor session types`, `multiparty session actors`, `typestate actors`, and
  `actor protocol verification`.
- Required extraction: exact static guarantee, endpoint/address assumptions,
  progress/fidelity theorem, dynamic discovery support, and whether the result
  needs runtime coordination or type erasure.

### Rebeca

- Begin from the original Rebeca language/formal-semantics and model-checking
  papers rather than tutorials.
- Inspect for: reactive class semantics, bounded abstractions, message servers,
  formal verification, timing variants, and distinctions from open unbounded
  actor systems.

## Post-2000 Agha algebra and formalism queue

Process these primary formal sources one at a time. Do not substitute production
framework documentation.

### Open distributed actor systems

- Gul Agha, Prasanna Thati, and Reza Ziaei, “Actors: A Model for Reasoning
  About Open Distributed Systems,” 2001.
- OSL record: <https://osl.cs.illinois.edu/publications/agha01actors.html>
- Inspect for: interfaces of open configurations, composition, locality,
  observational reasoning, and assumptions not present in a closed calculus.

### May testing for actors

- Prasannaa Thati, Reza Ziaei, and Gul Agha, “A Theory of May Testing for
  Actors,” FMOODS 2002, IFIP Conference Proceedings 209, pp. 147–162.
- OSL record:
  <https://osl.cs.illinois.edu/publications/conf/fmoods/ThatiZA02.html>
- Inspect for: actor observations, testing contexts, trace characterization,
  equivalence/preorder, and compositionality.

### Local asynchronous calculi

- Prasannaa Thati, Reza Ziaei, and Gul Agha, “A Theory of May Testing for
  Asynchronous Calculi with Locality and No Name Matching,” AMAST 2002, LNCS
  2422, pp. 223–238.
- OSL record:
  <https://osl.cs.illinois.edu/publications/conf/amast/ThatiZA02.html>
- DOI: <https://doi.org/10.1007/3-540-45719-4_16>
- Inspect for: locality/non-interference, absence of name matching, trace-based
  may testing, complete axiomatization of the finitary fragment, and whether
  Bombay address equality or routing violates transfer assumptions.

### Algebraic theory of actors

- Gul Agha and Prasanna Thati, “An Algebraic Theory of Actors and Its
  Application to a Simple Object-Based Language,” 2004, LNCS 2635, pp. 26–57.
- OSL record:
  <https://osl.cs.illinois.edu/publications/conf/birthday/AghaT04.html>
- DOI: <https://doi.org/10.1007/978-3-540-39993-3_4>
- Inspect completely for: primitive actor terms, algebraic operators,
  transition rules, equivalences, laws, derived object-language constructs,
  and any soundness/completeness result.

### Probabilistic rewrite theories

- Gul Agha, José Meseguer, and Koushik Sen, “PMaude: Rewrite-Based
  Specification Language for Probabilistic Object Systems,” QAPL 2005.
- Gul Agha, Carl Gunter, Michael Greenwald, Sanjeev Khanna, José Meseguer,
  Koushik Sen, and Prasanna Thati, “Formal Modeling and Analysis of DoS Using
  Probabilistic Rewrite Theories,” FCS 2005.
- Inspect for: rewriting-logic configuration structure, probabilistic rules,
  object/actor assumptions, and whether probability is behavior algebra or an
  interpreter/environment observation.

### Later actor summaries and formal modeling

- Rajesh K. Karmani and Gul Agha, “Actors,” *Encyclopedia of Parallel
  Computing*, Springer, 2011.
- OSL record:
  <https://osl.cs.illinois.edu/publications/reference/parallel/KarmaniA11.html>
- DOI: <https://doi.org/10.1007/978-0-387-09766-4_125>
- Use as a later Agha-coauthored semantic summary and discovery source; it does
  not override earlier primary formal definitions without an explicit revision.

### Latest actor lifecycle formal work

- Dan Plyukhin and Gul Agha, “Scalable Termination Detection for Distributed
  Actor Systems,” CONCUR 2020, LIPIcs 171, 11:1–11:23.
- Dan Plyukhin and Gul Agha, “A Scalable Algorithm for Decentralized Actor
  Termination Detection,” *Logical Methods in Computer Science*, 2022.
- Preprint locator: <https://arxiv.org/abs/2104.05128>
- Inspect for: formal actor termination, safety/liveness theorem, external actor
  assumptions, reference/listing information, and whether lifecycle detection
  belongs in pure behavior or the interpreter boundary.

### Continue the post-2000 citation graph

Search the complete OSL/DBLP record after 2000 for titles and abstracts that
mention actors together with algebra, calculus, semantics, equivalence,
testing, locality, rewriting, formal modeling, model checking, verification,
termination, or composition. Include a work only if reading it reveals a claim
that can affect the candidate primitive basis; disposition every candidate.

## Capability extraction template

Copy this block for every primary formal source:

```markdown
### Exact source title

- Bibliography:
- Stable URL/DOI:
- Source tier:
- Read status: unread | abstract-only | partial | complete
- Inclusion: semantic | capability | framework-comparison | excluded
- Primitive syntax/operations:
- Operational assumptions:
- Laws/theorems:
- Capability claims:
- Bombay classification: LAW | BOMBAY-DERIVED | BOMBAY-POLICY | INTERPRETER | APPLICATION
- Candidate-basis impact:
- Concrete derivation or obstruction:
- Limitations/non-transferable assumptions:
- Evidence location in code/tests:
```

Do not mark a source `complete` after reading only an abstract, index entry, or
secondary description.

## Sequential search log seed

The following discovery searches were run on 2026-08-09. Repeat them one at a
time only when the local sources above are insufficient:

1. `site:illinois.edu Gul Agha publications actors bibliography`
2. `Gul Agha actor model publications A Foundation for Actor Computation actor algebra`
3. `foundational actor model research Hewitt Clinger Greif Agha actor semantics`
4. `site:osl.cs.illinois.edu/publications Agha actor algebra after 2000`

Initial useful results were the OSL publications catalogue, OSL Agha profile,
DBLP Agha record, OSL JFP record/PDF, the 2001 open-systems record, both 2002
may-testing records, the 2004 algebraic-theory record, the 2011 actor reference,
and the 2020/2022 termination work. Search-engine summaries were used only for
discovery and must not appear as semantic evidence.

## Completion handoff

The architecture report links back to this file and now contains or links:

- the fully dispositioned Agha bibliography (AGHA-BIBLIOGRAPHY.md);
- foundational-semantics comparison (REPORT.md);
- post-2000 algebra/formalism comparison (REPORT.md);
- claim-to-primary-source map (REPORT.md '## Research-to-primitive claim
  map');
- inaccessible/excluded source log (REPORT.md method section +
  AGHA-BIBLIOGRAPHY.md exclusion groups E1–E4);
- candidate primitive worksheet (REPORT.md '## Candidate primitive basis');
- soundness evidence (REPORT.md '## Primitive soundness');
- eliminability results (REPORT.md '## Primitive eliminability'); and
- capability derivation trees (REPORT.md '## Capability derivation trees').

Per-source extraction blocks live in RESEARCH-EXTRACTIONS.md. The 2026-08-09
rerun completed the sequential survey: OSL catalogue (419 entries) and DBLP
XML export (244 records) fully dispositioned; primary PDFs read in the logged
order; access limitations recorded explicitly.
