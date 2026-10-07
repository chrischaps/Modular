//! Main application struct for the Modular Synth
//!
//! Contains the SynthApp which implements eframe::App and manages
//! the synthesizer's UI state, audio engine, and graph state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use eframe::egui::{self, RichText, Layout, Align};
use egui_node_graph2::{FlowGlyph, GraphEditorState, NodeResponse, NodeTemplateTrait, InputParamKind};

use crate::engine::{
    AudioEngine, AudioError, AudioProcessor, DeviceInfo, EngineChannels, EngineCommand, UiHandle,
    MidiDeviceInfo, MidiEngine, MidiEvent, MidiReceivers, TimestampedMidiEvent,
};
use rtrb::Consumer;
use crate::graph::{
    port_mapping, validate_connection, AllNodeTemplates, AnyParameterId, SynthDataType, SynthGraphState,
    SynthNodeData, SynthNodeTemplate, SynthValueType,
};
use crate::modules::keyboard::{key_to_note, relative_to_midi};
use crate::persistence::{
    capture_patch, examples, load_from_file, save_to_file, stage_patch, Example, MidiMapping, Patch, PatchError,
    EXAMPLES,
};
use crate::widgets::{cpu_meter, CpuMeterConfig};
use super::capture::{Capture, CaptureAction, CaptureConfig};
use super::editing;
use super::engine_sync;
use super::palette::{PaletteAction, QuickAdd};
use super::session::{self, Answer, Autosave, Discard, RecentFiles};
use super::theme;
use super::undo::{Applied, History};

/// Type alias for our graph editor state
type SynthGraphEditorState = GraphEditorState<SynthNodeData, SynthDataType, SynthValueType, SynthNodeTemplate, SynthGraphState>;

/// Max popup height for the toolbar device dropdowns. egui's default (200px)
/// fits only ~3 rows at the theme's padding; egui still clamps to the window.
const DEVICE_MENU_HEIGHT: f32 = 480.0;

/// Storage key for the mark drawn along cables (Cables menu)
const FLOW_GLYPH_KEY: &str = "cable_flow_glyph";

/// Target parameter for MIDI Learn mode.
///
/// When the user activates MIDI Learn on a knob, this stores the target
/// parameter information until a CC event is received.
#[derive(Debug, Clone)]
pub struct MidiLearnTarget {
    /// Engine node ID of the target parameter.
    pub node_id: u64,
    /// Parameter index within the node.
    pub param_index: usize,
    /// Parameter name for display.
    pub param_name: String,
    /// Minimum value of the parameter range.
    pub min_value: f32,
    /// Maximum value of the parameter range.
    pub max_value: f32,
}

/// Main application state for the Modular Synth
pub struct SynthApp {
    /// Audio engine handle
    audio_engine: Result<AudioEngine, AudioError>,

    /// UI-side handle for communicating with audio engine
    ui_handle: Option<UiHandle>,

    /// Last audio error message to display
    audio_error_message: Option<String>,

    /// Whether the transport is "playing" (audio graph processing active)
    is_playing: bool,

    /// Whether theme has been applied
    theme_applied: bool,

    /// Node graph editor state
    graph_state: SynthGraphEditorState,

    /// User state for the graph editor
    user_state: SynthGraphState,

    /// Cached list of audio devices
    audio_devices: Vec<DeviceInfo>,

    /// Index of currently selected device
    selected_device_index: usize,

    /// Cached parameter values for change detection.
    /// Key is (node_id as u64, param_index), value is the last sent value.
    cached_params: HashMap<(u64, usize), f32>,

    /// Current patch file path (None if unsaved/new).
    current_patch_path: Option<PathBuf>,
    /// The example the graph was opened from, until it's saved as a file or replaced.
    current_example: Option<&'static Example>,

    /// Status message for save/load operations (auto-clears after display).
    status_message: Option<String>,

    /// Problems skipped while loading the current patch, shown in the status bar.
    load_warnings: Vec<String>,

    /// Currently pressed keyboard keys for virtual keyboard.
    /// Stores (relative_note, egui::Key) in order of press for key priority.
    pressed_keys: Vec<(i32, egui::Key)>,

    /// Timestamp when the gate was last triggered (for minimum gate duration).
    last_gate_on: Option<Instant>,

    /// Whether the gate is currently being held high (for minimum duration).
    gate_held_high: bool,

    /// The last note that was triggered (to maintain pitch during gate hold).
    last_triggered_note: f32,

    /// Current CPU load percentage from the audio engine (0-100).
    cpu_load: f32,

    /// MIDI engine for receiving MIDI input.
    midi_engine: Option<MidiEngine>,

    /// Consumer for the UI's copy of incoming MIDI: monitor display, piano,
    /// MIDI Learn and CC mappings. Notes reach MIDI Note modules on the
    /// audio thread, through the processor's own queue.
    midi_event_consumer: Option<Consumer<TimestampedMidiEvent>>,

    /// Cached list of MIDI input devices.
    midi_devices: Vec<MidiDeviceInfo>,

    /// Index of currently selected MIDI device (None = no device).
    selected_midi_device: Option<usize>,

    /// MIDI error message to display.
    midi_error_message: Option<String>,

    /// MIDI notes currently held, in order of press, for the piano display.
    midi_held_notes: Vec<u8>,

    // --- MIDI CC Mapping state ---
    /// Active MIDI CC to parameter mappings.
    midi_mappings: Vec<MidiMapping>,

    /// Target for MIDI Learn mode (None = not learning).
    midi_learn_target: Option<MidiLearnTarget>,

    /// Undo and redo for edits to the patch.
    history: History,

    /// The quick-add palette, while it's open.
    quick_add: Option<QuickAdd>,

    /// Where the graph editor was drawn last frame, in screen points.
    editor_rect: egui::Rect,

    /// Where the last paste was aimed and where it landed, so pasting again
    /// without moving the mouse fans the copies out instead of stacking them.
    last_paste: Option<(egui::Pos2, egui::Pos2)>,

    // --- Session safety ---
    /// Patch files opened or saved lately, for the Recent menu.
    recent_files: RecentFiles,
    /// What's waiting on an answer to "Save changes?".
    pending_discard: Option<Discard>,
    /// A crash's autosave, until it's recovered or let go.
    recovery: Option<Autosave>,
    /// The MIDI mappings as last opened or saved. Undo history doesn't
    /// cover them, but they're saved with the patch.
    saved_midi_mappings: Vec<MidiMapping>,
    /// Whether the window may close: set once unsaved changes are dealt with.
    allow_close: bool,
    /// The window title last sent, so it's only sent when it changes.
    window_title: String,
    /// Filming the app with `--capture`: the script, clock and outputs.
    capture: Option<Capture>,
}

/// What a module's right-click menu asked for, handled once the graph is drawn.
enum NodeMenuAction {
    Select(egui_node_graph2::NodeId),
    Duplicate(egui_node_graph2::NodeId),
    Copy(egui_node_graph2::NodeId),
    Reset(egui_node_graph2::NodeId),
    Delete(egui_node_graph2::NodeId),
}

