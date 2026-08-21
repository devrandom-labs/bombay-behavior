# Sources

## Actor semantics

- Carl Hewitt and Henry Baker, *Laws for Communicating Parallel Processes*,
  MIT AI Working Paper 134A, 1977:
  <https://dspace.mit.edu/bitstream/handle/1721.1/41962/AI_WP_134A.pdf>
  — an acquaintance name is the information sufficient to send; acquaintance
  growth is constrained by creation and received messages.
- Gul Agha, *Actors: A Model of Concurrent Computation in Distributed
  Systems*, MIT AI-TR-844, 1985/1986:
  <https://dspace.mit.edu/bitstream/handle/1721.1/6952/AITR-844.pdf>
  — the actor transition basis and fresh actor allocation.
- Gul Agha, Ian Mason, Scott Smith, and Carolyn Talcott, *A Foundation for
  Actor Computation*, JFP 7(1), 1997:
  <https://osl.cs.illinois.edu/media/papers/agha-1997-jfp-a_foundation_for_actor_computation.pdf>
  — names, message packets, fresh allocation, and locality in the later
  operational foundation.
- Gul Agha and Christian Callsen, *ActorSpaces: An Open Distributed
  Programming Paradigm*, PPOPP 1993:
  <https://osl.cs.illinois.edu/media/papers/agha-1993-ppopp-actorspaces.pdf>
  — ActorSpaces add destination-pattern matching and passive scopes; they are
  not the basic point-to-point acquaintance mechanism.

## Rust representation constraints

- Rust error E0207:
  <https://doc.rust-lang.org/stable/error_codes/E0207.html>
  — an impl type parameter must be constrained by the implementing type,
  implemented trait, or an associated-type equality on a constrained type.
- Rust Reference, generic associated types:
  <https://doc.rust-lang.org/reference/items/associated-items.html#associated-types>
  — an associated type may be generic over a protocol type and selected by one
  concrete implementation.
- Rust Reference, generic implementations:
  <https://doc.rust-lang.org/reference/items/implementations.html#generic-implementations>
  — the language-level constraint that prevents an arbitrary hidden endpoint
  parameter on a host impl.
