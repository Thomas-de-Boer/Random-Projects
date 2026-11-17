//! Monte Carlo estimate of π.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::fmt;
use crate::rng::Rng;

pub const BENCH: Bench = Bench {
    name: "Monte Carlo π",
    about: "Throw 1 billion random darts at a square and count how many land inside the circle.",
    unit: "samples",
    run,
};

const SAMPLES: u64 = 1_000_000_000;
const CHUNKS: u64 = 1000;

fn run(progress: &Progress) -> Outcome {
    progress.start(CHUNKS);
    let start = Instant::now();
    let inside = estimate(SAMPLES, CHUNKS, Some(progress));
    let elapsed = start.elapsed();
    let pi = 4.0 * inside as f64 / SAMPLES as f64;
    Outcome {
        elapsed,
        ops: SAMPLES as f64,
        detail: format!(
            "{} darts: π ≈ {pi:.6} (off by {:.1e})",
            fmt::count(SAMPLES as f64),
            (pi - std::f64::consts::PI).abs()
        ),
        check: inside,
    }
}

/// How many of `samples` random points land inside the unit quarter circle.
pub fn estimate(samples: u64, chunks: u64, progress: Option<&Progress>) -> u64 {
    let per = samples / chunks;
    (0..chunks)
        .into_par_iter()
        .map(|c| {
            let mut rng = Rng::new(c);
            let mut inside = 0u64;
            for _ in 0..per {
                let r = rng.next_u64();
                // Two 31-bit coordinates from one random number, integer math only:
                // the dart is inside the quarter circle when x² + y² < (2^31)².
                let x = r >> 33;
                let y = r & 0x7FFF_FFFF;
                inside += (x * x + y * y < 1 << 62) as u64;
            }
            if let Some(p) = progress {
                p.add(1);
            }
            inside
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_to_pi() {
        let n = 10_000_000;
        let pi = 4.0 * estimate(n, 100, None) as f64 / n as f64;
        assert!((pi - std::f64::consts::PI).abs() < 0.01, "{pi}");
    }
}
