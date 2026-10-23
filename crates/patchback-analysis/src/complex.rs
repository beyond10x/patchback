//! The small complex type the FFT works on.

use std::ops::{Add, Mul, Sub};

/// A complex number in `f64`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub const ZERO: Complex = Complex { re: 0.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// `e^(i theta)`.
    pub fn from_angle(theta: f64) -> Self {
        Self::new(theta.cos(), theta.sin())
    }

    /// The modulus `|z|`.
    pub fn norm(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// `|z|^2`.
    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }

    pub fn scale(self, factor: f64) -> Self {
        Self::new(self.re * factor, self.im * factor)
    }

    pub fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

impl Add for Complex {
    type Output = Complex;
    fn add(self, other: Complex) -> Complex {
        Complex::new(self.re + other.re, self.im + other.im)
    }
}

impl Sub for Complex {
    type Output = Complex;
    fn sub(self, other: Complex) -> Complex {
        Complex::new(self.re - other.re, self.im - other.im)
    }
}

impl Mul for Complex {
    type Output = Complex;
    fn mul(self, other: Complex) -> Complex {
        Complex::new(
            self.re * other.re - self.im * other.im,
            self.re * other.im + self.im * other.re,
        )
    }
}
