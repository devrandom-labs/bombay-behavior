#!/usr/bin/env bash
echo "STEER: metric = SPACE fitness for Supervising's children table (score = 1e6/(1+space_bytes), MAXIMIZE). Shrink the liveness representation in src/supervising.rs."
echo "STEER: the generic Behavior interface is the CONTRACT — impl Behavior for Supervising (assoc types + observable step/next_deadline) stays IDENTICAL, and Supervising::new keeps its signature. Only bespoke getters an owner uses (children/alive) may be reshaped."
echo "STEER: FROZEN — other capability modules, both Cargo.toml, the perf example (the ruler), this harness. You edit ONLY src/supervising.rs (+ lib.rs re-export + Supervising's own tests). No new crate deps. Keep 'cargo test -p behaviorpass --all-targets' green."
