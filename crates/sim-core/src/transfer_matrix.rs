use num_complex::Complex64;

/// A 2×2 complex transfer matrix representing an acoustic element.
///
/// ```text
/// [p_out]   [a  b] [p_in ]
/// [U_out] = [c  d] [U_in ]
/// ```
#[derive(Debug, Clone, Copy)]
pub struct TransferMatrix {
    pub a: Complex64,
    pub b: Complex64,
    pub c: Complex64,
    pub d: Complex64,
}

impl TransferMatrix {
    pub fn new(a: Complex64, b: Complex64, c: Complex64, d: Complex64) -> Self {
        Self { a, b, c, d }
    }

    /// Identity matrix (no-op element).
    pub fn identity() -> Self {
        Self {
            a: Complex64::new(1.0, 0.0),
            b: Complex64::new(0.0, 0.0),
            c: Complex64::new(0.0, 0.0),
            d: Complex64::new(1.0, 0.0),
        }
    }

    /// Chain (multiply) this matrix with another: self · other.
    pub fn chain(&self, other: &TransferMatrix) -> TransferMatrix {
        TransferMatrix {
            a: self.a * other.a + self.b * other.c,
            b: self.a * other.b + self.b * other.d,
            c: self.c * other.a + self.d * other.c,
            d: self.c * other.b + self.d * other.d,
        }
    }

    /// Transmission loss (dB) given source and load characteristic impedances.
    ///
    /// TL = 20·log₁₀(|T₁₁ + T₁₂/Zₙ + Z₁·T₂₁ + Z₁·T₂₂/Zₙ| / 2)
    pub fn transmission_loss(&self, z_source: f64, z_load: f64) -> f64 {
        let zs = Complex64::new(z_source, 0.0);
        let zl = Complex64::new(z_load, 0.0);
        let numerator = self.a + self.b / zl + zs * self.c + zs * self.d / zl;
        20.0 * (numerator.norm() / 2.0).log10()
    }

    /// Complex pressure transfer function H(f).
    ///
    /// H(f) = 2 / (T₁₁ + T₁₂/Zₙ + Z₁·T₂₁ + Z₁·T₂₂/Zₙ)
    pub fn pressure_transfer(&self, z_source: f64, z_load: f64) -> Complex64 {
        let zs = Complex64::new(z_source, 0.0);
        let zl = Complex64::new(z_load, 0.0);
        let denom = self.a + self.b / zl + zs * self.c + zs * self.d / zl;
        Complex64::new(2.0, 0.0) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_chain() {
        let id = TransferMatrix::identity();
        let m = TransferMatrix::new(
            Complex64::new(1.0, 0.5),
            Complex64::new(0.0, 1.0),
            Complex64::new(0.0, -1.0),
            Complex64::new(1.0, 0.5),
        );
        let result = id.chain(&m);
        assert!((result.a - m.a).norm() < 1e-12);
        assert!((result.b - m.b).norm() < 1e-12);
        assert!((result.c - m.c).norm() < 1e-12);
        assert!((result.d - m.d).norm() < 1e-12);
    }

    #[test]
    fn test_identity_tl_is_zero() {
        let id = TransferMatrix::identity();
        let tl = id.transmission_loss(100.0, 100.0);
        assert!(tl.abs() < 1e-10, "TL of identity should be 0, got {tl}");
    }

    #[test]
    fn test_reciprocity() {
        // For a passive element, det(T) = 1
        // StraightDuct matrices have det = cos²(kL) + sin²(kL) = 1
        let k: f64 = 1.0;
        let l: f64 = 0.5;
        let z: f64 = 100.0;
        let cos_kl = Complex64::new((k * l).cos(), 0.0);
        let sin_kl = Complex64::new((k * l).sin(), 0.0);
        let j = Complex64::new(0.0, 1.0);
        let m = TransferMatrix::new(
            cos_kl,
            j * Complex64::new(z, 0.0) * sin_kl,
            j * Complex64::new(1.0 / z, 0.0) * sin_kl,
            cos_kl,
        );
        let det = m.a * m.d - m.b * m.c;
        assert!((det - Complex64::new(1.0, 0.0)).norm() < 1e-12, "det = {det}");
    }
}
