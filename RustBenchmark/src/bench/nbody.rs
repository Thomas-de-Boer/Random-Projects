//! All-pairs gravitational N-body simulation.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::fmt;
use crate::rng::Rng;

pub const BENCH: Bench = Bench {
    name: "N-body gravity",
    about: "Simulate 16,384 stars pulling on each other: every pair, every step (O(n²)).",
    unit: "interactions",
    run,
};

const N: usize = 16_384;
const STEPS: usize = 3;
const LANES: usize = 8;

pub struct Bodies {
    pub x: Vec<f32>,
    pub y: Vec<f32>,
    pub z: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub vz: Vec<f32>,
    pub m: Vec<f32>,
}

fn run(progress: &Progress) -> Outcome {
    let mut b = galaxy(N);
    progress.start((N * STEPS) as u64);
    let start = Instant::now();
    for _ in 0..STEPS {
        step(&mut b, 0.01, Some(progress));
    }
    let elapsed = start.elapsed();
    let interactions = (N * N * STEPS) as f64;
    let check = b.x.iter().chain(&b.y).fold(0u64, |h, v| h.rotate_left(7) ^ v.to_bits() as u64);
    Outcome {
        elapsed,
        ops: interactions,
        detail: format!(
            "{} steps × {}² bodies = {} force calculations",
            STEPS,
            fmt::grouped(N as u64),
            fmt::count(interactions)
        ),
        check,
    }
}

pub fn galaxy(n: usize) -> Bodies {
    let mut rng = Rng::new(9);
    let mut b = Bodies {
        x: vec![],
        y: vec![],
        z: vec![],
        vx: vec![],
        vy: vec![],
        vz: vec![],
        m: vec![],
    };
    for _ in 0..n {
        let a = rng.f32() * std::f32::consts::TAU;
        let r = 0.1 + rng.f32() * 10.0;
        b.x.push(a.cos() * r);
        b.y.push(a.sin() * r);
        b.z.push((rng.f32() - 0.5) * 0.5);
        b.vx.push(-a.sin() * 0.3);
        b.vy.push(a.cos() * 0.3);
        b.vz.push(0.0);
        b.m.push(1.0 / n as f32);
    }
    b
}

/// One step: compute every body's acceleration in parallel, then move everything.
pub fn step(b: &mut Bodies, dt: f32, progress: Option<&Progress>) {
    let n = b.x.len();
    assert_eq!(n % LANES, 0);
    let (x, y, z, m) = (&b.x, &b.y, &b.z, &b.m);
    let acc: Vec<[f32; 3]> = (0..n)
        .into_par_iter()
        .with_min_len(32)
        .map(|i| {
            let (xi, yi, zi) = (x[i], y[i], z[i]);
            // Eight independent lanes so the inner loop becomes SIMD.
            let mut ax = [0.0f32; LANES];
            let mut ay = [0.0f32; LANES];
            let mut az = [0.0f32; LANES];
            let chunks = x
                .chunks_exact(LANES)
                .zip(y.chunks_exact(LANES))
                .zip(z.chunks_exact(LANES))
                .zip(m.chunks_exact(LANES));
            for (((cx, cy), cz), cm) in chunks {
                for k in 0..LANES {
                    let dx = cx[k] - xi;
                    let dy = cy[k] - yi;
                    let dz = cz[k] - zi;
                    let d2 = dx * dx + dy * dy + dz * dz + 1e-3;
                    let inv = 1.0 / d2.sqrt();
                    let s = cm[k] * inv * inv * inv;
                    ax[k] += dx * s;
                    ay[k] += dy * s;
                    az[k] += dz * s;
                }
            }
            if let Some(p) = progress {
                p.add(1);
            }
            [ax.iter().sum(), ay.iter().sum(), az.iter().sum()]
        })
        .collect();
    for (i, a) in acc.iter().enumerate() {
        b.vx[i] += a[0] * dt;
        b.vy[i] += a[1] * dt;
        b.vz[i] += a[2] * dt;
        b.x[i] += b.vx[i] * dt;
        b.y[i] += b.vy[i] * dt;
        b.z[i] += b.vz[i] * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn momentum_is_conserved() {
        let mut b = galaxy(64);
        for v in [&mut b.vx, &mut b.vy, &mut b.vz] {
            v.fill(0.0);
        }
        for _ in 0..20 {
            step(&mut b, 0.05, None);
        }
        // Equal masses and symmetric forces: total momentum stays ~0.
        let p: f32 = b.vx.iter().sum::<f32>().abs() + b.vy.iter().sum::<f32>().abs();
        assert!(p < 1e-3, "net momentum {p}");
        assert!(b.x.iter().all(|v| v.is_finite()));
    }
}
