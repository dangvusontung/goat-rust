//! Domain-forked RNG streams for the calendar.
//!
//! The calendar draws from a stream that is independent of the match engine,
//! transfer engine, and injury engine. Forking ensures that "play vs skip a match"
//! does not shift any other system's random rolls (§9 — determinism is sacred).
//!
//! Usage:
//! ```ignore
//! let mut root = RngStream::new(save_seed);
//! let calendar_rng = root.fork("calendar");  // -> GoatRng, independent
//! let match_rng    = root.fork("match");     // -> GoatRng, independent
//! ```
//! Fork order is ABI: always derive streams in the same order so the root
//! advances identically across code paths.

use goat_rng::{GoatRng, RngSource};

/// An RNG stream that can produce independent child streams per domain.
pub struct RngStream(GoatRng);

impl RngStream {
    pub fn new(seed: u64) -> Self {
        RngStream(GoatRng::new(seed))
    }

    /// Derive an independent child `GoatRng` for `domain`.
    ///
    /// Mixes the stream's next value with a stable FNV-1a hash of the domain
    /// name, so two different domains always produce different child seeds even
    /// when called in sequence from the same root.
    pub fn fork(&mut self, domain: &str) -> GoatRng {
        let base = self.0.next_u64();
        GoatRng::new(base ^ fnv1a_u64(domain))
    }
}

impl RngSource for RngStream {
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
}

/// FNV-1a 64-bit hash — no alloc, no external crates.
fn fnv1a_u64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_fork_sequence() {
        let mut a = RngStream::new(42);
        let mut b = RngStream::new(42);
        let fa = a.fork("calendar");
        let fb = b.fork("calendar");
        assert_eq!(fa.state(), fb.state());
    }

    #[test]
    fn different_domains_produce_different_seeds() {
        let mut s = RngStream::new(1);
        // Consume one value to simulate having already forked once.
        let cal = s.fork("calendar");
        let mut s2 = RngStream::new(1);
        let mat = s2.fork("match");
        // calendar and match start from the same root but fnv hash differs.
        assert_ne!(cal.state(), mat.state());
    }

    #[test]
    fn forked_stream_is_independent() {
        let mut root = RngStream::new(99);
        let mut cal = root.fork("calendar");
        let mut mat = root.fork("match");
        // Drawing from cal must not affect mat's sequence.
        for _ in 0..100 {
            cal.next_u64();
        }
        let mut root2 = RngStream::new(99);
        let _ = root2.fork("calendar");
        let mut mat2 = root2.fork("match");
        assert_eq!(mat.next_u64(), mat2.next_u64());
    }
}
