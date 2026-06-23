#![forbid(unsafe_code)]

//! Seeded, injectable RNG for the GOAT simulation.
//! All core randomness must flow through an `RngSource` — never use `rand::thread_rng()`
//! or any other external source inside core crates.

/// Injectable RNG source. Every core function that needs randomness accepts
/// `&mut impl RngSource`, making the RNG testable and determinism provable.
pub trait RngSource {
    /// Returns the next raw u64.
    fn next_u64(&mut self) -> u64;

    /// Returns a value uniformly distributed in `[lo, hi]` (inclusive both ends).
    ///
    /// Uses rejection sampling to eliminate modulo bias.
    fn next_range_u64(&mut self, lo: u64, hi: u64) -> u64 {
        debug_assert!(lo <= hi, "lo must be <= hi");
        let range = hi - lo + 1;
        // Largest multiple of `range` that fits in u64.
        let limit = u64::MAX - (u64::MAX % range);
        loop {
            let v = self.next_u64();
            if v <= limit {
                return lo + (v % range);
            }
        }
    }

    /// Returns a value uniformly in `[lo, hi]` inclusive, as `u8`.
    fn next_range_u8(&mut self, lo: u8, hi: u8) -> u8 {
        self.next_range_u64(lo as u64, hi as u64) as u8
    }

    /// Returns a value uniformly in `[lo, hi]` inclusive, as `u32`.
    fn next_range_u32(&mut self, lo: u32, hi: u32) -> u32 {
        self.next_range_u64(lo as u64, hi as u64) as u32
    }
}

/// Canonical non-zero seed used when the caller supplies 0.
/// Xorshift64 requires a non-zero state; this constant is arbitrary but fixed.
const NON_ZERO_SEED: u64 = 0x8a5cd789635d2dff;

/// Xorshift64 implementation of `RngSource`.
///
/// A zero seed is silently promoted to `NON_ZERO_SEED` so callers can freely
/// use 0 as "no seed / default" without breaking the invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoatRng {
    state: u64,
}

impl GoatRng {
    /// Create a new RNG from a 64-bit seed.
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { NON_ZERO_SEED } else { seed },
        }
    }

    /// Return the current internal state (useful for save/restore).
    pub fn state(&self) -> u64 {
        self.state
    }
}

impl RngSource for GoatRng {
    fn next_u64(&mut self) -> u64 {
        // Xorshift64 — period 2^64 - 1, passes BigCrush.
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_seed_sequence() {
        let mut rng = GoatRng::new(1);
        assert_eq!(rng.next_u64(), 0x0000000040822041);
        assert_eq!(rng.next_u64(), 0x100041060c011441);
        assert_eq!(rng.next_u64(), 0x9b1e842f6e862629);
    }

    #[test]
    fn zero_seed_is_non_zero() {
        let rng = GoatRng::new(0);
        assert_ne!(rng.state(), 0);
    }

    #[test]
    fn same_seed_same_sequence() {
        let mut a = GoatRng::new(42);
        let mut b = GoatRng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = GoatRng::new(1);
        let mut b = GoatRng::new(2);
        // It would be astronomically unlikely for all 10 to match.
        let differs = (0..10).any(|_| a.next_u64() != b.next_u64());
        assert!(differs);
    }

    #[test]
    fn next_range_u64_within_bounds() {
        let mut rng = GoatRng::new(99);
        for _ in 0..1_000 {
            let v = rng.next_range_u64(10, 20);
            assert!((10..=20).contains(&v));
        }
    }

    #[test]
    fn next_range_u64_single_value() {
        let mut rng = GoatRng::new(7);
        for _ in 0..10 {
            assert_eq!(rng.next_range_u64(42, 42), 42);
        }
    }

    #[test]
    fn next_range_u8_within_bounds() {
        let mut rng = GoatRng::new(123);
        for _ in 0..1_000 {
            let v = rng.next_range_u8(1, 99);
            assert!((1..=99).contains(&v));
        }
    }

    #[test]
    fn range_covers_endpoints() {
        let mut rng = GoatRng::new(555);
        let mut saw_lo = false;
        let mut saw_hi = false;
        for _ in 0..10_000 {
            let v = rng.next_range_u8(1, 3);
            if v == 1 {
                saw_lo = true;
            }
            if v == 3 {
                saw_hi = true;
            }
        }
        assert!(saw_lo, "lo endpoint must be reachable");
        assert!(saw_hi, "hi endpoint must be reachable");
    }

    #[test]
    fn golden_range_sequence() {
        let mut rng = GoatRng::new(12345);
        // Exact outputs frozen after first green run.
        let vals: Vec<u8> = (0..5).map(|_| rng.next_range_u8(1, 99)).collect();
        assert_eq!(vals, vec![73, 88, 21, 25, 24]);
    }
}
