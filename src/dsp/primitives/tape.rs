//! The parts of a tape machine shared by the Delay's Tape mode and the Tape
//! module: the wobbling transport and the record head.

use super::saturation::fast_tanh;

/// A sine oscillator kept as a rotating unit vector: one complex multiply per
/// sample instead of a `sin` call.
#[derive(Clone, Copy, Debug)]
struct Rotor {
    re: f32,
    im: f32,
    cos: f32,
    sin: f32,
    /// Where the vector starts, and returns to on reset.
    start: (f32, f32),
}

impl Rotor {
    fn new(freq_hz: f32, sample_rate: f32) -> Self {
        let mut rotor = Self { re: 1.0, im: 0.0, cos: 1.0, sin: 0.0, start: (1.0, 0.0) };
        rotor.set_frequency(freq_hz, sample_rate);
        rotor
    }

    fn set_frequency(&mut self, freq_hz: f32, sample_rate: f32) {
        let w = std::f32::consts::TAU * freq_hz / sample_rate;
        self.cos = w.cos();
        self.sin = w.sin();
    }

    /// Starts the vector `turns` of the way round, rather than at zero.
    fn set_start(&mut self, turns: f32) {
        let angle = std::f32::consts::TAU * turns;
        self.start = (angle.cos(), angle.sin());
        self.reset();
    }

    /// Advances one sample and returns the sine.
    #[inline]
    fn next(&mut self) -> f32 {
        let re = self.re * self.cos - self.im * self.sin;
        self.im = self.re * self.sin + self.im * self.cos;
        self.re = re;
        self.im
    }

    /// Pulls the vector back onto the unit circle (rounding drifts it).
    fn renormalize(&mut self) {
        let gain = 1.5 - 0.5 * (self.re * self.re + self.im * self.im);
        self.re *= gain;
        self.im *= gain;
    }

    fn reset(&mut self) {
        (self.re, self.im) = self.start;
    }
}

/// The wobble of a tape transport, as an offset to the read head in samples.
///
/// Wow is the slow lurch of an off-centre reel, flutter the fast shiver of the
/// capstan. Each is a pair of sines at unrelated rates, so the pattern never
/// quite repeats. Depths are given as peak pitch deviation: a read head
/// swinging `A·sin(2πft)` samples bends the pitch by up to `A·2πf/sr`.
#[derive(Clone, Debug)]
pub struct TapeTransport {
    partials: [Rotor; 4],
    /// Peak offset of each partial, in samples, at the stock depths.
    depths: [f32; 4],
    /// How fast the reels and capstan turn, relative to the stock rates.
    rate_scale: f32,
}

impl TapeTransport {
    /// (rate in Hz, peak pitch deviation) for each partial: two of wow, then
    /// two of flutter.
    pub const PARTIALS: [(f32, f32); 4] = [
        (0.53, 0.0020), // wow
        (0.21, 0.0010), // slow drift of the reel
        (6.3, 0.0005),  // flutter
        (9.7, 0.0003),  // capstan shimmer
    ];

    pub fn new(sample_rate: f32) -> Self {
        let mut transport = Self {
            partials: [Rotor::new(1.0, sample_rate); 4],
            depths: [0.0; 4],
            rate_scale: 1.0,
        };
        transport.set_sample_rate(sample_rate);
        transport
    }

    /// A transport whose partials start part of the way round, given in
    /// turns: the second track of a stereo machine, wobbling a little
    /// differently from the first.
    pub fn with_start(sample_rate: f32, turns: [f32; 4]) -> Self {
        let mut transport = Self::new(sample_rate);
        for (rotor, turns) in transport.partials.iter_mut().zip(turns) {
            rotor.set_start(turns);
        }
        transport
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        for (i, &(freq, deviation)) in Self::PARTIALS.iter().enumerate() {
            let freq = freq * self.rate_scale;
            self.partials[i].set_frequency(freq, sample_rate);
            self.depths[i] = deviation * sample_rate / (std::f32::consts::TAU * freq);
        }
    }

    /// Runs the reels and capstan `scale` times as fast, as when the tape
    /// speed changes. The wobble keeps its place in its cycle.
    pub fn set_rate_scale(&mut self, scale: f32, sample_rate: f32) {
        if scale != self.rate_scale {
            self.rate_scale = scale;
            self.set_sample_rate(sample_rate);
        }
    }

    /// The rate of partial `i` now, in Hz.
    pub fn rate(&self, i: usize) -> f32 {
        Self::PARTIALS[i].0 * self.rate_scale
    }

