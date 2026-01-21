//! Modules module
//!
//! Built-in synthesizer modules.
//! Includes oscillators, filters, envelopes, LFOs, utilities, and output modules.

pub mod attenuverter;
pub mod clock;
pub mod envelope;
pub mod filter;
pub mod lfo;
pub mod oscillator;
pub mod output;
pub mod vca;

// Re-export commonly used types
pub use attenuverter::Attenuverter;
pub use clock::Clock;
pub use envelope::AdsrEnvelope;
pub use filter::SvfFilter;
pub use lfo::Lfo;
pub use oscillator::SineOscillator;
pub use output::AudioOutput;
pub use vca::Vca;
