pub mod app;
pub mod geometry_view;
pub mod plot_view;
pub mod ui;

use app::App;

/// Launch the application with eframe.
pub fn run() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Air-Sim — Expansion Chamber Muffler Simulator")
            .with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Air-Sim",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .expect("eframe::run_native failed");
}
