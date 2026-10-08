//! How long live input takes to reach the speakers.
//!
//! cpal stamps every buffer with when it happened on the device's clock:
//!
//! - an input buffer with when its first frame was **captured** and when the
//!   callback got it, so `callback − capture` is the input device's delay;
//! - an output buffer with when the callback runs and when its first frame
//!   will be **played**, so `playback − callback` is the output device's.
//!
//! Each callback hands its figure to a [`LatencyGauge`], which smooths it
//! with a store and nothing else. [`RoundTrip`] adds the two to Modular's own
//! share, the jitter buffer between the callbacks and the output limiter's
//! look-ahead, for the status bar.
//!
//! Some backends report no usable timestamps (zero, or running backwards).
//! A gauge that never had a usable reading reads `None`. Windows (WASAPI) is
//! one, on the input side: cpal stamps the callback with the capture clock's
//! last update, which is the capture time of this packet or the one before,
//! so the difference is always zero or negative. The input's delay is then
//! estimated as one packet, the least it can be: a packet isn't handed over
//! until its last frame is recorded, so its first frame has waited at least
//! that long. With no figure at all for a device, the round trip counts
//! only the parts that are known.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::audio_input::InputMonitor;
use crate::dsp::dynamics::PeakLimiter;

/// A device's delay, smoothed over its last few dozen callbacks.
#[derive(Debug, Default)]
pub struct LatencyGauge {
    /// The smoothed delay in microseconds; 0 until the first usable reading.
    micros: AtomicU64,
}

impl LatencyGauge {
    /// How much of each new reading the smoothed value takes in (1 / this).
    const SMOOTHING: i64 = 16;

    /// Takes in one callback's delay: `later.duration_since(earlier)` of
    /// its two timestamps, or `None` if they ran backwards. A zero or
    /// missing reading leaves the gauge as it was.
    ///
    /// REAL-TIME SAFE: an atomic load and store.
    #[inline]
    pub fn record(&self, delay: Option<Duration>) {
        let Some(delay) = delay.filter(|d| !d.is_zero()) else {
            return;
        };
        let reading = (delay.as_micros() as u64).max(1);
        let smoothed = match self.micros.load(Ordering::Relaxed) {
            0 => reading,
            previous => {
                let step = (reading as i64 - previous as i64) / Self::SMOOTHING;
                (previous as i64 + step).max(1) as u64
            }
        };
        self.micros.store(smoothed, Ordering::Relaxed);
    }

    /// The smoothed delay, or `None` if the device never reported one.
    pub fn get(&self) -> Option<Duration> {
        match self.micros.load(Ordering::Relaxed) {
            0 => None,
            micros => Some(Duration::from_micros(micros)),
        }
    }

    /// Forgets the readings, for a new stream.
    pub fn clear(&self) {
        self.micros.store(0, Ordering::Relaxed);
    }
}

/// The delay from the input jack to the speakers, part by part.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundTrip {
    /// The input device's delay: its own report, or else one packet's worth.
    pub input_device: Option<Duration>,
    /// Whether `input_device` is the one-packet estimate rather than the
    /// device's own report.
    pub input_estimated: bool,
    /// The jitter buffer between the input and output callbacks.
    pub buffer: Duration,
    /// The output device's delay, if it reports one.
    pub output_device: Option<Duration>,
    /// The output limiter's look-ahead.
    pub limiter: Duration,
}

impl RoundTrip {
    /// The round trip through an open input, at the output's `sample_rate`.
    pub fn of(input: &InputMonitor, output_device: Option<Duration>, sample_rate: u32) -> Self {
        Self::new(input.device_latency(), input.packet_frames(), input.buffered_frames(), output_device, sample_rate)
    }

    /// The round trip from its parts. `input_reported` is what the input
    /// device reported, `input_packet_frames` the size of its packets (to
    /// estimate from without a report), and `buffered_frames` the jitter
    /// buffer's level, all in frames at the output's `sample_rate`.
    pub fn new(
        input_reported: Option<Duration>,
        input_packet_frames: usize,
        buffered_frames: usize,
        output_device: Option<Duration>,
        sample_rate: u32,
    ) -> Self {
        let frames = |n: usize| Duration::from_secs_f64(n as f64 / sample_rate.max(1) as f64);
        let input_estimated = input_reported.is_none() && input_packet_frames > 0;
        Self {
            input_device: input_reported.or(input_estimated.then(|| frames(input_packet_frames))),
            input_estimated,
            buffer: frames(buffered_frames),
            output_device,
            limiter: frames(PeakLimiter::latency_at(sample_rate as f32)),
        }
    }

