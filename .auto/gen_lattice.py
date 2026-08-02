#!/usr/bin/env python3
"""Generate the 15 legal cap-set lattice point-binaries for the behaviorpass
monomorphization-slope harness (bombay card #298).

Caps: D=Deadlined W=Watching Sup=Supervising St=Stashing Ph=Phased.
Laws: Sup => W ; Ph excludes D and St.
Canonical nesting (outer->inner): Sup > W > D > St > Ph > Base.
All points fold u64 over channel::<Never,u64>; Phased points wrap a bool-phase
base (erased to Never), others a Never base.
"""
import itertools
import os

CAPS = ["Sup", "W", "D", "St", "Ph"]
OUT = os.path.expanduser("~/Code/devrandom/behaviorpass/crates/behaviorpass/examples")

def legal(s):
    if "Sup" in s and "W" not in s:
        return False
    if "Ph" in s and ("D" in s or "St" in s):
        return False
    return True

def name(s):
    if not s:
        return "p00_plain"
    order = [c for c in ["Sup", "W", "D", "St", "Ph"] if c in s]
    return "p_" + "_".join(c.lower() for c in order)

def build_expr(s):
    inner = "Phased::new(base_phase(), false, |_, _| Disposition::Deliver)" if "Ph" in s else "base()"
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
    if "D" in s: items.add("Deadlined")
    if "W" in s: items |= {"Watching", "otp_propagation"}
    if "Sup" in s: items.add("Supervising")
    if "St" in s: items |= {"Stashing", "StashRoute"}
    if "Ph" in s: items.add("Phased")
    return ", ".join(sorted(items))

def file_for(s):
    bn = (
        "\nfn base() -> Base<u64, u64, Never, &'static str> {\n"
        "    Base::new(0, |s: &mut u64, m: u64| {\n"
        "        *s += m;\n"
        "        Ok::<Step<Never, Exit>, &'static str>(if *s > 1000 { Step::Stop(Exit::Normal) } else { Step::Continue })\n"
        "    })\n"
        "}\n"
    ) if ("Sup" in s or "Ph" not in s) else ""
    bp = ""
    if "Ph" in s:
        bp = (
            "\nfn base_phase() -> Base<u64, u64, bool, &'static str> {\n"
            "    Base::new(0, |s: &mut u64, m: u64| {\n"
            "        *s += m;\n"
            "        Ok::<Step<bool, Exit>, &'static str>(if *s > 1000 { Step::Goto(true) } else { Step::Continue })\n"
            "    })\n"
            "}\n"
        )
    disp = "use bombay::capability::{Disposition, Never, Step};" if "Ph" in s \
        else "use bombay::capability::{Never, Step};"
    caps = " ".join(sorted(s)) or "(none)"
    return f"""//! Slope point — caps: {caps}. Generated (bombay card #298).
use behaviorpass::{{{imports(s)}}};
{disp}
use fastpass::{{Config, channel}};

{bn}{bp}
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

# Remove the hand-written 5 (regenerated under canonical names).
for f in ["plain", "deadlined", "watched", "supervised", "stack2", "stack3"]:
    p = os.path.join(OUT, f + ".rs")
    if os.path.exists(p):
        os.remove(p)

points = []
for r in range(len(CAPS) + 1):
    for combo in itertools.combinations(CAPS, r):
        s = set(combo)
        if legal(s):
            points.append(s)

points.sort(key=lambda s: (len(s), name(s)))
for s in points:
    fn = name(s) + ".rs"
    with open(os.path.join(OUT, fn), "w") as fh:
        fh.write(file_for(s))
    print(name(s), "->", sorted(s))
print(f"\n{len(points)} legal points generated")
