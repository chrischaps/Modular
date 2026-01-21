//! Widgets module
//!
//! Custom UI controls for the synthesizer interface.
//! Includes knobs, faders, waveform displays, and spectrum displays.

pub mod knob;
pub mod fader;
pub mod waveform_display;
pub mod spectrum_display;

// Future submodules:
// pub mod vu_meter;

// Re-export commonly used items
pub use knob::{knob, mini_knob, KnobConfig, ParamFormat};
pub use fader::{fader, horizontal_fader, FaderConfig};
pub use waveform_display::{
    waveform_display, WaveformConfig, WaveformMode, WaveformBuffer,
    WaveformType, GridStyle, generate_waveform_cycle,
};
pub use spectrum_display::{
    spectrum_display, SpectrumConfig, SpectrumStyle, FrequencyPoint,
    FilterResponseType, generate_filter_response,
};