    /// Whether both devices have a figure (reported or estimated), so the
    /// total is the whole trip rather than only part of it.
    pub fn complete(&self) -> bool {
        self.input_device.is_some() && self.output_device.is_some()
    }

    /// The sum of every part that's known.
    pub fn total(&self) -> Duration {
        self.input_device.unwrap_or_default() + self.buffer + self.output_device.unwrap_or_default() + self.limiter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn test_gauge_starts_unknown_and_takes_the_first_reading_whole() {
        let gauge = LatencyGauge::default();
        assert_eq!(gauge.get(), None);
        gauge.record(Some(ms(12)));
        assert_eq!(gauge.get(), Some(ms(12)));
    }

    #[test]
    fn test_gauge_smooths_toward_new_readings() {
        let gauge = LatencyGauge::default();
        gauge.record(Some(ms(10)));
        gauge.record(Some(ms(26)));
        assert_eq!(gauge.get(), Some(ms(11)), "one sixteenth of the way");
        for _ in 0..200 {
            gauge.record(Some(ms(26)));
        }
        let settled = gauge.get().unwrap();
        assert!(settled > ms(25) && settled <= ms(26), "{settled:?}");
    }

    #[test]
    fn test_gauge_ignores_missing_and_zero_readings() {
        let gauge = LatencyGauge::default();
        gauge.record(None);
        gauge.record(Some(Duration::ZERO));
        assert_eq!(gauge.get(), None, "a backend with no timestamps reads as unknown");

        gauge.record(Some(ms(14)));
        gauge.record(None);
        gauge.record(Some(Duration::ZERO));
        assert_eq!(gauge.get(), Some(ms(14)), "a stray bad reading doesn't wipe a good one");

        gauge.clear();
        assert_eq!(gauge.get(), None);
    }

    #[test]
    fn test_round_trip_adds_every_part() {
        // 21 ms of buffer at 48 kHz is 1008 frames
        let trip = RoundTrip::new(Some(ms(12)), 480, 1008, Some(ms(14)), 48000);
        assert_eq!(trip.buffer, ms(21));
        assert!(!trip.input_estimated, "a reported delay wins over the packet size");
        assert_eq!(trip.limiter, Duration::from_secs_f64(47.0 / 48000.0), "1 ms of look-ahead, less a sample");
        assert!(trip.complete());
        assert_eq!(trip.total(), ms(12) + ms(21) + ms(14) + trip.limiter);
        assert_eq!(trip.total().as_millis(), 47);
    }

    #[test]
    fn test_round_trip_estimates_an_unreported_input_as_one_packet() {
        // WASAPI: no input timestamps, 10 ms packets
        let trip = RoundTrip::new(None, 480, 1008, Some(ms(10)), 48000);
        assert!(trip.input_estimated);
        assert_eq!(trip.input_device, Some(ms(10)));
        assert!(trip.complete());
        assert_eq!(trip.total(), ms(10) + ms(21) + ms(10) + trip.limiter);
    }

    #[test]
    fn test_round_trip_without_timestamps_counts_only_what_is_known() {
        // Nothing reported, and no packet seen yet
        let trip = RoundTrip::new(None, 0, 1008, None, 48000);
        assert!(!trip.complete());
        assert!(!trip.input_estimated);
        assert_eq!(trip.input_device, None);
        assert_eq!(trip.total(), trip.buffer + trip.limiter);

        // One side reporting still counts that side
        let half = RoundTrip::new(None, 0, 1008, Some(ms(14)), 48000);
        assert!(!half.complete());
        assert_eq!(half.total(), ms(14) + half.buffer + half.limiter);
    }

    #[test]
    fn test_round_trip_survives_a_zero_sample_rate() {
        let trip = RoundTrip::new(None, 0, 100, None, 0);
        assert!(trip.total() > Duration::ZERO);
    }
}
