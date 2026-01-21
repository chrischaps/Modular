//! Graph module
//!
//! Node graph integration with egui_node_graph2.
//! Handles data types, node templates, connection validation, and custom rendering.

mod data_types;
mod node_data;
mod responses;
mod state;
mod templates;
mod validation;
mod value_types;

pub use data_types::SynthDataType;
pub use node_data::{KnobParam, LedIndicator, SynthNodeData};
pub use responses::SynthResponse;
pub use state::{create_editor_state, SynthGraphEditorState, SynthGraphState};
pub use templates::{AllNodeTemplates, SynthNodeTemplate};
pub use validation::{validate_connection, types_compatible, ConnectionError, ValidationResult};
pub use value_types::SynthValueType;

// Re-export useful types from egui_node_graph2
pub use egui_node_graph2::{NodeId, InputId, OutputId, AnyParameterId};
