//! Engine module
//!
//! Audio engine and processing graph.
//! Handles cpal integration, audio graph processing, and buffer management.

pub mod audio_engine;

// Future submodules:
// pub mod audio_graph;
// pub mod commands;
// pub mod buffer_pool;

pub use audio_engine::{AudioEngine, AudioError, DeviceInfo};
