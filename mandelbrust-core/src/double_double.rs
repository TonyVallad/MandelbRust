use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A double-double floating-point number: ~31 significant decimal digits.
///
/// Stores a value as `hi + lo` using two `f64` components with the invariant
/// `|lo| ≤ ε·|hi|`. Arithmetic uses Knuth's TwoSum and FMA-based TwoProd
/// (error-free transformations) to maintain full precision.
///
/// Reference: Hida, Li, Bailey — "Library for Double-Double and Quad-Double
/// Arithmetic" (2001).
#[derive(Debug, Clone, Copy)]
pub struct DoubleDouble {
    pub hi: f64,
    pub lo: f64,
}

// ---------------------------------------------------------------------------
// Error-free building blocks
// ---------------------------------------------------------------------------

/// Knuth's TwoSum: error-free addition of two `f64` values.
/// Returns `(s, e)` where `s + e = a + b` exactly.
#[inline]
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let v = s - a;
    let e = (a - (s - v)) + (b - v);
    (s, e)
}

/// Fast path for TwoSum when `|a| >= |b|`.
#[inline]
fn quick_two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let e = b - (s - a);
    (s, e)
}

/// FMA-based TwoProd: error-free multiplication.
/// Returns `(p, e)` where `p + e = a * b` exactly.
#[inline]
fn two_prod(a: f64, b: f64) -> (f64, f64) {
    let p = a * b;
    let e = a.mul_add(b, -p);
    (p, e)
}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl DoubleDouble {
    pub const ZERO: Self = Self { hi: 0.0, lo: 0.0 };

    #[inline]
    pub fn new(hi: f64, lo: f64) -> Self {
        Self { hi, lo }
    }

    /// The combined value as a single `f64` (loses the low-order bits).
    #[inline]
    pub fn to_f64(self) -> f64 {
        self.hi + self.lo
    }

    #[inline]
    pub fn abs(self) -> Self {
        if self.is_negative() {
            -self
        } else {
            self
        }
    }

    #[inline]
    pub fn is_positive(self) -> bool {
        self.hi > 0.0 || (self.hi == 0.0 && self.lo > 0.0)
    }

    #[inline]
    pub fn is_negative(self) -> bool {
        self.hi < 0.0 || (self.hi == 0.0 && self.lo < 0.0)
    }
}

impl From<f64> for DoubleDouble {
    #[inline]
    fn from(val: f64) -> Self {
        Self { hi: val, lo: 0.0 }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic: DD + DD
// ---------------------------------------------------------------------------

impl Add for DoubleDouble {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        let (s1, s2) = two_sum(self.hi, rhs.hi);
        let (t1, t2) = two_sum(self.lo, rhs.lo);
        let s2 = s2 + t1;
        let (s1, s2) = quick_two_sum(s1, s2);
        let s2 = s2 + t2;
        let (hi, lo) = quick_two_sum(s1, s2);
        Self { hi, lo }
    }
}

impl AddAssign for DoubleDouble {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

// ---------------------------------------------------------------------------
// Arithmetic: DD - DD
// ---------------------------------------------------------------------------

impl Sub for DoubleDouble {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl SubAssign for DoubleDouble {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

// ---------------------------------------------------------------------------
// Arithmetic: DD * DD
// ---------------------------------------------------------------------------

impl Mul for DoubleDouble {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        let (p1, p2) = two_prod(self.hi, rhs.hi);
        let p2 = p2 + self.hi * rhs.lo + self.lo * rhs.hi;
        let (hi, lo) = quick_two_sum(p1, p2);
        Self { hi, lo }
    }
}

impl MulAssign for DoubleDouble {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

/// Scalar multiplication: `DoubleDouble * f64`.
impl Mul<f64> for DoubleDouble {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self {
        let (p1, p2) = two_prod(self.hi, rhs);
        let p2 = p2 + self.lo * rhs;
        let (hi, lo) = quick_two_sum(p1, p2);
        Self { hi, lo }
    }
}

// ---------------------------------------------------------------------------
// Arithmetic: negation
// ---------------------------------------------------------------------------

impl Neg for DoubleDouble {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

impl PartialEq for DoubleDouble {
    fn eq(&self, other: &Self) -> bool {
        self.hi == other.hi && self.lo == other.lo
    }
}

impl PartialOrd for DoubleDouble {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self.hi.partial_cmp(&other.hi) {
            Some(Ordering::Equal) => self.lo.partial_cmp(&other.lo),
            ord => ord,
        }
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for DoubleDouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:+.17e} + {:+.17e})", self.hi, self.lo)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dd(val: f64) -> DoubleDouble {
        DoubleDouble::from(val)
    }

