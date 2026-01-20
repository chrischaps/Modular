//! Graph state for the synthesizer.
//!
//! Contains the user state passed to egui_node_graph2 callbacks.

use egui_node_graph2::{GraphEditorState, NodeId};
use std::collections::HashMap;

use crate::engine::NodeId as EngineNodeId;
use super::{SynthDataType, SynthNodeData, SynthValueType};
use super::templates::SynthNodeTemplate;

/// User state for the graph editor.
///
/// This is passed to all graph callbacks and can store any
/// application-specific data needed during graph editing.
#[derive(Default)]
pub struct SynthGraphState {
    /// Currently selected node, if any.
    pub selected_node: Option<NodeId>,

    /// Mapping from graph NodeId to audio engine NodeId.
    /// This is used to sync graph changes with the audio engine.
    pub node_id_map: HashMap<NodeId, EngineNodeId>,

    /// Counter for generating unique engine node IDs.
    next_engine_node_id: EngineNodeId,
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
