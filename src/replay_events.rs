use bincode::{Decode, Encode};
use egui::{Color32, Context};
use crate::timestamp::NanoTimestamp;

use crate::modal::{Modal, ModalStyle};

// A batch of events recorded/replayed in a single frame.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize, Encode, Decode)]
pub struct FrameEvents {
    #[bincode(with_serde)]
    pub time: NanoTimestamp,
    #[bincode(with_serde)]
    pub events: Vec<egui::Event>,
}

const UI_EVENTS_FILE_PREFIX: &str = "egui_replay";

fn get_first_ui_events_file() -> Option<String> {
    std::fs::read_dir("./")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let file_name = path.file_name()?.to_str()?;

            if path.is_file() && file_name.starts_with(UI_EVENTS_FILE_PREFIX) {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
        .min()
}

fn event_logfile(now: NanoTimestamp, use_bincode: bool) -> String {
    format!(
        "./{}_{}.{}",
        UI_EVENTS_FILE_PREFIX,
        now.as_rfc3339(),
        if use_bincode { "bin" } else { "json" }
    )
}

fn load_replay(file_name: &str) -> Result<Vec<FrameEvents>, std::io::Error> {
    let mut file = std::fs::File::open(file_name)?;
    let events = if file_name.ends_with(".bin") {
        bincode::decode_from_std_read(&mut file, bincode::config::standard()).map_err(std::io::Error::other)?
    } else if file_name.ends_with(".json") {
        serde_json::from_reader(file)?
    } else {
        return Err(std::io::Error::other("Unknown file extension"));
    };
    Ok(events)
}

fn save_replay(file_name: &str, frame_events: &Vec<FrameEvents>) {
    let mut file = std::fs::File::create(file_name).unwrap();
    let num_frames: usize = frame_events.len();
    let num_events: usize = frame_events.iter().map(|frame| frame.events.len()).sum();
    if file_name.ends_with(".bin") {
        bincode::encode_into_std_write(frame_events, &mut file, bincode::config::standard()).unwrap();
    } else if file_name.ends_with(".json") {
        serde_json::to_writer(file, &frame_events).unwrap();
    } else {
        // This should never happen.
        panic!("Unknown file extension: {}", file_name);
    }
    log::info!("Saved {} frames, {} events, to {}", num_frames, num_events, file_name);
}

// UI event recording. Useful for debugging to replay UI events.
// While replaying it displays a modal window that blocks other user
// interaction.
pub struct ReplayManager {
    is_window_open: bool,
    is_replaying: bool,
    is_recording: bool,

    // List of events being recorded/replayed.
    frame_events: Vec<FrameEvents>,
    // Index of the next frame to replay.
    replay_index: usize,
    // Input file name for replay.
    replay_file: String,
    // Whether to lookup the latest input file.
    should_lookup_replay: bool,

    // Recording settings.
    record_use_bincode: bool,
    record_apply_postprocessing: bool,
    simplify_pointer_events: bool,

    // Internal recording state.
    record_is_pointer_moving: bool,
}

fn is_f1_key(event: &egui::Event) -> bool {
    if let egui::Event::Key { key, .. } = event {
        *key == egui::Key::F1
    } else {
        false
    }
}

fn is_key_pressed(event: &egui::Event) -> bool {
    if let egui::Event::Key { pressed, .. } = event {
        *pressed
    } else {
        false
    }
}

fn is_pointer_moved(event: &egui::Event) -> bool {
    matches!(event, egui::Event::PointerMoved { .. })
}

// Merge all events into a single frame if possible. For merges, the first
// timestamp is used. PointerMoved events are kept in separate frames, otherwise
// replay cannot work.
fn apply_event_postprocessing(frames: Vec<FrameEvents>) -> Vec<FrameEvents> {
    let mut merged_frames = Vec::new();
    let mut current_group: Option<(bool, FrameEvents)> = None;

    // Add the first frame. This is a special pointer initial event.
    merged_frames.push(frames[0].clone());

    // Skip the first frame.
    for frame in frames.into_iter().skip(1) {
        // Process each event in each frame in order.
        for event in frame.events.into_iter() {
            let event_is_pointer = is_pointer_moved(&event);
            match current_group.as_mut() {
                // If the current group exists and the current event type
                // matches the group’s type, just accumulate the event.
                Some((group_type, group)) if *group_type == event_is_pointer => {
                    group.events.push(event);
                }
                // Otherwise flush the current group and start a new one.
                Some(_) => {
                    if let Some((_, finished_group)) = current_group.take() {
                        merged_frames.push(finished_group);
                    }
                    current_group = Some((
                        event_is_pointer,
                        FrameEvents {
                            // Use the current frame's timestamp for the new group.
                            // This is the first event in the new group.
                            time: frame.time,
                            events: vec![event],
                        },
                    ));
                }
                // No active group, so start one with the current event.
                None => {
                    current_group = Some((
                        event_is_pointer,
                        FrameEvents {
                            time: frame.time,
                            events: vec![event],
                        },
                    ));
                }
            }
        }
    }

    // Flush any pending events from the current group.
    if let Some((_, last_group)) = current_group.take() {
        merged_frames.push(last_group);
    }

    merged_frames
}

impl Default for ReplayManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayManager {
    pub fn new() -> Self {
        Self {
            is_window_open: false,
            is_replaying: false,
            is_recording: false,
            frame_events: Vec::new(),
            replay_index: 0,
            replay_file: "".to_string(),
            should_lookup_replay: true,

            // Recording settings.
            record_use_bincode: true,
            record_apply_postprocessing: true,
            simplify_pointer_events: true,

            // Recording state.
            record_is_pointer_moving: false,
        }
    }

    pub fn open_window(&mut self) {
        self.is_window_open = true;
        self.is_replaying = false;
        self.is_recording = false;
        self.frame_events.clear();
        self.replay_index = 0;
        self.should_lookup_replay = true;
    }

    pub fn close_window(&mut self) {
        self.is_window_open = false;
        self.is_replaying = false;
        self.is_recording = false;
        self.frame_events.clear();
        self.replay_index = 0;
    }

    pub fn is_replaying(&self) -> bool {
        self.is_replaying
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    pub fn num_recorded_frames(&self) -> usize {
        self.frame_events.len()
    }

    pub fn num_recorded_events(&self) -> usize {
        self.frame_events.iter().map(|frame| frame.events.len()).sum()
    }

    pub fn on_frame_update(&mut self, ctx: &Context) {
        if !self.is_window_open {
            return;
        }

        // Lookup for the latest input file if not set.
        if self.should_lookup_replay {
            self.replay_file = get_first_ui_events_file().unwrap_or(self.replay_file.clone());
            self.should_lookup_replay = false;
        }

        let modal = Modal::new(ctx, "replay_modal")
            // Modal should not consume events when replaying.
            // Otherwise it will block the input events from being processed.
            .with_consume_events(!self.is_replaying)
            .with_style(&ModalStyle {
                overlay_color: Color32::from_rgba_premultiplied(0, 0, 0, 50),
                ..Default::default()
            });

        modal.show(|ui| {
            modal.title(ui, "Replay UI events");

            modal.frame(ui, |ui| {
                if self.is_replaying {
                    ui.label(format!(
                        "Frame {} / {}",
                        self.replay_index + 1,
                        self.num_recorded_frames()
                    ));
                    ui.spinner();
                } else {
                    ui.label("Select input file [latest file is pre-filled]:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.replay_file)
                            .hint_text("No input file found")
                            .interactive(true)
                            .desired_width(ui.available_width()),
                    );
                }
            });

            modal.buttons(ui, |ui| {
                if self.is_replaying {
                    return;
                }

                if modal.button(ui, "Start replay").clicked() {
                    let ui_events = load_replay(&self.replay_file);
                    match ui_events {
                        Ok(ui_events) => {
                            let num_frames = ui_events.len();
                            let num_events = ui_events.iter().map(|frame| frame.events.len()).sum::<usize>();
                            log::info!(
                                "Loaded {} frames, {} events, from {}",
                                num_frames,
                                num_events,
                                &self.replay_file
                            );
                            self.is_replaying = true;
                            self.frame_events = ui_events;
                            self.replay_index = 0;
                        }
                        Err(err) => {
                            log::error!("Failed to parse UI events: {}", err);
                        }
                    }
                }
                if modal.button(ui, "Close").clicked() {
                    self.close_window();
                }
            });
        });

        modal.open();
    }

    pub fn on_raw_input_update(&mut self, now: NanoTimestamp, _ctx: &Context, raw_input: &mut egui::RawInput) {
        if self.is_replaying && self.replay_index < self.num_recorded_frames() {
            // Replay the events for the current frame index.
            log::info!(
                "Replaying frame {} / {}",
                self.replay_index + 1,
                self.num_recorded_frames()
            );
            raw_input.events = std::mem::take(&mut self.frame_events[self.replay_index].events);
            self.replay_index += 1;
            if self.replay_index >= self.num_recorded_frames() {
                self.close_window();
            }

            for event in raw_input.events.iter() {
                log::debug!("Replay event: {:?}", event);
            }
            return;
        }

        let mut event_batch = Vec::new();
        for (i, event) in raw_input.events.iter().enumerate() {
            // Start / stop recording events on F1 key.
            if is_f1_key(event) && is_key_pressed(event) {
                self.is_recording = !self.is_recording;
                if self.is_recording {
                    log::info!("Starting UI event recording");
                    self.frame_events.clear();
                    self.frame_events.push(FrameEvents {
                        time: now,
                        events: vec![egui::Event::PointerMoved(egui::Pos2::new(0.0, 0.0))],
                    });
                } else {
                    log::info!("Stopping UI event recording");
                    let file_name = event_logfile(now, self.record_use_bincode);
                    if self.record_apply_postprocessing {
                        self.frame_events = apply_event_postprocessing(std::mem::take(&mut self.frame_events));
                    }
                    save_replay(&file_name, &self.frame_events);
                }
            }

            if self.is_recording {
                if let egui::Event::PointerButton { pos, .. } = event {
                    if self.simplify_pointer_events {
                        // This is needed because the simplification in should_
                        // record_event does not capture the last pointer moved event,
                        // so the last recorded position can be off.
                        log::debug!("Recording (fake) UI event: {:?} {:?}", i, event);
                        event_batch.push(egui::Event::PointerMoved(*pos));
                    }
                }

                if self.should_record_event(event) {
                    log::debug!("Recording UI event: {:?} {:?}", i, event);
                    event_batch.push(event.clone());
                }
            }
        }

        if !event_batch.is_empty() {
            self.frame_events.push(FrameEvents {
                time: now,
                events: event_batch,
            });
        }
    }

    fn should_record_event(&mut self, event: &egui::Event) -> bool {
        if matches!(event, egui::Event::MouseMoved { .. }) {
            return false;
        }
        if is_f1_key(event) {
            return false;
        }
        if self.simplify_pointer_events {
            // Record only pointer start and end events.
            if is_pointer_moved(event) {
                if self.record_is_pointer_moving {
                    return false;
                } else {
                    self.record_is_pointer_moving = true;
                    return true;
                }
            } else {
                self.record_is_pointer_moving = false;
            }
        }

        true
    }
}