    fn approx_eq_f64(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn approx_eq_dd(a: DoubleDouble, b: DoubleDouble, eps: f64) -> bool {
        let diff = a - b;
        diff.abs().hi < eps
    }

    // -- Construction --

    #[test]
    fn from_f64() {
        let d = dd(3.14);
        assert_eq!(d.hi, 3.14);
        assert_eq!(d.lo, 0.0);
    }

    #[test]
    fn zero_constant() {
        let z = DoubleDouble::ZERO;
        assert_eq!(z.hi, 0.0);
        assert_eq!(z.lo, 0.0);
    }

    #[test]
    fn to_f64_roundtrip() {
        let d = dd(2.718281828);
        assert_eq!(d.to_f64(), 2.718281828);
    }

    // -- Basic arithmetic --

    #[test]
    fn addition_simple() {
        let a = dd(1.0);
        let b = dd(2.0);
        let c = a + b;
        assert!(approx_eq_f64(c.to_f64(), 3.0, 1e-15));
    }

    #[test]
    fn subtraction_simple() {
        let a = dd(5.0);
        let b = dd(3.0);
        let c = a - b;
        assert!(approx_eq_f64(c.to_f64(), 2.0, 1e-15));
    }

    #[test]
    fn multiplication_simple() {
        let a = dd(3.0);
        let b = dd(4.0);
        let c = a * b;
        assert!(approx_eq_f64(c.to_f64(), 12.0, 1e-15));
    }

    #[test]
    fn scalar_multiplication() {
        let a = dd(2.5);
        let c = a * 4.0;
        assert!(approx_eq_f64(c.to_f64(), 10.0, 1e-15));
    }

    #[test]
    fn negation() {
        let a = dd(7.0);
        let b = -a;
        assert_eq!(b.hi, -7.0);
        assert_eq!(b.lo, 0.0);
    }

    #[test]
    fn add_assign() {
        let mut a = dd(1.0);
        a += dd(2.0);
        assert!(approx_eq_f64(a.to_f64(), 3.0, 1e-15));
    }

    #[test]
    fn sub_assign() {
        let mut a = dd(5.0);
        a -= dd(2.0);
        assert!(approx_eq_f64(a.to_f64(), 3.0, 1e-15));
    }

    #[test]
    fn mul_assign() {
        let mut a = dd(3.0);
        a *= dd(4.0);
        assert!(approx_eq_f64(a.to_f64(), 12.0, 1e-15));
    }

    // -- Precision retention --

    #[test]
    fn precision_add_small_to_large() {
        // In f64: 1.0 + 1e-17 == 1.0 (the small part is lost).
        // In DD: the small part is preserved in lo.
        let a = dd(1.0);
        let b = dd(1e-17);
        let sum = a + b;
        let diff = sum - a;
        let recovered = diff.hi + diff.lo;
        assert!(
            (recovered - 1e-17).abs() < 1e-32,
            "DD should retain 1e-17 after adding to 1.0: got {recovered}"
        );
    }

    #[test]
    fn precision_multiply() {
        // (1 + 1e-16) * (1 + 1e-16) should be 1 + 2e-16 + 1e-32.
        // f64 loses the 1e-32 term; DD should retain it.
        let one_plus_eps = DoubleDouble::new(1.0, 1e-16);
        let sq = one_plus_eps * one_plus_eps;
        // Expected: 1 + 2e-16 + 1e-32
        let expected = DoubleDouble::new(1.0, 2e-16) + dd(1e-32);
        assert!(
            approx_eq_dd(sq, expected, 1e-31),
            "DD multiply should retain ~31 digits: got {sq}, expected {expected}"
        );
    }

    #[test]
    fn precision_catastrophic_cancellation() {
        // a = 1.0 + 1e-20, b = 1.0 → a - b should be 1e-20 in DD.
        let a = DoubleDouble::new(1.0, 1e-20);
        let b = dd(1.0);
        let diff = a - b;
        let val = diff.hi + diff.lo;
        assert!(
            (val - 1e-20).abs() < 1e-35,
            "DD subtraction should survive cancellation: got {val}"
        );
    }

    // -- Sign helpers --

    #[test]
    fn is_positive_negative() {
        assert!(dd(1.0).is_positive());
        assert!(!dd(1.0).is_negative());
        assert!(dd(-1.0).is_negative());
        assert!(!dd(-1.0).is_positive());
        assert!(!DoubleDouble::ZERO.is_positive());
        assert!(!DoubleDouble::ZERO.is_negative());
    }

    #[test]
    fn abs_positive() {
        let a = dd(3.0);
        assert_eq!(a.abs(), a);
    }

    #[test]
    fn abs_negative() {
        let a = dd(-3.0);
        assert_eq!(a.abs(), dd(3.0));
    }

    // -- Ordering --

    #[test]
    fn ordering_hi_differs() {
        assert!(dd(2.0) > dd(1.0));
        assert!(dd(-1.0) < dd(1.0));
    }

    #[test]
    fn ordering_hi_equal_lo_differs() {
        let a = DoubleDouble::new(1.0, 1e-17);
        let b = DoubleDouble::new(1.0, 0.0);
        assert!(a > b);
    }

    #[test]
    fn equality() {
        let a = DoubleDouble::new(1.0, 2e-17);
        let b = DoubleDouble::new(1.0, 2e-17);
        assert_eq!(a, b);
    }

    // -- Zero arithmetic --

    #[test]
    fn add_zero() {
        let a = dd(42.0);
        let b = a + DoubleDouble::ZERO;
        assert_eq!(b, a);
    }

    #[test]
    fn mul_zero() {
        let a = dd(42.0);
        let b = a * DoubleDouble::ZERO;
        assert!(approx_eq_f64(b.to_f64(), 0.0, 1e-30));
    }

    #[test]
    fn mul_one() {
        let a = DoubleDouble::new(3.14, 1e-17);
        let one = dd(1.0);
        let b = a * one;
        assert!(approx_eq_dd(b, a, 1e-30));
    }

    // -- Compound operations --

    #[test]
    fn square_of_small_number() {
        // 0.001^2 = 0.000001
        let a = dd(0.001);
        let sq = a * a;
        assert!(approx_eq_f64(sq.to_f64(), 1e-6, 1e-21));
    }

    #[test]
    fn distributive_property() {
        // a * (b + c) ≈ a*b + a*c
        let a = dd(3.7);
        let b = dd(2.1);
        let c = dd(4.3);
        let lhs = a * (b + c);
        let rhs = a * b + a * c;
        assert!(
            approx_eq_dd(lhs, rhs, 1e-28),
            "distributive property: {lhs} vs {rhs}"
        );
    }
}
