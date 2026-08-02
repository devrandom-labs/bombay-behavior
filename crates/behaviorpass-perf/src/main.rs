//! Concision metric for the behaviorpass loop (bombay card #298).
//!
//! Prints two lines; the loop MAXIMIZES `SCORE`:
//!
//! ```text
//! CODE_LOC=<code-only lines of the SUT capability machinery>
//! SCORE=<K / CODE_LOC>   (fewer lines ⇒ higher score)
//! ```
//!
//! "Code-only" excludes blank lines and comment-only lines (`//`, `//!`,
//! `///`, and `*`-led block-comment bodies) — a cheap, deterministic proxy
//! for the machinery's line cost. Correctness (trace-equality to the frozen
//! reference) and the 17 illegal-point `compile_fail`s are hard GATES in
//! `.auto/checks.sh`, not measured here: this bin only asks "how few lines is
//! the current design?".
//!
//! The per-site α term and the compile-time / binary-size instruments
//! (#298's marginal-cost curve) are deferred to phase-1; SCORE = K / CODE_LOC
//! is the phase-0 objective.

use std::fs;
use std::path::Path;

/// Scale constant so a few-hundred-line machinery scores in a readable range.
const K: f64 = 100_000.0;

/// The SUT source root, relative to the workspace root the bin is exec'd from.
const SUT_SRC: &str = "crates/behaviorpass/src";

fn main() {
    let loc = code_loc(Path::new(SUT_SRC));
    println!("CODE_LOC={loc}");
    if loc == 0 {
        // Empty scaffold (no machinery ported yet) — nothing to score. The
        // trace-equality gate reverts an empty SUT anyway; a real experiment
        // that adds working machinery scores > 0.
        println!("SCORE=0");
        return;
    }
    #[allow(clippy::cast_precision_loss)]
    let score = K / loc as f64;
    println!("SCORE={score:.6}");
}

/// Total code-only lines across every `.rs` file under `root`.
fn code_loc(root: &Path) -> usize {
    let mut total = 0;
    walk(root, &mut |path| {
        if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(src) = fs::read_to_string(path) {
                total += count_code_lines(&src);
            }
        }
    });
    total
}

/// Non-blank, non-comment-only lines. A line whose first non-whitespace runs
/// with `//` or `*` (block-comment body) or `/*` is treated as comment-only.
fn count_code_lines(src: &str) -> usize {
    src.lines()
        .filter(|line| {
            let t = line.trim_start();
            !t.is_empty()
                && !t.starts_with("//")
                && !t.starts_with('*')
                && !t.starts_with("/*")
        })
        .count()
}

/// Depth-first walk applying `f` to every file under `dir`.
fn walk(dir: &Path, f: &mut impl FnMut(&Path)) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, f);
        } else {
            f(&path);
        }
    }
}
