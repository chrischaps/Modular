//! Application module
//!
//! Contains the main egui application, theme definitions, and UI state management.

pub mod capture;
mod editing;
pub mod engine_sync;
mod input_device;
pub mod library;
mod palette;
mod recording;
mod session;
pub mod synth_app;
pub mod theme;
pub mod undo;

pub use synth_app::SynthApp;
