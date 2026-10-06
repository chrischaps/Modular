//! Application module
//!
//! Contains the main egui application, theme definitions, and UI state management.

mod editing;
mod engine_sync;
mod palette;
mod session;
pub mod synth_app;
pub mod theme;
pub mod undo;

pub use synth_app::SynthApp;
