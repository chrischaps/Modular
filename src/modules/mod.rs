//! Modules module
//!
//! Built-in synthesizer modules.
//! Includes oscillators, filters, envelopes, LFOs, and output modules.

pub mod envelope;
pub mod filter;
pub mod lfo;
pub mod oscillator;
pub mod output;

// Re-export commonly used types
pub use envelope::AdsrEnvelope;
pub use filter::SvfFilter;
pub use lfo::Lfo;
pub use oscillator::SineOscillator;
pub use output::AudioOutput;
