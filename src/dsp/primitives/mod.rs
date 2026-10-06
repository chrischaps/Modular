//! Small DSP building blocks shared between modules.
//!
//! Everything here is allocation-free and safe to call on the audio thread.

pub mod saturation;
pub mod tpt;

pub use saturation::{fast_tanh, SoftSaturator};
pub use tpt::{prewarp, TptIntegrator};
