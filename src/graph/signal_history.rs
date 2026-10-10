//! The recent past of each cabled output, which its cables draw as the
//! signal flowing through them.
//!
//! The audio engine reports each monitored output's peak once per callback.
//! Every UI frame those readings are folded into a ring of levels sampled at
//! a fixed rate, shaped for the kind of signal: audio as a smooth envelope,
//! control as its signed value, gates as on and off. While the transport is
//! stopped the rings fill with "no signal", so the last sound drains out of
//! the cables rather than vanishing.

use egui_node_graph2::SignalTrace;

use crate::dsp::SignalType;
use crate::engine::ChannelPeaks;

/// History samples per second
pub const SAMPLES_PER_SECOND: f32 = 120.0;
/// Seconds of history kept: enough for a long cable at the flow speed
const SECONDS: f32 = 16.0;
const SAMPLES: usize = (SAMPLES_PER_SECOND * SECONDS) as usize;
/// How quickly an audio envelope falls once its peaks stop, in seconds
const RELEASE_SECONDS: f32 = 0.09;

/// How a signal's readings become its history
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceShape {
    /// Audio: the size of its peaks, rising at once and falling smoothly
    Envelope,
    /// Control: its value and sign, drawn as a wave beside the cable
    Waveform,
    /// Gates: high or low, with hard edges
    Steps,
}

impl TraceShape {
    pub fn of(signal_type: SignalType) -> Self {
        match signal_type {
            // A bus is audio, a strand per side and send
            SignalType::Audio | SignalType::Bus => TraceShape::Envelope,
            SignalType::Control => TraceShape::Waveform,
            SignalType::Gate | SignalType::Midi => TraceShape::Steps,
        }
    }
}

/// One channel's levels, oldest overwritten first. NaN means no signal.
#[derive(Debug, Clone)]
struct Ring {
    samples: Box<[f32]>,
    newest: usize,
}

impl Ring {
    fn new() -> Self {
        Self { samples: vec![f32::NAN; SAMPLES].into_boxed_slice(), newest: 0 }
    }

    fn push(&mut self, level: f32) {
        self.newest = (self.newest + 1) % self.samples.len();
        self.samples[self.newest] = level;
    }

    fn latest(&self) -> f32 {
        self.samples[self.newest]
    }
}

/// The recent past of one output, channel by channel.
#[derive(Debug, Clone)]
pub struct OutputHistory {
    shape: TraceShape,
    channels: Vec<Ring>,
    /// The largest reading of each channel since the last tick
    observed: Option<ChannelPeaks>,
    /// The last readings, held until new ones arrive
    held: Option<ChannelPeaks>,
    /// Index of the last sample written, counted from the app's start
    written: Option<i64>,
    /// How long ago the newest sample was taken, in seconds
    lag: f32,
}

impl Default for OutputHistory {
    fn default() -> Self {
        Self {
            shape: TraceShape::Steps,
            channels: vec![Ring::new()],
            observed: None,
            held: None,
            written: None,
            lag: 0.0,
        }
    }
}

impl OutputHistory {
    pub fn set_shape(&mut self, shape: TraceShape) {
        self.shape = shape;
    }

    /// Takes in a reading from the audio engine, keeping each channel's
    /// largest until the next tick.
    pub fn observe(&mut self, reading: ChannelPeaks) {
        self.observed = Some(match self.observed {
            Some(earlier) => earlier.louder(&reading),
            None => reading,
        });
    }

