//! Sorting tens of millions of random 64-bit integers.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::fmt;
use crate::rng::Rng;

pub const BENCH: Bench = Bench {
    name: "Sort",
    about: "Sort 25 million random 64-bit integers in place (pattern-defeating quicksort).",
    unit: "elements",
    run,
};

const N: usize = 25_000_000;

fn run(progress: &Progress) -> Outcome {
    progress.start(1);
    let mut data = random_data(N, 1);
    let start = Instant::now();
    data.par_sort_unstable();
    let elapsed = start.elapsed();
    progress.add(1);
    assert!(data.windows(2).all(|w| w[0] <= w[1]));
    Outcome {
        elapsed,
        ops: N as f64,
        detail: format!("sorted {} numbers ({} MiB)", fmt::grouped(N as u64), (N * 8) >> 20),
        check: data[N / 2],
    }
}

/// Deterministic random data, generated in parallel (not part of the timing).
pub fn random_data(n: usize, seed: u64) -> Vec<u64> {
    let mut v = vec![0u64; n];
    v.par_chunks_mut(1 << 16).enumerate().for_each(|(i, chunk)| {
        let mut rng = Rng::new(seed.wrapping_mul(1_000_003) + i as u64);
        chunk.iter_mut().for_each(|x| *x = rng.next_u64());
    });
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts() {
        let mut v = random_data(200_000, 3);
        let mut expect = v.clone();
        expect.sort();
        v.par_sort_unstable();
        assert_eq!(v, expect);
    }
}
