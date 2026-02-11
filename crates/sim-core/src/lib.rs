pub mod audio;
pub mod constants;
pub mod elements;
pub mod frequency_response;
pub mod impulse_response;
pub mod muffler;
pub mod pump;
pub mod transfer_matrix;

use num_complex::Complex64;

// ---------------------------------------------------------------------------
// Shared interface types — all feature branches build against these
// ---------------------------------------------------------------------------

/// Physical and geometric parameters describing the full simulation state.
#[derive(Debug, Clone)]
pub struct SimParams {
    /// Inlet pipe inner diameter in metres.
    pub inlet_diameter: f64,
    /// Inlet pipe length in metres.
    pub inlet_length: f64,
    /// Expansion chamber inner diameter in metres.
    pub chamber_diameter: f64,
    /// Expansion chamber length in metres.
    pub chamber_length: f64,
    /// Outlet pipe inner diameter in metres.
    pub outlet_diameter: f64,
    /// Outlet pipe length in metres.
    pub outlet_length: f64,
    /// Pump motor speed in RPM.
    pub rpm: f64,
    /// Number of pump valves (diaphragms).
    pub num_valves: u32,
    /// Duty cycle of each valve pulse (0–1).
    pub duty_cycle: f64,
    /// Ambient temperature in °C.
    pub temperature: f64,
}

impl Default for SimParams {
    fn default() -> Self {
        Self {
            inlet_diameter: 6e-3,    // 6 mm
            inlet_length: 30e-3,     // 30 mm
            chamber_diameter: 40e-3, // 40 mm
            chamber_length: 80e-3,   // 80 mm
            outlet_diameter: 6e-3,   // 6 mm
            outlet_length: 30e-3,    // 30 mm
            rpm: 3000.0,
            num_valves: 3,
            duty_cycle: 0.5,
            temperature: 20.0,
        }
    }
}

/// Results of a simulation run — consumed by the UI for plotting and by
/// the audio pipeline for real-time convolution.
#[derive(Debug, Clone)]
pub struct SimResult {
    /// Frequency bins in Hz (length N).
    pub frequencies: Vec<f64>,
    /// Transmission loss in dB at each frequency bin.
    pub transmission_loss: Vec<f64>,
    /// Complex pressure transfer function H(f) at each frequency bin.
    pub transfer_function: Vec<Complex64>,
    /// Time-domain impulse response h(t), windowed and truncated.
    pub impulse_response: Vec<f64>,
    /// Sample rate used for the impulse response (Hz).
    pub sample_rate: f64,
}

/// Trait for acoustic elements that can produce a 2×2 transfer matrix
/// at a given angular frequency.
pub trait AcousticElement: Send + Sync {
    /// Compute the 2×2 transfer matrix at angular frequency `omega` (rad/s)
    /// with the given speed of sound `c` (m/s) and air density `rho` (kg/m³).
    fn transfer_matrix(&self, omega: f64, c: f64, rho: f64) -> transfer_matrix::TransferMatrix;
}

