use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};

use crate::pump::PumpSource;

// ---------------------------------------------------------------------------
// ConvolutionEngine
// ---------------------------------------------------------------------------

/// Overlap-add convolution engine.
///
/// Processes audio in fixed-size blocks, convolving with a hot-swappable
/// impulse response. The engine maintains an overlap buffer so that block
/// boundaries are seamless (no clicks).
pub struct ConvolutionEngine {
    /// Current impulse response (time domain), shared for hot-swap.
    impulse_response: Arc<Mutex<Vec<f64>>>,
    /// Block size for overlap-add processing (informational; `process()` works
    /// with any input length).
    #[allow(dead_code)]
    block_size: usize,
    /// Overlap buffer (tail from previous convolution that must be added to
    /// the beginning of the next block's output).
    overlap: Vec<f64>,
}

impl ConvolutionEngine {
    pub fn new(block_size: usize) -> Self {
        // Start with a unit impulse (delta) so pass-through works immediately.
        Self {
            impulse_response: Arc::new(Mutex::new(vec![1.0])),
            block_size,
            overlap: Vec::new(),
        }
    }

    /// Get a handle to the impulse response for hot-swapping from another
    /// thread (e.g. the simulation thread calls `swap_ir` via this handle).
    pub fn ir_handle(&self) -> Arc<Mutex<Vec<f64>>> {
        Arc::clone(&self.impulse_response)
    }

    /// Process a block of input samples through overlap-add convolution.
    ///
    /// The returned vector always has exactly `input.len()` samples; any
    /// excess (the "tail") is stored internally and added to the next block.
    pub fn process(&mut self, input: &[f64]) -> Vec<f64> {
        let ir = self.impulse_response.lock().unwrap_or_else(|e| e.into_inner()).clone();

        // Handle degenerate cases
        if ir.is_empty() || input.is_empty() {
            return vec![0.0; input.len()];
        }

        let conv_len = input.len() + ir.len() - 1;
        let mut convolved = vec![0.0; conv_len];

        // Direct (time-domain) convolution.
        // Fine for block_size = 512 and IR length up to ~2048.
        for (i, &x) in input.iter().enumerate() {
            for (j, &h) in ir.iter().enumerate() {
                convolved[i + j] += x * h;
            }
        }

        // Add the overlap (tail) from the *previous* block.
        let overlap_add_len = self.overlap.len().min(conv_len);
        for i in 0..overlap_add_len {
            convolved[i] += self.overlap[i];
        }
        // If the old overlap was longer than the new convolved result (can
        // happen when IR shrinks via hot-swap), carry the remainder forward.
        if self.overlap.len() > conv_len {
            // This case is unusual but handled for correctness.
            let leftover = self.overlap[conv_len..].to_vec();
            self.overlap = leftover;
        } else {
            self.overlap.clear();
        }

        // Split: first `input.len()` samples are the output; the rest become
        // the new overlap for the next block.
        let n = input.len();
        let output = convolved[..n].to_vec();
        if conv_len > n {
            // Merge any remaining old overlap that extends beyond our output
            let new_tail = &convolved[n..];
            // Extend existing overlap (which may have leftover from above)
            let needed = new_tail.len().max(self.overlap.len());
            let mut merged = vec![0.0; needed];
            for (i, &v) in self.overlap.iter().enumerate() {
                merged[i] += v;
            }
            for (i, &v) in new_tail.iter().enumerate() {
                merged[i] += v;
            }
            self.overlap = merged;
        }

        output
    }
}

// ---------------------------------------------------------------------------
// AudioPipeline
// ---------------------------------------------------------------------------

/// Shared ring buffer between the feeder thread and the cpal callback.
type RingBuffer = Arc<Mutex<VecDeque<f64>>>;

