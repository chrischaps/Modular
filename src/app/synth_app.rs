//! Main application struct for the Modular Synth
//!
//! Contains the SynthApp which implements eframe::App and manages
//! the synthesizer's UI state, audio engine, and graph state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use eframe::egui::{self, RichText, Layout, Align};
use egui_node_graph2::{GraphEditorState, NodeResponse, NodeTemplateTrait, InputParamKind};

use crate::engine::{
    AudioEngine, AudioError, AudioProcessor, DeviceInfo, EngineChannels, EngineCommand, UiHandle,
    MidiDeviceInfo, MidiEngine, MidiEvent, TimestampedMidiEvent,
};
use rtrb::Consumer;
use crate::graph::{
    validate_connection, AllNodeTemplates, AnyParameterId, SynthDataType, SynthGraphState,
    SynthNodeData, SynthNodeTemplate, SynthValueType,
};
use crate::modules::keyboard::{key_to_note, relative_to_midi};
use crate::modules::midi_note::MidiNote;
use crate::persistence::{
    ConnectionData, MidiMapping, NodeData, ParameterValue, Patch, PatchError,
    load_from_file, save_to_file,
};
use crate::widgets::{cpu_meter, CpuMeterConfig};
use super::theme;

/// Type alias for our graph editor state
type SynthGraphEditorState = GraphEditorState<SynthNodeData, SynthDataType, SynthValueType, SynthNodeTemplate, SynthGraphState>;

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

    /// Status message for save/load operations (auto-clears after display).
    status_message: Option<String>,

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

    /// Consumer for receiving MIDI events from the MIDI engine.
    midi_event_consumer: Option<Consumer<TimestampedMidiEvent>>,

    /// Cached list of MIDI input devices.
    midi_devices: Vec<MidiDeviceInfo>,

    /// Index of currently selected MIDI device (None = no device).
    selected_midi_device: Option<usize>,

    /// MIDI error message to display.
    midi_error_message: Option<String>,

    // --- MIDI Note module state ---
    /// Currently held MIDI notes for MIDI Note modules.
    /// Stores (note_number, velocity, channel) in order of press for voice priority.
    midi_held_notes: Vec<(u8, u8, u8)>,

    /// Timestamp when the MIDI gate was last triggered (for minimum gate duration).
    midi_last_gate_on: Option<Instant>,

    /// Whether the MIDI gate is currently being held high (for minimum duration).
    midi_gate_held_high: bool,

    /// The last MIDI note that was triggered (to maintain pitch during gate hold).
    midi_last_triggered_note: u8,

    /// The last MIDI velocity that was triggered.
    midi_last_velocity: u8,

    /// Current aftertouch value (channel pressure).
    midi_aftertouch: u8,

    // --- MIDI CC Mapping state ---
    /// Active MIDI CC to parameter mappings.
    midi_mappings: Vec<MidiMapping>,

    /// Target for MIDI Learn mode (None = not learning).
    midi_learn_target: Option<MidiLearnTarget>,
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

        // Create engine channels for communication with audio thread
        let channels = EngineChannels::with_defaults();
        let (ui_handle, engine_handle) = channels.split();

        // Create and start the audio processor if engine is available
        let ui_handle = if let Ok(ref mut engine) = audio_engine {
            let sample_rate = engine.sample_rate() as f32;
            let block_size = 256; // Standard block size
            let processor = AudioProcessor::new(sample_rate, block_size, engine_handle);

            if let Err(e) = engine.start_with_processor(processor) {
                eprintln!("Failed to start audio processor: {}", e);
            }

            Some(ui_handle)
        } else {
            // Drop the engine_handle since we can't use it
            drop(engine_handle);
            None
        };

        // Initialize MIDI engine
        let (midi_engine, midi_event_consumer, midi_devices, midi_error_message) =
            match MidiEngine::new() {
                Ok((mut engine, consumer)) => {
                    let devices = engine.enumerate_devices();
                    (Some(engine), Some(consumer), devices, None)
                }
                Err(e) => {
                    eprintln!("MIDI initialization failed: {}", e);
                    (None, None, Vec::new(), Some(e.to_string()))
                }
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
            status_message: None,
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
            // MIDI Note module state
            midi_held_notes: Vec::new(),
            midi_last_gate_on: None,
            midi_gate_held_high: false,
            midi_last_triggered_note: 60,
            midi_last_velocity: 100,
            midi_aftertouch: 0,
            // MIDI CC Mapping state
            midi_mappings: Vec::new(),
            midi_learn_target: None,
        };

        // Note: enable_test_tone is ignored - test tone was removed in favor of AudioProcessor
        let _ = enable_test_tone;

        app
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
                    self.midi_error_message = Some(e.to_string());
                }
            }
        }
    }

    /// Disconnect from the current MIDI device
    fn disconnect_midi_device(&mut self) {
        if let Some(ref mut engine) = self.midi_engine {
            engine.disconnect();
            self.selected_midi_device = None;
        }
    }

    /// Process pending MIDI events.
    /// - Stores events in user state for display by MIDI Monitor modules.
    /// - Routes note events to MIDI Note modules.
    /// - Handles CC events for MIDI Learn and mapped parameters.
    fn process_midi_events(&mut self) {
        let mut notes_changed = false;
        let mut cc_updates: Vec<(u64, usize, f32)> = Vec::new();
        let mut learned_mapping: Option<MidiMapping> = None;

        if let Some(ref mut consumer) = self.midi_event_consumer {
            while let Ok(timestamped) = consumer.pop() {
                let event = timestamped.event;

                // Store the event for MIDI Monitor display
                self.user_state.push_midi_event(event);

                // Process MIDI events for MIDI Note modules
                match event {
                    MidiEvent::NoteOn { channel, note, velocity } => {
                        // Add note if not already in list
                        if !self.midi_held_notes.iter().any(|(n, _, _)| *n == note) {
                            self.midi_held_notes.push((note, velocity, channel));
                            notes_changed = true;
                        }
                    }
                    MidiEvent::NoteOff { note, .. } => {
                        // Remove note from list
                        if let Some(pos) = self.midi_held_notes.iter().position(|(n, _, _)| *n == note) {
                            self.midi_held_notes.remove(pos);
                            notes_changed = true;
                        }
                    }
                    MidiEvent::ChannelPressure { pressure, .. } => {
                        // Update aftertouch
                        if self.midi_aftertouch != pressure {
                            self.midi_aftertouch = pressure;
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
                    _ => {
                        // Other events (pitch bend, etc.) are not handled yet
                    }
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

        // Update MIDI Note modules if note state changed
        if notes_changed {
            self.sync_midi_note_modules();
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

    /// Check if there are any MIDI Note modules in the graph.
    fn has_midi_note_modules(&self) -> bool {
        self.graph_state.graph.nodes.iter()
            .any(|(_, node)| node.user_data.module_id == "input.midi_note")
    }

    /// Minimum MIDI gate duration in milliseconds.
    const MIN_MIDI_GATE_DURATION_MS: u64 = 30;

    /// Sync the current MIDI note state to all MIDI Note modules in the graph.
    fn sync_midi_note_modules(&mut self) {
        // Determine the active note based on voice priority (for now, always use "Last" priority)
        let (active_note, active_velocity, should_gate_on) = if let Some(&(note, velocity, _channel)) = self.midi_held_notes.last() {
            (note, velocity, true)
        } else {
            (self.midi_last_triggered_note, self.midi_last_velocity, false)
        };

        // Determine actual gate state considering minimum duration
        let gate_value = if should_gate_on {
            // Note is pressed - gate should be on
            if !self.midi_gate_held_high {
                // New note trigger (gate was off)
                self.midi_last_gate_on = Some(Instant::now());
                self.midi_gate_held_high = true;
            }
            // Always update to the most recent note ("Last" priority)
            self.midi_last_triggered_note = active_note;
            self.midi_last_velocity = active_velocity;
            1.0
        } else if self.midi_gate_held_high {
            // Note released but check minimum duration
            if let Some(gate_time) = self.midi_last_gate_on {
                let elapsed_ms = gate_time.elapsed().as_millis() as u64;
                if elapsed_ms < Self::MIN_MIDI_GATE_DURATION_MS {
                    // Keep gate high until minimum duration
                    1.0
                } else {
                    // Minimum duration passed, can release
                    self.midi_gate_held_high = false;
                    self.midi_last_gate_on = None;
                    0.0
                }
            } else {
                self.midi_gate_held_high = false;
                0.0
            }
        } else {
            0.0
        };

        // Collect MIDI Note module engine IDs first to avoid borrow issues
        let midi_note_nodes: Vec<u64> = self.graph_state.graph.nodes.iter()
            .filter(|(_, node)| node.user_data.module_id == "input.midi_note")
            .filter_map(|(node_id, _)| self.user_state.get_engine_node_id(node_id))
            .collect();

        // Use the triggered note when gate is high, otherwise active_note
        let note_to_send = if self.midi_gate_held_high { self.midi_last_triggered_note } else { active_note };
        let velocity_to_send = if self.midi_gate_held_high { self.midi_last_velocity } else { active_velocity };

        // Update all MIDI Note modules
        for engine_node_id in midi_note_nodes {
            // Update Note parameter (param index 0)
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: MidiNote::PARAM_NOTE,
                value: note_to_send as f32,
            });

            // Update Gate parameter (param index 1)
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: MidiNote::PARAM_GATE,
                value: gate_value,
            });

            // Update Velocity parameter (param index 2) - raw 0-127 value
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: MidiNote::PARAM_VELOCITY,
                value: velocity_to_send as f32,
            });

            // Update Aftertouch parameter (param index 3) - raw 0-127 value
            self.send_command(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index: MidiNote::PARAM_AFTERTOUCH,
                value: self.midi_aftertouch as f32,
            });

            // Also update the cached params so sync_parameters doesn't overwrite
            self.cached_params.insert((engine_node_id, MidiNote::PARAM_NOTE), note_to_send as f32);
            self.cached_params.insert((engine_node_id, MidiNote::PARAM_GATE), gate_value);
            self.cached_params.insert((engine_node_id, MidiNote::PARAM_VELOCITY), velocity_to_send as f32);
            self.cached_params.insert((engine_node_id, MidiNote::PARAM_AFTERTOUCH), self.midi_aftertouch as f32);
        }
    }

    /// Check if MIDI gate needs to be released after minimum duration.
    fn update_midi_gate_timing(&mut self) {
        if self.midi_gate_held_high && self.midi_held_notes.is_empty() {
            // Note was released, check if we should release the gate now
            if let Some(gate_time) = self.midi_last_gate_on {
                if gate_time.elapsed().as_millis() as u64 >= Self::MIN_MIDI_GATE_DURATION_MS {
                    // Time to release
                    self.sync_midi_note_modules();
                }
            }
        }
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

            if ui.button("📄 New").on_hover_text("Clear patch").clicked() {
                actions.new_patch = true;
            }

            if ui.button("📂 Open").on_hover_text("Ctrl+O").clicked() {
                actions.load_patch = true;
            }

            if ui.button("💾 Save").on_hover_text("Ctrl+S").clicked() {
                actions.save_patch = true;
            }

            if ui.button("💾 Save As").on_hover_text("Save to new file").clicked() {
                actions.save_as_patch = true;
            }

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
            // Use lossy send - if buffer is full, command is dropped
            // This is acceptable for rapid updates like parameter changes
            handle.send_command_lossy(cmd);
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
                    crate::engine::EngineEvent::OutputValue { node_id, output_index, value } => {
                        // Store the output value for LED indicators
                        self.user_state.set_output_value(node_id, output_index, value);
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
                    // Other events are not currently handled by the app
                    // (OutputLevel, Started, Stopped, Error)
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
        // Track if we clicked in the editor area
        let mut cursor_in_editor = false;
        // Store editor rect for coordinate conversion
        let mut editor_rect = egui::Rect::NOTHING;

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                // Store editor rect for coordinate conversion
                editor_rect = ui.available_rect_before_wrap();

                // Reset widget context menu flag before drawing
                self.user_state.widget_context_menu_open = false;

                // Update zoom for widget scaling
                self.user_state.zoom = self.graph_state.pan_zoom.zoom;

                // Draw the node graph editor
                let graph_response = self.graph_state.draw_graph_editor(
                    ui,
                    AllNodeTemplates,
                    &mut self.user_state,
                    Vec::default(),
                );

                cursor_in_editor = graph_response.cursor_in_editor;

                // Disable the built-in node finder - we use our own context menu
                self.graph_state.node_finder = None;

                // Process graph responses
                for response in graph_response.node_responses {
                    match response {
                        NodeResponse::CreatedNode(node_id) => {
                            // Allocate engine node ID for the new node
                            let engine_node_id = self.user_state.allocate_engine_node_id(node_id);

                            // Get the module ID from the node's user data
                            if let Some(node) = self.graph_state.graph.nodes.get(node_id) {
                                let module_id = node.user_data.module_id;
                                commands_to_send.push(EngineCommand::AddModule {
                                    node_id: engine_node_id,
                                    module_id,
                                });

                                // Set up output monitoring for any LED indicators
                                for led_indicator in &node.user_data.led_indicators {
                                    commands_to_send.push(EngineCommand::MonitorOutput {
                                        node_id: engine_node_id,
                                        output_index: led_indicator.output_index,
                                    });
                                }

                                // Set up output monitoring for additional monitored outputs (e.g., phase)
                                for &output_index in &node.user_data.monitored_outputs {
                                    commands_to_send.push(EngineCommand::MonitorOutput {
                                        node_id: engine_node_id,
                                        output_index,
                                    });
                                }
                            }
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
                                // Always send a disconnect command first to clear any existing connection
                                // The graph library auto-disconnects old connections visually when a new
                                // connection is made to an input, but doesn't emit a DisconnectEvent.
                                // The engine's disconnect gracefully handles the case where nothing is connected.
                                if let Some(disconnect_cmd) = self.build_disconnect_command(input) {
                                    commands_to_send.push(disconnect_cmd);
                                }

                                // Connection is valid - send to engine
                                if let Some(cmd) = self.build_connect_command(output, input) {
                                    commands_to_send.push(cmd);
                                    // If this input is an exposed param, start monitoring it
                                    if let Some(monitor_cmd) = self.build_monitor_input_command(input) {
                                        commands_to_send.push(monitor_cmd);
                                    }
                                    // Monitor the output for cable animation signal feedback
                                    if let Some(monitor_cmd) = self.build_monitor_output_command(output) {
                                        commands_to_send.push(monitor_cmd);
                                    }
                                }
                            }
                        }
                        NodeResponse::DisconnectEvent { output, input } => {
                            // Send disconnect command to engine
                            if let Some(cmd) = self.build_disconnect_command(input) {
                                commands_to_send.push(cmd);
                                // Stop monitoring this input
                                if let Some(unmonitor_cmd) = self.build_unmonitor_input_command(input) {
                                    commands_to_send.push(unmonitor_cmd);
                                }
                            }
                            // Check if output has any remaining connections
                            // If not, stop monitoring it for cable animation
                            let has_other_connections = self.graph_state.graph.iter_connections()
                                .any(|(_, o)| o == output);
                            if !has_other_connections {
                                if let Some(unmonitor_cmd) = self.build_unmonitor_output_command(output) {
                                    commands_to_send.push(unmonitor_cmd);
                                }
                            }
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
                if let Some(click_pos) = ctx.input(|i| i.pointer.interact_pos()) {
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
                                    if ui.button(label.as_ref()).clicked() {
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
                // Convert screen position to graph coordinates
                let pan = self.graph_state.pan_zoom.pan;
                let zoom = self.graph_state.pan_zoom.zoom;
                let graph_pos = (menu_pos - editor_rect.min.to_vec2() - pan) / zoom;

                // Add the node to the graph
                let node_id = self.graph_state.graph.add_node(
                    template.node_graph_label(&mut self.user_state),
                    template.user_data(&mut self.user_state),
                    |graph, node_id| template.build_node(graph, &mut self.user_state, node_id),
                );

                // Set the node position and add to node_order
                self.graph_state.node_positions.insert(node_id, graph_pos);
                self.graph_state.node_order.push(node_id);

                // Allocate engine node ID and send command
                let engine_node_id = self.user_state.allocate_engine_node_id(node_id);
                commands_to_send.push(EngineCommand::AddModule {
                    node_id: engine_node_id,
                    module_id: template.module_id(),
                });

                // Set up output monitoring for any LED indicators and monitored outputs
                if let Some(node) = self.graph_state.graph.nodes.get(node_id) {
                    for led_indicator in &node.user_data.led_indicators {
                        commands_to_send.push(EngineCommand::MonitorOutput {
                            node_id: engine_node_id,
                            output_index: led_indicator.output_index,
                        });
                    }
                    for &output_index in &node.user_data.monitored_outputs {
                        commands_to_send.push(EngineCommand::MonitorOutput {
                            node_id: engine_node_id,
                            output_index,
                        });
                    }
                }

                close_menu = true;
            }

            if close_menu {
                self.user_state.context_menu_pos = None;
                self.user_state.context_menu_open_category = None;
                self.user_state.context_menu_hover_intent = None;
            }
        }

        // Send collected commands
        for cmd in commands_to_send {
            self.send_command(cmd);
        }

        // Remove invalid connections outside the UI closure
        for (output, input) in invalid_connections {
            self.graph_state.graph.remove_connection(input, output);
        }
    }

    /// Build a Connect command from graph port IDs.
    fn build_connect_command(
        &self,
        output: egui_node_graph2::OutputId,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        let output_data = self.graph_state.graph.get_output(output);
        let input_data = self.graph_state.graph.get_input(input);

        let from_node = self.user_state.get_engine_node_id(output_data.node)?;
        let to_node = self.user_state.get_engine_node_id(input_data.node)?;

        // Get port indices
        // For output ports, we count only output ports up to this one
        let from_port = self.get_output_port_index(output_data.node, output)?;

        // For input ports, we count only input ports up to this one
        let to_port = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::Connect {
            from_node,
            from_port,
            to_node,
            to_port,
        })
    }

    /// Build a Disconnect command from a graph input port ID.
    fn build_disconnect_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let port = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::Disconnect {
            node_id,
            port,
            is_input: true,
        })
    }

    /// Build a MonitorInput command if this input is an exposed parameter.
    /// Exposed parameters have both an input port and a knob at the bottom.
    fn build_monitor_input_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node = self.graph_state.graph.nodes.get(input_data.node)?;

        // Get the input name
        let input_name = node.inputs.iter()
            .find(|(_, id)| *id == input)
            .map(|(name, _)| name)?;

        // Check if this input name corresponds to an exposed knob parameter
        let is_exposed_param = node.user_data.knob_params.iter()
            .any(|kp| kp.param_name == *input_name && kp.has_input_port());

        if !is_exposed_param {
            return None;
        }

        // Get engine node ID and input port index
        let engine_node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let input_index = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::MonitorInput {
            node_id: engine_node_id,
            input_index,
        })
    }

    /// Build an UnmonitorInput command for a given input.
    fn build_unmonitor_input_command(
        &self,
        input: egui_node_graph2::InputId,
    ) -> Option<EngineCommand> {
        // Use get() to safely check if the input exists (avoid panic on stale IDs)
        let input_data = self.graph_state.graph.inputs.get(input)?;
        let node = self.graph_state.graph.nodes.get(input_data.node)?;

        // Get the input name
        let input_name = node.inputs.iter()
            .find(|(_, id)| *id == input)
            .map(|(name, _)| name)?;

        // Check if this input name corresponds to an exposed knob parameter
        let is_exposed_param = node.user_data.knob_params.iter()
            .any(|kp| kp.param_name == *input_name && kp.has_input_port());

        if !is_exposed_param {
            return None;
        }

        // Get engine node ID and input port index
        let engine_node_id = self.user_state.get_engine_node_id(input_data.node)?;
        let input_index = self.get_input_port_index(input_data.node, input)?;

        Some(EngineCommand::UnmonitorInput {
            node_id: engine_node_id,
            input_index,
        })
    }

    /// Build a MonitorOutput command for cable animation.
    /// This enables signal-level feedback for the cable connecting from this output.
    fn build_monitor_output_command(
        &self,
        output: egui_node_graph2::OutputId,
    ) -> Option<EngineCommand> {
        let output_data = self.graph_state.graph.outputs.get(output)?;
        let node = self.graph_state.graph.nodes.get(output_data.node)?;

        // Get engine node ID
        let engine_node_id = self.user_state.get_engine_node_id(output_data.node)?;

        // Find the output index (position in node.outputs)
        let output_index = node.outputs
            .iter()
            .position(|(_, id)| *id == output)?;

        Some(EngineCommand::MonitorOutput {
            node_id: engine_node_id,
            output_index,
        })
    }

    /// Build an UnmonitorOutput command for a given output.
    fn build_unmonitor_output_command(
        &self,
        output: egui_node_graph2::OutputId,
    ) -> Option<EngineCommand> {
        let output_data = self.graph_state.graph.outputs.get(output)?;
        let node = self.graph_state.graph.nodes.get(output_data.node)?;

        // Get engine node ID
        let engine_node_id = self.user_state.get_engine_node_id(output_data.node)?;

        // Find the output index (position in node.outputs)
        let output_index = node.outputs
            .iter()
            .position(|(_, id)| *id == output)?;

        Some(EngineCommand::UnmonitorOutput {
            node_id: engine_node_id,
            output_index,
        })
    }

    /// Get the DspModule port index for a given egui output ID.
    ///
    /// In egui_node_graph2, outputs are numbered separately from inputs.
    /// In DspModule, all ports are in a single array with inputs first.
    /// So we need to offset the egui output index by the number of input ports.
    fn get_output_port_index(
        &self,
        node_id: egui_node_graph2::NodeId,
        output_id: egui_node_graph2::OutputId,
    ) -> Option<usize> {
        let node = self.graph_state.graph.nodes.get(node_id)?;

        // Count the number of connectable inputs (ConnectionOnly or ConnectionOrConstant)
        // These map to DspModule input ports
        let num_input_ports = node.inputs
            .iter()
            .filter(|(_, id)| {
                let input = self.graph_state.graph.get_input(*id);
                matches!(
                    input.kind,
                    egui_node_graph2::InputParamKind::ConnectionOnly
                        | egui_node_graph2::InputParamKind::ConnectionOrConstant
                )
            })
            .count();

        // Find the egui output index
        let egui_output_idx = node.outputs
            .iter()
            .position(|(_, id)| *id == output_id)?;

        // DspModule port index = num_input_ports + egui_output_index
        Some(num_input_ports + egui_output_idx)
    }

    /// Get the DspModule port index for a given egui input ID.
    ///
    /// Both ConnectionOnly and ConnectionOrConstant inputs map to DspModule input ports.
    /// ConstantOnly inputs do NOT have ports (they are parameter-only).
    fn get_input_port_index(
        &self,
        node_id: egui_node_graph2::NodeId,
        input_id: egui_node_graph2::InputId,
    ) -> Option<usize> {
        let node = self.graph_state.graph.nodes.get(node_id)?;

        // Count connectable inputs (ConnectionOnly or ConnectionOrConstant) up to target
        let mut port_index = 0;
        for (_, id) in &node.inputs {
            let input = self.graph_state.graph.get_input(*id);

            // Check if this input can accept connections
            let is_connectable = matches!(
                input.kind,
                egui_node_graph2::InputParamKind::ConnectionOnly
                    | egui_node_graph2::InputParamKind::ConnectionOrConstant
            );

            if *id == input_id {
                // Return port index if this input can accept connections
                if is_connectable {
                    return Some(port_index);
                } else {
                    // ConstantOnly inputs don't map to DspModule ports
                    return None;
                }
            }

            // Count connectable inputs as ports
            if is_connectable {
                port_index += 1;
            }
        }

        None
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

            // Check if this is a keyboard or MIDI note module - we handle Note/Gate params separately
            let is_keyboard = node.user_data.module_id == "input.keyboard";
            let is_midi_note = node.user_data.module_id == "input.midi_note";

            // Track which param index we're at (only count ConstantOnly params)
            let mut param_index = 0;

            // Iterate through inputs to find parameters
            for (_param_name, input_id) in &node.inputs {
                let input = self.graph_state.graph.get_input(*input_id);

                // Only process ConstantOnly or ConnectionOrConstant params
                use egui_node_graph2::InputParamKind;
                match input.kind {
                    InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                        // Skip Note (0) and Gate (1) params for keyboard modules
                        // These are controlled by keyboard events, not the graph UI
                        if is_keyboard && param_index < 2 {
                            param_index += 1;
                            continue;
                        }

                        // Skip Note (0), Gate (1), Velocity (2), and Aftertouch (3) params for MIDI Note modules
                        // These are controlled by MIDI events, not the graph UI
                        if is_midi_note && param_index < 4 {
                            param_index += 1;
                            continue;
                        }

                        // Get the actual value (not normalized) for the audio engine
                        // This ensures frequency values are in Hz, time values in seconds, etc.
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
        let mut patch = Patch::new(name);

        // Collect nodes
        for (node_id, node) in self.graph_state.graph.nodes.iter() {
            // Get engine node ID for this graph node
            let Some(engine_node_id) = self.user_state.get_engine_node_id(node_id) else {
                continue;
            };

            // Get node position, normalized to zoom=1.0 coordinates for persistence.
            // The library's update_node_positions_after_zoom modifies positions when zooming,
            // so we need to reverse that transformation to get zoom-independent positions.
            // On load, we reset to zoom=1.0 and pan=0, so positions saved this way will match.
            let position = self.graph_state.node_positions
                .get(node_id)
                .map(|pos| {
                    let zoom = self.graph_state.pan_zoom.zoom;
                    let pan = self.graph_state.pan_zoom.pan;
                    let clip_rect = self.graph_state.pan_zoom.clip_rect;

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
                .unwrap_or((0.0, 0.0));

            let mut node_data = NodeData::new(
                engine_node_id,
                node.user_data.module_id,
                position,
            );

            // Collect parameter values
            for (_name, input_id) in &node.inputs {
                let input = self.graph_state.graph.get_input(*input_id);

                // Only save parameter values (not connection-only ports)
                match input.kind {
                    InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                        let param_value = match &input.value {
                            SynthValueType::Scalar { value, .. } => ParameterValue::Scalar(*value),
                            SynthValueType::Frequency { value, .. } => ParameterValue::Frequency(*value),
                            SynthValueType::LinearHz { value, .. } => ParameterValue::LinearHz(*value),
                            SynthValueType::Time { value, .. } => ParameterValue::Time(*value),
                            SynthValueType::LinearRange { value, .. } => ParameterValue::LinearRange(*value),
                            SynthValueType::Toggle { value, .. } => ParameterValue::Toggle(*value),
                            SynthValueType::Select { value, .. } => ParameterValue::Select(*value),
                        };
                        node_data.parameters.push(param_value);
                    }
                    InputParamKind::ConnectionOnly => {
                        // Skip connection-only inputs
                    }
                }
            }

            patch.nodes.push(node_data);
        }

        // Collect connections
        for (input_id, output_id) in self.graph_state.graph.iter_connections() {
            let input = self.graph_state.graph.get_input(input_id);
            let output = self.graph_state.graph.get_output(output_id);

            // Get node data to find port names
            let from_node = self.graph_state.graph.nodes.get(output.node);
            let to_node = self.graph_state.graph.nodes.get(input.node);

            if let (Some(from_node), Some(to_node)) = (from_node, to_node) {
                // Find output port name
                let from_port = from_node.outputs
                    .iter()
                    .find(|(_, id)| *id == output_id)
                    .map(|(name, _)| name.clone());

                // Find input port name
                let to_port = to_node.inputs
                    .iter()
                    .find(|(_, id)| *id == input_id)
                    .map(|(name, _)| name.clone());

                // Get engine node IDs
                let from_engine_id = self.user_state.get_engine_node_id(output.node);
                let to_engine_id = self.user_state.get_engine_node_id(input.node);

                if let (Some(from_port), Some(to_port), Some(from_id), Some(to_id)) =
                    (from_port, to_port, from_engine_id, to_engine_id)
                {
                    patch.connections.push(ConnectionData::new(from_id, from_port, to_id, to_port));
                }
            }
        }

        // Copy MIDI mappings to the patch
        patch.midi_mappings = self.midi_mappings.clone();

        patch
    }

    /// Load a patch, replacing the current graph.
    fn load_patch(&mut self, patch: &Patch) -> Result<(), PatchError> {
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

        // Map from patch node IDs to graph node IDs
        let mut id_map: HashMap<u64, egui_node_graph2::NodeId> = HashMap::new();

        // Create nodes
        for node_data in &patch.nodes {
            // Find the template for this module ID
            let template = self.find_template_for_module(&node_data.module_id)
                .ok_or_else(|| PatchError::UnknownModule(node_data.module_id.clone()))?;

            // Create the node
            let graph_node_id = self.graph_state.graph.add_node(
                template.node_graph_label(&mut self.user_state),
                template.user_data(&mut self.user_state),
                |graph, node_id| template.build_node(graph, &mut self.user_state, node_id),
            );

            // Set node position
            let pos = egui::pos2(node_data.position.0, node_data.position.1);
            self.graph_state.node_positions.insert(graph_node_id, pos);
            self.graph_state.node_order.push(graph_node_id);

            // Allocate engine node ID (use the patch ID to maintain consistency)
            // Note: We use our own ID allocation to keep engine and graph in sync
            let engine_node_id = self.user_state.allocate_engine_node_id(graph_node_id);

            // Send command to create the module in the audio engine
            self.send_command(EngineCommand::AddModule {
                node_id: engine_node_id,
                module_id: template.module_id(),
            });

            // Set up output monitoring for LED indicators and monitored outputs
            // Collect indices first to avoid borrow issues
            let (led_output_indices, monitored_output_indices): (Vec<usize>, Vec<usize>) = self.graph_state.graph.nodes
                .get(graph_node_id)
                .map(|node| (
                    node.user_data.led_indicators.iter().map(|led| led.output_index).collect(),
                    node.user_data.monitored_outputs.clone(),
                ))
                .unwrap_or_default();

            for output_index in led_output_indices {
                self.send_command(EngineCommand::MonitorOutput {
                    node_id: engine_node_id,
                    output_index,
                });
            }

            for output_index in monitored_output_indices {
                self.send_command(EngineCommand::MonitorOutput {
                    node_id: engine_node_id,
                    output_index,
                });
            }

            // Map patch ID to graph ID for connection restoration
            id_map.insert(node_data.id, graph_node_id);

            // Restore parameter values
            if let Some(node) = self.graph_state.graph.nodes.get_mut(graph_node_id) {
                let mut param_idx = 0;
                for (_name, input_id) in &node.inputs {
                    if let Some(input) = self.graph_state.graph.inputs.get_mut(*input_id) {
                        match input.kind {
                            InputParamKind::ConstantOnly | InputParamKind::ConnectionOrConstant => {
                                if let Some(saved_value) = node_data.parameters.get(param_idx) {
                                    input.value.set_actual_value(saved_value.as_f32());
                                }
                                param_idx += 1;
                            }
                            InputParamKind::ConnectionOnly => {
                                // Skip
                            }
                        }
                    }
                }
            }
        }

        // Restore connections
        for conn in &patch.connections {
            // Find graph node IDs from patch IDs
            let from_graph_id = id_map.get(&conn.from_node);
            let to_graph_id = id_map.get(&conn.to_node);

            if let (Some(&from_graph_id), Some(&to_graph_id)) = (from_graph_id, to_graph_id) {
                // Find output port by name
                let output_id = self.graph_state.graph.nodes.get(from_graph_id)
                    .and_then(|node| {
                        node.outputs.iter()
                            .find(|(name, _)| *name == conn.from_port)
                            .map(|(_, id)| *id)
                    });

                // Find input port by name
                let input_id = self.graph_state.graph.nodes.get(to_graph_id)
                    .and_then(|node| {
                        node.inputs.iter()
                            .find(|(name, _)| *name == conn.to_port)
                            .map(|(_, id)| *id)
                    });

                if let (Some(output_id), Some(input_id)) = (output_id, input_id) {
                    // Add connection to graph (pos=0 adds at beginning, order doesn't matter for audio)
                    self.graph_state.graph.add_connection(output_id, input_id, 0);

                    // Send connection command to engine
                    if let Some(cmd) = self.build_connect_command(output_id, input_id) {
                        self.send_command(cmd);
                    }

                    // Set up input monitoring if this is an exposed parameter
                    if let Some(monitor_cmd) = self.build_monitor_input_command(input_id) {
                        self.send_command(monitor_cmd);
                    }

                    // Set up output monitoring for cable animation
                    if let Some(monitor_cmd) = self.build_monitor_output_command(output_id) {
                        self.send_command(monitor_cmd);
                    }
                }
            }
        }

        // Load MIDI mappings
        self.midi_mappings = patch.midi_mappings.clone();
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

        Ok(())
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
        self.current_patch_path = None;
        self.status_message = Some("New patch created".to_string());
    }

    /// Find the template for a given module ID.
    fn find_template_for_module(&self, module_id: &str) -> Option<SynthNodeTemplate> {
        use egui_node_graph2::NodeTemplateIter;
        AllNodeTemplates.all_kinds()
            .into_iter()
            .find(|t| t.module_id() == module_id)
    }

    /// Show a save file dialog and save the current patch.
    fn show_save_dialog(&mut self) {
        let default_name = self.current_patch_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
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
                    self.status_message = Some(format!("Saved: {}", path.display()));
                }
                Err(e) => {
                    self.status_message = Some(format!("Save failed: {}", e));
                }
            }
        }
    }

    /// Show a load file dialog and load the selected patch.
    fn show_load_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Synth Patch", &["json"])
            .pick_file()
        {
            match load_from_file(&path) {
                Ok(patch) => {
                    match self.load_patch(&patch) {
                        Ok(()) => {
                            self.current_patch_path = Some(path.clone());
                            self.status_message = Some(format!("Loaded: {}", patch.name));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Load failed: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.status_message = Some(format!("Load failed: {}", e));
                }
            }
        }
    }

    /// Quick save to the current path, or show save dialog if no path.
    fn quick_save(&mut self) {
        if let Some(path) = self.current_patch_path.clone() {
            let name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled");

            let patch = self.create_patch(name);
            match save_to_file(&patch, &path) {
                Ok(()) => {
                    self.status_message = Some(format!("Saved: {}", path.display()));
                }
                Err(e) => {
                    self.status_message = Some(format!("Save failed: {}", e));
                }
            }
        } else {
            self.show_save_dialog();
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
                // Show current patch name if any
                if let Some(ref path) = self.current_patch_path {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        ui.label(RichText::new(name)
                            .color(theme::text::SECONDARY)
                            .small());
                        ui.label(RichText::new("|")
                            .color(theme::text::DISABLED)
                            .small());
                    }
                }
                ui.label(RichText::new("Modular Synth v0.1")
                    .color(theme::text::DISABLED)
                    .small());
            });
        });
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

        // Process raw keyboard events - these haven't been consumed yet
        let mut keys_changed = false;

        ctx.input(|i| {
            // Check raw events for key presses/releases
            for event in &i.raw.events {
                if let egui::Event::Key { key, pressed, repeat, .. } = event {
                    // Skip key repeat events
                    if *repeat {
                        continue;
                    }

                    if let Some(relative_note) = key_to_note(*key) {
                        if *pressed {
                            // Add key if not already in list
                            if !self.pressed_keys.iter().any(|(_, k)| k == key) {
                                self.pressed_keys.push((relative_note, *key));
                                keys_changed = true;
                            }
                        } else {
                            // Remove key from list
                            if let Some(pos) = self.pressed_keys.iter().position(|(_, k)| k == key) {
                                self.pressed_keys.remove(pos);
                                keys_changed = true;
                            }
                        }
                    }
                }
            }
        });

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

