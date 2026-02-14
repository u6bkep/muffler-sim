use sim_core::audio::AudioPipeline;
use sim_core::{SimParams, SimResult};

use crate::{geometry_view, plot_view, ui, ui::UiState};

pub struct App {
    params: SimParams,
    ui_state: UiState,
    result: SimResult,
    audio: AudioPipeline,
    was_playing: bool,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext) -> Self {
        let params = SimParams::default();
        let result = sim_core::compute(&params).expect("default params must be valid");
        let audio = AudioPipeline::new();
        audio.swap_ir(result.impulse_response.clone());
        audio.set_pump_params(params.rpm, params.num_valves, params.duty_cycle);

        Self {
            params,
            ui_state: UiState::default(),
            result,
            audio,
            was_playing: false,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        geometry_view::draw_geometry(ctx, &self.params);
        let changed = ui::draw_controls(ctx, &mut self.params, &mut self.ui_state);

        if changed {
            match sim_core::compute(&self.params) {
                Ok(result) => {
                    self.result = result;
                    self.audio.swap_ir(self.result.impulse_response.clone());
                    self.audio.set_pump_params(
                        self.params.rpm,
                        self.params.num_valves,
                        self.params.duty_cycle,
                    );
                }
                Err(e) => {
                    eprintln!("Simulation error: {e}");
                }
            }
        }

        plot_view::draw_tl_plot(ctx, &self.result);

        // Handle audio play/stop toggle.
        self.audio.set_volume(self.ui_state.volume as f64);
        if self.ui_state.play_audio && !self.was_playing {
            self.audio.play();
            self.was_playing = true;
        } else if !self.ui_state.play_audio && self.was_playing {
            self.audio.stop();
            self.was_playing = false;
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.audio.stop();
    }
}
