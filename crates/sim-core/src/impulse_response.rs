use num_complex::Complex64;
use realfft::RealFftPlanner;
use std::f64::consts::PI;

/// Convert a frequency-domain transfer function H(f) (N/2+1 complex bins)
/// into a time-domain impulse response h(t) of length `fft_size`.
///
/// Applies a Hann window and truncates to `fft_size / 2` samples.
pub fn compute(transfer_function: &[Complex64], fft_size: usize) -> Vec<f64> {
    let expected_bins = fft_size / 2 + 1;
    assert_eq!(
        transfer_function.len(),
        expected_bins,
        "H(f) length must be fft_size/2 + 1"
    );

    // IRFFT: complex spectrum → real time-domain
    let mut planner = RealFftPlanner::<f64>::new();
    let ifft = planner.plan_fft_inverse(fft_size);

    let mut spectrum: Vec<_> = transfer_function
        .iter()
        .map(|&c| realfft::num_complex::Complex { re: c.re, im: c.im })
        .collect();

    // realfft requires DC and Nyquist bins to be purely real.
    // Force imaginary parts to zero (use magnitude to preserve energy).
    spectrum[0].im = 0.0;
    let last = spectrum.len() - 1;
    spectrum[last].im = 0.0;

    let mut output = vec![0.0f64; fft_size];

    ifft.process(&mut spectrum, &mut output)
        .expect("IRFFT failed");

    // Normalize by fft_size (realfft convention)
    let norm = 1.0 / fft_size as f64;
    for s in &mut output {
        *s *= norm;
    }

    // Apply Hann window and truncate to fft_size/2
    let ir_len = fft_size / 2;
    let mut ir = Vec::with_capacity(ir_len);
    for i in 0..ir_len {
        let window = 0.5 * (1.0 - (2.0 * PI * i as f64 / ir_len as f64).cos());
        ir.push(output[i] * window);
    }

    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_impulse_identity() {
        // Unity transfer function → delta-like impulse
        let fft_size = 256;
        let bins = fft_size / 2 + 1;
        let hf = vec![Complex64::new(1.0, 0.0); bins];
        let ir = compute(&hf, fft_size);
        assert_eq!(ir.len(), fft_size / 2);
        // First sample should be the largest (near delta)
        let max_val = ir.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert_eq!(ir[0], max_val);
    }
}
