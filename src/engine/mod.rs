//! Engine module
//!
//! Audio engine and processing graph.
//! Handles cpal integration, audio graph processing, and buffer management.

pub mod audio_engine;
pub mod channels;
pub mod commands;

// Future submodules:
// pub mod audio_graph;
// pub mod buffer_pool;

pub use audio_engine::{AudioEngine, AudioError, DeviceInfo};
pub use channels::{
    EngineChannels, EngineHandle, UiHandle, DEFAULT_COMMAND_BUFFER_SIZE, DEFAULT_EVENT_BUFFER_SIZE,
};
pub use commands::{EngineCommand, EngineEvent, NodeId, PortIndex};
