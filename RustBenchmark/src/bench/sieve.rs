//! Segmented, odd-only Sieve of Eratosthenes.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::fmt;

pub const BENCH: Bench = Bench {
    name: "Prime sieve",
    about: "Find every prime below 2 billion (segmented Sieve of Eratosthenes, odd numbers only).",
    unit: "numbers",
    run,
};

const LIMIT: u64 = 2_000_000_000;
/// Odd numbers per segment: 256 KiB of flags, which fits in L2 cache.
const SEG: usize = 1 << 18;

fn run(progress: &Progress) -> Outcome {
    let start = Instant::now();
    let primes = count_primes(LIMIT, Some(progress));
    let elapsed = start.elapsed();
    Outcome {
        elapsed,
        ops: LIMIT as f64,
        detail: format!("found {} primes below {}", fmt::grouped(primes), fmt::grouped(LIMIT)),
        check: primes,
    }
}

/// Odd primes up to and including `n`.
fn small_odd_primes(n: u64) -> Vec<u64> {
    let n = n as usize;
    let mut composite = vec![false; n + 1];
    let mut out = Vec::new();
    let mut p = 3;
    while p <= n {
        if !composite[p] {
            out.push(p as u64);
            let mut m = p * p;
            while m <= n {
                composite[m] = true;
                m += 2 * p;
            }
        }
        p += 2;
    }
    out
}

/// Number of primes `<= limit`.
pub fn count_primes(limit: u64, progress: Option<&Progress>) -> u64 {
    if limit < 2 {
        return 0;
    }
    let mut root = (limit as f64).sqrt() as u64;
    while root * root > limit {
        root -= 1;
    }
    while (root + 1) * (root + 1) <= limit {
        root += 1;
    }
    let base = small_odd_primes(root);
    let span = 2 * SEG as u64;
    let segments = limit / span + 1;
    if let Some(p) = progress {
        p.start(segments);
    }

    let odd: u64 = (0..segments)
        .into_par_iter()
        .map_init(
            || vec![0u8; SEG],
            |flags, s| {
                // Flag j stands for the odd number lo + 2j + 1.
                let lo = s * span;
                let hi = lo + span;
                flags.fill(1);
                for &p in &base {
                    if p * p >= hi {
                        break;
                    }
                    let mut m = (lo + 1).div_ceil(p) * p;
                    if m % 2 == 0 {
                        m += p;
                    }
                    m = m.max(p * p);
                    let mut j = ((m - lo - 1) / 2) as usize;
                    let p = p as usize;
                    while j < SEG {
                        flags[j] = 0;
                        j += p;
                    }
                }
                if s == 0 {
                    flags[0] = 0; // 1 is not prime
                }
                let valid = if limit < lo + 1 {
                    0
                } else {
                    (((limit - lo - 1) / 2 + 1) as usize).min(SEG)
                };
                let count = flags[..valid].iter().map(|&f| f as u64).sum::<u64>();
                if let Some(p) = progress {
                    p.add(1);
                }
                count
            },
        )
        .sum();
    odd + 1 // the prime 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_prime_counts() {
        assert_eq!(count_primes(1, None), 0);
        assert_eq!(count_primes(2, None), 1);
        assert_eq!(count_primes(10, None), 4);
        assert_eq!(count_primes(1_000_000, None), 78_498);
        assert_eq!(count_primes(10_000_000, None), 664_579);
        // Crosses many segment boundaries.
        assert_eq!(count_primes(100_000_000, None), 5_761_455);
    }
}
