//! Graph state for the synthesizer.
//!
//! Contains the user state passed to egui_node_graph2 callbacks.

use egui::Pos2;
use egui_node_graph2::{GraphEditorState, NodeId};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use crate::engine::NodeId as EngineNodeId;
use crate::engine::midi_engine::MidiEvent;
use super::{SynthDataType, SynthNodeData, SynthValueType};
use super::templates::SynthNodeTemplate;

/// Duration to show validation messages before auto-clearing.
const VALIDATION_MESSAGE_DURATION_SECS: f32 = 3.0;

/// Maximum number of MIDI events to store for display.
const MAX_MIDI_EVENTS: usize = 10;

/// A MIDI event with display timestamp for the MIDI Monitor.
#[derive(Clone, Debug)]
pub struct DisplayMidiEvent {
    /// The MIDI event.
    pub event: MidiEvent,
    /// Relative timestamp (seconds since first event).
    pub timestamp: f32,
}

/// Oscilloscope waveform data for display.
#[derive(Clone, Debug, Default)]
pub struct ScopeData {
    /// Channel 1 waveform samples.
    pub channel1: Vec<f32>,
    /// Channel 2 waveform samples.
    pub channel2: Vec<f32>,
    /// Whether this capture was triggered (vs free-running).
    pub triggered: bool,
}

/// Info about a MIDI CC mapping for display in the UI.
#[derive(Clone, Debug)]
pub struct MidiMappingInfo {
    /// CC number that is mapped.
    pub cc_number: u8,
    /// MIDI channel (0 = omni).
    pub channel: u8,
}

/// User state for the graph editor.
///
/// This is passed to all graph callbacks and can store any
/// application-specific data needed during graph editing.
pub struct SynthGraphState {
    /// Currently selected node, if any.
    pub selected_node: Option<NodeId>,

    /// Mapping from graph NodeId to audio engine NodeId.
    /// This is used to sync graph changes with the audio engine.
    pub node_id_map: HashMap<NodeId, EngineNodeId>,

    /// Counter for generating unique engine node IDs.
    next_engine_node_id: EngineNodeId,

    /// Last validation error message for display in UI.
    validation_message: Option<String>,

    /// When the validation message was set (for auto-clear).
    validation_message_time: Option<Instant>,

    /// Position where context menu was opened (screen coords).
    /// None when menu is closed.
    pub context_menu_pos: Option<Pos2>,

    /// Currently open submenu category index in the context menu.
    /// Used for hover-delay submenu behavior.
    pub context_menu_open_category: Option<usize>,

    /// Category being hovered and when hover started.
    /// Used to implement hover delay before opening a new submenu.
    pub context_menu_hover_intent: Option<(usize, Instant)>,

    /// Current input values received from the audio engine for signal feedback.
    /// Key: (engine_node_id, input_port_index), Value: sampled signal value.
    /// These values animate the knobs when their inputs are connected.
    pub input_values: HashMap<(EngineNodeId, usize), f32>,

    /// Current output values received from the audio engine for LED indicators.
    /// Key: (engine_node_id, output_port_index), Value: sampled signal value.
    /// These values light up LED indicators on nodes.
    pub output_values: HashMap<(EngineNodeId, usize), f32>,

    /// Recent MIDI events for display in MIDI Monitor modules.
    pub midi_events: VecDeque<DisplayMidiEvent>,

    /// Timestamp of the first MIDI event received (for relative timestamps).
    midi_first_event_time: Option<Instant>,

    /// MIDI CC mappings for UI display.
    /// Key: (engine_node_id, param_index), Value: mapping info.
    pub midi_mappings: HashMap<(EngineNodeId, usize), MidiMappingInfo>,

    /// Whether MIDI Learn mode is active.
    pub midi_learn_active: bool,

    /// Target parameter for MIDI Learn (if active).
    /// Tuple of (engine_node_id, param_index).
    pub midi_learn_target: Option<(EngineNodeId, usize)>,

    /// Flag set when a widget context menu is shown this frame.
    /// Used to prevent the add-node menu from also appearing.
    pub widget_context_menu_open: bool,

    /// Oscilloscope waveform data received from the audio engine.
    /// Key: engine_node_id, Value: scope waveform data.
    pub scope_data: HashMap<EngineNodeId, ScopeData>,
}

impl Default for SynthGraphState {
    fn default() -> Self {
        Self {
            selected_node: None,
            node_id_map: HashMap::new(),
            next_engine_node_id: 0,
            validation_message: None,
            validation_message_time: None,
            context_menu_pos: None,
            context_menu_open_category: None,
            context_menu_hover_intent: None,
            input_values: HashMap::new(),
            output_values: HashMap::new(),
            midi_events: VecDeque::new(),
            midi_first_event_time: None,
            midi_mappings: HashMap::new(),
            midi_learn_active: false,
            midi_learn_target: None,
            widget_context_menu_open: false,
            scope_data: HashMap::new(),
        }
    }
}

