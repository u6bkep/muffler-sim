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

    // -----------------------------------------------------------------------
    // Test Group 1: Transfer function stability with extreme parameters
    // -----------------------------------------------------------------------

    #[test]
    fn test_extreme_large_chamber_produces_finite_tl() {
        // Very large chamber: 10 m diameter
        use crate::constants::{area_from_diameter, speed_of_sound_and_density};
        use crate::elements::StraightDuct;
        use crate::muffler::Muffler;

        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 10.0; // 10 metres
        let chamber_length = 1.0;

        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        // Test at several frequencies
        for freq in [100.0, 1000.0, 5000.0, 10000.0] {
            let omega = 2.0 * std::f64::consts::PI * freq;
            let tl = muffler.transmission_loss(omega, c, rho);
            assert!(
                tl.is_finite(),
                "TL must be finite for 10m chamber at {freq} Hz, got {tl}"
            );
        }
    }

    #[test]
    fn test_extreme_small_chamber_produces_finite_tl() {
        // Very small chamber: 1 mm diameter
        use crate::constants::{area_from_diameter, speed_of_sound_and_density};
        use crate::elements::StraightDuct;
        use crate::muffler::Muffler;

        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 1e-3; // 1 mm
        let chamber_length = 5e-3;   // 5 mm

        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        for freq in [100.0, 1000.0, 5000.0, 10000.0] {
            let omega = 2.0 * std::f64::consts::PI * freq;
            let tl = muffler.transmission_loss(omega, c, rho);
            assert!(
                tl.is_finite(),
                "TL must be finite for 1mm chamber at {freq} Hz, got {tl}"
            );
        }
    }

    #[test]
    fn test_very_high_frequency_near_nyquist_produces_finite_tl() {
        // Near Nyquist: 22050 Hz (half of 44100 sample rate)
        use crate::constants::{area_from_diameter, speed_of_sound_and_density};
        use crate::elements::StraightDuct;
        use crate::muffler::Muffler;

        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;

        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let freq = 22050.0;
        let omega = 2.0 * std::f64::consts::PI * freq;
        let tl = muffler.transmission_loss(omega, c, rho);
        assert!(
            tl.is_finite(),
            "TL must be finite at Nyquist (22050 Hz), got {tl}"
        );

        let hf = muffler.pressure_transfer(omega, c, rho);
        assert!(
            hf.norm().is_finite(),
            "Pressure transfer must be finite at Nyquist, got {hf}"
        );
    }

    #[test]
    fn test_very_low_frequency_produces_finite_tl() {
        // Very low frequency: 1 Hz
        use crate::constants::{area_from_diameter, speed_of_sound_and_density};
        use crate::elements::StraightDuct;
        use crate::muffler::Muffler;

        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;

        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let freq = 1.0;
        let omega = 2.0 * std::f64::consts::PI * freq;
        let tl = muffler.transmission_loss(omega, c, rho);
        assert!(
            tl.is_finite(),
            "TL must be finite at 1 Hz, got {tl}"
        );
        // At very low frequencies, TL should be very small (near zero) because
        // wavelength >> chamber length
        assert!(
            tl < 1.0,
            "TL at 1 Hz should be very small, got {tl} dB"
        );
    }
}
