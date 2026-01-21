//! Graph state for the synthesizer.
//!
//! Contains the user state passed to egui_node_graph2 callbacks.

use egui::Pos2;
use egui_node_graph2::{GraphEditorState, NodeId};
use std::collections::HashMap;
use std::time::Instant;

use crate::engine::NodeId as EngineNodeId;
use super::{SynthDataType, SynthNodeData, SynthValueType};
use super::templates::SynthNodeTemplate;

/// Duration to show validation messages before auto-clearing.
const VALIDATION_MESSAGE_DURATION_SECS: f32 = 3.0;

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

    /// Current input values received from the audio engine for signal feedback.
    /// Key: (engine_node_id, input_port_index), Value: sampled signal value.
    /// These values animate the knobs when their inputs are connected.
    pub input_values: HashMap<(EngineNodeId, usize), f32>,

    /// Current output values received from the audio engine for LED indicators.
    /// Key: (engine_node_id, output_port_index), Value: sampled signal value.
    /// These values light up LED indicators on nodes.
    pub output_values: HashMap<(EngineNodeId, usize), f32>,
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
            input_values: HashMap::new(),
            output_values: HashMap::new(),
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
        self.input_values.clear();
        self.output_values.clear();
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
