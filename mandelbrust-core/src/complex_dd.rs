use std::ops::{Add, Mul, Neg, Sub};

use crate::complex::Complex;
use crate::double_double::DoubleDouble;

/// A complex number using double-double components (~31 decimal digits per axis).
///
/// Mirrors [`Complex`] but uses [`DoubleDouble`] arithmetic for extended
/// precision. Used in the deep-zoom iteration paths where `f64` alone
/// cannot distinguish adjacent pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexDD {
    pub re: DoubleDouble,
    pub im: DoubleDouble,
}

impl ComplexDD {
    pub const ZERO: Self = Self {
        re: DoubleDouble::ZERO,
        im: DoubleDouble::ZERO,
    };

    #[inline]
    pub fn new(re: DoubleDouble, im: DoubleDouble) -> Self {
        Self { re, im }
    }

    /// Returns `re² + im²` without taking the square root.
    #[inline]
    pub fn norm_sq(self) -> DoubleDouble {
        self.re * self.re + self.im * self.im
    }

    /// Downcast to `f64` complex (takes the `hi` parts).
    #[inline]
    pub fn to_complex(self) -> Complex {
        Complex::new(self.re.to_f64(), self.im.to_f64())
    }
}

impl From<Complex> for ComplexDD {
    #[inline]
    fn from(c: Complex) -> Self {
        Self {
            re: DoubleDouble::from(c.re),
            im: DoubleDouble::from(c.im),
        }
    }
}

// -- Arithmetic operators --

impl Add for ComplexDD {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl Sub for ComplexDD {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl Mul for ComplexDD {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl Neg for ComplexDD {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

/// Scalar multiplication: `ComplexDD * DoubleDouble`.
impl Mul<DoubleDouble> for ComplexDD {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: DoubleDouble) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl std::fmt::Display for ComplexDD {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} + {}·i", self.re, self.im)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-12;
    const DD_EPSILON: f64 = 1e-28;

    fn dd(val: f64) -> DoubleDouble {
        DoubleDouble::from(val)
    }

    fn cdd(re: f64, im: f64) -> ComplexDD {
        ComplexDD::new(dd(re), dd(im))
    }

    fn approx_eq_dd(a: DoubleDouble, b: DoubleDouble, eps: f64) -> bool {
        (a - b).abs().hi < eps
    }

    fn approx_eq_cdd(a: ComplexDD, b: ComplexDD, eps: f64) -> bool {
        approx_eq_dd(a.re, b.re, eps) && approx_eq_dd(a.im, b.im, eps)
    }

    // -- Construction --

    #[test]
    fn from_complex() {
        let c = Complex::new(1.0, 2.0);
        let cdd = ComplexDD::from(c);
        assert_eq!(cdd.re.hi, 1.0);
        assert_eq!(cdd.im.hi, 2.0);
    }

    #[test]
    fn to_complex_roundtrip() {
        let c = Complex::new(3.14, -2.71);
        let cdd = ComplexDD::from(c);
        let back = cdd.to_complex();
        assert!((back.re - c.re).abs() < EPSILON);
        assert!((back.im - c.im).abs() < EPSILON);
    }

    // -- Arithmetic matching Complex --

    #[test]
    fn addition() {
        let a = cdd(1.0, 2.0);
        let b = cdd(3.0, 4.0);
        let c = a + b;
        assert!(approx_eq_cdd(c, cdd(4.0, 6.0), EPSILON));
    }

    #[test]
    fn subtraction() {
        let a = cdd(5.0, 3.0);
        let b = cdd(2.0, 1.0);
        let c = a - b;
        assert!(approx_eq_cdd(c, cdd(3.0, 2.0), EPSILON));
    }

    #[test]
    fn multiplication() {
        // (1 + 2i)(3 + 4i) = -5 + 10i
        let a = cdd(1.0, 2.0);
        let b = cdd(3.0, 4.0);
        let c = a * b;
        assert!(approx_eq_cdd(c, cdd(-5.0, 10.0), EPSILON));
    }

    #[test]
    fn negation() {
        let a = cdd(1.0, -2.0);
        let b = -a;
        assert!(approx_eq_cdd(b, cdd(-1.0, 2.0), EPSILON));
    }

    #[test]
    fn scalar_multiplication() {
        let a = cdd(2.0, 3.0);
        let c = a * dd(4.0);
        assert!(approx_eq_cdd(c, cdd(8.0, 12.0), EPSILON));
    }

    // -- norm_sq --

    #[test]
    fn norm_sq() {
        let a = cdd(3.0, 4.0);
        assert!(approx_eq_dd(a.norm_sq(), dd(25.0), EPSILON));
    }

    // -- Squaring --

    #[test]
    fn squaring() {
        // z² where z = 1 + i → 0 + 2i
        let z = cdd(1.0, 1.0);
        let z2 = z * z;
        assert!(approx_eq_cdd(z2, cdd(0.0, 2.0), EPSILON));
    }

    // -- Precision --

    #[test]
    fn precision_retained_in_multiplication() {
        // Multiply two values near 1.0 that differ only in low-order bits.
        // DD should track the cross terms that f64 drops.
        let a = ComplexDD::new(DoubleDouble::new(1.0, 1e-17), dd(0.0));
        let b = ComplexDD::new(DoubleDouble::new(1.0, 2e-17), dd(0.0));
        let c = a * b;
        // Expected: re = 1 + 3e-17 + 2e-34, im = 0
        let expected_re = DoubleDouble::new(1.0, 3e-17);
        assert!(
            approx_eq_dd(c.re, expected_re, 1e-30),
            "DD complex mul should retain precision: re = {}, expected ≈ {}",
            c.re,
            expected_re
        );
        assert!(approx_eq_dd(c.im, DoubleDouble::ZERO, DD_EPSILON));
    }

    #[test]
    fn mandelbrot_iteration_step() {
        // z² + c where z = (0.5, 0.5), c = (-0.75, 0.1)
        // z² = (0.25 - 0.25, 0.5) = (0, 0.5)
        // z² + c = (-0.75, 0.6)
        let z = cdd(0.5, 0.5);
        let c = cdd(-0.75, 0.1);
        let next = z * z + c;
        assert!(approx_eq_cdd(next, cdd(-0.75, 0.6), EPSILON));
    }
}
