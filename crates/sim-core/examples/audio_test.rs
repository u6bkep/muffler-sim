//! CLI test harness for the audio pipeline.
//!
//! Builds a default muffler simulation, computes its impulse response,
//! feeds it into the AudioPipeline, plays for 3 seconds, then exits.
//!
//! Run with:
//!   cargo run -p sim-core --example audio_test

use sim_core::audio::AudioPipeline;
use sim_core::SimParams;

fn main() {
    println!("=== Audio Pipeline Test ===");

    // 1. Run the simulation to get an impulse response.
    let params = SimParams::default();
    println!(
        "SimParams: RPM={}, valves={}, duty_cycle={:.2}",
        params.rpm, params.num_valves, params.duty_cycle
    );
    println!("Computing impulse response...");

    let result = sim_core::compute(&params);
    println!(
        "IR length: {} samples, sample_rate: {} Hz",
        result.impulse_response.len(),
        result.sample_rate
    );

    // 2. Create and configure the audio pipeline.
    let mut pipeline = AudioPipeline::new();
    pipeline.set_pump_params(params.rpm, params.num_valves, params.duty_cycle);
    pipeline.set_volume(0.3);

    // 3. Hot-swap in the computed impulse response.
    pipeline.swap_ir(result.impulse_response);
    println!("IR loaded into pipeline.");

    // 4. Start playback.
    println!("Starting playback for 3 seconds...");
    pipeline.play();

    std::thread::sleep(std::time::Duration::from_secs(3));

    // 5. Stop and exit.
    println!("Stopping playback.");
    pipeline.stop();
    println!("Done.");
}
