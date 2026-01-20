//! Modules module
//!
//! Built-in synthesizer modules.
//! Includes oscillators, filters, envelopes, LFOs, and output modules.

pub mod oscillator;
pub mod output;

// Re-export commonly used types
pub use oscillator::SineOscillator;
pub use output::AudioOutput;

// Future submodules:
// pub mod filter;
// pub mod envelope;
// pub mod lfo;
