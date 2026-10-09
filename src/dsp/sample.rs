//! Recorded audio a module plays from: a sample loaded from a file.
//!
//! [`SampleData`] is built off the audio thread (decoded, cut to length and
//! resampled to the engine's rate there) and handed to the module that
//! plays it as an `Arc`, so the audio thread only ever swaps a pointer.
//! When it's replaced, the module hands the old one back to be dropped on
//! the UI side, so freeing a long file never happens in the audio callback.

use super::primitives::hermite;

/// The longest sample a module keeps, in seconds: five minutes, about
/// 115 MB of stereo at 48 kHz. A longer file is cut to this.
pub const MAX_SAMPLE_SECONDS: f32 = 300.0;

/// A mono or stereo recording at a known sample rate.
#[derive(Clone, PartialEq)]
pub struct SampleData {
    /// The left channel, or the only one.
    left: Vec<f32>,
    /// The right channel, or `None` for a mono recording, which plays on
    /// both sides.
    right: Option<Vec<f32>>,
    /// Frames per second the samples were recorded (or resampled) at.
    sample_rate: f32,
}

impl std::fmt::Debug for SampleData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Not the samples: there may be millions
        f.debug_struct("SampleData")
            .field("frames", &self.frames())
            .field("stereo", &self.is_stereo())
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

impl SampleData {
    /// A mono recording.
    pub fn mono(samples: Vec<f32>, sample_rate: f32) -> Self {
        Self { left: samples, right: None, sample_rate: sample_rate.max(1.0) }
    }

    /// A stereo recording. The longer channel is cut to the shorter.
    pub fn stereo(mut left: Vec<f32>, mut right: Vec<f32>, sample_rate: f32) -> Self {
        let frames = left.len().min(right.len());
        left.truncate(frames);
        right.truncate(frames);
        Self { left, right: Some(right), sample_rate: sample_rate.max(1.0) }
    }

    /// How many frames it holds.
    pub fn frames(&self) -> usize {
        self.left.len()
    }

    /// Whether it holds no audio at all.
    pub fn is_empty(&self) -> bool {
        self.left.is_empty()
    }

    /// Whether it has a right channel of its own.
    pub fn is_stereo(&self) -> bool {
        self.right.is_some()
    }

