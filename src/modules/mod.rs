//! Modules module
//!
//! Built-in synthesizer modules.
//! Includes oscillators, filters, envelopes, LFOs, utilities, and output modules.

pub mod attenuverter;
pub mod clock;
pub mod delay;
pub mod envelope;
pub mod filter;
pub mod keyboard;
pub mod lfo;
pub mod midi_monitor;
pub mod midi_note;
pub mod oscillator;
pub mod oscilloscope;
pub mod output;
pub mod sample_hold;
pub mod sequencer;
pub mod vca;

// Re-export commonly used types
pub use attenuverter::Attenuverter;
pub use clock::Clock;
pub use delay::StereoDelay;
pub use envelope::AdsrEnvelope;
pub use filter::SvfFilter;
pub use keyboard::KeyboardInput;
pub use lfo::Lfo;
pub use midi_monitor::MidiMonitor;
pub use midi_note::MidiNote;
pub use oscillator::SineOscillator;
pub use oscilloscope::Oscilloscope;
pub use output::AudioOutput;
pub use sample_hold::SampleHold;
pub use sequencer::StepSequencer;
pub use vca::Vca;