    /// The read-head offset for the next sample at the stock depths.
    /// Zero-mean, starting at zero.
    #[inline]
    pub fn next_offset(&mut self) -> f32 {
        let mut offset = 0.0;
        for (rotor, depth) in self.partials.iter_mut().zip(self.depths) {
            offset += rotor.next() * depth;
        }
        offset
    }

    /// Advances one sample and returns how far each partial has pulled the
    /// tape back, `1 − cos`, from 0 to 2.
    ///
    /// A read head lagging `A·(1 − cos 2πft)` samples never runs ahead of
    /// the write head, so it needs no lookahead, and it rests at zero lag
    /// when `A` is zero. Its pitch swings by `A·2πf/sr`, as a centred one's
    /// does.
    #[inline]
    pub fn next_drag(&mut self) -> [f32; 4] {
        let mut drag = [0.0; 4];
        for (rotor, drag) in self.partials.iter_mut().zip(drag.iter_mut()) {
            rotor.next();
            // Rounding can carry the vector a hair past the unit circle
            *drag = (1.0 - rotor.re).max(0.0);
        }
        drag
    }

    pub fn renormalize(&mut self) {
        self.partials.iter_mut().for_each(Rotor::renormalize);
    }

    pub fn reset(&mut self) {
        self.partials.iter_mut().for_each(Rotor::reset);
    }
}

/// The record head: unity gain for quiet signals, a soft and slightly
/// lopsided squash for loud ones.
///
/// `level·(tanh(x/level + b) − tanh b)·cosh²b` passes zero through zero with a
/// slope of exactly 1, so it leaves the loop gain alone until the tape fills
/// up. The bias tilts the ceilings (+0.78 / −1.06), which adds the even
/// harmonics of magnetised tape.
#[derive(Clone, Copy, Debug)]
pub struct RecordHead {
    tanh_bias: f32,
    slope_gain: f32,
}

impl RecordHead {
    pub const LEVEL: f32 = 0.9;
    pub const BIAS: f32 = 0.15;

    pub fn new() -> Self {
        // The same tanh as `record`, so silence records as exactly zero
        let tanh_bias = fast_tanh(Self::BIAS);
        Self { tanh_bias, slope_gain: 1.0 / (1.0 - tanh_bias * tanh_bias) }
    }

    #[inline]
    pub fn record(&self, x: f32) -> f32 {
        Self::LEVEL * (fast_tanh(x / Self::LEVEL + Self::BIAS) - self.tanh_bias) * self.slope_gain
    }
}

impl Default for RecordHead {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_head_is_transparent_when_quiet() {
        let head = RecordHead::new();
        assert!((head.record(1e-3) - 1e-3).abs() < 1e-6);
        assert_eq!(head.record(0.0), 0.0);
        // Loud signals squash, harder on one side than the other
        assert!(head.record(10.0) < 0.8 && head.record(10.0) > 0.7);
        assert!(head.record(-10.0) < -1.0 && head.record(-10.0) > -1.1);
    }

    #[test]
    fn test_drag_starts_at_rest_and_never_leads() {
        let sr = 48000.0;
        let mut transport = TapeTransport::new(sr);
        let first = transport.next_drag();
        assert!(first.iter().all(|&d| (0.0..1e-3).contains(&d)), "{first:?}");
        for n in 0..sr as usize * 10 {
            let drag = transport.next_drag();
            assert!(drag.iter().all(|&d| (0.0..=2.0001).contains(&d)), "{drag:?}");
            if n % 480 == 0 {
                transport.renormalize();
            }
        }
    }

    #[test]
    fn test_rate_scale_changes_the_rate_not_the_place() {
        // Halving the speed halves every rate and keeps each partial where it was
        let sr = 48000.0;
        let mut transport = TapeTransport::new(sr);
        for _ in 0..1000 {
            transport.next_drag();
        }
        let before = transport.partials.map(|r| (r.re, r.im));
        transport.set_rate_scale(0.5, sr);
        assert_eq!(transport.partials.map(|r| (r.re, r.im)), before);
        assert_eq!(transport.rate(0), 0.265);
    }

    #[test]
    fn test_second_track_starts_elsewhere_and_resets_there() {
        let sr = 48000.0;
        let mut track = TapeTransport::with_start(sr, [0.25, 0.0, 0.5, 0.0]);
        let drag = track.next_drag();
        assert!((drag[0] - 1.0).abs() < 1e-3 && (drag[2] - 2.0).abs() < 1e-2, "{drag:?}");
        for _ in 0..500 {
            track.next_drag();
        }
        track.reset();
        assert_eq!(track.next_drag()[0], drag[0]);
    }
}
