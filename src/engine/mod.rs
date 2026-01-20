//! Engine module
//!
//! Audio engine and processing graph.
//! Handles cpal integration, audio graph processing, and buffer management.

pub mod audio_engine;
pub mod audio_graph;
pub mod buffer_pool;
pub mod channels;
pub mod commands;

pub use audio_engine::{AudioEngine, AudioError, DeviceInfo};
pub use audio_graph::{AudioGraph, Connection};
pub use buffer_pool::{BufferPool, BufferSlot};
pub use channels::{
    EngineChannels, EngineHandle, UiHandle, DEFAULT_COMMAND_BUFFER_SIZE, DEFAULT_EVENT_BUFFER_SIZE,
};
pub use commands::{EngineCommand, EngineEvent, NodeId, PortIndex};
