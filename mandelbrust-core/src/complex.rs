use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A complex number represented as two `f64` components.
///
/// This is a lightweight, `Copy` type optimized for the tight iteration loop.
/// We roll our own instead of using `num::Complex` to keep the dependency graph
/// minimal and retain full control over the arithmetic.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Self = Self { re: 0.0, im: 0.0 };

    #[inline]
    pub fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// Returns `re² + im²` without taking the square root.
    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    /// Returns `√(re² + im²)`.
    #[inline]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }
}

// -- Arithmetic operators --

impl Add for Complex {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl AddAssign for Complex {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.re += rhs.re;
        self.im += rhs.im;
    }
}

impl Sub for Complex {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl SubAssign for Complex {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.re -= rhs.re;
        self.im -= rhs.im;
    }
}

impl Mul for Complex {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl MulAssign for Complex {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Neg for Complex {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

/// Scalar multiplication: `Complex * f64`.
impl Mul<f64> for Complex {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f64) -> Self {
        Self {
            re: self.re * rhs,
            im: self.im * rhs,
        }
    }
}

impl std::fmt::Display for Complex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.im >= 0.0 {
            write!(f, "{} + {}i", self.re, self.im)
        } else {
            write!(f, "{} - {}i", self.re, -self.im)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-12;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn zero_constant() {
        let z = Complex::ZERO;
        assert_eq!(z.re, 0.0);
        assert_eq!(z.im, 0.0);
    }

    #[test]
    fn addition() {
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, 4.0);
        let c = a + b;
        assert!(approx_eq(c.re, 4.0));
        assert!(approx_eq(c.im, 6.0));
    }

    #[test]
    fn subtraction() {
        let a = Complex::new(5.0, 3.0);
        let b = Complex::new(2.0, 1.0);
        let c = a - b;
        assert!(approx_eq(c.re, 3.0));
        assert!(approx_eq(c.im, 2.0));
    }

    #[test]
    fn multiplication() {
        // (1 + 2i)(3 + 4i) = 3 + 4i + 6i + 8i² = 3 + 10i - 8 = -5 + 10i
        let a = Complex::new(1.0, 2.0);
        let b = Complex::new(3.0, 4.0);
        let c = a * b;
        assert!(approx_eq(c.re, -5.0));
        assert!(approx_eq(c.im, 10.0));
    }

    #[test]
    fn scalar_multiplication() {
        let a = Complex::new(2.0, 3.0);
        let c = a * 4.0;
        assert!(approx_eq(c.re, 8.0));
        assert!(approx_eq(c.im, 12.0));
    }

    #[test]
    fn negation() {
        let a = Complex::new(1.0, -2.0);
        let b = -a;
        assert!(approx_eq(b.re, -1.0));
        assert!(approx_eq(b.im, 2.0));
    }

    #[test]
    fn norm_sq() {
        let a = Complex::new(3.0, 4.0);
        assert!(approx_eq(a.norm_sq(), 25.0));
    }

    #[test]
    fn norm() {
        let a = Complex::new(3.0, 4.0);
        assert!(approx_eq(a.norm(), 5.0));
    }

    #[test]
    fn squaring() {
        // z² where z = 1 + i → (1+i)(1+i) = 1 + 2i - 1 = 0 + 2i
        let z = Complex::new(1.0, 1.0);
        let z2 = z * z;
        assert!(approx_eq(z2.re, 0.0));
        assert!(approx_eq(z2.im, 2.0));
    }
}
