//! Persistence module
//!
//! Patch save/load functionality using serde and JSON.

pub mod compile;
pub mod patch;

pub use compile::{compile_patch, CompiledPatch};

pub use patch::{
    ConnectionData, MidiMapping, NodeData, ParameterValue, Patch, PatchError,
    load_from_file, save_to_file, PATCH_VERSION,
};
