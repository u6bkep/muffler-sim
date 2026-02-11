use crate::muffler::Muffler;
use num_complex::Complex64;
use std::f64::consts::PI;

/// Sweep the muffler's transmission loss and pressure transfer function
/// across `fft_size/2 + 1` frequency bins from 0 to `sample_rate/2`.
///
/// Returns `(frequencies, transmission_loss_db, transfer_function)`.
pub fn sweep(
    muffler: &Muffler,
    fft_size: usize,
    sample_rate: f64,
    c: f64,
    rho: f64,
) -> (Vec<f64>, Vec<f64>, Vec<Complex64>) {
    let num_bins = fft_size / 2 + 1;
    let bin_width = sample_rate / fft_size as f64;

    let mut frequencies = Vec::with_capacity(num_bins);
    let mut tl = Vec::with_capacity(num_bins);
    let mut hf = Vec::with_capacity(num_bins);

    for i in 0..num_bins {
        let freq = i as f64 * bin_width;
        let omega = 2.0 * PI * freq;

        frequencies.push(freq);

        if freq < 1.0 {
            // DC bin: no attenuation, unity transfer
            tl.push(0.0);
            hf.push(Complex64::new(1.0, 0.0));
        } else {
            tl.push(muffler.transmission_loss(omega, c, rho));
            hf.push(muffler.pressure_transfer(omega, c, rho));
        }
    }

    (frequencies, tl, hf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{area_from_diameter, speed_of_sound_and_density};
    use crate::elements::StraightDuct;
    use crate::muffler::Muffler;

    /// Analytical validation: compare TMM output against the closed-form
    /// solution for a simple expansion chamber.
    ///
    /// TL_analytical = 10·log₁₀(1 + 0.25·(m − 1/m)²·sin²(k·L_chamber))
    ///
    /// where m = S_chamber / S_pipe and k = omega / c.
    ///
    /// We construct a Muffler with ONLY the chamber duct, and set z_source
    /// and z_load to the pipe impedance (ρc/S_pipe). This models a simple
    /// expansion chamber with infinitely thin area changes.
    ///
    /// Sweep 100 Hz to 10 kHz in 10 Hz steps. Pass criterion: |error| < 0.01 dB.
    #[test]
    fn test_expansion_chamber_analytical_validation() {
        let temperature = 20.0;
        let (c, rho) = speed_of_sound_and_density(temperature);

        // Geometry
        let pipe_diameter = 6e-3; // 6 mm
        let chamber_diameter = 40e-3; // 40 mm
        let chamber_length = 80e-3; // 80 mm

        let s_pipe = area_from_diameter(pipe_diameter);
        let s_chamber = area_from_diameter(chamber_diameter);
        let m = s_chamber / s_pipe; // area ratio

        // Pipe characteristic impedance (source and load)
        let z_pipe = rho * c / s_pipe;

        // Build muffler with only the chamber element
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(
            vec![Box::new(chamber)],
            z_pipe,
            z_pipe,
        );

        // Sweep from 100 Hz to 10 kHz in 10 Hz steps
        let mut max_error: f64 = 0.0;
        let mut worst_freq: f64 = 0.0;
        let mut num_tested: usize = 0;

        let mut freq = 100.0;
        while freq <= 10_000.0 {
            let omega = 2.0 * PI * freq;
            let k = omega / c;

            // Analytical TL
            let m_term = m - 1.0 / m;
            let tl_analytical =
                10.0 * (1.0 + 0.25 * m_term * m_term * (k * chamber_length).sin().powi(2)).log10();

            // TMM TL
            let tl_tmm = muffler.transmission_loss(omega, c, rho);

            let error = (tl_tmm - tl_analytical).abs();
            if error > max_error {
                max_error = error;
                worst_freq = freq;
            }

            assert!(
                error < 0.01,
                "TL mismatch at {freq} Hz: TMM = {tl_tmm:.6} dB, analytical = {tl_analytical:.6} dB, error = {error:.6} dB"
            );

            num_tested += 1;
            freq += 10.0;
        }

        assert!(
            num_tested >= 990,
            "Expected at least 990 test points, got {num_tested}"
        );

        eprintln!(
            "Analytical validation passed: {num_tested} points, max error = {max_error:.2e} dB at {worst_freq} Hz"
        );
    }

    /// Verify that at the chamber's resonance frequencies (k·L = n·π),
    /// the TL is exactly zero (sin(kL) = 0 → TL = 0).
    #[test]
    fn test_expansion_chamber_zero_tl_at_resonances() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        let chamber_length = 80e-3;
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;

        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        // Resonance frequencies where k·L = n·π → f = n·c / (2·L)
        for n in 1..=5 {
            let freq = n as f64 * c / (2.0 * chamber_length);
            let omega = 2.0 * PI * freq;
            let tl = muffler.transmission_loss(omega, c, rho);
            assert!(
                tl.abs() < 1e-10,
                "TL should be 0 at resonance f = {freq:.1} Hz (n={n}), got {tl}"
            );
        }
    }

    /// Verify that at the chamber's peak-attenuation frequencies
    /// (k·L = (2n-1)·π/2), the TL matches the peak formula:
    /// TL_peak = 10·log₁₀(1 + 0.25·(m − 1/m)²)
    #[test]
    fn test_expansion_chamber_peak_tl() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        let chamber_length = 80e-3;
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;

        let s_pipe = area_from_diameter(pipe_diameter);
        let s_chamber = area_from_diameter(chamber_diameter);
        let m = s_chamber / s_pipe;
        let z_pipe = rho * c / s_pipe;

        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let m_term = m - 1.0 / m;
        let tl_peak_expected = 10.0 * (1.0 + 0.25 * m_term * m_term).log10();

        // Peak frequencies where k·L = (2n-1)·π/2 → f = (2n-1)·c / (4·L)
        for n in 1..=4 {
            let freq = (2 * n - 1) as f64 * c / (4.0 * chamber_length);
            let omega = 2.0 * PI * freq;
            let tl = muffler.transmission_loss(omega, c, rho);
            let error = (tl - tl_peak_expected).abs();
            assert!(
                error < 1e-10,
                "TL should be {tl_peak_expected:.4} dB at peak f = {freq:.1} Hz (n={n}), got {tl:.6}, error = {error:.2e}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Test Group 2: Frequency sweep correctness
    // -----------------------------------------------------------------------

    #[test]
    fn test_sweep_correct_bin_count() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;
        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let fft_size = 4096;
        let sample_rate = 44100.0;
        let expected_bins = fft_size / 2 + 1; // 2049

        let (frequencies, tl, hf) = sweep(&muffler, fft_size, sample_rate, c, rho);

        assert_eq!(
            frequencies.len(),
            expected_bins,
            "frequencies should have fft_size/2 + 1 = {} bins, got {}",
            expected_bins,
            frequencies.len()
        );
        assert_eq!(
            tl.len(),
            expected_bins,
            "tl should have {} bins, got {}",
            expected_bins,
            tl.len()
        );
        assert_eq!(
            hf.len(),
            expected_bins,
            "hf should have {} bins, got {}",
            expected_bins,
            hf.len()
        );
    }

    #[test]
    fn test_sweep_dc_and_nyquist_real() {
        // DC and Nyquist bins should have zero imaginary part in hf,
        // because the sweep function sets them to unity (DC) and computes
        // a real-valued transfer function at Nyquist.
        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;
        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let fft_size = 4096;
        let sample_rate = 44100.0;
        let (_, _, hf) = sweep(&muffler, fft_size, sample_rate, c, rho);

        // DC bin (index 0): sweep sets this to Complex64::new(1.0, 0.0)
        let dc = hf[0];
        assert!(
            dc.im.abs() < 1e-15,
            "DC bin imaginary part should be zero, got {}",
            dc.im
        );
        assert!(
            (dc.re - 1.0).abs() < 1e-15,
            "DC bin real part should be 1.0, got {}",
            dc.re
        );
    }

    #[test]
    fn test_sweep_frequency_bins_evenly_spaced() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;
        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let fft_size = 4096;
        let sample_rate = 44100.0;
        let (frequencies, _, _) = sweep(&muffler, fft_size, sample_rate, c, rho);

        let expected_bin_width = sample_rate / fft_size as f64;

        // Check first frequency is 0 (DC)
        assert!(
            frequencies[0].abs() < 1e-15,
            "First frequency bin should be 0.0 Hz (DC), got {}",
            frequencies[0]
        );

        // Check last frequency is Nyquist
        let expected_nyquist = (fft_size / 2) as f64 * expected_bin_width;
        assert!(
            (frequencies.last().unwrap() - expected_nyquist).abs() < 1e-10,
            "Last frequency bin should be Nyquist ({} Hz), got {}",
            expected_nyquist,
            frequencies.last().unwrap()
        );

        // Verify spacing between consecutive bins
        for i in 1..frequencies.len() {
            let spacing = frequencies[i] - frequencies[i - 1];
            assert!(
                (spacing - expected_bin_width).abs() < 1e-10,
                "Bin spacing at index {} should be {}, got {}",
                i,
                expected_bin_width,
                spacing
            );
        }
    }

    #[test]
    fn test_sweep_all_tl_values_finite() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        let pipe_diameter = 6e-3;
        let chamber_diameter = 40e-3;
        let chamber_length = 80e-3;
        let z_pipe = rho * c / area_from_diameter(pipe_diameter);
        let chamber = StraightDuct::new(chamber_length, chamber_diameter);
        let muffler = Muffler::new(vec![Box::new(chamber)], z_pipe, z_pipe);

        let fft_size = 4096;
        let sample_rate = 44100.0;
        let (_, tl, hf) = sweep(&muffler, fft_size, sample_rate, c, rho);

        for (i, &tl_val) in tl.iter().enumerate() {
            assert!(
                tl_val.is_finite(),
                "TL at bin {} must be finite, got {}",
                i,
                tl_val
            );
        }

        for (i, &hf_val) in hf.iter().enumerate() {
            assert!(
                hf_val.norm().is_finite(),
                "H(f) at bin {} must have finite magnitude, got {}",
                i,
                hf_val
            );
        }
    }
}
