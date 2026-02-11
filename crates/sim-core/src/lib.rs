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
