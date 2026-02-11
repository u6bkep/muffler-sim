pub mod app;
pub mod geometry_view;
pub mod plot_view;
pub mod renderer;
pub mod ui;

use winit::event_loop::EventLoop;

use app::App;

/// Launch the application: create event loop and run the App.
pub fn run() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}
