use crate::constants::area_from_diameter;
use crate::transfer_matrix::TransferMatrix;
use crate::AcousticElement;
use num_complex::Complex64;

/// A straight cylindrical duct.
#[derive(Debug, Clone)]
pub struct StraightDuct {
    /// Length in metres.
    pub length: f64,
    /// Inner diameter in metres.
    pub diameter: f64,
}

impl StraightDuct {
    pub fn new(length: f64, diameter: f64) -> Self {
        Self { length, diameter }
    }

    /// Cross-sectional area in m².
    pub fn area(&self) -> f64 {
        area_from_diameter(self.diameter)
    }

    /// Characteristic impedance Z = ρc/S.
    pub fn impedance(&self, c: f64, rho: f64) -> f64 {
        rho * c / self.area()
    }
}

impl AcousticElement for StraightDuct {
    fn transfer_matrix(&self, omega: f64, c: f64, rho: f64) -> TransferMatrix {
        let k = omega / c;
        let z = self.impedance(c, rho);
        let kl = k * self.length;

        let cos_kl = Complex64::new(kl.cos(), 0.0);
        let sin_kl = Complex64::new(kl.sin(), 0.0);
        let j = Complex64::new(0.0, 1.0);

        TransferMatrix::new(
            cos_kl,
            j * Complex64::new(z, 0.0) * sin_kl,
            j * Complex64::new(1.0 / z, 0.0) * sin_kl,
            cos_kl,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn test_quarter_wave_duct() {
        // At quarter wavelength, kL = π/2, cos(kL) = 0
        let c = 343.0;
        let rho = 1.204;
        let diameter = 0.01; // 10 mm
        let freq = 1000.0;
        let wavelength = c / freq;
        let length = wavelength / 4.0;

        let duct = StraightDuct::new(length, diameter);
        let omega = 2.0 * PI * freq;
        let t = duct.transfer_matrix(omega, c, rho);

        assert!(t.a.norm() < 1e-10, "T11 should be ~0 at quarter wave");
        assert!(t.d.norm() < 1e-10, "T22 should be ~0 at quarter wave");
    }
}
