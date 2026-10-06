//! Persistence module
//!
//! Patch save/load functionality using serde and JSON.

pub mod compile;
pub mod examples;
pub mod graph_io;
pub mod patch;

pub use compile::{compile_patch, CompiledPatch};
pub use examples::{Example, EXAMPLES};
pub use graph_io::{capture_patch, stage_patch, StagedNode, StagedPatch};

pub use patch::{
    ConnectionData, MidiMapping, NamedParameter, NodeData, ParameterValue, Patch, PatchError,
    load_from_file, migrate_v2_to_v3, patch_from_json, save_to_file, PATCH_VERSION,
};
