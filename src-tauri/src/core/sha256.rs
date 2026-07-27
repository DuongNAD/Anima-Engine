//! SHA-256, so an artifact can carry a checksum a reader can recompute with any tool.
//!
//! The engine already fingerprints things with FNV-1a ([`crate::core::world_artifact`]) and with the
//! bespoke hashes behind `ExperimentManifest::fingerprint`. Neither is verifiable from outside this
//! repository: a reader handed `paired-report.json` and a 64-bit FNV value has to trust our code to
//! check it. A published experiment result needs the other property — `sha256sum`, `Get-FileHash`,
//! `certutil` and `node:crypto` all agree on the answer independently of us.
//!
//! Written out rather than pulled in as `sha2`: this is eighty lines of a fully specified standard
//! (FIPS 180-4), it is exercised against the published vectors below, and adding a dependency for it
//! would move `NOTICE`, the SBOM and the audit surface for a hash the repo already computes in Node.
//! `scripts/verify_e2_artifacts.mjs` re-derives the same checksums with `node:crypto`, so the two
//! implementations check each other rather than this one checking itself.

/// The 64 round constants: the first 32 bits of the fractional parts of the cube roots of the first
/// 64 primes (FIPS 180-4 §4.2.2).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// The initial hash value: the first 32 bits of the fractional parts of the square roots of the
/// first 8 primes (FIPS 180-4 §5.3.3).
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// The SHA-256 digest of `data`, as 32 raw bytes.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = H0;

    // Padding: a 1 bit, then zeros to 56 mod 64, then the length in bits, big-endian.
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(data.len() + 72);
    padded.extend_from_slice(data);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    let mut w = [0u32; 64];
    for chunk in padded.chunks_exact(64) {
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (dst, src) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *dst = dst.wrapping_add(src);
        }
    }

    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The SHA-256 digest of `data` as lowercase hex — the form `sha256sum` prints and every other tool
/// accepts.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for byte in sha256(data) {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// The SHA-256 of a file's exact bytes.
pub fn sha256_file(path: &std::path::Path) -> std::io::Result<String> {
    Ok(sha256_hex(&std::fs::read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The published FIPS 180-4 / NIST CAVP vectors. A hash function that agrees with itself proves
    /// nothing; these are the numbers the rest of the world produces.
    #[test]
    fn the_published_vectors_come_out_right() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "the empty string"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "FIPS 180-4 one-block message"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            "FIPS 180-4 two-block message"
        );
    }

    /// Length handling is where a hand-written SHA-256 goes wrong, so the boundaries are checked
    /// explicitly: one byte short of a padding block, exactly at it, and one past.
    #[test]
    fn the_padding_boundaries_are_right() {
        let a = |n: usize| sha256_hex(&vec![b'a'; n]);
        assert_eq!(
            a(55),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318",
            "55 bytes: the last message that fits in one block with its padding"
        );
        assert_eq!(
            a(56),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a",
            "56 bytes: forces a second block"
        );
        assert_eq!(
            a(64),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb",
            "64 bytes: exactly one block of message"
        );
        assert_eq!(
            a(1_000_000),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0",
            "the million-'a' vector"
        );
    }

    #[test]
    fn a_single_flipped_bit_changes_the_digest() {
        // Otherwise every assertion above could pass on a function that ignores most of its input.
        assert_ne!(sha256_hex(b"anima"), sha256_hex(b"animb"));
        assert_ne!(sha256_hex(&[0u8; 32]), sha256_hex(&[0u8; 33]));
    }
}
