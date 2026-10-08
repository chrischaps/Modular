//! Graph module
//!
//! Node graph integration with egui_node_graph2.
//! Handles data types, node templates, connection validation, and custom rendering.

pub mod annotation_ui;
pub mod annotations;
pub mod catalog;
mod clock_display;
mod data_types;
pub mod hints;
pub mod input_display;
mod mixer_strips;
mod module_ui;
mod node_data;
pub mod port_mapping;
mod responses;
pub mod signal_history;
mod state;
mod templates;
mod validation;
mod value_types;

pub use data_types::SynthDataType;
pub use port_mapping::SynthGraph;
pub use node_data::{KnobParam, LedIndicator, NodeDisplay, SynthNodeData};
pub use responses::SynthResponse;
pub use state::{create_editor_state, DisplayMidiEvent, MidiMappingInfo, SynthGraphEditorState, SynthGraphState};
pub use templates::{AllNodeTemplates, SynthNodeTemplate};
pub use validation::{validate_connection, types_compatible, ConnectionError, ValidationResult};
pub use value_types::{NumberSpec, SynthValueType};

// Re-export useful types from egui_node_graph2
pub use egui_node_graph2::{NodeId, InputId, OutputId, AnyParameterId};
