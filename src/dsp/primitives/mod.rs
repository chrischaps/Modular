//! Small DSP building blocks shared between modules.
//!
//! Everything here is allocation-free and safe to call on the audio thread.

pub mod blep;
pub mod delay;
pub mod noise;
pub mod oversample;
pub mod saturation;
pub mod tpt;

pub use blep::BlepDelay;
pub use delay::FracDelay;
pub use noise::NoiseFloor;
pub use oversample::{Downsampler2x, Upsampler2x};
pub use saturation::{fast_tanh, SoftSaturator};
pub use tpt::{prewarp, TptIntegrator};
