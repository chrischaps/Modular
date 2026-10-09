//! Application module
//!
//! Contains the main egui application, theme definitions, and UI state management.

#[cfg(feature = "asio")]
mod asio_badge;
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
#[cfg(target_arch = "wasm32")]
pub mod web;

/// Whether this is the browser build, where there are no files, threads
/// or MIDI devices: see [`web`].
pub const WEB: bool = cfg!(target_arch = "wasm32");

pub use synth_app::SynthApp;
