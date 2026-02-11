// egui control panel: sliders, toggles, readouts — Phase 3 implementation.

use sim_core::SimParams;

/// Extra UI-only state that doesn't belong in SimParams.
pub struct UiState {
    pub play_audio: bool,
    pub volume: f32,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            play_audio: false,
            volume: 0.5,
        }
    }
}

/// Draw the right-side control panel. Returns `true` if any simulation
/// parameter changed (meaning the sim needs to be re-run).
pub fn draw_controls(
    ctx: &egui::Context,
    params: &mut SimParams,
    ui_state: &mut UiState,
) -> bool {
    let mut changed = false;

    egui::SidePanel::right("controls")
        .min_width(260.0)
        .show(ctx, |ui| {
            ui.heading("Muffler Parameters");
            ui.separator();

            // --- Chamber ---
            ui.label("Chamber Diameter (mm)");
            let mut chamber_diam_mm = (params.chamber_diameter * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut chamber_diam_mm, 10.0..=100.0))
                .changed()
            {
                params.chamber_diameter = chamber_diam_mm as f64 / 1000.0;
                changed = true;
            }

            ui.label("Chamber Length (mm)");
            let mut chamber_len_mm = (params.chamber_length * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut chamber_len_mm, 10.0..=300.0))
                .changed()
            {
                params.chamber_length = chamber_len_mm as f64 / 1000.0;
                changed = true;
            }

            ui.separator();

            // --- Inlet ---
            ui.label("Inlet Diameter (mm)");
            let mut inlet_diam_mm = (params.inlet_diameter * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut inlet_diam_mm, 2.0..=20.0))
                .changed()
            {
                params.inlet_diameter = inlet_diam_mm as f64 / 1000.0;
                changed = true;
            }

            ui.label("Inlet Length (mm)");
            let mut inlet_len_mm = (params.inlet_length * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut inlet_len_mm, 5.0..=200.0))
                .changed()
            {
                params.inlet_length = inlet_len_mm as f64 / 1000.0;
                changed = true;
            }

            ui.separator();

            // --- Outlet ---
            ui.label("Outlet Diameter (mm)");
            let mut outlet_diam_mm = (params.outlet_diameter * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut outlet_diam_mm, 2.0..=20.0))
                .changed()
            {
                params.outlet_diameter = outlet_diam_mm as f64 / 1000.0;
                changed = true;
            }

            ui.label("Outlet Length (mm)");
            let mut outlet_len_mm = (params.outlet_length * 1000.0) as f32;
            if ui
                .add(egui::Slider::new(&mut outlet_len_mm, 5.0..=200.0))
                .changed()
            {
                params.outlet_length = outlet_len_mm as f64 / 1000.0;
                changed = true;
            }

            ui.separator();

            // --- Pump ---
            ui.label("Pump RPM");
            let mut rpm = params.rpm as f32;
            if ui
                .add(egui::Slider::new(&mut rpm, 500.0..=10000.0))
                .changed()
            {
                params.rpm = rpm as f64;
                changed = true;
            }

            ui.label("Num Valves");
            let mut num_valves = params.num_valves as i32;
            if ui
                .add(egui::Slider::new(&mut num_valves, 1..=6))
                .changed()
            {
                params.num_valves = num_valves as u32;
                changed = true;
            }

            ui.label("Duty Cycle");
            let mut duty = params.duty_cycle as f32;
            if ui
                .add(egui::Slider::new(&mut duty, 0.1..=0.9))
                .changed()
            {
                params.duty_cycle = duty as f64;
                changed = true;
            }

            ui.separator();

            // --- Environment ---
            ui.label("Temperature (°C)");
            let mut temp = params.temperature as f32;
            if ui
                .add(egui::Slider::new(&mut temp, -20.0..=60.0))
                .changed()
            {
                params.temperature = temp as f64;
                changed = true;
            }

            ui.separator();

            // --- Audio ---
            if ui
                .add(egui::Button::new(if ui_state.play_audio {
                    "Stop Audio"
                } else {
                    "Play Audio"
                }))
                .clicked()
            {
                ui_state.play_audio = !ui_state.play_audio;
            }

            ui.label("Volume");
            ui.add(egui::Slider::new(&mut ui_state.volume, 0.0..=1.0));
        });

    changed
}
