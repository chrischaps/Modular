//! Persistence module
//!
//! Patch save/load functionality using serde and JSON.

pub mod patch;

pub use patch::{
    ConnectionData, NodeData, ParameterValue, Patch, PatchError,
    load_from_file, save_to_file, PATCH_VERSION,
};