impl SynthApp {
    /// Create a new SynthApp instance
    ///
    /// If `enable_test_tone` is true, audio will start with a test tone immediately.
    pub fn new(enable_test_tone: bool) -> Self {
        let mut audio_engine = AudioEngine::new();

        let audio_error_message = match &audio_engine {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        // Get initial device list and find the default device index
        let (audio_devices, selected_device_index) = match &audio_engine {
            Ok(engine) => {
                let devices = engine.enumerate_devices();
                let default_idx = devices.iter()
                    .position(|d| d.is_default)
                    .unwrap_or(0);
                (devices, default_idx)
            }
            Err(_) => (Vec::new(), 0),
        };

        // Initialize MIDI engine
        let (midi_engine, midi_receivers, midi_devices, midi_error_message) =
            match MidiEngine::new() {
                Ok((mut engine, receivers)) => {
                    let devices = engine.enumerate_devices();
                    (Some(engine), Some(receivers), devices, None)
                }
                Err(e) => {
                    eprintln!("MIDI initialization failed: {}", e);
                    (None, None, Vec::new(), Some(e.to_string()))
                }
            };
        let (midi_audio_consumer, midi_event_consumer) = match midi_receivers {
            Some(MidiReceivers { audio, ui }) => (Some(audio), Some(ui)),
            None => (None, None),
        };

        // Create engine channels for communication with audio thread
        let channels = EngineChannels::with_defaults();
        let (ui_handle, engine_handle) = channels.split();

        // Create and start the audio processor if engine is available
        let ui_handle = if let Ok(ref mut engine) = audio_engine {
            let sample_rate = engine.sample_rate() as f32;
            let block_size = 256; // Standard block size
            let mut processor = AudioProcessor::new(sample_rate, block_size, engine_handle);
            if let Some(midi) = midi_audio_consumer {
                processor.set_midi_input(midi);
            }

            if let Err(e) = engine.start_with_processor(processor) {
                eprintln!("Failed to start audio processor: {}", e);
            }

            Some(ui_handle)
        } else {
            // Drop the engine_handle since we can't use it
            drop(engine_handle);
            None
        };

        let app = Self {
            audio_engine,
            ui_handle,
            audio_error_message,
            is_playing: false,
            theme_applied: false,
            graph_state: GraphEditorState::new(1.0),
            user_state: SynthGraphState::default(),
            audio_devices,
            selected_device_index,
            cached_params: HashMap::new(),
            current_patch_path: None,
            current_example: None,
            status_message: None,
            load_warnings: Vec::new(),
            pressed_keys: Vec::new(),
            last_gate_on: None,
            gate_held_high: false,
            last_triggered_note: 60.0,
            cpu_load: 0.0,
            midi_engine,
            midi_event_consumer,
            midi_devices,
            selected_midi_device: None,
            midi_error_message,
            midi_held_notes: Vec::new(),
            // MIDI CC Mapping state
            midi_mappings: Vec::new(),
            midi_learn_target: None,
            history: History::default(),
            quick_add: None,
            editor_rect: egui::Rect::NOTHING,
            last_paste: None,
            recent_files: RecentFiles::default(),
            pending_discard: None,
            recovery: None,
            saved_midi_mappings: Vec::new(),
            allow_close: false,
            window_title: String::new(),
            capture: None,
        };

        // Note: enable_test_tone is ignored - test tone was removed in favor of AudioProcessor
        let _ = enable_test_tone;

        app
    }

    /// Hands the audio engine's clock to a capture: from here on audio is
    /// rendered one video frame at a time, in step with the picture.
    pub fn start_capture(&mut self, config: CaptureConfig) -> Result<(), String> {
        let engine = self.audio_engine.as_mut().map_err(|e| e.to_string())?;
        engine.go_offline(config.sample_rate as f32).map_err(|e| e.to_string())?;
        self.capture = Some(Capture::start(config)?);
        Ok(())
    }

    /// Runs the capture's cues for a new frame, then renders the audio the
    /// frame covers, so this frame's picture shows what it sounds like.
    fn step_capture(&mut self) {
        let Some(capture) = self.capture.as_mut() else { return };
        if !capture.is_fresh() {
            return;
        }
        let mut actions = capture.take_actions();
        let graph = &self.graph_state.graph;
        actions.extend(capture.param_values(|module, nth, input| {
            find_input(graph, module, nth, input).map(|id| graph.get_input(id).value.actual_value())
        }));

        let mut midi = Vec::new();
        for action in actions {
            match action {
                CaptureAction::Play(on) => {
                    if on != self.is_playing {
                        self.is_playing = on;
                        self.user_state.is_playing = on;
                        self.send_command(EngineCommand::SetPlaying(on));
                    }
                }
                CaptureAction::Midi { event, offset } => {
                    // The UI's copy lights the pianos; the audio's copy is
                    // placed by sample below instead
                    if let Some(engine) = self.midi_engine.as_ref() {
                        engine.send(event);
                    }
                    midi.extend(event.to_dsp(offset));
                }
                CaptureAction::SetParam { module, nth, input, value } => {
                    match find_input(&self.graph_state.graph, &module, nth, &input) {
                        Some(id) => self.graph_state.graph.inputs[id].value.set_actual_value(value),
                        None => eprintln!("capture: no input {} on {} #{}", input, module, nth + 1),
                    }
                }
            }
        }

        // Ship the frame's edits before rendering it
        self.sync_parameters();
        if let Some(handle) = self.ui_handle.as_mut() {
            handle.flush();
        }
        midi.sort_by_key(|e| e.sample_offset);
        let (Ok(engine), Some(capture)) = (self.audio_engine.as_ref(), self.capture.as_mut()) else { return };
        let buffer = capture.audio_buffer();
        engine.render_offline(buffer, 2, &mut midi);
        capture.commit_audio();
    }

    /// Refresh the list of available audio devices
    fn refresh_devices(&mut self) {
        if let Ok(ref engine) = self.audio_engine {
            self.audio_devices = engine.enumerate_devices();
        }
    }

    /// Refresh the list of available MIDI devices
    fn refresh_midi_devices(&mut self) {
        if let Some(ref mut engine) = self.midi_engine {
            self.midi_devices = engine.enumerate_devices();
        }
    }

    /// Connect to a MIDI device by index
    fn connect_midi_device(&mut self, index: usize) {
        if let Some(ref mut engine) = self.midi_engine {
            match engine.connect(index) {
                Ok(()) => {
                    self.selected_midi_device = Some(index);
                    self.midi_error_message = None;
                }
                Err(e) => {
                    // connect() drops the previous connection before trying
                    self.selected_midi_device = None;
                    self.midi_error_message = Some(e.to_string());
                }
            }
            // Notes held on the previous device were released (All Notes Off)
            self.clear_midi_held_notes();
        }
    }

    /// Disconnect from the current MIDI device
    fn disconnect_midi_device(&mut self) {
        if let Some(ref mut engine) = self.midi_engine {
            engine.disconnect();
            self.selected_midi_device = None;
            self.midi_error_message = None;
            self.clear_midi_held_notes();
        }
    }

    /// Clears the piano display's held notes.
    fn clear_midi_held_notes(&mut self) {
        self.midi_held_notes.clear();
        self.user_state.set_midi_active_notes(Vec::new());
    }

    /// Process the UI's copy of pending MIDI events.
    /// - Stores events in user state for display by MIDI Monitor modules.
    /// - Tracks held notes for the MIDI piano display.
    /// - Handles CC events for MIDI Learn and mapped parameters.
    ///
    /// Notes reach MIDI Note modules on the audio thread, sample-accurately;
    /// nothing here affects what they play.
    fn process_midi_events(&mut self) {
        let mut notes_changed = false;
        let mut cc_updates: Vec<(u64, usize, f32)> = Vec::new();
        let mut learned_mapping: Option<MidiMapping> = None;

        if let Some(ref mut consumer) = self.midi_event_consumer {
            while let Ok(timestamped) = consumer.pop() {
                let event = timestamped.event;

                // Store the event for MIDI Monitor display
                self.user_state.push_midi_event(event);

                match event {
                    MidiEvent::NoteOn { note, .. } => {
                        if !self.midi_held_notes.contains(&note) {
                            self.midi_held_notes.push(note);
                            notes_changed = true;
                        }
                    }
                    MidiEvent::NoteOff { note, .. } => {
                        if let Some(pos) = self.midi_held_notes.iter().position(|&n| n == note) {
                            self.midi_held_notes.remove(pos);
                            notes_changed = true;
                        }
                    }
                    MidiEvent::ControlChange { channel, controller, value } => {
                        // Check if we're in MIDI Learn mode
                        if let Some(ref target) = self.midi_learn_target {
                            // Create new mapping from the received CC
                            learned_mapping = Some(MidiMapping::new(
                                controller,
                                0, // Omni channel for learned mappings
                                target.node_id,
                                target.param_index,
                                target.param_name.clone(),
                                target.min_value,
                                target.max_value,
                            ));
                        } else {
                            // Apply CC to all matching mappings
                            for mapping in &self.midi_mappings {
                                if mapping.matches(controller, channel) {
                                    let param_value = mapping.cc_to_value(value);
                                    cc_updates.push((mapping.node_id, mapping.param_index, param_value));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle MIDI Learn completion
        if let Some(mapping) = learned_mapping {
            // Remove any existing mapping for the same parameter (from user_state too)
            self.midi_mappings.retain(|m| {
                !(m.node_id == mapping.node_id && m.param_index == mapping.param_index)
            });
            self.user_state.remove_midi_mapping(mapping.node_id, mapping.param_index);

            // Also remove any existing mapping for the same CC
            self.midi_mappings.retain(|m| {
                !(m.cc_number == mapping.cc_number && (m.channel == 0 || m.channel == mapping.channel))
            });

            // Add the new mapping
            self.user_state.set_midi_mapping(
                mapping.node_id,
                mapping.param_index,
                mapping.cc_number,
                mapping.channel,
            );
            self.midi_mappings.push(mapping);

            // Exit learn mode
            self.midi_learn_target = None;
            self.user_state.midi_learn_active = false;
            self.user_state.midi_learn_target = None;
            self.status_message = Some("MIDI CC mapped successfully".to_string());
        }

        // Apply CC updates to parameters
        for (node_id, param_index, value) in cc_updates {
            self.send_command(EngineCommand::SetParameter {
                node_id,
                param_index,
                value,
            });
            // Update cached param so sync_parameters doesn't overwrite
            self.cached_params.insert((node_id, param_index), value);

            // Also update the graph UI to reflect the change
            self.update_graph_param_from_cc(node_id, param_index, value);
        }

        if notes_changed {
            self.user_state.set_midi_active_notes(self.midi_held_notes.clone());
        }
    }

    /// Update a graph parameter value from a CC change.
    fn update_graph_param_from_cc(&mut self, engine_node_id: u64, param_index: usize, value: f32) {
        // Find the graph node ID for this engine node
        let graph_node_id = self.user_state.node_id_map.iter()
            .find(|(_, &engine_id)| engine_id == engine_node_id)
            .map(|(graph_id, _)| *graph_id);

        if let Some(graph_node_id) = graph_node_id {
            if let Some(node) = self.graph_state.graph.nodes.get_mut(graph_node_id) {
                // Find the parameter by index
                let mut current_param_index = 0;
                for (_name, input_id) in &node.inputs {
                    if let Some(input) = self.graph_state.graph.inputs.get_mut(*input_id) {
                        match input.kind {
                            InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                                if current_param_index == param_index {
                                    input.value.set_actual_value(value);
                                    // Playing a controller isn't an edit to undo
                                    self.history.absorb_param(engine_node_id, param_index, input.value.actual_value());
                                    return;
                                }
                                current_param_index += 1;
                            }
                            InputParamKind::ConnectionOnly => {
                                // Skip connection-only inputs
                            }
                        }
                    }
                }
            }
        }
    }

    /// Start MIDI Learn mode for a parameter.
    pub fn start_midi_learn(&mut self, target: MidiLearnTarget) {
        self.midi_learn_target = Some(target);
        self.status_message = Some("Move a MIDI CC to map it...".to_string());
    }

    /// Cancel MIDI Learn mode.
    pub fn cancel_midi_learn(&mut self) {
        self.midi_learn_target = None;
        self.user_state.midi_learn_active = false;
        self.user_state.midi_learn_target = None;
        self.status_message = Some("MIDI Learn cancelled".to_string());
    }

    /// Check if currently in MIDI Learn mode.
    pub fn is_midi_learning(&self) -> bool {
        self.midi_learn_target.is_some()
    }

    /// Get the MIDI mapping for a specific parameter, if any.
    pub fn get_mapping_for_param(&self, node_id: u64, param_index: usize) -> Option<&MidiMapping> {
        self.midi_mappings.iter()
            .find(|m| m.node_id == node_id && m.param_index == param_index)
    }

    /// Remove MIDI mapping for a specific parameter.
    pub fn clear_mapping_for_param(&mut self, node_id: u64, param_index: usize) {
        self.midi_mappings.retain(|m| {
            !(m.node_id == node_id && m.param_index == param_index)
        });
        self.status_message = Some("MIDI mapping cleared".to_string());
    }

    /// Clear all MIDI mappings.
    pub fn clear_all_midi_mappings(&mut self) {
        self.midi_mappings.clear();
        self.status_message = Some("All MIDI mappings cleared".to_string());
    }

    /// Check if there are any MIDI Note or Poly MIDI modules in the graph.
    fn has_midi_note_modules(&self) -> bool {
        self.graph_state.graph.nodes.iter()
            .any(|(_, node)| matches!(node.user_data.module_id, "input.midi_note" | "input.poly_midi"))
    }

    /// Check if there are any Poly MIDI modules in the graph.
    fn has_poly_midi_modules(&self) -> bool {
        self.graph_state.graph.nodes.iter()
            .any(|(_, node)| node.user_data.module_id == "input.poly_midi")
    }

    /// Select an audio output device by index
    fn select_device(&mut self, index: usize) {
        if let Ok(ref mut engine) = self.audio_engine {
            match engine.select_device(index) {
                Ok(()) => {
                    self.selected_device_index = index;
                    self.audio_error_message = None;
                }
                Err(e) => {
                    self.audio_error_message = Some(e.to_string());
                }
            }
        }
    }

    // Note: start_audio/stop_audio removed - we now use AudioProcessor which
    // starts automatically. Use the Play/Stop transport button to control audio.

    /// Toggle the test tone on/off (legacy - may conflict with AudioProcessor)
    #[allow(dead_code)]
    fn toggle_test_tone(&mut self) {
        // Test tone is disabled when using AudioProcessor
        // This function is kept for potential future debug use
    }

    /// Draw the top toolbar with transport controls and status
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) -> ToolbarActions {
        let mut actions = ToolbarActions::default();

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Application title
            ui.label(RichText::new("MODULAR SYNTH")
                .size(18.0)
                .color(theme::text::PRIMARY)
                .strong());

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Transport controls
            ui.label(RichText::new("Transport").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            // Play/Stop button - controls whether the audio graph is processing
            let play_text = if self.is_playing { "⏹ Stop" } else { "▶ Play" };
            let play_color = if self.is_playing {
                theme::accent::WARNING
            } else {
                theme::accent::SUCCESS
            };

            if ui.button(RichText::new(play_text).color(play_color)).clicked() {
                actions.toggle_playing = true;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // File operations
            ui.label(RichText::new("File").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            if ui.button("📄 New").on_hover_text("Start an empty patch (Ctrl+N)").clicked() {
                actions.new_patch = true;
            }

            if ui.button("📂 Open").on_hover_text("Ctrl+O").clicked() {
                actions.load_patch = true;
            }

            ui.menu_button("🕘 Recent", |ui| {
                if self.recent_files.is_empty() {
                    ui.label(RichText::new("No recent patches").color(theme::text::DISABLED).italics());
                    return;
                }
                for path in self.recent_files.iter() {
                    let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
                    let button = ui.add_enabled(path.exists(), egui::Button::new(name));
                    let clicked = button
                        .on_hover_text(path.display().to_string())
                        .on_disabled_hover_text(format!("Missing: {}", path.display()))
                        .clicked();
                    if clicked {
                        actions.open_recent = Some(path.to_path_buf());
                        ui.close_menu();
                    }
                }
                ui.separator();
                if ui.button(RichText::new("Clear Recent").color(theme::text::SECONDARY)).clicked() {
                    actions.clear_recent = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("📚 Examples", |ui| {
                for example in EXAMPLES {
                    if ui.button(example.name).on_hover_text(example.description).clicked() {
                        actions.open_example = Some(example);
                        ui.close_menu();
                    }
                }
            });

            if ui.button("💾 Save").on_hover_text("Ctrl+S").clicked() {
                actions.save_patch = true;
            }

            if ui.button("💾 Save As").on_hover_text("Save to a new file (Ctrl+Shift+S)").clicked() {
                actions.save_as_patch = true;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Edit history
            ui.label(RichText::new("Edit").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            let undo_label = self.history.undo_label();
            let undo = ui.add_enabled(undo_label.is_some(), egui::Button::new("↩ Undo"));
            if undo.on_hover_text(history_hint("Undo", undo_label, "Ctrl+Z")).clicked() {
                actions.undo = true;
            }
            let redo_label = self.history.redo_label();
            let redo = ui.add_enabled(redo_label.is_some(), egui::Button::new("↪ Redo"));
            if redo.on_hover_text(history_hint("Redo", redo_label, "Ctrl+Shift+Z")).clicked() {
                actions.redo = true;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // How the signal is drawn flowing along the cables
            ui.menu_button("〰 Cables", |ui| {
                ui.label(RichText::new("Signal flow marks").color(theme::text::SECONDARY));
                for glyph in FlowGlyph::ALL {
                    ui.radio_value(&mut self.user_state.flow_glyph, glyph, glyph.name());
                }
            })
            .response
            .on_hover_text("How signal flow is drawn along cables");

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Audio output selector
            match &self.audio_engine {
                Ok(engine) => {
                    let is_running = engine.is_running();

                    // Device selector
                    ui.label(RichText::new("Output").color(theme::text::SECONDARY));
                    ui.add_space(8.0);

                    // Get current device name for display
                    let current_device = self.audio_devices
                        .get(self.selected_device_index)
                        .map(|d| d.name.as_str())
                        .unwrap_or("No device");

                    // Truncate long device names
                    let display_name = if current_device.len() > 30 {
                        format!("{}...", &current_device[..27])
                    } else {
                        current_device.to_string()
                    };

                    egui::ComboBox::from_id_salt("device_selector")
                        .selected_text(display_name)
                        .width(200.0)
                        .height(DEVICE_MENU_HEIGHT)
                        .show_ui(ui, |ui| {
                            for device in &self.audio_devices {
                                let label = if device.is_default {
                                    format!("{} (Default)", device.name)
                                } else {
                                    device.name.clone()
                                };

                                if ui.selectable_label(
                                    device.index == self.selected_device_index,
                                    label
                                ).clicked() {
                                    actions.select_device = Some(device.index);
                                }
                            }

                            ui.separator();
                            if ui.button("🔄 Refresh").clicked() {
                                actions.refresh_devices = true;
                            }
                        });

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);

                    // MIDI input selector
                    ui.label(RichText::new("MIDI In").color(theme::text::SECONDARY));
                    ui.add_space(8.0);

                    // Get current MIDI device name for display
                    let current_midi = self.selected_midi_device
                        .and_then(|idx| self.midi_devices.get(idx))
                        .map(|d| d.name.as_str())
                        .unwrap_or("None");

                    // Truncate long device names
                    let midi_display_name = if current_midi.len() > 25 {
                        format!("{}...", &current_midi[..22])
                    } else {
                        current_midi.to_string()
                    };

                    // MIDI connection indicator
                    let midi_connected = self.selected_midi_device.is_some()
                        && self.midi_engine.as_ref().map(|e| e.is_connected()).unwrap_or(false);
                    let midi_indicator = if midi_connected { "● " } else { "○ " };

                    egui::ComboBox::from_id_salt("midi_device_selector")
                        .selected_text(format!("{}{}", midi_indicator, midi_display_name))
                        .width(180.0)
                        .height(DEVICE_MENU_HEIGHT)
                        .show_ui(ui, |ui| {
                            // Option to disconnect / select none
                            if ui.selectable_label(
                                self.selected_midi_device.is_none(),
                                "None (Disconnect)"
                            ).clicked() {
                                actions.disconnect_midi = true;
                            }

                            ui.separator();

                            // List available MIDI devices
                            if self.midi_devices.is_empty() {
                                ui.label(RichText::new("No MIDI devices found")
                                    .color(theme::text::DISABLED)
                                    .italics());
                            } else {
                                for device in &self.midi_devices {
                                    let is_selected = self.selected_midi_device == Some(device.index);
                                    if ui.selectable_label(is_selected, &device.name).clicked() {
                                        actions.connect_midi_device = Some(device.index);
                                    }
                                }
                            }

                            ui.separator();
                            if ui.button("🔄 Refresh").clicked() {
                                actions.refresh_midi_devices = true;
                            }
                        });

                    // Status indicator (right-to-left layout: items appear from right to left)
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Running status (rightmost)
                        let status_color = if is_running {
                            theme::accent::SUCCESS
                        } else {
                            theme::text::DISABLED
                        };
                        let status_text = if is_running { "● Running" } else { "○ Stopped" };
                        ui.label(RichText::new(status_text).color(status_color).small());

                        // Sample rate info
                        ui.label(RichText::new(format!(
                            "{}Hz • {}ch",
                            engine.sample_rate(),
                            engine.channels()
                        )).color(theme::text::SECONDARY).small());

                        ui.add_space(8.0);

                        // CPU meter (only show when playing)
                        if self.is_playing {
                            cpu_meter(ui, self.cpu_load, &CpuMeterConfig::compact());
                        }
                    });
                }
                Err(e) => {
                    ui.label(RichText::new(format!("⚠ Audio unavailable: {}", e))
                        .color(theme::accent::ERROR));
                }
            }
        });

        actions
    }

    /// Send a command to the audio engine.
    fn send_command(&mut self, cmd: EngineCommand) {
        if let Some(ref mut handle) = self.ui_handle {
            // Never dropped: anything that doesn't fit in the queue now is
            // held and delivered by a later flush
            handle.send_command(cmd);
        }
    }

    /// Process events from the audio engine.
    /// This handles InputValue events for knob animation, OutputValue events for LED indicators,
    /// ScopeBuffer events for oscilloscope display, and CpuLoad events for CPU metering.
    fn process_engine_events(&mut self) {
        if let Some(ref mut handle) = self.ui_handle {
            // Drain all available events
            while let Some(event) = handle.recv_event() {
                match event {
                    crate::engine::EngineEvent::InputValue { node_id, input_index, value } => {
                        // Store the input value for UI feedback
                        self.user_state.set_input_value(node_id, input_index, value);
                    }
                    crate::engine::EngineEvent::OutputValue { node_id, output_index, value, channels } => {
                        // Store the output value for LED indicators, and each
                        // channel's for drawing poly cables strand by strand
                        self.user_state.set_output_value(node_id, output_index, value);
                        self.user_state.set_output_channels(node_id, output_index, channels);
                    }
                    crate::engine::EngineEvent::ScopeBuffer { node_id, channel1, channel2, triggered } => {
                        // Store the oscilloscope waveform data for display
                        self.user_state.set_scope_data(
                            node_id,
                            channel1.into_vec(),
                            channel2.into_vec(),
                            triggered,
                        );
                    }
                    crate::engine::EngineEvent::CpuLoad(load) => {
                        // Update CPU load for display
                        self.cpu_load = load;
                    }
                    crate::engine::EngineEvent::OutputLevel(levels) => {
                        // Feed the Audio Output node's meter
                        self.user_state.output_meter.feed(levels);
                    }
                    // Other events are not currently handled by the app
                    // (Started, Stopped, Error)
                    _ => {}
                }
            }
        }
    }

    /// Draw the main content area with the node graph editor
    fn draw_main_area(&mut self, ctx: &egui::Context) {
        // Collect connections to remove (validated after drawing)
        let mut invalid_connections: Vec<(egui_node_graph2::OutputId, egui_node_graph2::InputId)> = Vec::new();
        // Collect commands to send (to avoid borrow issues)
        let mut commands_to_send: Vec<EngineCommand> = Vec::new();
        // Nodes whose bypass switch was clicked
        let mut bypass_toggles: Vec<egui_node_graph2::NodeId> = Vec::new();
        // What modules' right-click menus asked for
        let mut node_menu_actions: Vec<NodeMenuAction> = Vec::new();
        // Track if we clicked in the editor area
        let mut cursor_in_editor = false;
        // Store editor rect for coordinate conversion
        let mut editor_rect = egui::Rect::NOTHING;

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                // Store editor rect for coordinate conversion
                editor_rect = ui.available_rect_before_wrap();
                self.editor_rect = editor_rect;

                // Reset widget context menu flag before drawing
                self.user_state.widget_context_menu_open = false;

                // Update zoom for widget scaling
                self.user_state.zoom = self.graph_state.pan_zoom.zoom;

                // The grid sits under the patch and moves with it
                let grid_origin = editor_rect.min + self.graph_state.pan_zoom.pan + self.history.view_origin();
                theme::draw_grid_background(ui.painter(), editor_rect, grid_origin, self.graph_state.pan_zoom.zoom);

                // Draw the node graph editor
                let (zoom_before, pan_before) = (self.graph_state.pan_zoom.zoom, self.graph_state.pan_zoom.pan);
                // A capture script moves the view like a camera
                if let Some(capture) = self.capture.as_mut() {
                    let zoom = capture.take_zoom();
                    if zoom != 1.0 {
                        self.graph_state.zoom(ui, zoom);
                    }
                    if let Some(pan) = capture.take_pan(self.graph_state.pan_zoom.pan) {
                        self.graph_state.pan_zoom.pan = pan;
                    }
                }
                let graph_response = self.graph_state.draw_graph_editor(
                    ui,
                    AllNodeTemplates,
                    &mut self.user_state,
                    Vec::default(),
                );
                // Zooming moves every node; undo keeps positions that don't
                self.history.follow_zoom(zoom_before, pan_before, &self.graph_state.pan_zoom);

                cursor_in_editor = graph_response.cursor_in_editor;

                // Disable the built-in node finder - we use our own context menu
                self.graph_state.node_finder = None;

                // Process graph responses
                for response in graph_response.node_responses {
                    match response {
                        NodeResponse::CreatedNode(node_id) => {
                            // Allocate engine node ID for the new node
                            let engine_node_id = self.user_state.allocate_engine_node_id(node_id);
                            commands_to_send.extend(engine_sync::add_module(
                                &self.graph_state.graph,
                                node_id,
                                engine_node_id,
                            ));
                        }
                        NodeResponse::DeleteNodeFull { node_id, .. } => {
                            // Get engine node ID before removing from mapping
                            if let Some(engine_node_id) = self.user_state.remove_node(node_id) {
                                commands_to_send.push(EngineCommand::RemoveModule {
                                    node_id: engine_node_id,
                                });
                            }
                        }
                        NodeResponse::ConnectEventEnded { output, input, .. } => {
                            // Validate the connection after it was made
                            if let Some(error_msg) = self.validate_and_check_connection(output, input) {
                                // Mark for removal
                                invalid_connections.push((output, input));
                                // Show error message
                                self.user_state.set_validation_error(error_msg);
                            } else {
                                commands_to_send.extend(engine_sync::cable_connected(
                                    &self.graph_state.graph,
                                    &self.user_state,
                                    output,
                                    input,
                                ));
                            }
                        }
                        NodeResponse::DisconnectEvent { output, input } => {
                            commands_to_send.extend(engine_sync::cable_disconnected(
                                &self.graph_state.graph,
                                &self.user_state,
                                output,
                                input,
                            ));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::ParameterChanged {
                            node_id: response_node_id,
                            param_name,
                            value,
                        }) => {
                            // Handle parameter changes from bottom_ui knobs
                            // Find the input param by name and update its value
                            if let Some(node) = self.graph_state.graph.nodes.get_mut(response_node_id) {
                                if let Some((_name, input_id)) = node.inputs.iter().find(|(name, _)| *name == param_name) {
                                    let input_id = *input_id;
                                    if let Some(input) = self.graph_state.graph.inputs.get_mut(input_id) {
                                        input.value.set_actual_value(value);
                                    }
                                }
                            }
                        }
                        NodeResponse::User(crate::graph::SynthResponse::MidiLearnStart {
                            engine_node_id,
                            param_index,
                            param_name,
                            min_value,
                            max_value,
                        }) => {
                            // Start MIDI Learn mode for this parameter
                            self.start_midi_learn(MidiLearnTarget {
                                node_id: engine_node_id,
                                param_index,
                                param_name,
                                min_value,
                                max_value,
                            });
                            // Update the user state to show visual feedback
                            self.user_state.midi_learn_active = true;
                            self.user_state.midi_learn_target = Some((engine_node_id, param_index));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::MidiLearnClear {
                            engine_node_id,
                            param_index,
                        }) => {
                            // Clear MIDI mapping for this parameter
                            self.clear_mapping_for_param(engine_node_id, param_index);
                            // Update the user state
                            self.user_state.remove_midi_mapping(engine_node_id, param_index);
                        }
                        NodeResponse::User(crate::graph::SynthResponse::ToggleBypass(node_id)) => {
                            bypass_toggles.push(node_id);
                        }
                        NodeResponse::User(crate::graph::SynthResponse::NodeSelected(node_id)) => {
                            node_menu_actions.push(NodeMenuAction::Select(node_id));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::DuplicateNode(node_id)) => {
                            node_menu_actions.push(NodeMenuAction::Duplicate(node_id));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::CopyNode(node_id)) => {
                            node_menu_actions.push(NodeMenuAction::Copy(node_id));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::ResetNode(node_id)) => {
                            node_menu_actions.push(NodeMenuAction::Reset(node_id));
                        }
                        NodeResponse::User(crate::graph::SynthResponse::DeleteNode(node_id)) => {
                            node_menu_actions.push(NodeMenuAction::Delete(node_id));
                        }
                        _ => {
                            // Other responses not yet handled
                        }
                    }
                }
            });

        // Detect right-click in editor to open context menu
        // Only show "add node" menu when clicking on empty canvas, not on nodes/widgets
        if ctx.input(|i| i.pointer.secondary_clicked()) && cursor_in_editor {
            // Only open if not already showing a menu and no widget context menu is open
            if self.user_state.context_menu_pos.is_none() && !self.user_state.widget_context_menu_open {
                let click_pos = ctx.input(|i| i.pointer.interact_pos());
                // A module has its own menu, and its knobs theirs. Neither says
                // so on the frame of the click, so go by where the click was
                if let Some(click_pos) = click_pos.filter(|pos| !self.is_over_module(ctx, *pos)) {
                    self.user_state.context_menu_pos = Some(click_pos);
                }
            }
        }

        // Show custom context menu for adding nodes
        if let Some(menu_pos) = self.user_state.context_menu_pos {
            let mut close_menu = false;
            let mut template_to_create: Option<SynthNodeTemplate> = None;

            // Hover delay before switching submenus (in seconds)
            const SUBMENU_HOVER_DELAY: f32 = 0.15;

            let categories = AllNodeTemplates::by_category();

            let menu_id = egui::Id::new("add_node_context_menu");
            let menu_response = egui::Area::new(menu_id)
                .fixed_pos(menu_pos)
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(120.0);

                        for (cat_index, (category, _templates)) in categories.iter().enumerate() {
                            // Create category button with arrow indicator
                            let button_text = egui::RichText::new(format!("{}  \u{25B6}", category.name()))
                                .color(category.color());

                            let response = ui.add(
                                egui::Button::new(button_text)
                                    .min_size(egui::vec2(110.0, 0.0))
                                    .frame(false)
                            );

                            // Handle hover intent with delay
                            if response.hovered() {
                                let now = std::time::Instant::now();

                                // Check if we're already tracking this category
                                if let Some((tracked_cat, hover_start)) = self.user_state.context_menu_hover_intent {
                                    if tracked_cat == cat_index {
                                        // Same category - check if delay has passed
                                        if hover_start.elapsed().as_secs_f32() >= SUBMENU_HOVER_DELAY {
                                            self.user_state.context_menu_open_category = Some(cat_index);
                                        }
                                    } else {
                                        // Different category - start new tracking
                                        self.user_state.context_menu_hover_intent = Some((cat_index, now));
                                    }
                                } else {
                                    // No tracking yet - start tracking
                                    self.user_state.context_menu_hover_intent = Some((cat_index, now));

                                    // If no submenu is open, open immediately
                                    if self.user_state.context_menu_open_category.is_none() {
                                        self.user_state.context_menu_open_category = Some(cat_index);
                                    }
                                }
                            }

                            // Handle click to immediately open/toggle
                            if response.clicked() {
                                self.user_state.context_menu_open_category = Some(cat_index);
                                self.user_state.context_menu_hover_intent = None;
                            }
                        }
                    });
                });

            // Show submenu for open category
            let mut submenu_rect: Option<egui::Rect> = None;
            if let Some(open_cat_index) = self.user_state.context_menu_open_category {
                if let Some((_category, templates)) = categories.get(open_cat_index) {
                    // Position submenu to the right of the main menu
                    let submenu_pos = menu_response.response.rect.right_top() + egui::vec2(4.0, open_cat_index as f32 * 22.0);

                    let submenu_id = egui::Id::new("add_node_submenu");
                    let submenu_response = egui::Area::new(submenu_id)
                        .fixed_pos(submenu_pos)
                        .order(egui::Order::Foreground)
                        .show(ctx, |ui| {
                            egui::Frame::menu(ui.style()).show(ui, |ui| {
                                ui.set_min_width(100.0);

                                for template in templates {
                                    let label = template.node_finder_label(&mut self.user_state);
                                    if ui.button(label.as_ref()).on_hover_text(template.description()).clicked() {
                                        template_to_create = Some(*template);
                                        close_menu = true;
                                    }
                                }
                            });
                        });

                    submenu_rect = Some(submenu_response.response.rect);

                    // Keep submenu open if mouse is inside it
                    if submenu_response.response.rect.contains(ctx.input(|i| i.pointer.hover_pos().unwrap_or_default())) {
                        // Reset hover intent when mouse is in submenu
                        self.user_state.context_menu_hover_intent = None;
                    }
                }
            }

            // Close menu on click outside
            let menu_rect = menu_response.response.rect;
            if ctx.input(|i| i.pointer.any_click()) {
                if let Some(pos) = ctx.input(|i| i.pointer.interact_pos()) {
                    // Check if click was outside both main menu and submenu
                    let in_main_menu = menu_rect.contains(pos);
                    let in_submenu = submenu_rect.map_or(false, |r| r.contains(pos));
                    if !in_main_menu && !in_submenu && template_to_create.is_none() {
                        close_menu = true;
                    }
                }
            }

            // Close menu on Escape
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                close_menu = true;
            }

            // Create node if a template was selected
            if let Some(template) = template_to_create {
                self.add_module_at(template, menu_pos);
                close_menu = true;
            }

            if close_menu {
                self.user_state.context_menu_pos = None;
                self.user_state.context_menu_open_category = None;
                self.user_state.context_menu_hover_intent = None;
            }
        }

        // The quick-add palette
        if let Some(palette) = &mut self.quick_add {
            match palette.show(ctx) {
                PaletteAction::None => {}
                PaletteAction::Close => self.quick_add = None,
                PaletteAction::Add(template) => {
                    let anchor = palette.anchor();
                    self.quick_add = None;
                    self.add_module_at(template, anchor);
                }
            }
        }

        // Send collected commands
        for cmd in commands_to_send {
            self.send_command(cmd);
        }
        for node_id in bypass_toggles {
            self.toggle_bypass(node_id);
        }
        for action in node_menu_actions {
            self.handle_node_menu(ctx, action);
        }

        // Remove invalid connections outside the UI closure
        for (output, input) in invalid_connections {
            self.graph_state.graph.remove_connection(input, output);
        }
    }

    /// Undoes the last edit to the patch.
    fn undo(&mut self) {
        let applied = self.history.undo(&mut self.graph_state, &mut self.user_state);
        self.finish_history_move(applied, "Undo", "Nothing to undo");
    }

    /// Redoes the last undone edit.
    fn redo(&mut self) {
        let applied = self.history.redo(&mut self.graph_state, &mut self.user_state);
        self.finish_history_move(applied, "Redo", "Nothing to redo");
    }

    /// Sends the engine what undo or redo changed, and says what it was.
    fn finish_history_move(&mut self, applied: Option<Applied>, verb: &str, nothing: &str) {
        let Some(applied) = applied else {
            self.status_message = Some(nothing.to_string());
            return;
        };
        for cmd in applied.commands {
            // The engine now has these values, so parameter sync needn't resend them
            if let EngineCommand::SetParameter { node_id, param_index, value } = cmd {
                self.cached_params.insert((node_id, param_index), value);
            }
            self.send_command(cmd);
        }
        self.status_message = Some(format!("{}: {}", verb, applied.label));
    }

    /// Bypasses a filter or effect, or switches it back in.
    fn toggle_bypass(&mut self, node_id: egui_node_graph2::NodeId) {
        let Some(node) = self.graph_state.graph.nodes.get_mut(node_id) else {
            return;
        };
        if !node.user_data.bypassable {
            return;
        }
        node.user_data.bypassed = !node.user_data.bypassed;
        let bypassed = node.user_data.bypassed;
        if let Some(engine_node_id) = self.user_state.get_engine_node_id(node_id) {
            self.send_command(EngineCommand::SetBypass { node_id: engine_node_id, bypassed });
        }
    }

    /// Whether a point on screen is over a module, anywhere on it.
    fn is_over_module(&self, ctx: &egui::Context, screen: egui::Pos2) -> bool {
        // The editor senses clicks on each whole node under this id
        self.graph_state.node_order.iter().any(|&node_id| {
            ctx.read_response(egui::Id::new((node_id, "window"))).is_some_and(|r| r.rect.contains(screen))
        })
    }

    /// The editor node position under a point on screen.
    fn screen_to_node(&self, screen: egui::Pos2) -> egui::Pos2 {
        screen - self.editor_rect.min.to_vec2() - self.graph_state.pan_zoom.pan
    }

    /// Where something placed "at the cursor" goes: the pointer if it's over
    /// the graph, otherwise near the middle of the view. In screen points.
    fn cursor_or_center(&self, ctx: &egui::Context) -> egui::Pos2 {
        ctx.input(|i| i.pointer.hover_pos())
            .filter(|pos| self.editor_rect.contains(*pos))
            .unwrap_or_else(|| self.editor_rect.center() - egui::vec2(150.0, 120.0))
    }

    /// Adds a module with its top-left corner at a point on screen, and selects it.
    fn add_module_at(&mut self, template: SynthNodeTemplate, screen: egui::Pos2) {
        let position = self.screen_to_node(screen);
        let (node_id, commands) = editing::add_module(&mut self.graph_state, &mut self.user_state, template, position);
        for cmd in commands {
            self.send_command(cmd);
        }
        self.graph_state.selected_nodes = vec![node_id];
    }

    /// Opens the quick-add palette at the cursor.
    fn open_quick_add(&mut self, ctx: &egui::Context) {
        let anchor = self.cursor_or_center(ctx);
        self.user_state.context_menu_pos = None;
        self.quick_add = Some(QuickAdd::new(anchor));
    }

    /// The nodes a right-click menu action applies to: the whole selection
    /// if the node is part of it, otherwise just the node.
    fn menu_targets(&self, node_id: egui_node_graph2::NodeId) -> Vec<egui_node_graph2::NodeId> {
        if self.graph_state.selected_nodes.contains(&node_id) {
            self.graph_state.selected_nodes.clone()
        } else {
            vec![node_id]
        }
    }

    fn handle_node_menu(&mut self, ctx: &egui::Context, action: NodeMenuAction) {
        match action {
            NodeMenuAction::Select(node_id) => {
                if !self.graph_state.selected_nodes.contains(&node_id) {
                    self.graph_state.selected_nodes = vec![node_id];
                }
            }
            NodeMenuAction::Duplicate(node_id) => self.duplicate_modules(&self.menu_targets(node_id)),
            NodeMenuAction::Copy(node_id) => self.copy_modules(ctx, &self.menu_targets(node_id)),
            NodeMenuAction::Reset(node_id) => self.reset_modules(&self.menu_targets(node_id)),
            NodeMenuAction::Delete(node_id) => self.delete_modules(&self.menu_targets(node_id), "Delete", "Deleted"),
        }
    }

    /// Deletes modules with their cables. `verb` names the undo step and
    /// `done` the status message: "Delete" and "Deleted", or "Cut" and "Cut".
    fn delete_modules(&mut self, nodes: &[egui_node_graph2::NodeId], verb: &str, done: &str) {
        if nodes.is_empty() {
            return;
        }
        let what = editing::describe_modules(&self.graph_state, nodes);
        for cmd in editing::delete_modules(&mut self.graph_state, &mut self.user_state, nodes) {
            self.send_command(cmd);
        }
        self.history.name_next(format!("{verb} {what}"));
        self.status_message = Some(format!("{done} {what}"));
    }

    /// Duplicates modules, with the cables between them, and selects the copies.
    fn duplicate_modules(&mut self, nodes: &[egui_node_graph2::NodeId]) {
        let what = editing::describe_modules(&self.graph_state, nodes);
        let Some(pasted) = editing::duplicate(&mut self.graph_state, &mut self.user_state, nodes) else {
            return;
        };
        self.finish_paste(pasted, &format!("Duplicate {what}"));
        self.status_message = Some(format!("Duplicated {what}"));
    }

    /// Puts modules' knobs back to their defaults.
    fn reset_modules(&mut self, nodes: &[egui_node_graph2::NodeId]) {
        let mut changed = false;
        for &node_id in nodes {
            changed |= editing::reset_parameters(&mut self.graph_state, node_id);
        }
        let what = editing::describe_modules(&self.graph_state, nodes);
        if changed {
            self.history.name_next(format!("Reset {what}"));
            self.status_message = Some(format!("Reset {what} to defaults"));
        } else {
            self.status_message = Some(format!("{what} already at defaults"));
        }
    }

    /// Puts modules and the cables between them on the clipboard, as patch
    /// JSON. They paste back into this window or another one.
    fn copy_modules(&mut self, ctx: &egui::Context, nodes: &[egui_node_graph2::NodeId]) {
        let Some(patch) = editing::copy_modules(&self.graph_state, &self.user_state, nodes) else {
            return;
        };
        match serde_json::to_string_pretty(&patch) {
            Ok(json) => {
                ctx.copy_text(json);
                self.last_paste = None;
                let what = editing::describe_modules(&self.graph_state, nodes);
                self.status_message = Some(format!("Copied {what}"));
            }
            Err(e) => self.status_message = Some(format!("Couldn't copy: {e}")),
        }
    }

    /// Pastes modules from clipboard text at the cursor.
    fn paste_modules(&mut self, ctx: &egui::Context, text: &str) {
        let Ok(patch) = crate::persistence::patch_from_json(text) else {
            self.status_message = Some("Nothing to paste: the clipboard doesn't hold modules".to_string());
            return;
        };
        let aim = self.screen_to_node(self.cursor_or_center(ctx));
        // Pasting again at the same spot fans out, like duplicating
        let at = match self.last_paste {
            Some((last_aim, landed)) if (last_aim - aim).length() < 1.0 => {
                landed + editing::DUPLICATE_OFFSET * self.graph_state.pan_zoom.zoom
            }
            _ => aim,
        };
        match editing::paste(&mut self.graph_state, &mut self.user_state, &patch, at) {
            Ok(pasted) if !pasted.nodes.is_empty() => {
                self.last_paste = Some((aim, at));
                let what = editing::describe_modules(&self.graph_state, &pasted.nodes);
                if !pasted.warnings.is_empty() {
                    self.load_warnings = pasted.warnings.clone();
                }
                self.finish_paste(pasted, &format!("Paste {what}"));
                self.status_message = Some(format!("Pasted {what}"));
            }
            Ok(_) => self.status_message = Some("Nothing to paste: no modules this version knows".to_string()),
            Err(e) => self.status_message = Some(format!("Couldn't paste: {e}")),
        }
    }

    /// Sends a paste's commands, selects what it added, and names the undo step.
    fn finish_paste(&mut self, pasted: editing::Pasted, label: &str) {
        for cmd in pasted.commands {
            self.send_command(cmd);
        }
        self.graph_state.selected_nodes = pasted.nodes;
        self.history.name_next(label);
    }

    /// Sync parameter values from the graph UI to the audio engine.
    ///
    /// This iterates through all nodes and their parameters, compares against
    /// cached values, and sends SetParameter commands for any changes.
    fn sync_parameters(&mut self) {
        let mut commands_to_send: Vec<EngineCommand> = Vec::new();

        // Iterate through all nodes
        for (node_id, node) in self.graph_state.graph.nodes.iter() {
            // Get the engine node ID for this graph node
            let Some(engine_node_id) = self.user_state.get_engine_node_id(node_id) else {
                continue;
            };

            // Keyboard and MIDI Note drive their leading params (Note, Gate, ...)
            // from live input, not from the graph UI
            let live_params = port_mapping::live_input_parameter_count(node.user_data.module_id);

            // Track which param index we're at (only count ConstantOnly params)
            let mut param_index = 0;

            // Iterate through inputs to find parameters
            for (_param_name, input_id) in &node.inputs {
                let input = self.graph_state.graph.get_input(*input_id);

                // Only process ConstantOnly or ConnectionOrConstant params
                use egui_node_graph2::InputParamKind;
                match input.kind {
                    InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                        if param_index < live_params {
                            param_index += 1;
                            continue;
                        }

                        // The engine takes real units: Hz, seconds, dB, ...
                        let actual_value = input.value.actual_value();

                        // Create cache key
                        let cache_key = (engine_node_id, param_index);

                        // Check if value has changed (use relative tolerance for large values like frequency)
                        let needs_update = match self.cached_params.get(&cache_key) {
                            Some(&cached_value) => {
                                let diff = (actual_value - cached_value).abs();
                                let threshold = if actual_value.abs() > 10.0 {
                                    actual_value.abs() * 0.0001 // Relative tolerance for large values
                                } else {
                                    0.0001 // Absolute tolerance for small values
                                };
                                diff > threshold
                            }
                            None => true, // New parameter, needs initial sync
                        };

                        if needs_update {
                            commands_to_send.push(EngineCommand::SetParameter {
                                node_id: engine_node_id,
                                param_index,
                                value: actual_value,
                            });
                            self.cached_params.insert(cache_key, actual_value);
                        }

                        param_index += 1;
                    }
                    InputParamKind::ConnectionOnly => {
                        // This is a connection-only port, not a parameter
                        // Don't increment param_index
                    }
                }
            }
        }

        // Send collected commands
        for cmd in commands_to_send {
            self.send_command(cmd);
        }
    }

    /// Create a Patch from the current graph state.
    fn create_patch(&self, name: &str) -> Patch {
        let pan_zoom = &self.graph_state.pan_zoom;

        // Get node positions normalized to zoom=1.0 coordinates for persistence.
        // The library's update_node_positions_after_zoom modifies positions when zooming,
        // so we need to reverse that transformation to get zoom-independent positions.
        // On load, we reset to zoom=1.0 and pan=0, so positions saved this way will match.
        let position = |node_id| {
            self.graph_state.node_positions
                .get(node_id)
                .map(|pos| {
                    let zoom = pan_zoom.zoom;
                    let pan = pan_zoom.pan;
                    let clip_rect = pan_zoom.clip_rect;

                    // If zoom is ~1.0 or clip_rect is invalid, use position as-is
                    if (zoom - 1.0).abs() < 0.001 || clip_rect.is_negative() {
                        (pos.x, pos.y)
                    } else {
                        // Reverse the zoom transformation to get canonical position
                        // This inverts what update_node_positions_after_zoom does
                        let half_size = clip_rect.size() / 2.0;
                        let local_pos = pos.to_vec2() - half_size + pan;
                        let unscaled = local_pos / zoom;
                        // For loading with pan=0, canonical position is:
                        let canonical = (unscaled + half_size).to_pos2();
                        (canonical.x, canonical.y)
                    }
                })
                .unwrap_or((0.0, 0.0))
        };

        capture_patch(
            name,
            &self.graph_state.graph,
            |node_id| self.user_state.get_engine_node_id(node_id),
            position,
            &self.midi_mappings,
        )
    }

    /// Load a patch, replacing the current graph.
    ///
    /// The whole patch is built into a staging graph first, and the current
    /// graph is only replaced once that has succeeded. Anything that couldn't
    /// be restored is skipped and returned as warnings.
    fn load_patch(&mut self, patch: &Patch) -> Result<Vec<String>, PatchError> {
        let mut staged = stage_patch(patch)?;

        // Stop playback during load
        let was_playing = self.is_playing;
        if was_playing {
            self.is_playing = false;
            self.user_state.is_playing = false;
            self.send_command(EngineCommand::SetPlaying(false));
        }

        // Clear the current graph
        self.clear_graph();

        // Reset pan/zoom to default (zoom=1.0, pan=0) before loading positions.
        // This is critical because the library's update_node_positions_after_zoom
        // mutates node_positions based on the current zoom level. Loading positions
        // at a different zoom than they were saved at would cause layout drift.
        self.graph_state.pan_zoom = egui_node_graph2::PanZoom::default();
        self.history.reset_view();

        // Swap in the staged graph. Its node IDs stay valid.
        self.graph_state.graph = std::mem::take(&mut staged.graph);

        for node in &staged.nodes {
            let pos = egui::pos2(node.position.0, node.position.1);
            self.graph_state.node_positions.insert(node.graph_id, pos);
            self.graph_state.node_order.push(node.graph_id);

            // Engine IDs keep counting across loads, so they differ from the
            // IDs in the patch file. MIDI mappings are remapped below.
            let engine_node_id = self.user_state.allocate_engine_node_id(node.graph_id);

            // Create the module in the audio engine. Parameter values follow
            // with the next parameter sync.
            for cmd in engine_sync::add_module(&self.graph_state.graph, node.graph_id, engine_node_id) {
                self.send_command(cmd);
            }
        }

        // Send the staged connections to the engine
        let connections: Vec<_> = self.graph_state.graph.iter_connections().collect();
        for (input_id, output_id) in connections {
            for cmd in engine_sync::cable_connected(&self.graph_state.graph, &self.user_state, output_id, input_id) {
                self.send_command(cmd);
            }
        }

        // Load MIDI mappings, retargeted from the patch's node IDs to the new ones
        self.midi_mappings = staged.remap_midi_mappings(|graph_id| self.user_state.get_engine_node_id(graph_id));
        // Sync mappings to user state for UI display
        for mapping in &self.midi_mappings {
            self.user_state.set_midi_mapping(
                mapping.node_id,
                mapping.param_index,
                mapping.cc_number,
                mapping.channel,
            );
        }

        // Restore playback state
        if was_playing {
            self.is_playing = true;
            self.user_state.is_playing = true;
            self.send_command(EngineCommand::SetPlaying(true));
        }

        // A loaded patch starts its own history, with nothing unsaved
        self.history.reset(&self.graph_state, &self.user_state);
        self.mark_saved();

        Ok(staged.warnings)
    }

    /// Clear the entire graph.
    fn clear_graph(&mut self) {
        // Send clear command to audio engine
        self.send_command(EngineCommand::ClearGraph);

        // Clear graph state
        self.graph_state.graph = egui_node_graph2::Graph::default();
        self.graph_state.node_positions.clear();
        self.graph_state.node_order.clear();

        // Clear user state (also clears MIDI mapping UI state)
        self.user_state.clear();

        // Clear cached parameters
        self.cached_params.clear();

        // Clear MIDI mappings
        self.midi_mappings.clear();
        self.midi_learn_target = None;
    }

    /// Start a new patch - clears the graph and resets the current file path.
    fn new_patch(&mut self) {
        self.clear_graph();
        self.history.reset(&self.graph_state, &self.user_state);
        self.mark_saved();
        self.load_warnings.clear();
        self.current_patch_path = None;
        self.current_example = None;
        self.status_message = Some("New patch created".to_string());
    }

    /// Show a save file dialog and save the current patch. Returns whether
    /// it was saved, not cancelled or failed.
    fn show_save_dialog(&mut self) -> bool {
        let default_name = self.current_patch_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .or(self.current_example.map(|e| e.file_name))
            .unwrap_or("patch.json");

        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Synth Patch", &["json"])
            .set_file_name(default_name)
            .save_file()
        {
            // Derive patch name from filename
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled");

            let patch = self.create_patch(name);
            match save_to_file(&patch, &path) {
                Ok(()) => {
                    self.current_patch_path = Some(path.clone());
                    self.current_example = None;
                    self.status_message = Some(format!("Saved: {}", path.display()));
                    self.saved_as(&path);
                    return true;
                }
                Err(e) => {
                    self.status_message = Some(format!("Save failed: {}", e));
                }
            }
        }
        false
    }

    /// Show a load file dialog and load the selected patch.
    fn show_load_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Synth Patch", &["json"])
            .pick_file()
        {
            self.open_file(&path);
        }
    }

    /// Load the patch file at `path`, replacing the current graph.
    fn open_file(&mut self, path: &Path) {
        match load_from_file(path).and_then(|patch| Ok((self.load_patch(&patch)?, patch.name))) {
            Ok((warnings, name)) => {
                self.current_patch_path = Some(path.to_path_buf());
                self.current_example = None;
                self.status_message = Some(format!("Loaded: {}", name));
                self.show_load_warnings(warnings);
                self.recent_files.push(path);
            }
            Err(e) => {
                self.status_message = Some(format!("Load failed: {}", e));
                // A file that's gone stops being offered
                if !path.exists() {
                    self.recent_files.remove(path);
                }
            }
        }
    }

    /// Open one of the bundled example patches. It has no file, so saving
    /// it asks where to put the copy.
    fn open_example(&mut self, example: &'static Example) {
        match example.patch().and_then(|patch| self.load_patch(&patch)) {
            Ok(warnings) => {
                self.current_patch_path = None;
                self.current_example = Some(example);
                self.status_message = Some(format!("Opened example: {}", example.name));
                self.show_load_warnings(warnings);
            }
            Err(e) => {
                self.status_message = Some(format!("Couldn't open example {}: {}", example.name, e));
            }
        }
    }

    /// Opens the patch the app starts with: the file named on the command
    /// line, or else the First Sound example, so the canvas is never empty.
    pub fn open_on_launch(&mut self, path: Option<&Path>) {
        match path {
            Some(path) => self.open_file(path),
            None => self.open_example(examples::first_sound()),
        }
    }

    /// Keep the problems from a load on screen (and on stderr) until dismissed.
    fn show_load_warnings(&mut self, warnings: Vec<String>) {
        for warning in &warnings {
            eprintln!("Patch load warning: {}", warning);
        }
        self.load_warnings = warnings;
    }

    /// Quick save to the current path, or show save dialog if no path.
    /// Returns whether the patch was saved.
    fn quick_save(&mut self) -> bool {
        if let Some(path) = self.current_patch_path.clone() {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled");

            let patch = self.create_patch(name);
            match save_to_file(&patch, &path) {
                Ok(()) => {
                    self.status_message = Some(format!("Saved: {}", path.display()));
                    self.saved_as(&path);
                    true
                }
                Err(e) => {
                    self.status_message = Some(format!("Save failed: {}", e));
                    false
                }
            }
        } else {
            self.show_save_dialog()
        }
    }

    /// Notes a successful save to `path`.
    fn saved_as(&mut self, path: &Path) {
        // Edits made this frame are in the file, so they're in the history first
        self.sync_history();
        self.mark_saved();
        self.recent_files.push(path);
    }

    /// Notes that the patch as it stands is what was last opened or saved.
    fn mark_saved(&mut self) {
        self.history.mark_saved();
        self.saved_midi_mappings = self.midi_mappings.clone();
    }

    /// Records any edit not yet in the undo history, so it counts as a change.
    fn sync_history(&mut self) {
        self.history.record(&self.graph_state, &self.user_state, false, Instant::now());
    }

    /// Whether the patch has changes that aren't saved anywhere.
    fn has_unsaved_changes(&self) -> bool {
        self.history.has_unsaved_changes() || self.midi_mappings != self.saved_midi_mappings
    }

    /// The patch's name: its file's, its example's, or "Untitled".
    fn patch_title(&self) -> String {
        match (&self.current_patch_path, self.current_example) {
            (Some(path), _) => path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").to_string(),
            (None, Some(example)) => example.name.to_string(),
            (None, None) => "Untitled".to_string(),
        }
    }

    /// Does `action` now if nothing would be lost, or asks first.
    fn request(&mut self, ctx: &egui::Context, action: Discard) {
        self.sync_history();
        if self.has_unsaved_changes() {
            self.pending_discard = Some(action);
        } else {
            self.perform(ctx, action);
        }
    }

    /// Does something that replaces or closes the patch, once its unsaved
    /// changes are saved or let go.
    fn perform(&mut self, ctx: &egui::Context, action: Discard) {
        match action {
            Discard::New => self.new_patch(),
            Discard::Open => self.show_load_dialog(),
            Discard::OpenFile(path) => self.open_file(&path),
            Discard::OpenExample(example) => self.open_example(example),
            Discard::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Shows whichever prompt is waiting, and acts on its answer.
    fn show_prompts(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.pending_discard.clone() {
            if let Some(answer) = session::unsaved_changes_prompt(ctx, &self.patch_title(), &action) {
                self.pending_discard = None;
                match answer {
                    // A cancelled save dialog cancels the whole thing
                    Answer::Save => {
                        if self.quick_save() {
                            self.perform(ctx, action);
                        }
                    }
                    Answer::Discard => self.perform(ctx, action),
                    Answer::Cancel => {}
                }
            }
        } else if let Some(autosave) = &self.recovery {
            if let Some(recover) = session::recovery_prompt(ctx, autosave) {
                let autosave = self.recovery.take().expect("shown above");
                if recover {
                    self.recover(&autosave);
                }
            }
        }
    }

    /// Whether a prompt is up, so shortcuts wait.
    fn prompt_open(&self) -> bool {
        self.pending_discard.is_some() || self.recovery.is_some()
    }

    /// Brings back the patch an autosave holds. It's still unsaved.
    fn recover(&mut self, autosave: &Autosave) {
        match autosave.patch().and_then(|patch| self.load_patch(&patch)) {
            Ok(warnings) => {
                self.current_patch_path = autosave.path.clone();
                self.current_example = autosave.example.as_deref()
                    .and_then(|name| EXAMPLES.iter().find(|e| e.name == name));
                self.history.mark_unsaved();
                self.status_message = Some(format!("Recovered unsaved changes to {}", autosave.name));
                self.show_load_warnings(warnings);
            }
            Err(e) => {
                self.status_message = Some(format!("Couldn't recover {}: {}", autosave.name, e));
            }
        }
    }

    /// Picks up the last session: its recent files, and any autosave a crash
    /// left, which is offered back on the first frame.
    pub fn restore_session(&mut self, storage: Option<&dyn eframe::Storage>) {
        self.recent_files = RecentFiles::load(storage);
        self.recovery = Autosave::load(storage);
        if let Some(glyph) = storage.and_then(|s| s.get_string(FLOW_GLYPH_KEY)) {
            self.user_state.flow_glyph = FlowGlyph::ALL
                .into_iter()
                .find(|g| g.name() == glyph)
                .unwrap_or_default();
        }
    }

    /// Validate a connection and return an error message if invalid.
    ///
    /// Returns None if the connection is valid, Some(error_msg) if invalid.
    fn validate_and_check_connection(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> Option<String> {
        // Get the signal types for both ports
        let output_type = self.graph_state.graph
            .any_param_type(AnyParameterId::Output(output))
            .ok()
            .map(|dt| dt.signal_type());

        let input_type = self.graph_state.graph
            .any_param_type(AnyParameterId::Input(input))
            .ok()
            .map(|dt| dt.signal_type());

        match (output_type, input_type) {
            (Some(from_type), Some(to_type)) => {
                let result = validate_connection(from_type, to_type);
                if !result.is_valid() {
                    result.error_message().map(|s| s.to_string())
                } else {
                    None
                }
            }
            _ => {
                // Couldn't get types - shouldn't happen
                Some("Could not determine port types".to_string())
            }
        }
    }

    /// Check if a connection between two nodes would create a self-loop.
    #[allow(dead_code)]
    fn is_self_connection(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> bool {
        let output_node = self.graph_state.graph.get_output(output).node;
        let input_node = self.graph_state.graph.get_input(input).node;
        output_node == input_node
    }

    /// Draw the bottom status bar
    fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Priority order: status message > validation message > audio error > default status
            if let Some(ref status_msg) = self.status_message {
                // Show status message (from save/load)
                ui.label(RichText::new(status_msg)
                    .color(theme::accent::SUCCESS)
                    .small());
            } else if let Some(validation_msg) = self.user_state.validation_message() {
                // Show validation error with warning icon
                ui.label(RichText::new(format!("⚠ {}", validation_msg))
                    .color(theme::accent::WARNING)
                    .small());
            } else if let Some(ref error) = self.audio_error_message {
                // Show audio error
                ui.label(RichText::new(format!("⚠ {}", error))
                    .color(theme::accent::ERROR)
                    .small());
            } else if let Some(ref error) = self.midi_error_message {
                ui.label(RichText::new(format!("⚠ {}", error))
                    .color(theme::accent::ERROR)
                    .small());
            } else {
                // Show node and connection count
                let node_count = self.graph_state.graph.nodes.len();
                let connection_count = self.graph_state.graph.iter_connections().count();

                let status = if node_count == 0 {
                    "Right-click to add nodes".to_string()
                } else if connection_count == 0 {
                    format!("{} node{}", node_count, if node_count == 1 { "" } else { "s" })
                } else {
                    format!(
                        "{} node{}, {} connection{}",
                        node_count,
                        if node_count == 1 { "" } else { "s" },
                        connection_count,
                        if connection_count == 1 { "" } else { "s" }
                    )
                };
                ui.label(RichText::new(status)
                    .color(theme::text::SECONDARY)
                    .small());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // Problems from the last load stay visible until dismissed
                if !self.load_warnings.is_empty() {
                    let count = self.load_warnings.len();
                    let label = ui.add(
                        egui::Label::new(
                            RichText::new(format!(
                                "⚠ {} load warning{}",
                                count,
                                if count == 1 { "" } else { "s" }
                            ))
                            .color(theme::accent::WARNING)
                            .small(),
                        )
                        .sense(egui::Sense::click()),
                    );
                    let details = format!("{}\n\nClick to dismiss", self.load_warnings.join("\n"));
                    if label.on_hover_text(details).clicked() {
                        self.load_warnings.clear();
                    }
                    ui.label(RichText::new("|")
                        .color(theme::text::DISABLED)
                        .small());
                }

                // Show current patch name if any
                let patch_name = match (&self.current_patch_path, self.current_example) {
                    (Some(path), _) => path.file_stem().and_then(|s| s.to_str()).map(str::to_string),
                    (None, Some(example)) => Some(format!("{} (example)", example.name)),
                    (None, None) => self.has_unsaved_changes().then(|| "Untitled".to_string()),
                };
                if let Some(name) = patch_name {
                    // Right to left: the dot sits after the name
                    if self.has_unsaved_changes() {
                        ui.label(RichText::new("●").color(theme::accent::WARNING).small())
                            .on_hover_text("Unsaved changes (Ctrl+S to save)");
                    }
                    ui.label(RichText::new(name)
                        .color(theme::text::SECONDARY)
                        .small());
                    ui.label(RichText::new("|")
                        .color(theme::text::DISABLED)
                        .small());
                }
                ui.label(RichText::new(concat!("Modular Synth v", env!("CARGO_PKG_VERSION")))
                    .color(theme::text::DISABLED)
                    .small());
            });
        });
    }

    /// Delete, duplicate, copy, cut, paste, and Space or Tab for the
    /// quick-add palette.
    fn handle_editing_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Event, Key, KeyboardShortcut, Modifiers};
        let mut copy = false;
        let mut cut = false;
        let mut paste = None;
        let (delete, duplicate, palette) = ctx.input_mut(|i| {
            let delete = i.consume_key(Modifiers::NONE, Key::Delete) || i.consume_key(Modifiers::NONE, Key::Backspace);
            let duplicate = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::D));
            let palette = i.consume_key(Modifiers::NONE, Key::Space) || i.consume_key(Modifiers::NONE, Key::Tab);
            // The window turns Ctrl+C, Ctrl+X and Ctrl+V into these, not key presses
            i.events.retain(|event| match event {
                Event::Copy => { copy = true; false }
                Event::Cut => { cut = true; false }
                Event::Paste(text) => { paste = Some(text.clone()); false }
                // The Space that opens the palette isn't typed into it
                Event::Text(text) if palette && text == " " => false,
                _ => true,
            });
            (delete, duplicate, palette)
        });

        let selected = self.graph_state.selected_nodes.clone();
        if copy || cut {
            self.copy_modules(ctx, &selected);
        }
        if cut {
            self.delete_modules(&selected, "Cut", "Cut");
        } else if delete {
            self.delete_modules(&selected, "Delete", "Deleted");
        }
        if duplicate {
            self.duplicate_modules(&selected);
        }
        if let Some(text) = paste {
            self.paste_modules(ctx, &text);
        }
        if palette {
            self.open_quick_add(ctx);
        }
    }

    /// Check if there are any Keyboard modules in the graph.
    fn has_keyboard_modules(&self) -> bool {
        self.graph_state.graph.nodes.iter()
            .any(|(_, node)| node.user_data.module_id == "input.keyboard")
    }

    /// Handle keyboard events for the virtual keyboard module.
    ///
    /// Uses raw input events to capture keyboard input before the UI consumes them.
    fn handle_keyboard_events(&mut self, ctx: &egui::Context) {
        // Skip if modifier keys are held (those are shortcuts, not notes)
        let skip_keyboard = ctx.input(|i| {
            i.modifiers.ctrl || i.modifiers.alt || i.modifiers.command
        });
        if skip_keyboard {
            return;
        }

        // Letters typed into a text field (the palette, say) aren't notes.
        // Releases still count, so a key held while it opened doesn't stick
        let typing = ctx.wants_keyboard_input() || self.quick_add.is_some();

        // Process raw keyboard events - these haven't been consumed yet
        let mut keys_changed = false;
        // With a Poly MIDI module to hear them, the keys also play as MIDI,
        // so chords can be played without a MIDI keyboard
        let play_midi = self.has_poly_midi_modules();
        let mut midi_notes = Vec::new();

        ctx.input(|i| {
            // Check raw events for key presses/releases
            for event in &i.raw.events {
                if let egui::Event::Key { key, pressed, repeat, .. } = event {
                    // Skip key repeat events
                    if *repeat {
                        continue;
                    }

                    if let Some(relative_note) = key_to_note(*key) {
                        if *pressed && typing {
                            continue;
                        }
                        if *pressed {
                            // Add key if not already in list
                            if !self.pressed_keys.iter().any(|(_, k)| k == key) {
                                self.pressed_keys.push((relative_note, *key));
                                keys_changed = true;
                                midi_notes.push((relative_note, true));
                            }
                        } else {
                            // Remove key from list
                            if let Some(pos) = self.pressed_keys.iter().position(|(_, k)| k == key) {
                                self.pressed_keys.remove(pos);
                                keys_changed = true;
                                midi_notes.push((relative_note, false));
                            }
                        }
                    }
                }
            }
        });

        if let (true, Some(engine)) = (play_midi, self.midi_engine.as_ref()) {
            for (relative_note, pressed) in midi_notes {
                let note = relative_to_midi(relative_note, 0);
                engine.send(if pressed {
                    MidiEvent::NoteOn { channel: 0, note, velocity: 100 }
                } else {
                    MidiEvent::NoteOff { channel: 0, note, velocity: 0 }
                });
            }
        }

        // Update Keyboard modules if key state changed
        if keys_changed {
            self.sync_keyboard_modules();
        }
    }

    /// Minimum gate duration in milliseconds to ensure audio thread sees the trigger.
    const MIN_GATE_DURATION_MS: u64 = 30;

    /// Sync the current keyboard state to all Keyboard modules in the graph.
    ///
    /// Updates the Note and Gate parameters based on the currently pressed keys.
    /// Implements minimum gate duration to ensure reliable triggering.
    fn sync_keyboard_modules(&mut self) {
        // Determine the active note based on key priority (for now, always use "Last" priority)
        let (active_note, should_gate_on) = if let Some((note, _key)) = self.pressed_keys.last() {
            let midi_note = relative_to_midi(*note, 0);
            (midi_note as f32, true)
        } else {
            (self.last_triggered_note, false)
        };

        // Determine actual gate state considering minimum duration
        let gate_value = if should_gate_on {
            // Key is pressed - gate should be on
            if !self.gate_held_high {
                // New note trigger
                self.last_gate_on = Some(Instant::now());
                self.gate_held_high = true;
                self.last_triggered_note = active_note;
            }
            1.0
        } else if self.gate_held_high {
            // Key released but check minimum duration
            if let Some(gate_time) = self.last_gate_on {
                let elapsed_ms = gate_time.elapsed().as_millis() as u64;
                if elapsed_ms < Self::MIN_GATE_DURATION_MS {
                    // Keep gate high until minimum duration
                    1.0
                } else {
                    // Minimum duration passed, can release
                    self.gate_held_high = false;
                    self.last_gate_on = None;
                    0.0
                }
            } else {
                self.gate_held_high = false;
                0.0
            }
        } else {
            0.0
        };

        // Collect Keyboard module engine IDs first to avoid borrow issues
        let keyboard_nodes: Vec<u64> = self.graph_state.graph.nodes.iter()
            .filter(|(_, node)| node.user_data.module_id == "input.keyboard")
            .filter_map(|(node_id, _)| self.user_state.get_engine_node_id(node_id))
            .collect();

        // Parameters are in order: Note(0), Gate(1), Octave(2), Velocity(3), Priority(4)
        let note_param_idx = 0;
        let gate_param_idx = 1;

        // Use the triggered note when gate is high, otherwise active_note
        let note_to_send = if self.gate_held_high { self.last_triggered_note } else { active_note };

        // Update all Keyboard modules
        for engine_node_id in keyboard_nodes {
            // Update Note parameter
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: note_param_idx,
                value: note_to_send,
            });

            // Update Gate parameter
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: gate_param_idx,
                value: gate_value,
            });

            // Also update the cached params so sync_parameters doesn't overwrite
            self.cached_params.insert((engine_node_id, note_param_idx), note_to_send);
            self.cached_params.insert((engine_node_id, gate_param_idx), gate_value);
        }

        // Update active notes for piano display (convert relative notes to MIDI notes)
        let active_notes: Vec<u8> = self.pressed_keys.iter()
            .map(|(rel, _)| relative_to_midi(*rel, 0) as u8)
            .collect();
        self.user_state.set_keyboard_active_notes(active_notes);
    }

    /// Check if gate needs to be released after minimum duration.
    fn update_gate_timing(&mut self) {
        if self.gate_held_high && self.pressed_keys.is_empty() {
            // Key was released, check if we should release the gate now
            if let Some(gate_time) = self.last_gate_on {
                if gate_time.elapsed().as_millis() as u64 >= Self::MIN_GATE_DURATION_MS {
                    // Time to release
                    self.sync_keyboard_modules();
                }
            }
        }
    }
}

