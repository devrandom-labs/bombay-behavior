#!/usr/bin/env bash
echo "STEER: metric = mutants CAUGHT over crates/behaviorpass/src (cargo-mutants). Maximize. A MISSED mutant = an invariant no test pins."
echo "STEER: FROZEN — never edit src/**, tests/oracle.rs, or any Cargo.toml. ONLY add crates/behaviorpass/tests/adv_*.rs (public API + proptest)."
echo "STEER: every test asserts exact values on the real API and MUST pass on the real code. A should-pass-but-fails test is a FINDING — report it, do not weaken it."
