# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo check                          # Fast type-check (whole workspace)
cargo build                          # Debug build
cargo build --release                # Optimized build
cargo test -p sim-core               # Run all 19 unit tests
cargo test -p sim-core <test_name>   # Run a single test by name
cargo test -p sim-core <test_name> -- --nocapture  # See eprintln output
cargo run -p air-sim                 # Launch the GUI application
cargo run -p sim-core --example audio_test  # CLI audio test (3s playback)
```

## Architecture

Three-crate workspace: `sim-core` (pure computation, no GUI deps) → `sim-render` (eframe + egui) → `air-sim` (thin binary).

### sim-core: Acoustic Simulation Engine

Implements the **Transfer Matrix Method (TMM)** for expansion chamber muffler analysis.

**Data flow**: `SimParams` → `Muffler::from_params()` builds element chain → `frequency_response::sweep()` computes TL(f) and H(f) at 4096 FFT bins → `impulse_response::compute()` does IRFFT + Hann window → `SimResult` with frequencies, TL, transfer function, and IR.

Key types:
- `SimParams` / `SimResult` — shared interface between all crates
- `AcousticElement` trait — implement this to add new duct/chamber types (only `StraightDuct` exists now)
- `TransferMatrix` — 2×2 complex ABCD matrix with `chain()`, `transmission_loss()`, `pressure_transfer()`
- `Muffler` — ordered chain of `AcousticElement`s with source/load impedances
- `AudioPipeline` — manages feeder thread (pump → convolution → ring buffer) and cpal stream

### sim-render: eframe + egui UI

`App` implements `eframe::App`. On each frame: draw geometry, controls, recompute sim if changed, draw plot. If any slider changed, `sim_core::compute()` reruns and the IR is hot-swapped into the audio pipeline.

Panels: top = geometry cross-section, right = parameter sliders, center = TL plot.

### Thread Model

- **Main thread**: eframe event loop, egui UI, synchronous `compute()` on param change
- **Feeder thread** (spawned by `AudioPipeline::play()`): generates pump samples in 512-sample blocks, convolves with IR, pushes to `Arc<Mutex<VecDeque<f64>>>` ring buffer
- **cpal callback thread**: pulls from ring buffer, applies volume, outputs to device

IR hot-swap and pump param updates use `Arc<Mutex<_>>`. Play/stop uses `AtomicBool`.

## Critical Version Pins

`eframe 0.31` bundles `egui 0.31` + `winit 0.30` + wgpu/glow backends. `cpal` is `0.15` (not 0.17).

## Key Invariants

- `SimParams` stores all dimensions in **metres**. The UI converts mm ↔ m.
- `realfft` requires DC (bin 0) and Nyquist (last bin) to have **zero imaginary parts** — `impulse_response::compute()` enforces this.
- The analytical validation test (`test_expansion_chamber_analytical_validation`) compares TMM against the closed-form TL formula at 991 frequency points with <0.01 dB tolerance. Any change to `TransferMatrix`, `StraightDuct`, or `Muffler` must keep this test passing.
- `ConvolutionEngine.impulse_response` is an `Arc<Mutex<Vec<f64>>>` shared between the feeder thread and the outside world for lock-free hot-swap.

## Git Worktrees

```
air-sim/
  main/           # Primary checkout (main branch) — work here
  wt-sim-core/    # feature/sim-core
  wt-audio/       # feature/audio-pipeline
  wt-renderer/    # feature/renderer
```
