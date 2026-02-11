// 2D muffler cross-section drawn with egui painter — Phase 3 implementation.

use sim_core::SimParams;

/// Draw a simplified 2D cross-section of the muffler in a top panel.
///
/// Three rectangles represent the inlet pipe, expansion chamber, and outlet pipe.
/// Widths are proportional to pipe/chamber lengths, heights to diameters.
pub fn draw_geometry(ctx: &egui::Context, params: &SimParams) {
    egui::TopBottomPanel::top("geometry")
        .min_height(120.0)
        .show(ctx, |ui| {
            ui.heading("Muffler Cross-Section");

            let available = ui.available_size();
            let (response, painter) =
                ui.allocate_painter(available, egui::Sense::hover());
            let rect = response.rect;

            // Compute scale so the full muffler fits in the available width
            // with some padding.
            let total_length_m =
                params.inlet_length + params.chamber_length + params.outlet_length;
            let max_diameter_m = params
                .chamber_diameter
                .max(params.inlet_diameter)
                .max(params.outlet_diameter);

            if total_length_m <= 0.0 || max_diameter_m <= 0.0 {
                return;
            }

            let padding = 20.0;
            let draw_width = rect.width() - 2.0 * padding;
            let draw_height = rect.height() - 2.0 * padding;

            let scale_x = draw_width / total_length_m as f32;
            let scale_y = draw_height / max_diameter_m as f32;

            let center_y = rect.center().y;
            let start_x = rect.left() + padding;

            // Helper to draw a pipe/chamber segment as a centered rectangle.
            let draw_segment =
                |painter: &egui::Painter, x: f32, length_m: f64, diameter_m: f64, color: egui::Color32| {
                    let w = length_m as f32 * scale_x;
                    let h = diameter_m as f32 * scale_y;
                    let segment_rect = egui::Rect::from_center_size(
                        egui::pos2(x + w / 2.0, center_y),
                        egui::vec2(w, h),
                    );
                    painter.rect_filled(segment_rect, 2.0, color);
                    painter.rect_stroke(
                        segment_rect,
                        2.0,
                        egui::Stroke::new(1.5, egui::Color32::WHITE),
                        egui::StrokeKind::Outside,
                    );
                    w
                };

            // Draw inlet pipe
            let mut x = start_x;
            let inlet_color = egui::Color32::from_rgb(80, 120, 180);
            let w = draw_segment(&painter, x, params.inlet_length, params.inlet_diameter, inlet_color);
            x += w;

            // Draw expansion chamber
            let chamber_color = egui::Color32::from_rgb(180, 100, 60);
            let w = draw_segment(&painter, x, params.chamber_length, params.chamber_diameter, chamber_color);
            x += w;

            // Draw outlet pipe
            let outlet_color = egui::Color32::from_rgb(80, 160, 120);
            draw_segment(&painter, x, params.outlet_length, params.outlet_diameter, outlet_color);
        });
}