/// The `nth` module of a kind (by registry id, in graph order) and its input
/// named `input`, for capture scripts.
fn find_input(graph: &crate::graph::SynthGraph, module: &str, nth: usize, input: &str) -> Option<egui_node_graph2::InputId> {
    let node = graph.nodes.values().filter(|n| n.user_data.module_id == module).nth(nth)?;
    node.inputs.iter().find(|(name, _)| name.eq_ignore_ascii_case(input)).map(|(_, id)| *id)
}

/// Tooltip for the Undo and Redo buttons, e.g. "Undo Move Oscillator (Ctrl+Z)".
fn history_hint(verb: &str, label: Option<&str>, shortcut: &str) -> String {
    match label {
        Some(label) => format!("{verb} {label} ({shortcut})"),
        None => format!("Nothing to {} ({shortcut})", verb.to_lowercase()),
    }
}

/// Actions collected from the toolbar for deferred execution
#[derive(Default)]
struct ToolbarActions {
    toggle_playing: bool,
    select_device: Option<usize>,
    refresh_devices: bool,
    save_patch: bool,
    save_as_patch: bool,
    load_patch: bool,
    open_example: Option<&'static Example>,
    open_recent: Option<PathBuf>,
    clear_recent: bool,
    new_patch: bool,
    undo: bool,
    redo: bool,
    // MIDI actions
    connect_midi_device: Option<usize>,
    disconnect_midi: bool,
    refresh_midi_devices: bool,
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on first frame
        if !self.theme_applied {
            theme::apply_theme(ctx);
            self.theme_applied = true;
        }

