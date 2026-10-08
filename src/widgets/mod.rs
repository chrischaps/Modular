//! Widgets module
//!
//! Custom UI controls for the synthesizer interface.
//! Includes knobs, faders, waveform displays, spectrum displays, LED indicators, CPU meters,
//! and specialized displays for envelopes and other module types.

pub mod knob;
pub mod fader;
pub mod waveform_display;
pub mod spectrum_display;
pub mod led;
pub mod cpu_meter;
pub mod oscilloscope_display;
pub mod adsr_display;
pub mod piano;
pub mod level_meter;
pub mod noise_display;

// Re-export commonly used items
pub use knob::{knob, mini_knob, KnobConfig, KnobStyle, ParamFormat};
pub use fader::{fader, horizontal_fader, FaderConfig};
pub use waveform_display::{
    waveform_display, WaveformConfig, WaveformMode, WaveformBuffer,
    WaveformType, GridStyle, generate_waveform_cycle,
};
pub use spectrum_display::{
    spectrum_display, SpectrumConfig, SpectrumStyle, FrequencyPoint,
    FilterResponseType, generate_filter_response,
};
pub use led::{led, LedConfig};
pub use cpu_meter::{cpu_meter, CpuMeterConfig, cpu_load_color};
pub use oscilloscope_display::{oscilloscope_display, OscilloscopeConfig, TriggerMode};
pub use adsr_display::{adsr_display, AdsrConfig, AdsrParams, AdsrSegment, generate_adsr_curve, get_adsr_segment_boundaries};
pub use piano::{piano, piano_keys, PianoConfig, PianoData};
pub use level_meter::{column_meter, level_meter, LevelMeter, LevelMeterConfig, ModuleMeters, PeakBallistics};
pub use noise_display::{noise_display, NoiseDisplayConfig};