/// Actions collected from the toolbar for deferred execution
#[derive(Default)]
struct ToolbarActions {
    toggle_playing: bool,
    select_device: Option<usize>,
    refresh_devices: bool,
    save_patch: bool,
    save_as_patch: bool,
    load_patch: bool,
    new_patch: bool,
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

        // Process events from the audio engine
        self.process_engine_events();

        // Clear status message after it's been shown (user will see it on first frame)
        // We clear it on the next frame after it was set
        let had_status_message = self.status_message.is_some();

        // Request continuous repaints when playing (for LED indicators and other visualizations)
        // Also repaint continuously when there are keyboard or MIDI Note modules to catch all events
        if self.is_playing || self.has_keyboard_modules() || self.has_midi_note_modules() {
            ctx.request_repaint();
        }

        // Handle keyboard shortcuts
        let mut keyboard_save = false;
        let mut keyboard_load = false;

        ctx.input(|i| {
            // Ctrl+S: Save
            if i.modifiers.ctrl && i.key_pressed(egui::Key::S) {
                keyboard_save = true;
            }
            // Ctrl+O: Open/Load
            if i.modifiers.ctrl && i.key_pressed(egui::Key::O) {
                keyboard_load = true;
            }
        });

        // Handle musical keyboard input (QWERTY to notes)
        self.handle_keyboard_events(ctx);

        // Update gate timing (for minimum gate duration)
        self.update_gate_timing();
        self.update_midi_gate_timing();

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
        if toolbar_actions.save_as_patch {
            self.show_save_dialog();
        }
        if toolbar_actions.load_patch || keyboard_load {
            self.show_load_dialog();
        }
        if toolbar_actions.new_patch {
            self.new_patch();
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

        // Clear status message after showing it for one frame
        // This gives user time to read it but doesn't persist forever
        if had_status_message {
            // Request one more repaint to clear the message
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
        }
    }
}
