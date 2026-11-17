//! Hashing a large buffer as fast as memory allows.

use std::time::Instant;

use rayon::prelude::*;

use super::{Bench, Outcome, Progress};
use crate::rng::Rng;

pub const BENCH: Bench = Bench {
    name: "Hashing",
    about: "Hash 4 GiB of data (a 512 MiB buffer, 8 passes) with a 4-lane 64-bit multiply-rotate hash.",
    unit: "bytes",
    run,
};

const BUF: usize = 512 << 20;
const PASSES: usize = 8;
const CHUNK: usize = 1 << 20;

fn run(progress: &Progress) -> Outcome {
    let mut data = vec![0u8; BUF];
    data.par_chunks_mut(CHUNK).enumerate().for_each(|(i, c)| {
        let mut rng = Rng::new(i as u64);
        for w in c.chunks_exact_mut(8) {
            w.copy_from_slice(&rng.next_u64().to_le_bytes());
        }
    });
    progress.start((PASSES * BUF / CHUNK) as u64);
    let start = Instant::now();
    let mut digest = 0u64;
    for pass in 0..PASSES {
        digest ^= hash(&data, pass as u64, Some(progress));
    }
    let elapsed = start.elapsed();
    Outcome {
        elapsed,
        ops: (BUF * PASSES) as f64,
        detail: format!("hashed {} GiB, digest {digest:016x}", (BUF * PASSES) >> 30),
        check: digest,
    }
}

/// Hashes 1 MiB chunks independently (in parallel), then combines them in order,
/// so the result does not depend on the number of threads.
pub fn hash(data: &[u8], seed: u64, progress: Option<&Progress>) -> u64 {
    let parts: Vec<u64> = data
        .par_chunks(CHUNK)
        .map(|c| {
            let h = hash_chunk(c, seed);
            if let Some(p) = progress {
                p.add(1);
            }
            h
        })
        .collect();
    parts.iter().fold(seed, |h, &p| mix(h ^ p))
}

const K: u64 = 0x9E37_79B9_7F4A_7C15;

#[inline]
fn mix(h: u64) -> u64 {
    (h ^ (h >> 29)).wrapping_mul(K).rotate_left(27)
}

fn hash_chunk(c: &[u8], seed: u64) -> u64 {
    let mut lanes = [seed ^ 1, seed ^ 2, seed ^ 3, seed ^ 4];
    let blocks = c.chunks_exact(32);
    let tail = blocks.remainder();
    for b in blocks {
        for (k, lane) in lanes.iter_mut().enumerate() {
            let w = u64::from_le_bytes(b[k * 8..k * 8 + 8].try_into().unwrap());
            *lane = (*lane ^ w).wrapping_mul(K).rotate_left(31);
        }
    }
    let mut h = lanes.iter().fold(c.len() as u64, |h, &l| mix(h ^ l));
    for &byte in tail {
        h = mix(h ^ byte as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_sensitive() {
        let mut data = vec![7u8; 3 * CHUNK + 13];
        let a = hash(&data, 0, None);
        assert_eq!(a, hash(&data, 0, None));
        data[2 * CHUNK + 5] ^= 1;
        assert_ne!(a, hash(&data, 0, None));
        let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
        assert_eq!(hash(&data, 0, None), pool.install(|| hash(&data, 0, None)));
    }
}