impl SynthGraphState {
    /// Create a new graph state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new engine node ID and map it to a graph node.
    pub fn allocate_engine_node_id(&mut self, graph_node_id: NodeId) -> EngineNodeId {
        let id = self.next_engine_node_id;
        self.next_engine_node_id += 1;
        self.node_id_map.insert(graph_node_id, id);
        id
    }

    /// Get the engine node ID for a graph node.
    pub fn get_engine_node_id(&self, graph_node_id: NodeId) -> Option<EngineNodeId> {
        self.node_id_map.get(&graph_node_id).copied()
    }

    /// Remove a graph node from the mapping.
    pub fn remove_node(&mut self, graph_node_id: NodeId) -> Option<EngineNodeId> {
        self.node_id_map.remove(&graph_node_id)
    }

    /// Clear all mappings.
    pub fn clear(&mut self) {
        self.node_id_map.clear();
        self.selected_node = None;
        self.validation_message = None;
        self.validation_message_time = None;
        self.context_menu_pos = None;
        self.context_menu_open_category = None;
        self.context_menu_hover_intent = None;
        self.input_values.clear();
        self.output_values.clear();
        self.midi_events.clear();
        self.midi_first_event_time = None;
        self.midi_mappings.clear();
        self.midi_learn_active = false;
        self.midi_learn_target = None;
        self.widget_context_menu_open = false;
        self.scope_data.clear();
    }

    /// Get the MIDI mapping info for a parameter, if any.
    pub fn get_midi_mapping(&self, engine_node_id: EngineNodeId, param_index: usize) -> Option<&MidiMappingInfo> {
        self.midi_mappings.get(&(engine_node_id, param_index))
    }

    /// Set or update a MIDI mapping for a parameter.
    pub fn set_midi_mapping(&mut self, engine_node_id: EngineNodeId, param_index: usize, cc_number: u8, channel: u8) {
        self.midi_mappings.insert(
            (engine_node_id, param_index),
            MidiMappingInfo { cc_number, channel },
        );
    }

    /// Remove a MIDI mapping for a parameter.
    pub fn remove_midi_mapping(&mut self, engine_node_id: EngineNodeId, param_index: usize) {
        self.midi_mappings.remove(&(engine_node_id, param_index));
    }

    /// Check if a parameter is currently the MIDI Learn target.
    pub fn is_midi_learn_target(&self, engine_node_id: EngineNodeId, param_index: usize) -> bool {
        self.midi_learn_target == Some((engine_node_id, param_index))
    }

    /// Set a validation error message to display.
    pub fn set_validation_error(&mut self, message: impl Into<String>) {
        self.validation_message = Some(message.into());
        self.validation_message_time = Some(Instant::now());
    }

    /// Clear the validation message.
    pub fn clear_validation_message(&mut self) {
        self.validation_message = None;
        self.validation_message_time = None;
    }

    /// Get the current validation message if it hasn't expired.
    pub fn validation_message(&mut self) -> Option<&str> {
        // Auto-clear after duration
        if let Some(time) = self.validation_message_time {
            if time.elapsed().as_secs_f32() > VALIDATION_MESSAGE_DURATION_SECS {
                self.clear_validation_message();
            }
        }
        self.validation_message.as_deref()
    }

    /// Update an input value from the audio engine feedback.
    pub fn set_input_value(&mut self, engine_node_id: EngineNodeId, input_index: usize, value: f32) {
        self.input_values.insert((engine_node_id, input_index), value);
    }

    /// Get the current input value for a node's input port.
    /// Returns None if no value has been received yet.
    pub fn get_input_value(&self, engine_node_id: EngineNodeId, input_index: usize) -> Option<f32> {
        self.input_values.get(&(engine_node_id, input_index)).copied()
    }

    /// Clear input values for a specific node (e.g., when node is deleted).
    pub fn clear_input_values_for_node(&mut self, engine_node_id: EngineNodeId) {
        self.input_values.retain(|(node_id, _), _| *node_id != engine_node_id);
    }

    /// Update an output value from the audio engine feedback.
    pub fn set_output_value(&mut self, engine_node_id: EngineNodeId, output_index: usize, value: f32) {
        self.output_values.insert((engine_node_id, output_index), value);
    }

    /// Get the current output value for a node's output port.
    /// Returns None if no value has been received yet.
    pub fn get_output_value(&self, engine_node_id: EngineNodeId, output_index: usize) -> Option<f32> {
        self.output_values.get(&(engine_node_id, output_index)).copied()
    }

