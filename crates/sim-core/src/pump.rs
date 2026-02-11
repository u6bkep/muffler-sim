use std::f64::consts::PI;

/// A multi-valve diaphragm pump pressure source.
///
/// Each valve produces a half-rectified sinusoidal pulse once per motor
/// revolution, phase-shifted by `2π / num_valves` from the previous valve.
pub struct PumpSource {
    /// Motor speed in RPM.
    pub rpm: f64,
    /// Number of valves.
    pub num_valves: u32,
    /// Duty cycle (fraction of revolution each valve is active), 0–1.
    pub duty_cycle: f64,
    /// Current phase angle in radians (wraps at 2π).
    phase: f64,
    /// Sample rate in Hz.
    sample_rate: f64,
}

impl PumpSource {
    pub fn new(rpm: f64, num_valves: u32, duty_cycle: f64, sample_rate: f64) -> Self {
        Self {
            rpm,
            num_valves,
            duty_cycle,
            phase: 0.0,
            sample_rate,
        }
    }

    /// Fundamental pump frequency in Hz: `num_valves × RPM / 60`.
    pub fn fundamental_frequency(&self) -> f64 {
        self.num_valves as f64 * self.rpm / 60.0
    }

    /// Update RPM, valves, and duty cycle without resetting phase.
    pub fn set_params(&mut self, rpm: f64, num_valves: u32, duty_cycle: f64) {
        self.rpm = rpm;
        self.num_valves = num_valves;
        self.duty_cycle = duty_cycle;
    }

    /// Generate `count` samples of the pump pressure waveform.
    pub fn generate(&mut self, count: usize) -> Vec<f64> {
        let d_phase = 2.0 * PI * (self.rpm / 60.0) / self.sample_rate;
        let mut output = Vec::with_capacity(count);

        for _ in 0..count {
            let mut sample = 0.0;
            for v in 0..self.num_valves {
                let valve_phase = self.phase + 2.0 * PI * v as f64 / self.num_valves as f64;
                let theta = valve_phase % (2.0 * PI);
                let active_angle = self.duty_cycle * 2.0 * PI;
                if theta < active_angle {
                    // Half-rectified sinusoid within the active window
                    sample += (PI * theta / active_angle).sin();
                }
            }
            output.push(sample);
            self.phase += d_phase;
            if self.phase >= 2.0 * PI {
                self.phase -= 2.0 * PI;
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fundamental_frequency() {
        let pump = PumpSource::new(3000.0, 3, 0.5, 44100.0);
        assert!((pump.fundamental_frequency() - 150.0).abs() < 1e-10);
    }

    #[test]
    fn test_output_bounded() {
        let mut pump = PumpSource::new(3000.0, 3, 0.5, 44100.0);
        let samples = pump.generate(44100);
        for &s in &samples {
            assert!(s >= 0.0, "Pump output should be non-negative");
            assert!(s <= 3.1, "Pump output too large: {s}"); // max ~ num_valves
        }
    }
}
