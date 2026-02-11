use std::sync::{Arc, Mutex};

/// Overlap-add convolution engine.
///
/// Processes audio in fixed-size blocks, convolving with a hot-swappable
/// impulse response.
pub struct ConvolutionEngine {
    /// Current impulse response (time domain).
    impulse_response: Arc<Mutex<Vec<f64>>>,
    /// Block size for overlap-add processing.
    block_size: usize,
    /// Overlap buffer (tail from previous block).
    overlap: Vec<f64>,
}

impl ConvolutionEngine {
    pub fn new(block_size: usize) -> Self {
        Self {
            impulse_response: Arc::new(Mutex::new(vec![0.0])),
            block_size,
            overlap: Vec::new(),
        }
    }

    /// Get a handle to the impulse response for hot-swapping.
    pub fn ir_handle(&self) -> Arc<Mutex<Vec<f64>>> {
        Arc::clone(&self.impulse_response)
    }

    /// Process a block of input samples through the convolution engine.
    pub fn process(&mut self, input: &[f64]) -> Vec<f64> {
        let ir = self.impulse_response.lock().unwrap().clone();
        let out_len = input.len() + ir.len() - 1;
        let mut convolved = vec![0.0; out_len];

        // Direct convolution (brute force for now; fine for block_size=512, IR≤2048)
        for (i, &x) in input.iter().enumerate() {
            for (j, &h) in ir.iter().enumerate() {
                convolved[i + j] += x * h;
            }
        }

        // Add overlap from previous block
        let overlap_len = self.overlap.len().min(convolved.len());
        for i in 0..overlap_len {
            convolved[i] += self.overlap[i];
        }

        // Split into output (block_size samples) and new overlap
        let output_len = input.len().min(convolved.len());
        let output = convolved[..output_len].to_vec();
        self.overlap = if convolved.len() > output_len {
            convolved[output_len..].to_vec()
        } else {
            Vec::new()
        };

        output
    }
}

/// Audio output pipeline managing pump generation, convolution, and cpal output.
pub struct AudioPipeline {
    /// Whether audio is currently playing.
    pub playing: bool,
    /// Output volume (0–1).
    pub volume: f64,
}

impl AudioPipeline {
    pub fn new() -> Self {
        Self {
            playing: false,
            volume: 0.5,
        }
    }
}
