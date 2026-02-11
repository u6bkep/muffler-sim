use crate::elements::StraightDuct;
use crate::transfer_matrix::TransferMatrix;
use crate::{AcousticElement, SimParams};

/// An ordered chain of acoustic elements forming a muffler.
pub struct Muffler {
    elements: Vec<Box<dyn AcousticElement>>,
    /// Characteristic impedance of the inlet (source side).
    pub z_source: f64,
    /// Characteristic impedance of the outlet (load side).
    pub z_load: f64,
}

impl Muffler {
    /// Create a muffler from a custom list of elements and impedances.
    pub fn new(
        elements: Vec<Box<dyn AcousticElement>>,
        z_source: f64,
        z_load: f64,
    ) -> Self {
        Self {
            elements,
            z_source,
            z_load,
        }
    }

    /// Build a single expansion chamber muffler from simulation parameters.
    pub fn from_params(params: &SimParams) -> Self {
        let inlet = StraightDuct::new(params.inlet_length, params.inlet_diameter);
        let chamber = StraightDuct::new(params.chamber_length, params.chamber_diameter);
        let outlet = StraightDuct::new(params.outlet_length, params.outlet_diameter);

        let (c, rho) = crate::constants::speed_of_sound_and_density(params.temperature);
        let z_source = inlet.impedance(c, rho);
        let z_load = outlet.impedance(c, rho);

        Self {
            elements: vec![Box::new(inlet), Box::new(chamber), Box::new(outlet)],
            z_source,
            z_load,
        }
    }

    /// Compute the total transfer matrix at angular frequency `omega`.
    pub fn total_transfer_matrix(&self, omega: f64, c: f64, rho: f64) -> TransferMatrix {
        let mut total = TransferMatrix::identity();
        for elem in &self.elements {
            let t = elem.transfer_matrix(omega, c, rho);
            total = total.chain(&t);
        }
        total
    }

    /// Transmission loss in dB at angular frequency `omega`.
    pub fn transmission_loss(&self, omega: f64, c: f64, rho: f64) -> f64 {
        let t = self.total_transfer_matrix(omega, c, rho);
        t.transmission_loss(self.z_source, self.z_load)
    }

    /// Complex pressure transfer function at angular frequency `omega`.
    pub fn pressure_transfer(
        &self,
        omega: f64,
        c: f64,
        rho: f64,
    ) -> num_complex::Complex64 {
        let t = self.total_transfer_matrix(omega, c, rho);
        t.pressure_transfer(self.z_source, self.z_load)
    }
}
