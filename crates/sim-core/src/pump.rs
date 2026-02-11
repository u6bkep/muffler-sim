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

    // -----------------------------------------------------------------------
    // Test Group 4: Pump source validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_pump_signal_periodic_at_expected_frequency() {
        // The pump waveform should repeat at the motor revolution frequency
        // (RPM / 60). We verify this by generating exactly two full motor
        // revolutions of samples and checking the second revolution matches
        // the first.
        let rpm = 6000.0;
        let num_valves = 3;
        let duty_cycle = 0.5;
        let sample_rate = 44100.0;

        let mut pump = PumpSource::new(rpm, num_valves, duty_cycle, sample_rate);

        // Motor frequency = RPM / 60 = 100 Hz
        // Period in samples = sample_rate / motor_freq = 441
        let motor_freq = rpm / 60.0;
        let period_samples = (sample_rate / motor_freq).round() as usize;

        // Generate two full periods worth of samples
        let samples = pump.generate(period_samples * 2);

        // Compare second period against the first
        let mut max_diff: f64 = 0.0;
        for i in 0..period_samples {
            let diff = (samples[i] - samples[i + period_samples]).abs();
            if diff > max_diff {
                max_diff = diff;
            }
        }

        assert!(
            max_diff < 1e-10,
            "Pump signal should be periodic with period {} samples (motor freq {} Hz), max diff = {}",
            period_samples,
            motor_freq,
            max_diff
        );
    }

    #[test]
    fn test_duty_cycle_affects_signal_shape() {
        // With duty_cycle = 1.0, each valve is active for the full revolution,
        // so the signal should be nonzero for (almost) all samples.
        // With duty_cycle = 0.1, each valve is active for only 10% of
        // revolution, so many more samples should be zero.
        let rpm = 3000.0;
        let num_valves = 1; // Use single valve for clarity
        let sample_rate = 44100.0;

        let mut pump_wide = PumpSource::new(rpm, num_valves, 1.0, sample_rate);
        let mut pump_narrow = PumpSource::new(rpm, num_valves, 0.1, sample_rate);

        let n = 44100; // 1 second
        let wide_samples = pump_wide.generate(n);
        let narrow_samples = pump_narrow.generate(n);

        let wide_nonzero = wide_samples.iter().filter(|&&s| s > 1e-10).count();
        let narrow_nonzero = narrow_samples.iter().filter(|&&s| s > 1e-10).count();

        assert!(
            wide_nonzero > narrow_nonzero,
            "Wider duty cycle ({}) should have more nonzero samples ({}) than narrow ({}) ({})",
            1.0,
            wide_nonzero,
            0.1,
            narrow_nonzero
        );

        // The ratio should roughly reflect the duty cycle ratio (10:1)
        let ratio = wide_nonzero as f64 / narrow_nonzero as f64;
        assert!(
            ratio > 3.0,
            "Expected wide/narrow nonzero ratio >> 1, got {}",
            ratio
        );
    }

    #[test]
    fn test_pump_output_amplitude_bounded_various_configs() {
        // Test that output stays bounded for various RPMs and valve counts
        let configs: Vec<(f64, u32, f64)> = vec![
            (1000.0, 1, 0.5),
            (6000.0, 2, 0.3),
            (12000.0, 4, 0.8),
            (300.0, 6, 1.0),
        ];

        for (rpm, num_valves, duty_cycle) in configs {
            let mut pump = PumpSource::new(rpm, num_valves, duty_cycle, 44100.0);
            let samples = pump.generate(44100);

            let max_val = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_val = samples.iter().cloned().fold(f64::INFINITY, f64::min);

            assert!(
                min_val >= 0.0,
                "Pump output should be non-negative (rpm={}, valves={}, dc={}): min={}",
                rpm,
                num_valves,
                duty_cycle,
                min_val
            );
            // Maximum possible amplitude is num_valves (when all valves peak simultaneously)
            let upper_bound = num_valves as f64 + 0.1;
            assert!(
                max_val <= upper_bound,
                "Pump output too large (rpm={}, valves={}, dc={}): max={}, bound={}",
                rpm,
                num_valves,
                duty_cycle,
                max_val,
                upper_bound
            );
        }
    }
}
