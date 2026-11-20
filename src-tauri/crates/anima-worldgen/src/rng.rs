//! Domain-separated deterministic PRNG derivation for World v2 (`ChaCha12Rng`).

use rand_chacha::{rand_core::SeedableRng, ChaCha12Rng};
use sha2::{Digest, Sha256};

const DOMAIN_SEPARATOR: &[u8] = b"ANIMA-WV2";

/// Computes a 32-byte SHA-256 stage seed bound to the world seed, dimensions, schema tag, and stage tag.
#[must_use]
pub fn stage_seed(
    seed_u64: u64,
    width: u32,
    height: u32,
    schema_tag: &str,
    stage_tag: &str,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(seed_u64.to_le_bytes());
    hasher.update(width.to_le_bytes());
    hasher.update(height.to_le_bytes());
    hasher.update((schema_tag.len() as u32).to_le_bytes());
    hasher.update(schema_tag.as_bytes());
    hasher.update((stage_tag.len() as u32).to_le_bytes());
    hasher.update(stage_tag.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// Derives the `ChaCha12Rng` stream for an entire pipeline stage.
#[must_use]
pub fn stage_rng(
    seed_u64: u64,
    width: u32,
    height: u32,
    schema_tag: &str,
    stage_tag: &str,
) -> ChaCha12Rng {
    ChaCha12Rng::from_seed(stage_seed(seed_u64, width, height, schema_tag, stage_tag))
}

/// Derives an independent `ChaCha12Rng` for a single grid cell `(row, col)` from a stage seed.
#[must_use]
pub fn cell_rng(stage_seed_bytes: &[u8; 32], row: u32, col: u32) -> ChaCha12Rng {
    let mut hasher = Sha256::new();
    hasher.update(stage_seed_bytes);
    hasher.update(row.to_le_bytes());
    hasher.update(col.to_le_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    ChaCha12Rng::from_seed(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn stage_and_cell_rngs_are_reproducible_and_domain_separated() {
        let mut r1 = stage_rng(42, 256, 256, "v3", "S1");
        let mut r2 = stage_rng(42, 256, 256, "v3", "S1");
        let mut r3 = stage_rng(42, 256, 256, "v3", "S2");
        assert_eq!(r1.next_u64(), r2.next_u64());
        assert_ne!(r1.next_u64(), r3.next_u64());

        let s = stage_seed(42, 256, 256, "v3", "S1");
        let mut c01 = cell_rng(&s, 0, 1);
        let mut c10 = cell_rng(&s, 1, 0);
        assert_ne!(c01.next_u64(), c10.next_u64());
    }
}
