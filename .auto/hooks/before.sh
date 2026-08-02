#!/usr/bin/env bash
# Fires before each iteration; stdout is delivered to the agent as a steer.
# Keep it cheap — just reinforce the guardrails.
echo "STEER: MAXIMIZE SCORE = K / code-only-LOC of crates/behaviorpass/src (.auto/measure.sh) — this is CONCISION golf, not throughput. Edit crates/behaviorpass/src/** + its Cargo.toml freely."
echo "STEER: do NOT touch behaviorpass-reference / behaviorpass-testkit / behaviorpass-perf / the frozen test files (checks.sh reverts you)."
echo "STEER: gate = .auto/checks.sh — trace-equal to the frozen fold at every lattice point, 17 illegal points still compile_fail, clippy bar holds. Fewer lines that stay correct AND readable is the whole game."