    /// The rate its frames are at, in Hz.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// How long it plays at its own rate, in seconds.
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.sample_rate
    }

    /// The left channel, or the only one.
    pub fn left(&self) -> &[f32] {
        &self.left
    }

    /// The right channel; a mono recording's one channel.
    pub fn right(&self) -> &[f32] {
        self.right.as_deref().unwrap_or(&self.left)
    }

    /// Cuts it to at most `seconds` long. Returns true if anything was cut.
    pub fn truncate_seconds(&mut self, seconds: f32) -> bool {
        let keep = (seconds.max(0.0) * self.sample_rate) as usize;
        if keep >= self.frames() {
            return false;
        }
        self.left.truncate(keep);
        if let Some(right) = &mut self.right {
            right.truncate(keep);
        }
        true
    }

    /// The frame at `position` (in frames, fractional), read between
    /// samples on a Hermite curve, as (left, right). Outside the recording
    /// it holds the first or last sample, so a curve near either end is
    /// still smooth.
    ///
    /// REAL-TIME SAFE.
    #[inline]
    pub fn read(&self, position: f64) -> (f32, f32) {
        let frames = self.frames();
        if frames == 0 {
            return (0.0, 0.0);
        }
        let last = frames as i64 - 1;
        let base = position.floor();
        let t = (position - base) as f32;
        let i = base as i64;
        let at = |k: i64| (i + k).clamp(0, last) as usize;
        let (a, b, c, d) = (at(-1), at(0), at(1), at(2));
        let curve = |x: &[f32]| hermite(x[a], x[b], x[c], x[d], t);
        let left = curve(&self.left);
        let right = match &self.right {
            Some(right) => curve(right),
            None => left,
        };
        (left, right)
    }

    /// The same recording at another sample rate, read between samples on
    /// a Hermite curve, so a voice can play it back one frame per frame.
    pub fn resampled(&self, sample_rate: f32) -> Self {
        let sample_rate = sample_rate.max(1.0);
        if sample_rate == self.sample_rate || self.is_empty() {
            return Self { sample_rate, ..self.clone() };
        }
        let step = self.sample_rate as f64 / sample_rate as f64;
        let frames = ((self.frames() as f64) / step).floor().max(1.0) as usize;
        let mut left = Vec::with_capacity(frames);
        let mut right = self.right.as_ref().map(|_| Vec::with_capacity(frames));
        for n in 0..frames {
            let (l, r) = self.read(n as f64 * step);
            left.push(l);
            if let Some(right) = &mut right {
                right.push(r);
            }
        }
        Self { left, right, sample_rate }
    }

    /// Its loudest and quietest sample in each of `columns` equal slices,
    /// both channels together, for drawing it as a waveform.
    pub fn overview(&self, columns: usize) -> Vec<(f32, f32)> {
        let frames = self.frames();
        if frames == 0 || columns == 0 {
            return Vec::new();
        }
        (0..columns)
            .map(|column| {
                let start = column * frames / columns;
                let end = ((column + 1) * frames / columns).max(start + 1).min(frames);
                let mut low = f32::INFINITY;
                let mut high = f32::NEG_INFINITY;
                for channel in [self.left(), self.right()] {
                    for &sample in &channel[start..end] {
                        low = low.min(sample);
                        high = high.max(sample);
                    }
                }
                (low, high)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, rate: f32, seconds: f32) -> Vec<f32> {
        (0..(rate * seconds) as usize).map(|n| (std::f32::consts::TAU * hz * n as f32 / rate).sin()).collect()
    }

    #[test]
    fn reads_whole_frames_exactly() {
        let sample = SampleData::stereo(vec![0.0, 1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0, 7.0], 48000.0);
        assert_eq!(sample.read(2.0), (2.0, 6.0));
        // Past either end it holds the end
        assert_eq!(sample.read(-3.0), (0.0, 4.0));
        assert_eq!(sample.read(10.0), (3.0, 7.0));
    }

    #[test]
    fn mono_plays_on_both_sides() {
        let sample = SampleData::mono(vec![0.25; 8], 44100.0);
        assert_eq!(sample.read(3.5), (0.25, 0.25));
        assert_eq!(sample.right(), sample.left());
    }

    #[test]
    fn resampling_keeps_length_and_pitch() {
        let source = SampleData::mono(sine(1000.0, 44100.0, 1.0), 44100.0);
        let resampled = source.resampled(48000.0);
        assert_eq!(resampled.sample_rate(), 48000.0);
        assert!((resampled.seconds() - 1.0).abs() < 0.001);
        // Count rising zero crossings: a 1 kHz tone still has a thousand a second
        let crossings = resampled.left().windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
        assert!((999..=1001).contains(&crossings), "{crossings}");
        // Matches the sine it was cut from
        for n in (100..47000).step_by(997) {
            let expected = (std::f32::consts::TAU * 1000.0 * n as f32 / 48000.0).sin();
            assert!((resampled.left()[n] - expected).abs() < 0.01, "frame {n}");
        }
    }

    #[test]
    fn truncating_cuts_both_channels() {
        let mut sample = SampleData::stereo(vec![0.0; 1000], vec![0.0; 1000], 100.0);
        assert!(sample.truncate_seconds(2.5));
        assert_eq!(sample.frames(), 250);
        assert_eq!(sample.right().len(), 250);
        assert!(!sample.truncate_seconds(60.0));
    }

    #[test]
    fn overview_holds_each_slices_extremes() {
        let mut samples = vec![0.0; 100];
        samples[10] = 0.9;
        samples[60] = -0.5;
        let overview = SampleData::mono(samples, 1000.0).overview(4);
        assert_eq!(overview.len(), 4);
        assert_eq!(overview[0], (0.0, 0.9));
        assert_eq!(overview[2], (-0.5, 0.0));
        assert_eq!(overview[3], (0.0, 0.0));
    }
}
