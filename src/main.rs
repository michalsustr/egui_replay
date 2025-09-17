use egui_replay::app::ReplayApp;

fn make_app(_cc: &eframe::CreationContext<'_>) -> ReplayApp {
    ReplayApp::new()
}

fn main() -> eframe::Result {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_max_inner_size([512.0, 512.0])
            .with_min_inner_size([512.0, 512.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Replay demo",
        native_options,
        Box::new(|cc| Ok(Box::new(make_app(cc)))),
    )
}