    /// Clear output values for a specific node (e.g., when node is deleted).
    pub fn clear_output_values_for_node(&mut self, engine_node_id: EngineNodeId) {
        self.output_values.retain(|(node_id, _), _| *node_id != engine_node_id);
    }

    /// Add a MIDI event for display in MIDI Monitor modules.
    ///
    /// Events are stored with a relative timestamp from the first event.
    /// Only the most recent MAX_MIDI_EVENTS are kept.
    pub fn push_midi_event(&mut self, event: MidiEvent) {
        let now = Instant::now();
        let timestamp = if let Some(first_time) = self.midi_first_event_time {
            first_time.elapsed().as_secs_f32()
        } else {
            self.midi_first_event_time = Some(now);
            0.0
        };

        self.midi_events.push_back(DisplayMidiEvent { event, timestamp });

        // Keep only the most recent events
        while self.midi_events.len() > MAX_MIDI_EVENTS {
            self.midi_events.pop_front();
        }
    }

    /// Get the recent MIDI events for display.
    pub fn midi_events(&self) -> &VecDeque<DisplayMidiEvent> {
        &self.midi_events
    }

    /// Clear all stored MIDI events.
    pub fn clear_midi_events(&mut self) {
        self.midi_events.clear();
        self.midi_first_event_time = None;
    }

    /// Update oscilloscope waveform data from the audio engine.
    pub fn set_scope_data(
        &mut self,
        engine_node_id: EngineNodeId,
        channel1: Vec<f32>,
        channel2: Vec<f32>,
        triggered: bool,
    ) {
        self.scope_data.insert(
            engine_node_id,
            ScopeData {
                channel1,
                channel2,
                triggered,
            },
        );
    }

    /// Get the current scope data for an oscilloscope node.
    pub fn get_scope_data(&self, engine_node_id: EngineNodeId) -> Option<&ScopeData> {
        self.scope_data.get(&engine_node_id)
    }

    /// Clear scope data for a specific node (e.g., when node is deleted).
    pub fn clear_scope_data_for_node(&mut self, engine_node_id: EngineNodeId) {
        self.scope_data.remove(&engine_node_id);
    }
}

/// Type alias for the complete graph editor state with our custom types.
/// Order: NodeData, DataType, ValueType, NodeTemplate, UserState
pub type SynthGraphEditorState = GraphEditorState<SynthNodeData, SynthDataType, SynthValueType, SynthNodeTemplate, SynthGraphState>;

/// Create a new graph editor state with default configuration.
pub fn create_editor_state() -> SynthGraphEditorState {
    GraphEditorState::new(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_state_default() {
        let state = SynthGraphState::default();
        assert!(state.selected_node.is_none());
        assert!(state.node_id_map.is_empty());
    }

    #[test]
    fn test_allocate_engine_node_id() {
        let mut state = SynthGraphState::new();

        // Create a fake graph node id for testing
        let graph_node_id: NodeId = unsafe { std::mem::transmute(1u64) };

        let engine_id = state.allocate_engine_node_id(graph_node_id);
        assert_eq!(engine_id, 0);

        let graph_node_id2: NodeId = unsafe { std::mem::transmute(2u64) };
        let engine_id2 = state.allocate_engine_node_id(graph_node_id2);
        assert_eq!(engine_id2, 1);
    }

    #[test]
    fn test_get_engine_node_id() {
        let mut state = SynthGraphState::new();
        let graph_node_id: NodeId = unsafe { std::mem::transmute(1u64) };

        // Before allocation
        assert!(state.get_engine_node_id(graph_node_id).is_none());

        // After allocation
        let engine_id = state.allocate_engine_node_id(graph_node_id);
        assert_eq!(state.get_engine_node_id(graph_node_id), Some(engine_id));
    }

    #[test]
    fn test_remove_node() {
        let mut state = SynthGraphState::new();
        let graph_node_id: NodeId = unsafe { std::mem::transmute(1u64) };

        let engine_id = state.allocate_engine_node_id(graph_node_id);
        assert_eq!(state.remove_node(graph_node_id), Some(engine_id));
        assert!(state.get_engine_node_id(graph_node_id).is_none());
    }

    #[test]
    fn test_clear() {
        let mut state = SynthGraphState::new();
        let graph_node_id: NodeId = unsafe { std::mem::transmute(1u64) };
        state.allocate_engine_node_id(graph_node_id);
        state.selected_node = Some(graph_node_id);

        state.clear();

        assert!(state.selected_node.is_none());
        assert!(state.node_id_map.is_empty());
    }
}
