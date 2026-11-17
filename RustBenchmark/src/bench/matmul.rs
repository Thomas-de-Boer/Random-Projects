//! Dense matrix multiplication in double precision.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::rng::Rng;

pub const BENCH: Bench = Bench {
    name: "Matrix multiply",
    about: "Multiply two 1536×1536 matrices of f64 (B transposed, 8-wide SIMD-friendly dot products).",
    unit: "FLOP",
    run,
};

const N: usize = 1536;

fn run(progress: &Progress) -> Outcome {
    let a = random_matrix(N, 1);
    let b = random_matrix(N, 2);
    progress.start(N as u64);
    let start = Instant::now();
    let c = multiply(&a, &b, N, Some(progress));
    let elapsed = start.elapsed();
    let flops = 2.0 * (N as f64).powi(3);
    let check = c.iter().fold(0u64, |h, v| h.rotate_left(5) ^ v.to_bits());
    Outcome {
        elapsed,
        ops: flops,
        detail: format!("{N}×{N} by {N}×{N}: {:.1} billion floating-point operations", flops / 1e9),
        check,
    }
}

fn random_matrix(n: usize, seed: u64) -> Vec<f64> {
    let mut rng = Rng::new(seed);
    (0..n * n).map(|_| rng.f64() * 2.0 - 1.0).collect()
}

/// C = A·B for row-major `n`×`n` matrices.
pub fn multiply(a: &[f64], b: &[f64], n: usize, progress: Option<&Progress>) -> Vec<f64> {
    // Transposing B makes both operands of every dot product contiguous in memory.
    let mut bt = vec![0.0; n * n];
    bt.par_chunks_mut(n).enumerate().for_each(|(j, col)| {
        for (k, v) in col.iter_mut().enumerate() {
            *v = b[k * n + j];
        }
    });
    let mut c = vec![0.0; n * n];
    c.par_chunks_mut(n).enumerate().for_each(|(i, row)| {
        let ar = &a[i * n..][..n];
        for (j, out) in row.iter_mut().enumerate() {
            *out = dot(ar, &bt[j * n..][..n]);
        }
        if let Some(p) = progress {
            p.add(1);
        }
    });
    c
}

#[inline]
fn dot(a: &[f64], b: &[f64]) -> f64 {
    // Eight independent accumulators let the compiler use SIMD registers.
    let mut acc = [0.0f64; 8];
    let ca = a.chunks_exact(8);
    let cb = b.chunks_exact(8);
    let tail: f64 = ca.remainder().iter().zip(cb.remainder()).map(|(x, y)| x * y).sum();
    for (x, y) in ca.zip(cb) {
        for k in 0..8 {
            acc[k] += x[k] * y[k];
        }
    }
    acc.iter().sum::<f64>() + tail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_naive() {
        let n = 67;
        let a = random_matrix(n, 5);
        let b = random_matrix(n, 6);
        let c = multiply(&a, &b, n, None);
        for i in 0..n {
            for j in 0..n {
                let naive: f64 = (0..n).map(|k| a[i * n + k] * b[k * n + j]).sum();
                assert!((c[i * n + j] - naive).abs() < 1e-9);
            }
        }
    }
}
