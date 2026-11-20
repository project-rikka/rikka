use std::time::{Duration, Instant};

use sha1::{Digest, Sha1};

const CHALLENGE_LIFETIME: Duration = Duration::from_secs(5);
const TARGET: std::ops::Range<usize> = 12..20;

pub const SUFFIX_LEN: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Solution {
    pub suffix: [u8; SUFFIX_LEN],
    pub elapsed: Duration,
}

#[must_use]
pub fn solve(context: &[u8], prefix: &[u8], zero_bits: u32) -> Option<Solution> {
    let started = Instant::now();
    let target = tail_word(&Sha1::digest(context));
    let mut counter = 0u64;
    while started.elapsed() < CHALLENGE_LIFETIME {
        let mut suffix = [0u8; SUFFIX_LEN];
        suffix[..8].copy_from_slice(&target.wrapping_add(counter).to_be_bytes());
        suffix[8..].copy_from_slice(&counter.to_be_bytes());
        let digest = Sha1::new()
            .chain_update(prefix)
            .chain_update(suffix)
            .finalize();
        if tail_word(&digest).trailing_zeros() >= zero_bits {
            return Some(Solution {
                suffix,
                elapsed: started.elapsed(),
            });
        }
        counter += 1;
    }
    None
}

fn tail_word(digest: &[u8]) -> u64 {
    u64::from_be_bytes(
        digest[TARGET]
            .try_into()
            .expect("a SHA-1 digest has twenty bytes"),
    )
}
