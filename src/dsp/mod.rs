//! DSP module
//!
//! Core DSP traits and types.
//! Defines the DspModule trait, ports, parameters, and signal types.

pub mod signal;

// Re-export commonly used types
pub use signal::{MidiEvent, MidiMessage, SignalBuffer, SignalType};

// Future submodules:
// pub mod module_trait;
// pub mod port;
// pub mod parameter;
// pub mod registry;
