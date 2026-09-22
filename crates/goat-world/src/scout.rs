//! Scouting with noise (design B.2): clubs judge a player by OBSERVED
//! performance — form, season output — never by reading hidden attributes,
//! and scouts sometimes get it wrong.

use goat_rng::RngSource;

/// Percent chance a scouting report is a big miss (in either direction).
const BIG_MISS_PCT: u64 = 10;
/// Noise half-range for a normal report (±8).
const NORMAL_NOISE: i32 = 8;
/// Noise half-range for a big miss (±20).
const MISS_NOISE: i32 = 20;

/// A scout's estimate of a player's level from observed performance.
/// `observed` is a public metric (form, season output average, ...).
/// Usually within ±8 of the truth, but about 1 report in 10 is a big miss
/// (±20) — overrated gems and overlooked late bloomers both happen.
/// Deterministic in the caller's rng.
pub fn scout_estimate(observed: i32, rng: &mut impl RngSource) -> i32 {
    let big_miss = rng.next_range_u64(1, 100) <= BIG_MISS_PCT;
    let span = if big_miss { MISS_NOISE } else { NORMAL_NOISE };
    let noise = rng.next_range_u64(0, (2 * span) as u64) as i32 - span;
    (observed + noise).clamp(1, 99)
}

#[cfg(test)]
mod tests {
    use super::*;
    use goat_rng::GoatRng;

    #[test]
    fn estimate_is_deterministic() {
        let a = scout_estimate(70, &mut GoatRng::new(42));
        let b = scout_estimate(70, &mut GoatRng::new(42));
        assert_eq!(a, b);
    }

    #[test]
    fn estimates_center_on_observed_with_occasional_misses() {
        let mut rng = GoatRng::new(7);
        let n = 10_000;
        let mut sum = 0i64;
        let mut big_dev = 0u32;
        for _ in 0..n {
            let e = scout_estimate(70, &mut rng);
            assert!((1..=99).contains(&e));
            sum += e as i64;
            if (e - 70).abs() > NORMAL_NOISE {
                big_dev += 1;
            }
        }
        let mean = sum as f64 / n as f64;
        assert!((mean - 70.0).abs() < 2.0, "mean {mean} should center on 70");
        let miss_pct = big_dev as f64 / n as f64 * 100.0;
        assert!(
            (3.0..=20.0).contains(&miss_pct),
            "big-miss rate {miss_pct}% out of expected band"
        );
    }
}
