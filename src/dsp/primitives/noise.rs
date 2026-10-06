//! A tiny noise source standing in for circuit noise.

/// White noise at a fixed, very low level (xorshift32).
///
/// A self-oscillating filter with nothing patched in needs something to grow
/// from, the way an analog filter grows from the hiss of its own components.
/// Added to a filter's input at -120 dBFS it starts the oscillation within a
/// fraction of a second and is far below anything audible.
#[derive(Clone, Copy, Debug)]
pub struct NoiseFloor {
    seed: u32,
    scale: f32,
}

impl NoiseFloor {
    /// Peak level of [`NoiseFloor::default`]: -120 dBFS.
    pub const DEFAULT_LEVEL: f32 = 1e-6;

    /// Noise peaking at ±`level`.
    pub const fn new(level: f32) -> Self {
        Self { seed: 0x2545_f491, scale: level / 2_147_483_648.0 }
    }

    /// The next noise sample.
    #[inline]
    pub fn sample(&mut self) -> f32 {
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x as i32 as f32) * self.scale
    }
}

impl Default for NoiseFloor {
    fn default() -> Self {
        Self::new(Self::DEFAULT_LEVEL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_floor_is_bounded_and_not_silent() {
        let mut noise = NoiseFloor::default();
        let samples: Vec<f32> = (0..10000).map(|_| noise.sample()).collect();
        assert!(samples.iter().all(|s| s.abs() <= NoiseFloor::DEFAULT_LEVEL));
        let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
        // Uniform white noise: rms = level / sqrt(3)
        let expected = NoiseFloor::DEFAULT_LEVEL / 3.0f32.sqrt();
        assert!((mean_square.sqrt() / expected - 1.0).abs() < 0.05);
    }
}