/// Audio output pipeline managing pump generation, convolution, and cpal output.
///
/// Architecture:
///   - A *feeder thread* generates pump samples in 512-sample blocks,
///     convolves them through the `ConvolutionEngine`, and pushes results
///     into a ring buffer (`VecDeque<f64>` behind `Arc<Mutex<_>>`).
///   - The cpal stream callback pulls samples from the ring buffer,
///     multiplies by the volume scalar, and writes them to the output.
///   - If the ring buffer is empty the callback outputs silence.
pub struct AudioPipeline {
    /// Whether audio is currently playing.
    playing: Arc<AtomicBool>,
    /// Output volume (0.0 to 1.0).
    volume: Arc<Mutex<f64>>,
    /// Handle into the ConvolutionEngine's IR for hot-swap.
    ir_handle: Arc<Mutex<Vec<f64>>>,
    /// Handle into the PumpSource parameters.
    pump_params: Arc<Mutex<PumpParams>>,
    /// Sample rate used by the pipeline.
    sample_rate: f64,
    /// Block size used by the feeder.
    block_size: usize,
    /// cpal stream (held to keep it alive; dropped on stop).
    stream: Option<Stream>,
    /// Join handle for the feeder thread.
    feeder_handle: Option<thread::JoinHandle<()>>,
    /// Signal the feeder thread to shut down.
    feeder_running: Arc<AtomicBool>,
}

/// Snapshot of pump parameters, shared between the main thread and the feeder.
#[derive(Clone)]
struct PumpParams {
    rpm: f64,
    num_valves: u32,
    duty_cycle: f64,
}

