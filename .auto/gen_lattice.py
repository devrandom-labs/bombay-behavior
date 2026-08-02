#!/usr/bin/env python3
"""Generate the legal cap-set lattice point-binaries for the behaviorpass
monomorphization-slope harness (bombay card #298).

Core capabilities: D=Deadlined W=Watching Sup=Supervising St=Stashing.
(`Phased` left core — it's the `Fsm` helper / nexus aggregate, not a capability.)
Law: Sup => W. Canonical nesting (outer->inner): Sup > W > D > St > Base.
All points fold u64 over channel::<Never,u64>.
"""
import itertools
import os

CAPS = ["Sup", "W", "D", "St"]
OUT = os.path.expanduser("~/Code/devrandom/behaviorpass/crates/behaviorpass/examples")


def legal(s):
    return not ("Sup" in s and "W" not in s)


def name(s):
    if not s:
        return "p00_plain"
    order = [c for c in ["Sup", "W", "D", "St"] if c in s]
    return "p_" + "_".join(c.lower() for c in order)


def build_expr(s):
    inner = "base()"
    if "St" in s:
        inner = f"Stashing::new({inner}, |_: &u64| StashRoute::Deliver)"
    if "D" in s:
        inner = f"Deadlined::new({inner}, None, |_| Ok(Step::Continue))"
    if "W" in s:
        inner = f"Watching::new({inner}, otp_propagation)"
    if "Sup" in s:
        inner = f"Supervising::new({inner}, vec![base()], |_| base(), 3)"
    return inner


def imports(s):
    items = {"Base", "Exit", "run"}
    if "D" in s:
        items.add("Deadlined")
    if "W" in s:
        items |= {"Watching", "otp_propagation"}
    if "Sup" in s:
        items.add("Supervising")
    if "St" in s:
        items |= {"Stashing", "StashRoute"}
    return ", ".join(sorted(items))


def file_for(s):
    caps = " ".join(sorted(s)) or "(none)"
    return f"""//! Slope point — caps: {caps}. Generated (bombay card #298).
use behaviorpass::{{{imports(s)}}};
use bombay::capability::{{Never, Step}};
use fastpass::{{Config, channel}};

fn base() -> Base<u64, u64, Never, &'static str> {{
    Base::new(0, |s: &mut u64, m: u64| {{
        *s += m;
        Ok::<Step<Never, Exit>, &'static str>(if *s > 1000 {{ Step::Stop(Exit::Normal) }} else {{ Step::Continue }})
    }})
}}

#[tokio::main(flavor = "current_thread")]
async fn main() {{
    let (ctl, usr, rx) = channel::<Never, u64>(Config::new(8));
    let stack = {build_expr(s)};
    let handle = tokio::spawn(run(stack, rx));
    let _ = usr.send(1).await;
    drop((usr, ctl));
    let _ = handle.await;
}}
"""


for f in os.listdir(OUT):
    if f.startswith("p") and f.endswith(".rs"):
        os.remove(os.path.join(OUT, f))

points = []
for r in range(len(CAPS) + 1):
    for combo in itertools.combinations(CAPS, r):
        s = set(combo)
        if legal(s):
            points.append(s)

points.sort(key=lambda s: (len(s), name(s)))
for s in points:
    with open(os.path.join(OUT, name(s) + ".rs"), "w") as fh:
        fh.write(file_for(s))
    print(name(s), "->", sorted(s))
print(f"\n{len(points)} legal points generated")