    /// Brings the history up to `now` (seconds on the UI clock). While `live`
    /// it records the latest readings; otherwise it records no signal.
    pub fn tick(&mut self, now: f64, live: bool) {
        let rate = SAMPLES_PER_SECOND as f64;
        let index = (now * rate).floor() as i64;
        self.lag = (now - index as f64 / rate) as f32;
        let new_samples = match self.written {
            None => 1,
            Some(written) => (index - written).clamp(0, SAMPLES as i64) as usize,
        };
        if new_samples == 0 {
            return;
        }
        self.written = Some(index);
        if let Some(observed) = self.observed.take() {
            self.held = Some(observed);
        }

        let reading = self.held.filter(|_| live);
        if let Some(reading) = reading {
            self.channels.resize_with(reading.count().max(1), Ring::new);
        }
        let release = (-1.0 / (SAMPLES_PER_SECOND * RELEASE_SECONDS)).exp();
        for (channel, ring) in self.channels.iter_mut().enumerate() {
            let target = reading.map(|reading| reading.peak(channel));
            let start = ring.latest();
            let mut level = start;
            for step in 1..=new_samples {
                level = match (target, self.shape) {
                    (None, _) => f32::NAN,
                    (Some(peak), TraceShape::Envelope) => {
                        let falling = if level.is_nan() { 0.0 } else { level * release };
                        peak.abs().max(falling)
                    }
                    // Glide between readings, so a slow LFO draws a smooth curve
                    (Some(value), TraceShape::Waveform) if !start.is_nan() => {
                        start + (value - start) * step as f32 / new_samples as f32
                    }
                    (Some(value), _) => value,
                };
                ring.push(level);
            }
        }
    }

    /// The trace a cable from this output draws for `channel`
    pub fn trace(&self, channel: usize) -> Option<SignalTrace<'_>> {
        let ring = self.channels.get(channel)?;
        let trace = SignalTrace::history(&ring.samples, ring.newest, SAMPLES_PER_SECOND, self.lag);
        Some(match self.shape {
            TraceShape::Waveform => trace.as_waveform(),
            _ => trace,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::SignalBuffer;

    fn reading(level: f32) -> ChannelPeaks {
        let mut buffer = SignalBuffer::new(4, SignalType::Audio);
        buffer.samples.fill(level);
        ChannelPeaks::of(&buffer)
    }

    fn level(history: &OutputHistory, age: f32) -> Option<f32> {
        history.trace(0).unwrap().at(age)
    }

    #[test]
    fn records_readings_as_they_arrive() {
        let mut history = OutputHistory::default();
        history.observe(reading(0.5));
        history.tick(1.0, true);
        history.observe(reading(1.0));
        history.tick(1.5, true);
        assert_eq!(level(&history, 0.0), Some(1.0));
        assert_eq!(level(&history, 0.5), Some(0.5));
    }

    #[test]
    fn holds_the_last_reading_between_callbacks() {
        let mut history = OutputHistory::default();
        history.observe(reading(0.7));
        history.tick(1.0, true);
        history.tick(1.1, true);
        history.tick(1.2, true);
        assert_eq!(level(&history, 0.0), Some(0.7));
    }

    #[test]
    fn stopping_drains_the_signal() {
        let mut history = OutputHistory::default();
        history.observe(reading(0.7));
        history.tick(1.0, true);
        history.tick(1.5, true);
        history.tick(2.0, false);
        assert_eq!(level(&history, 0.0), None);
        assert_eq!(level(&history, 0.75), Some(0.7));
    }

    #[test]
    fn audio_envelope_falls_smoothly() {
        let mut history = OutputHistory::default();
        history.set_shape(TraceShape::Envelope);
        history.observe(reading(-0.8));
        history.tick(1.0, true);
        history.observe(reading(0.0));
        history.tick(1.05, true);
        let tail = level(&history, 0.0).unwrap();
        assert!(tail > 0.2 && tail < 0.8, "envelope fell to {tail}");
        history.tick(2.0, true);
        assert!(level(&history, 0.0).unwrap() < 0.01);
    }

    #[test]
    fn waveform_keeps_its_sign() {
        let mut history = OutputHistory::default();
        history.set_shape(TraceShape::Waveform);
        history.observe(reading(-0.6));
        history.tick(1.0, true);
        let trace = history.trace(0).unwrap();
        assert!(trace.is_waveform());
        assert_eq!(trace.at(0.0), Some(-0.6));
    }

    #[test]
    fn keeps_the_loudest_reading_of_a_frame() {
        let mut history = OutputHistory::default();
        history.observe(reading(0.9));
        history.observe(reading(0.1));
        history.tick(1.0, true);
        assert_eq!(level(&history, 0.0), Some(0.9));
    }
}
