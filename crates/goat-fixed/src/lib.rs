#![forbid(unsafe_code)]

//! Fixed-point arithmetic for the GOAT simulation.
//!
//! `Fixed` is a signed 32-bit integer with an implicit denominator of 1 000,
//! i.e. three decimal places: `Fixed(1_000)` represents `1.0`.
//!
//! This crate's public API and golden test values are frozen. Do not modify
//! existing tests or expected values; new behaviour gets new tests.

use core::fmt;
use core::ops::{Add, Div, Mul, Sub};

/// Fixed-point number — denominator 1 000.
///
/// `Fixed(n)` represents the real value `n / 1_000`.
/// Range: roughly −2 147 483 to +2 147 483 with 0.001 precision.
///
/// Attribute values live in \[1 000, 99 000\] (= 1.000 … 99.000).
/// Weights and multipliers are in \[0, 1 000\] (= 0.000 … 1.000).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Fixed(i32);

impl Fixed {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_000);

    /// Minimum valid attribute value (1).
    pub const MIN_ATTR: Self = Self(1_000);
    /// Maximum valid attribute value (99).
    pub const MAX_ATTR: Self = Self(99_000);

    /// Construct from an integer `n`; result represents `n.000`.
    #[inline]
    pub const fn from_int(n: i32) -> Self {
        Self(n * 1_000)
    }

    /// Construct directly from the internal raw representation.
    /// Use for compile-time constants only; prefer `from_int` elsewhere.
    #[inline]
    pub const fn raw(n: i32) -> Self {
        Self(n)
    }

    /// Return the internal raw value (`self * 1_000` as an integer).
    #[inline]
    pub const fn to_raw(self) -> i32 {
        self.0
    }

    /// Truncating integer part (rounds toward zero).
    #[inline]
    pub const fn to_int(self) -> i32 {
        self.0 / 1_000
    }

    /// Clamp to `[min, max]`.
    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }

    /// Clamp to the valid attribute range `[1, 99]`.
    #[inline]
    pub fn clamp_attr(self) -> Self {
        self.clamp(Self::MIN_ATTR, Self::MAX_ATTR)
    }
}

// ── Standard operator traits ──────────────────────────────────────────────────

/// `(a/1000) + (b/1000)` — saturating at `i32` bounds.
impl Add for Fixed {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }
}

/// `(a/1000) - (b/1000)` — saturating at `i32` bounds.
impl Sub for Fixed {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// `(a/1000) × (b/1000) → result/1000`.
///
/// Uses i64 intermediate to avoid overflow for values up to ~±2 000.
impl Mul for Fixed {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self(((self.0 as i64 * rhs.0 as i64) / 1_000) as i32)
    }
}

/// `(a/1000) / (b/1000) → result/1000`.
///
/// Uses i64 intermediate. Panics in debug mode if `rhs` is zero.
impl Div for Fixed {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        debug_assert!(rhs.0 != 0, "Fixed: division by zero");
        Self(((self.0 as i64 * 1_000) / rhs.0 as i64) as i32)
    }
}

impl fmt::Debug for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fixed({}.{:03})", self.0 / 1_000, (self.0 % 1_000).abs())
    }
}

impl fmt::Display for Fixed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:03}", self.0 / 1_000, (self.0 % 1_000).abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_from_int() {
        assert_eq!(Fixed::from_int(50).to_raw(), 50_000);
        assert_eq!(Fixed::from_int(99).to_raw(), 99_000);
        assert_eq!(Fixed::from_int(1).to_raw(), 1_000);
    }

    #[test]
    fn golden_add() {
        assert_eq!(
            Fixed::from_int(10) + Fixed::from_int(20),
            Fixed::from_int(30)
        );
        assert_eq!(Fixed::raw(500) + Fixed::raw(500), Fixed::ONE);
    }

    #[test]
    fn golden_mul() {
        // 99 × 1.0 = 99
        assert_eq!(Fixed::from_int(99) * Fixed::ONE, Fixed::from_int(99));
        // 50 × 0.5 = 25
        assert_eq!(Fixed::from_int(50) * Fixed::raw(500), Fixed::from_int(25));
        // 0.9 × 99 = 89.1
        assert_eq!(Fixed::raw(900) * Fixed::from_int(99), Fixed::raw(89_100));
    }

    #[test]
    fn golden_div() {
        // 99 / 1.0 = 99
        assert_eq!(Fixed::from_int(99) / Fixed::ONE, Fixed::from_int(99));
        // 50 / 2.0 = 25
        assert_eq!(
            Fixed::from_int(50) / Fixed::from_int(2),
            Fixed::from_int(25)
        );
        // weighted-average check: 99000 / 9250 ≈ 10.702
        assert_eq!(Fixed::raw(99_000) / Fixed::raw(9_250), Fixed::raw(10_702));
    }

    #[test]
    fn golden_clamp_attr() {
        assert_eq!(Fixed::ZERO.clamp_attr(), Fixed::MIN_ATTR);
        assert_eq!(Fixed::from_int(50).clamp_attr(), Fixed::from_int(50));
        assert_eq!(Fixed::from_int(150).clamp_attr(), Fixed::MAX_ATTR);
    }

    #[test]
    fn ordering() {
        assert!(Fixed::from_int(1) < Fixed::from_int(99));
        assert!(Fixed::ZERO < Fixed::ONE);
        assert_eq!(Fixed::from_int(50), Fixed::from_int(50));
    }

    #[test]
    fn sub_saturates() {
        let result = Fixed::ZERO - Fixed::ONE;
        assert_eq!(result, Fixed::raw(-1_000));
    }
}
