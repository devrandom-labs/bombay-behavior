//! FITNESS FUNCTION for the behaviorpass PERF loop (Supervising children
//! mechanism). Claude-owned + FROZEN: the loop optimizes `src/supervising.rs`,
//! this measures the result. It touches Supervising ONLY through its public
//! constructor + the generic `Behavior::step` — never a bespoke getter — so it
//! stays valid whatever internal liveness representation the loop chooses.
//!
//! PRIMARY metric = SPACE: bytes a counting allocator sees while constructing a
//! supervisor over a spread of child counts, plus the struct's own footprint.
//! Smaller ⇒ higher score. `Vec<Child>` (1 byte/child + a 24-byte control
//! block) is the naive floor; a bit-packed / inline representation collapses it.
//!
//! SECONDARY (info only, no-regress) = the `step(ChildStopped)` hot-path
//! throughput, so a space win that wrecks the fold shows up.

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::{Duration, Instant};

use behaviorpass::{Actions, Base, Behavior, Crash, Envelope, MailAddr, RestartPolicy, Strategy, Supervising};
use behaviorpass::Never;
use tokio::time::Instant as TokioInstant;

// A pass-through allocator that counts bytes handed out. The measured regions
// below are single-threaded; `Relaxed` is sound because each read happens-after
// the allocations it sums within the same thread's program order.
struct Counting;
static BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        BYTES.fetch_add(l.size(), Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

type Inner = Base<MailAddr, (), u64, Never, &'static str, Never, Kid>;
type Kid = Base<MailAddr, u32, u32, Never, &'static str>;
type Sup = Supervising<Inner, Kid>;

fn inner() -> Inner {
    Base::new((), |(): &mut (), _: u64| {
        Ok::<Actions<MailAddr, Never, Never, Kid>, &'static str>(Actions::cont())
    })
}

fn kid() -> Kid {
    Base::new(0_u32, |c: &mut u32, n: u32| {
        *c += n;
        Ok::<Actions<MailAddr, Never, Never, Never>, &'static str>(Actions::cont())
    })
}

/// Bytes the allocator sees for exactly `Supervising::new(.., n, ..)` — `inner`
/// is built beforehand and moved in, so the delta is the children table alone.
fn construct_bytes(n: usize) -> usize {
    let seed = inner();
    let before = BYTES.load(Relaxed);
    let sup = Supervising::new(seed, |i| i as u64, n, |_| kid(), Strategy::OneForOne, RestartPolicy::Transient, u32::MAX, Duration::MAX);
    let after = BYTES.load(Relaxed);
    black_box(&sup);
    after - before
}

fn main() {
    let counts = [0_usize, 1, 8, 64, 1024];
    let alloc_bytes: usize = counts.iter().map(|&n| construct_bytes(n)).sum();
    let struct_size = std::mem::size_of::<Sup>();
    let space = alloc_bytes + struct_size;

    // Space is the target; guard the hot path so a space win can't gut the fold.
    let rt = tokio::runtime::Builder::new_current_thread().build().expect("rt");
    let mut sup = Supervising::new(inner(), |i| i as u64, 64, |_| kid(), Strategy::OneForOne, RestartPolicy::Transient, u32::MAX, Duration::MAX);
    let iters = 200_000_u64;
    let at = TokioInstant::now();
    let t = Instant::now();
    rt.block_on(async {
        for i in 0..iters {
            let idx = (i % 64) as usize;
            let a = sup.step(Envelope::ChildStopped { nonce: idx as u64, outcome: Err(Crash::Failed), at }).await.expect("ok");
            black_box(a);
        }
    });
    let secs = t.elapsed().as_secs_f64();
    let throughput = f64::from(u32::try_from(iters).unwrap_or(u32::MAX)) / secs;

    // MAXIMIZE: smaller footprint ⇒ larger score. A compile/run failure never
    // reaches here — measure.sh emits score=0 and the loop auto-reverts.
    let score = 1_000_000.0 / (1.0 + space as f64);
    println!("METRIC score={score:.4}");
    println!("info space_bytes={space} alloc_bytes={alloc_bytes} struct_size={struct_size}");
    println!("info step_throughput_per_s={throughput:.0}");
}
