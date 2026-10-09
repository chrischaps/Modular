//! Persistence module
//!
//! Patch save/load functionality using serde and JSON.

pub mod compile;
pub mod examples;
pub mod graph_io;
pub mod patch;

pub use compile::{compile_patch, CompiledPatch};
pub use examples::{Example, EXAMPLES};
pub use graph_io::{
    capture_level, capture_patch, merge_patch, renumber_groups, stage_patch, CapturedLevel, Merged, StagedNode, StagedPart,
    StagedPatch,
};

pub use patch::{
    ConnectionData, FrameData, GroupData, JackData, MidiMapping, NamedParameter, NodeData, NoteData, ParameterValue,
    Patch, PatchError,
    load_from_file, migrate_v2_to_v3, patch_from_json, patch_to_json, save_to_file, PATCH_VERSION,
};