        // A capture renders this frame's audio before anything is drawn
        self.step_capture();

        // Process events from the audio engine
        self.process_engine_events();
        let now = ctx.input(|i| i.time);
        self.user_state.tick_signal_history(now, &self.graph_state.graph);

        // Advance meter ballistics; keep repainting until it has settled
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.user_state.output_meter.tick(dt);
        if !self.user_state.output_meter.is_idle() {
            ctx.request_repaint();
        }

        // Clear status message after it's been shown (user will see it on first frame)
        // We clear it on the next frame after it was set
        let had_status_message = self.status_message.is_some();

        // Request continuous repaints when playing (for LED indicators and other visualizations)
        // Also repaint continuously when there are keyboard or MIDI Note modules, so
        // their pianos follow the keys
        if self.is_playing || self.has_keyboard_modules() || self.has_midi_note_modules() {
            ctx.request_repaint();
        }

        // Handle keyboard shortcuts
        let mut keyboard_save = false;
        let mut keyboard_save_as = false;
        let mut keyboard_load = false;
        let mut keyboard_bypass = false;

        // Closing the window with unsaved changes asks first
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(ctx, Discard::Quit);
        }

        // Undo and redo, unless a text field has the keys (it has its own undo).
        // Redo is checked first: Ctrl+Z alone would also match Ctrl+Shift+Z.
        let prompt_open = self.prompt_open();
        if !ctx.wants_keyboard_input() && !prompt_open {
            use egui::{Key, KeyboardShortcut, Modifiers};
            let (redo, undo) = ctx.input_mut(|i| {
                let redo = i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z))
                    || i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y));
                (redo, i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z)))
            });
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            }
        }

        let mut keyboard_new = false;
        ctx.input(|i| {
            if prompt_open {
                return;
            }
            // Ctrl+N: New
            if i.modifiers.ctrl && i.key_pressed(egui::Key::N) {
                keyboard_new = true;
            }
            // Ctrl+S: Save, Ctrl+Shift+S: Save As
            if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                if i.modifiers.shift {
                    keyboard_save_as = true;
                } else {
                    keyboard_save = true;
                }
            }
            // Ctrl+O: Open/Load
            if i.modifiers.ctrl && i.key_pressed(egui::Key::O) {
                keyboard_load = true;
            }
            // Ctrl+B: Bypass the selected nodes
            if i.modifiers.ctrl && i.key_pressed(egui::Key::B) {
                keyboard_bypass = true;
            }
        });

        if keyboard_bypass {
            for node_id in self.graph_state.selected_nodes.clone() {
                self.toggle_bypass(node_id);
            }
        }

        // Editing shortcuts act on the graph, so they wait while a text field
        // or the palette has the keys
        if !ctx.wants_keyboard_input() && self.quick_add.is_none() && !prompt_open {
            self.handle_editing_shortcuts(ctx);
        }

        // Handle musical keyboard input (QWERTY to notes)
        self.handle_keyboard_events(ctx);

        // Update gate timing (for minimum gate duration)
        self.update_gate_timing();

        // Top toolbar panel
        let toolbar_actions = egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 8.0)))
            .show(ctx, |ui| {
                self.draw_toolbar(ui)
            })
            .inner;

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 4.0)))
            .show(ctx, |ui| {
                self.draw_status_bar(ui);
            });

        // Main content area - the node graph editor
        self.draw_main_area(ctx);

        // Sync parameter values to the audio engine
        self.sync_parameters();

        // Handle deferred actions (to avoid borrow checker issues)
        if toolbar_actions.toggle_playing {
            self.is_playing = !self.is_playing;
            self.user_state.is_playing = self.is_playing;
            self.send_command(EngineCommand::SetPlaying(self.is_playing));
        }
        if toolbar_actions.refresh_devices {
            self.refresh_devices();
        }
        if let Some(device_index) = toolbar_actions.select_device {
            self.select_device(device_index);
        }

        // Handle save/load actions (from toolbar buttons or keyboard shortcuts)
        if toolbar_actions.save_patch || keyboard_save {
            self.quick_save();
        }
        if toolbar_actions.save_as_patch || keyboard_save_as {
            self.show_save_dialog();
        }
        if toolbar_actions.load_patch || keyboard_load {
            self.request(ctx, Discard::Open);
        }
        if let Some(example) = toolbar_actions.open_example {
            self.request(ctx, Discard::OpenExample(example));
        }
        if let Some(path) = toolbar_actions.open_recent {
            self.request(ctx, Discard::OpenFile(path));
        }
        if toolbar_actions.clear_recent {
            self.recent_files.clear();
        }
        if toolbar_actions.new_patch || keyboard_new {
            self.request(ctx, Discard::New);
        }
        if toolbar_actions.undo {
            self.undo();
        }
        if toolbar_actions.redo {
            self.redo();
        }

        // Handle MIDI actions
        if toolbar_actions.refresh_midi_devices {
            self.refresh_midi_devices();
        }
        if let Some(device_index) = toolbar_actions.connect_midi_device {
            self.connect_midi_device(device_index);
        }
        if toolbar_actions.disconnect_midi {
            self.disconnect_midi_device();
        }

        // Process pending MIDI events
        self.process_midi_events();

        // "Save changes?" and crash recovery, over everything else
        self.show_prompts(ctx);

        // Whatever this frame changed becomes an undo step, once the mouse
        // button is up: a knob turn or a drag is one step, not one per frame
        let gesture_held = ctx.input(|i| i.pointer.any_down());
        self.history.record(&self.graph_state, &self.user_state, gesture_held, Instant::now());

        // Ship this frame's graph edits to the audio thread as one compiled plan
        if let Some(ref mut handle) = self.ui_handle {
            if !handle.flush() {
                // The audio thread is behind; retry soon even if nothing else
                // asks for a frame, so held-back edits aren't left waiting
                ctx.request_repaint_after(std::time::Duration::from_millis(15));
            }
        }

        // The title names the patch, with a dot while it has unsaved changes
        let title = format!(
            "{}{} · Modular Synth",
            if self.has_unsaved_changes() { "● " } else { "" },
            self.patch_title()
        );
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }

        // Clear status message after showing it for one frame
        // This gives user time to read it but doesn't persist forever
        if had_status_message {
            // Request one more repaint to clear the message
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
        }

        if let Some(capture) = self.capture.as_mut() {
            capture.draw_cursor(ctx);
            capture.end_frame(ctx);
            if capture.finish() {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// While capturing, the script replaces the mouse and keyboard, and the
    /// clock moves one frame per pass.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if let Some(capture) = self.capture.as_mut() {
            capture.prepare_input(ctx, raw_input);
        }
    }

    /// Stores recent files, and the patch while it has unsaved changes, so
    /// a crash loses at most [`session::AUTOSAVE_INTERVAL`] of work.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // A capture's edits are the script's, not work to keep
        if self.capture.is_some() {
            return;
        }
        self.recent_files.store(storage);
        storage.set_string(FLOW_GLYPH_KEY, self.user_state.flow_glyph.name().to_string());
        let autosave = if let Some(recovery) = &self.recovery {
            // Not answered yet: keep it for next time
            Some(recovery.clone())
        } else if self.has_unsaved_changes() && !self.allow_close {
            let patch = self.create_patch(&self.patch_title());
            let example = self.current_example.map(|e| e.name.to_string());
            Autosave::new(&patch, self.current_patch_path.clone(), example).ok()
        } else {
            // Saved, or let go on the way out
            None
        };
        Autosave::store(storage, autosave.as_ref());
    }

    fn auto_save_interval(&self) -> std::time::Duration {
        session::AUTOSAVE_INTERVAL
    }

    /// Only the app's own state is kept; egui's (open menus, scroll
    /// positions) starts fresh each launch.
    fn persist_egui_memory(&self) -> bool {
        false
    }
}
