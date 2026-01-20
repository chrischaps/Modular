//! Modules module
//!
//! Built-in synthesizer modules.
//! Includes oscillators, filters, envelopes, LFOs, and output modules.

pub mod oscillator;

// Re-export commonly used types
pub use oscillator::SineOscillator;

// Future submodules:
// pub mod filter;
// pub mod envelope;
// pub mod lfo;
// pub mod output;