impl AudioPipeline {
    /// Create a new audio pipeline.  Does *not* start playback.
    pub fn new() -> Self {
        let block_size = 512;
        let sample_rate = 44_100.0;

        // The ConvolutionEngine lives in the feeder thread, but we keep a
        // clone of its IR handle so the outside world can hot-swap.
        let engine = ConvolutionEngine::new(block_size);
        let ir_handle = engine.ir_handle();

        let pump_params = PumpParams {
            rpm: 3000.0,
            num_valves: 3,
            duty_cycle: 0.5,
        };

        Self {
            playing: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(Mutex::new(0.5)),
            ir_handle,
            pump_params: Arc::new(Mutex::new(pump_params)),
            sample_rate,
            block_size,
            stream: None,
            feeder_handle: None,
            feeder_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Replace the impulse response used by the convolution engine.
    ///
    /// This is thread-safe and can be called from the simulation thread
    /// while audio is playing.
    pub fn swap_ir(&self, ir: Vec<f64>) {
        // Reject IR containing non-finite values (NaN, inf).
        if !ir.iter().all(|v| v.is_finite()) {
            eprintln!("swap_ir: rejected IR with non-finite values; keeping previous IR");
            return;
        }
        let mut guard = self.ir_handle.lock().unwrap_or_else(|e| e.into_inner());
        *guard = ir;
    }

    /// Update the pump source parameters without restarting the stream.
    pub fn set_pump_params(&self, rpm: f64, num_valves: u32, duty_cycle: f64) {
        let mut guard = self.pump_params.lock().unwrap_or_else(|e| e.into_inner());
        guard.rpm = rpm;
        guard.num_valves = num_valves;
        guard.duty_cycle = duty_cycle;
    }

    /// Set output volume (clamped to 0.0..=1.0).
    pub fn set_volume(&self, vol: f64) {
        let mut guard = self.volume.lock().unwrap_or_else(|e| e.into_inner());
        *guard = vol.clamp(0.0, 1.0);
    }

    /// Returns true if the pipeline is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// Start audio playback: opens the default output device, spawns the
    /// feeder thread, and begins streaming.
    pub fn play(&mut self) {
        if self.playing.load(Ordering::Relaxed) {
            return; // already playing
        }

        // -- cpal device setup ------------------------------------------------
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .expect("No default audio output device found");

        let supported_config = device
            .default_output_config()
            .expect("No default output config");
        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();

        let actual_sample_rate = config.sample_rate.0 as f64;
        let channels = config.channels as usize;

        // Update our record of the sample rate (the device may differ from 44100)
        self.sample_rate = actual_sample_rate;

        // -- Shared ring buffer -----------------------------------------------
        // Pre-allocate capacity for ~100 ms of audio as headroom.
        let capacity = (actual_sample_rate * 0.1) as usize;
        let ring: RingBuffer = Arc::new(Mutex::new(VecDeque::with_capacity(capacity)));

        // -- Feeder thread ----------------------------------------------------
        let feeder_ring = Arc::clone(&ring);
        let feeder_ir = Arc::clone(&self.ir_handle);
        let feeder_pump = Arc::clone(&self.pump_params);
        let feeder_running = Arc::clone(&self.feeder_running);
        let block_size = self.block_size;

        self.feeder_running.store(true, Ordering::Relaxed);

        let feeder_handle = thread::spawn(move || {
            // The ConvolutionEngine and PumpSource live entirely in this thread.
            let mut engine = ConvolutionEngine::new(block_size);
            // Point the engine's IR at the shared handle so hot-swaps are visible.
            engine.impulse_response = feeder_ir;

            let params = feeder_pump.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let mut pump = PumpSource::new(
                params.rpm,
                params.num_valves,
                params.duty_cycle,
                actual_sample_rate,
            );

            // Maximum ring buffer occupancy before we sleep (avoid unbounded growth).
            let max_buffered = block_size * 8;

            while feeder_running.load(Ordering::Relaxed) {
                // Refresh pump parameters each block (cheap lock).
                {
                    let p = feeder_pump.lock().unwrap_or_else(|e| e.into_inner());
                    pump.set_params(p.rpm, p.num_valves, p.duty_cycle);
                }

                // Check ring buffer level; if already full enough, sleep briefly.
                {
                    let buf = feeder_ring.lock().unwrap_or_else(|e| e.into_inner());
                    if buf.len() >= max_buffered {
                        drop(buf);
                        thread::sleep(std::time::Duration::from_millis(5));
                        continue;
                    }
                }

                // Generate and convolve a block.
                let raw = pump.generate(block_size);
                let processed = engine.process(&raw);

                // Push into ring buffer.
                {
                    let mut buf = feeder_ring.lock().unwrap_or_else(|e| e.into_inner());
                    for &s in &processed {
                        buf.push_back(s);
                    }
                }
            }
        });
        self.feeder_handle = Some(feeder_handle);

        // -- cpal stream callback ---------------------------------------------
        let cb_ring = Arc::clone(&ring);
        let cb_volume = Arc::clone(&self.volume);

        let err_fn = |err: cpal::StreamError| {
            eprintln!("cpal stream error: {err}");
        };

        let stream = match sample_format {
            SampleFormat::F32 => device
                .build_output_stream(
                    &config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let vol = *cb_volume.lock().unwrap_or_else(|e| e.into_inner());
                        let mut buf = cb_ring.lock().unwrap_or_else(|e| e.into_inner());
                        for frame in data.chunks_mut(channels) {
                            let sample = buf.pop_front().unwrap_or(0.0) * vol;
                            let out = sample as f32;
                            for s in frame.iter_mut() {
                                *s = out;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .expect("Failed to build f32 output stream"),
            SampleFormat::I16 => device
                .build_output_stream(
                    &config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        let vol = *cb_volume.lock().unwrap_or_else(|e| e.into_inner());
                        let mut buf = cb_ring.lock().unwrap_or_else(|e| e.into_inner());
                        for frame in data.chunks_mut(channels) {
                            let sample = buf.pop_front().unwrap_or(0.0) * vol;
                            let out = (sample * i16::MAX as f64) as i16;
                            for s in frame.iter_mut() {
                                *s = out;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .expect("Failed to build i16 output stream"),
            SampleFormat::U16 => device
                .build_output_stream(
                    &config,
                    move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        let vol = *cb_volume.lock().unwrap_or_else(|e| e.into_inner());
                        let mut buf = cb_ring.lock().unwrap_or_else(|e| e.into_inner());
                        for frame in data.chunks_mut(channels) {
                            let sample = buf.pop_front().unwrap_or(0.0) * vol;
                            let out =
                                ((sample * 0.5 + 0.5) * u16::MAX as f64) as u16;
                            for s in frame.iter_mut() {
                                *s = out;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .expect("Failed to build u16 output stream"),
            _ => {
                eprintln!("Unsupported sample format: {sample_format:?}; audio will not play");
                // Stop the feeder thread we just spawned since we can't play.
                self.feeder_running.store(false, Ordering::Relaxed);
                if let Some(handle) = self.feeder_handle.take() {
                    let _ = handle.join();
                }
                return;
            }
        };

        stream.play().expect("Failed to start cpal stream");
        self.stream = Some(stream);
        self.playing.store(true, Ordering::Relaxed);
    }

    /// Stop audio playback: drops the cpal stream and joins the feeder thread.
    pub fn stop(&mut self) {
        if !self.playing.load(Ordering::Relaxed) {
            return;
        }

        // Signal feeder to exit.
        self.feeder_running.store(false, Ordering::Relaxed);

        // Drop the stream first (stops the callback).
        self.stream.take();

        // Join the feeder thread.
        if let Some(handle) = self.feeder_handle.take() {
            let _ = handle.join();
        }

        self.playing.store(false, Ordering::Relaxed);
    }
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convolution_delta_passthrough() {
        // Convolving with a single-sample delta [1.0] should return the input
        // unchanged.
        let mut engine = ConvolutionEngine::new(8);
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let output = engine.process(&input);
        assert_eq!(output.len(), input.len());
        for (a, b) in output.iter().zip(input.iter()) {
            assert!(
                (a - b).abs() < 1e-12,
                "delta passthrough failed: {a} != {b}"
            );
        }
    }

    #[test]
    fn test_convolution_overlap_continuity() {
        // Use a two-sample IR [0.5, 0.5] (simple low-pass) and process two
        // consecutive blocks.  Verify that the boundary sample is correct.
        let mut engine = ConvolutionEngine::new(4);
        {
            let mut ir = engine.impulse_response.lock().unwrap();
            *ir = vec![0.5, 0.5];
        }

        let block1 = vec![1.0, 0.0, 0.0, 0.0];
        let out1 = engine.process(&block1);
        // Expected full convolution: [0.5, 0.5, 0.0, 0.0, 0.0]
        // out1 = first 4 samples = [0.5, 0.5, 0.0, 0.0], overlap = [0.0]
        assert_eq!(out1.len(), 4);
        assert!((out1[0] - 0.5).abs() < 1e-12);
        assert!((out1[1] - 0.5).abs() < 1e-12);
        assert!((out1[2] - 0.0).abs() < 1e-12);
        assert!((out1[3] - 0.0).abs() < 1e-12);

        let block2 = vec![0.0, 0.0, 1.0, 0.0];
        let out2 = engine.process(&block2);
        assert_eq!(out2.len(), 4);
        // The overlap from block1 (one sample of 0.0) is added to out2[0].
        // Convolution of block2 with [0.5, 0.5]: [0, 0, 0.5, 0.5, 0]
        // Plus overlap [0.0] at index 0 → same.
        assert!((out2[0] - 0.0).abs() < 1e-12);
        assert!((out2[1] - 0.0).abs() < 1e-12);
        assert!((out2[2] - 0.5).abs() < 1e-12);
        assert!((out2[3] - 0.5).abs() < 1e-12);
    }

    #[test]
    fn test_convolution_empty_ir() {
        let mut engine = ConvolutionEngine::new(4);
        {
            let mut ir = engine.impulse_response.lock().unwrap();
            *ir = vec![];
        }
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = engine.process(&input);
        assert_eq!(output.len(), 4);
        for &s in &output {
            assert!((s - 0.0).abs() < 1e-12);
        }
    }

    #[test]
    fn test_convolution_single_sample_ir() {
        // IR = [2.0] should scale input by 2.
        let mut engine = ConvolutionEngine::new(4);
        {
            let mut ir = engine.impulse_response.lock().unwrap();
            *ir = vec![2.0];
        }
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = engine.process(&input);
        assert_eq!(output.len(), 4);
        for (i, &s) in output.iter().enumerate() {
            let expected = input[i] * 2.0;
            assert!(
                (s - expected).abs() < 1e-12,
                "sample {i}: {s} != {expected}"
            );
        }
    }

    #[test]
    fn test_pipeline_creation() {
        let pipeline = AudioPipeline::new();
        assert!(!pipeline.is_playing());
    }

    #[test]
    fn test_pipeline_volume() {
        let pipeline = AudioPipeline::new();
        pipeline.set_volume(0.75);
        assert!((*pipeline.volume.lock().unwrap() - 0.75).abs() < 1e-12);

        // Clamping
        pipeline.set_volume(1.5);
        assert!((*pipeline.volume.lock().unwrap() - 1.0).abs() < 1e-12);
        pipeline.set_volume(-0.5);
        assert!((*pipeline.volume.lock().unwrap() - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_pipeline_swap_ir() {
        let pipeline = AudioPipeline::new();
        let new_ir = vec![0.5, 0.3, 0.1];
        pipeline.swap_ir(new_ir.clone());
        let stored = pipeline.ir_handle.lock().unwrap().clone();
        assert_eq!(stored, new_ir);
    }

    #[test]
    fn test_pipeline_set_pump_params() {
        let pipeline = AudioPipeline::new();
        pipeline.set_pump_params(6000.0, 5, 0.3);
        let p = pipeline.pump_params.lock().unwrap();
        assert!((p.rpm - 6000.0).abs() < 1e-12);
        assert_eq!(p.num_valves, 5);
        assert!((p.duty_cycle - 0.3).abs() < 1e-12);
    }
}
