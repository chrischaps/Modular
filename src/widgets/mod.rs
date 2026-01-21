//! Widgets module
//!
//! Custom UI controls for the synthesizer interface.
//! Includes knobs, faders, waveform displays, and VU meters.

pub mod knob;
pub mod fader;

// Future submodules:
// pub mod waveform_display;
// pub mod vu_meter;

// Re-export commonly used items
pub use knob::{knob, mini_knob, KnobConfig, ParamFormat};
pub use fader::{fader, horizontal_fader, FaderConfig};
