use crate::clock::{Clock, SystemClock};
use crate::timestamp::NanoTimestamp;
use crate::replay_events::ReplayManager;

pub struct ReplayApp {
    replay_manager: ReplayManager,
    check_states: [bool; 10],
}

impl ReplayApp {
    /// Called once before the first frame.
    pub fn new() -> Self {
        Self {
            replay_manager: ReplayManager::new(),
            check_states: [false; 10],
        }
    }
}

impl eframe::App for ReplayApp {
    /// Called each time the UI needs repainting, which may be many times per
    /// second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.replay_manager.on_frame_update(ctx);

        egui::TopBottomPanel::bottom("bottom_panel")
            .min_height(150.)
            .show(ctx, |ui| {
                let recording_label = if self.replay_manager.is_recording() {
                    format!(
                        "Recording UI: ON, {} frames, {} events recorded",
                        self.replay_manager.num_recorded_frames(),
                        self.replay_manager.num_recorded_events()
                    )
                } else {
                    "Recording UI: OFF, press F1 to start/stop".to_string()
                };
                ui.label(recording_label);

                // Add a button to open the replay modal
                if ui.button("Replay UI Events").clicked() {
                    log::info!("Opening replay modal");
                    self.replay_manager.open_window();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Add 10 steteful checkboxes to toggle.
            for i in 0..10 {
                ui.checkbox(&mut self.check_states[i], "Checked");
            }
        });
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let now: NanoTimestamp = SystemClock.now();
        self.replay_manager.on_raw_input_update(now, ctx, raw_input);
    }
}
