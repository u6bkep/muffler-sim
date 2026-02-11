// ApplicationHandler, event loop, state orchestration.

use std::cell::Cell;

use egui_winit_vulkano::{Gui, GuiConfig};
use sim_core::audio::AudioPipeline;
use sim_core::{SimParams, SimResult};
use vulkano::sync::GpuFuture;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::WindowId,
};

use crate::{geometry_view, plot_view, renderer::Renderer, ui, ui::UiState};

pub struct App {
    renderer: Option<Renderer>,
    gui: Option<Gui>,
    params: SimParams,
    ui_state: UiState,
    result: SimResult,
    audio: AudioPipeline,
    /// Track previous audio toggle state to detect edges.
    was_playing: bool,
}

impl App {
    pub fn new() -> Self {
        let params = SimParams::default();
        let result = sim_core::compute(&params).expect("default params must be valid");
        let audio = AudioPipeline::new();
        // Pre-load the impulse response from the default params.
        audio.swap_ir(result.impulse_response.clone());
        audio.set_pump_params(params.rpm, params.num_valves, params.duty_cycle);

        Self {
            renderer: None,
            gui: None,
            params,
            ui_state: UiState::default(),
            result,
            audio,
            was_playing: false,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let renderer = Renderer::new(event_loop);

        let gui = Gui::new(
            event_loop,
            renderer.surface.clone(),
            renderer.queue.clone(),
            renderer.swapchain_format(),
            GuiConfig {
                is_overlay: false,
                ..Default::default()
            },
        );

        self.renderer = Some(renderer);
        self.gui = Some(gui);

        // Request the very first frame.
        self.renderer.as_ref().unwrap().window.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Let egui process the event first.
        if let Some(gui) = self.gui.as_mut() {
            gui.update(&event);
        }

        match event {
            WindowEvent::CloseRequested => {
                self.audio.stop();
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.recreate_swapchain = true;
                }
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
                return; // already rendering — no need to request another redraw
            }
            _ => {}
        }

        // Any input / resize event means egui state may have changed — repaint.
        if let Some(renderer) = self.renderer.as_ref() {
            renderer.window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Intentionally empty — only repaint in response to window events so
        // the event loop sleeps when idle instead of busy-looping at 100 % CPU.
    }
}

impl App {
    fn render_frame(&mut self) {
        let renderer = match self.renderer.as_mut() {
            Some(r) => r,
            None => return,
        };

        let (image_index, acquire_future) = match renderer.begin_frame() {
            Some(r) => r,
            None => return,
        };

        let before_future = renderer.take_previous_frame_end().join(acquire_future);

        // Run the egui immediate-mode UI.
        let changed = Cell::new(false);
        {
            let gui = match self.gui.as_mut() {
                Some(g) => g,
                None => return,
            };
            let params = &mut self.params;
            let ui_state = &mut self.ui_state;
            let result = &self.result;

            gui.immediate_ui(|gui| {
                let ctx = gui.context();
                geometry_view::draw_geometry(&ctx, params);
                let c = ui::draw_controls(&ctx, params, ui_state);
                plot_view::draw_tl_plot(&ctx, result);
                changed.set(c);
            });
        }

        // Re-run simulation if any parameter changed.
        if changed.get() {
            match sim_core::compute(&self.params) {
                Ok(result) => {
                    self.result = result;
                    // Hot-swap impulse response into audio pipeline.
                    self.audio.swap_ir(self.result.impulse_response.clone());
                    // Update pump params in audio pipeline.
                    self.audio.set_pump_params(
                        self.params.rpm,
                        self.params.num_valves,
                        self.params.duty_cycle,
                    );
                    // The plot was drawn with the old result; schedule one more frame
                    // so the updated TL curve is shown without waiting for user input.
                    if let Some(r) = self.renderer.as_ref() {
                        r.window.request_redraw();
                    }
                }
                Err(e) => {
                    eprintln!("Simulation error: {e}");
                    // Keep previous self.result; continue rendering the frame.
                }
            }
        }

        // Handle audio play/stop toggle.
        self.audio.set_volume(self.ui_state.volume as f64);
        if self.ui_state.play_audio && !self.was_playing {
            self.audio.play();
            self.was_playing = true;
        } else if !self.ui_state.play_audio && self.was_playing {
            self.audio.stop();
            self.was_playing = false;
        }

        // Draw egui onto the swapchain image.
        let renderer = match self.renderer.as_ref() {
            Some(r) => r,
            None => return,
        };
        let image_view = renderer.image_views[image_index as usize].clone();
        let gui = match self.gui.as_mut() {
            Some(g) => g,
            None => return,
        };
        let after_future = gui.draw_on_image(before_future, image_view);

        // Present.
        let renderer = match self.renderer.as_mut() {
            Some(r) => r,
            None => return,
        };
        let final_future = renderer.present(after_future, image_index);
        renderer.end_frame(final_future);
    }
}
