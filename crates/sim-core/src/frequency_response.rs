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
