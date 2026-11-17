//! The benchmark suite: each test runs once on one core and once on all cores.

pub mod hash;
pub mod matmul;
pub mod nbody;
pub mod pi;
pub mod sieve;
pub mod sort;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;
use std::time::Duration;

/// Work-done counter shared between a running benchmark and the UI.
#[derive(Default)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
}

impl Progress {
    pub fn start(&self, total: u64) {
        self.done.store(0, Ordering::Relaxed);
        self.total.store(total.max(1), Ordering::Relaxed);
    }
    pub fn add(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed).max(1);
        (self.done.load(Ordering::Relaxed) as f64 / total as f64).min(1.0) as f32
    }
}

#[derive(Clone, Debug)]
pub struct Outcome {
    /// Time spent on the measured work only (setup excluded).
    pub elapsed: Duration,
    /// Units of work done, in `Bench::unit`.
    pub ops: f64,
    /// One human sentence about what was computed.
    pub detail: String,
    /// A fingerprint of the answer, so single- and multi-core runs can be compared.
    pub check: u64,
}

pub struct Bench {
    pub name: &'static str,
    pub about: &'static str,
    /// What `Outcome::ops` counts, e.g. "numbers" or "FLOP".
    pub unit: &'static str,
    pub run: fn(&Progress) -> Outcome,
}

pub fn all() -> Vec<Bench> {
    vec![
        sieve::BENCH,
        sort::BENCH,
        matmul::BENCH,
        nbody::BENCH,
        hash::BENCH,
        pi::BENCH,
    ]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Single = 0,
    All = 1,
}

pub enum Msg {
    Started(usize, Mode),
    Done(usize, Mode, Outcome),
    Finished,
}

/// Runs the chosen benchmarks on a background thread, so the UI never stalls.
pub fn spawn_runner(which: Vec<usize>, progress: Arc<Progress>) -> Receiver<Msg> {
    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
    thread::Builder::new()
        .name("bench-runner".into())
        .spawn(move || {
            let benches = all();
            let single = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
            let every = rayon::ThreadPoolBuilder::new().build().unwrap();
            for i in which {
                for mode in [Mode::Single, Mode::All] {
                    if tx.send(Msg::Started(i, mode)).is_err() {
                        return;
                    }
                    progress.start(1);
                    let pool = if mode == Mode::Single { &single } else { &every };
                    let out = pool.install(|| (benches[i].run)(&progress));
                    let _ = tx.send(Msg::Done(i, mode, out));
                }
            }
            let _ = tx.send(Msg::Finished);
        })
        .expect("spawn benchmark thread");
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Prints the whole suite's timings: `cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn suite_timings() {
        let rx = spawn_runner((0..all().len()).collect(), Arc::new(Progress::default()));
        let benches = all();
        let mut single = [None, None, None, None, None, None];
        for msg in rx {
            if let Msg::Done(i, mode, o) = msg {
                let b = &benches[i];
                let rate = o.ops / o.elapsed.as_secs_f64();
                let mut line = format!(
                    "{:<16} {:<6}  {:>9}  {:>24}  {}",
                    b.name,
                    format!("{mode:?}"),
                    crate::fmt::duration(o.elapsed),
                    crate::fmt::throughput(rate, b.unit),
                    o.detail
                );
                match mode {
                    Mode::Single => single[i] = Some(o),
                    Mode::All => {
                        let s = single[i].as_ref().unwrap();
                        assert_eq!(s.check, o.check, "{} differs between modes", b.name);
                        line += &format!("  ({:.1}x)", s.elapsed.as_secs_f64() / o.elapsed.as_secs_f64());
                    }
                }
                println!("{line}");
            }
        }
    }
}
