// TL plot via egui_plot — Phase 3 implementation.

use egui_plot::{Line, Plot};
use sim_core::SimResult;

/// Draw the transmission loss plot in the central panel.
pub fn draw_tl_plot(ctx: &egui::Context, result: &SimResult) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.heading("Transmission Loss");

        // Build plot points from simulation result
        let points: Vec<[f64; 2]> = result
            .frequencies
            .iter()
            .zip(result.transmission_loss.iter())
            .filter(|(&f, _)| f > 0.0) // skip DC for cleaner plot
            .map(|(&f, &tl)| [f, tl])
            .collect();

        let line = Line::new(points).name("TL (dB)");

        Plot::new("tl_plot")
            .x_axis_label("Frequency (Hz)")
            .y_axis_label("TL (dB)")
            .legend(egui_plot::Legend::default())
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });
    });
}
