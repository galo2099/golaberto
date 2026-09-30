//! Optional allocation diagnostic; the production binary does not install it.
//! Counts requested Rust heap bytes using the same system allocator. CSV output
//! is elapsed milliseconds, live bytes, peak bytes; it excludes OS/thread stacks
//! and allocator overhead, so it is not a replacement for process RSS.
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
struct Tracked;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static DONE: AtomicBool = AtomicBool::new(false);
fn add(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}
unsafe impl GlobalAlloc for Tracked {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            add(l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            add(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        System.dealloc(p, l);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let p = System.realloc(p, l, n);
        if !p.is_null() {
            if n >= l.size() {
                add(n - l.size());
            } else {
                LIVE.fetch_sub(l.size() - n, Ordering::Relaxed);
            }
        }
        p
    }
}
#[global_allocator]
static ALLOC: Tracked = Tracked;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let request: golaberto_odds::model::Request =
        serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let start = Instant::now();
    let trace = std::thread::spawn(move || {
        let mut samples = Vec::with_capacity(5000);
        while !DONE.load(Ordering::Relaxed) {
            samples.push((
                start.elapsed().as_secs_f64() * 1000.,
                LIVE.load(Ordering::Relaxed),
                PEAK.load(Ordering::Relaxed),
            ));
            std::thread::sleep(Duration::from_millis(2));
        }
        samples
    });
    let (_, timing) = golaberto_odds::api::calculate(request, 808, 4, 20000).unwrap();
    DONE.store(true, Ordering::Relaxed);
    let trace = trace.join().unwrap();
    eprintln!(
        "profile-summary live={} peak={} timing={}",
        LIVE.load(Ordering::Relaxed),
        PEAK.load(Ordering::Relaxed),
        serde_json::to_string(&timing).unwrap()
    );
    for (t, l, p) in trace {
        println!("{t:.3},{l},{p}");
    }
}