/// Run the full simulation pipeline: build muffler from params, sweep
/// frequency response, compute impulse response.
pub fn compute(params: &SimParams) -> SimResult {
    let (c, rho) = constants::speed_of_sound_and_density(params.temperature);

    // Build element chain
    let chain = muffler::Muffler::from_params(params);

    // Sweep frequency response
    let sample_rate = 44100.0;
    let fft_size = 4096;
    let (frequencies, tl, transfer_fn) =
        frequency_response::sweep(&chain, fft_size, sample_rate, c, rho);

    // Compute impulse response
    let ir = impulse_response::compute(&transfer_fn, fft_size);

    SimResult {
        frequencies,
        transmission_loss: tl,
        transfer_function: transfer_fn,
        impulse_response: ir,
        sample_rate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Test Group 5: Parameter boundary conditions
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_params_produce_valid_results() {
        let params = SimParams::default();
        let result = compute(&params);

        // Check that all output arrays have the expected sizes
        let expected_bins = 4096 / 2 + 1; // fft_size = 4096
        assert_eq!(result.frequencies.len(), expected_bins);
        assert_eq!(result.transmission_loss.len(), expected_bins);
        assert_eq!(result.transfer_function.len(), expected_bins);
        assert_eq!(result.impulse_response.len(), 4096 / 2); // truncated to fft_size/2
        assert!((result.sample_rate - 44100.0).abs() < 1e-10);

        // All TL values should be finite
        for (i, &tl) in result.transmission_loss.iter().enumerate() {
            assert!(
                tl.is_finite(),
                "TL at bin {} is not finite: {}",
                i,
                tl
            );
        }

        // All impulse response values should be finite
        for (i, &s) in result.impulse_response.iter().enumerate() {
            assert!(
                s.is_finite(),
                "IR sample {} is not finite: {}",
                i,
                s
            );
        }
    }

    #[test]
    fn test_changing_chamber_diameter_changes_tl() {
        let mut params_small = SimParams::default();
        params_small.chamber_diameter = 20e-3; // 20 mm — smaller expansion ratio

        let mut params_large = SimParams::default();
        params_large.chamber_diameter = 80e-3; // 80 mm — larger expansion ratio

        let result_small = compute(&params_small);
        let result_large = compute(&params_large);

        // Compare TL at a non-DC, non-resonance frequency (pick bin 100)
        // Larger expansion ratio should produce higher TL in general
        // We just verify they are different
        let bin = 100;
        let tl_small = result_small.transmission_loss[bin];
        let tl_large = result_large.transmission_loss[bin];

        assert!(
            (tl_small - tl_large).abs() > 0.01,
            "Changing chamber diameter should change TL: small={} dB, large={} dB",
            tl_small,
            tl_large
        );

        // A larger chamber diameter (bigger area ratio) should generally produce
        // higher peak TL values
        let max_tl_small: f64 = result_small
            .transmission_loss
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);
        let max_tl_large: f64 = result_large
            .transmission_loss
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        assert!(
            max_tl_large > max_tl_small,
            "Larger chamber should have higher peak TL: small_max={} dB, large_max={} dB",
            max_tl_small,
            max_tl_large
        );
    }

    #[test]
    fn test_changing_chamber_length_changes_tl() {
        let mut params_short = SimParams::default();
        params_short.chamber_length = 40e-3; // 40 mm

        let mut params_long = SimParams::default();
        params_long.chamber_length = 160e-3; // 160 mm

        let result_short = compute(&params_short);
        let result_long = compute(&params_long);

        // The TL pattern should differ — different resonance spacing.
        // Compare the full TL curves: they should not be identical.
        let mut total_diff: f64 = 0.0;
        let mut max_diff: f64 = 0.0;
        for i in 1..result_short.transmission_loss.len() {
            let diff = (result_short.transmission_loss[i] - result_long.transmission_loss[i]).abs();
            total_diff += diff;
            if diff > max_diff {
                max_diff = diff;
            }
        }

        assert!(
            max_diff > 0.1,
            "Changing chamber length should change TL curve: max diff = {} dB",
            max_diff
        );

        assert!(
            total_diff > 1.0,
            "TL curves should differ substantially: total diff = {} dB",
            total_diff
        );

        // Both results should be valid (all finite)
        for &tl in &result_short.transmission_loss {
            assert!(tl.is_finite(), "TL should be finite for short chamber");
        }
        for &tl in &result_long.transmission_loss {
            assert!(tl.is_finite(), "TL should be finite for long chamber");
        }
    }

    #[test]
    fn test_very_small_muffler_geometry() {
        let params = SimParams {
            inlet_diameter: 1e-3,    // 1 mm
            inlet_length: 5e-3,      // 5 mm
            chamber_diameter: 5e-3,  // 5 mm
            chamber_length: 10e-3,   // 10 mm
            outlet_diameter: 1e-3,   // 1 mm
            outlet_length: 5e-3,     // 5 mm
            rpm: 3000.0,
            num_valves: 3,
            duty_cycle: 0.5,
            temperature: 20.0,
        };
        let result = compute(&params);

        // All values should be finite
        for &tl in &result.transmission_loss {
            assert!(tl.is_finite(), "TL should be finite for tiny muffler");
        }
        for &s in &result.impulse_response {
            assert!(s.is_finite(), "IR should be finite for tiny muffler");
        }
    }

    #[test]
    fn test_very_large_muffler_geometry() {
        let params = SimParams {
            inlet_diameter: 0.1,     // 100 mm
            inlet_length: 1.0,       // 1 m
            chamber_diameter: 1.0,   // 1 m
            chamber_length: 2.0,     // 2 m
            outlet_diameter: 0.1,    // 100 mm
            outlet_length: 1.0,      // 1 m
            rpm: 3000.0,
            num_valves: 3,
            duty_cycle: 0.5,
            temperature: 20.0,
        };
        let result = compute(&params);

        // All values should be finite
        for &tl in &result.transmission_loss {
            assert!(tl.is_finite(), "TL should be finite for large muffler");
        }
        for &s in &result.impulse_response {
            assert!(s.is_finite(), "IR should be finite for large muffler");
        }
    }
}
