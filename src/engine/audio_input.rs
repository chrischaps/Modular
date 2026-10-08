//! Live audio input: carries a microphone, guitar or line source from the
//! input device's callback to the output callback.
//!
//! The two callbacks run on their own clocks, in their own buffer sizes, and
//! sometimes at their own sample rates. The input side's [`InputSender`]
//! pushes every frame it gets, as stereo, into an `rtrb` ring, converting it
//! to the output's rate first if the input device runs at another one. The
//! output side's [`InputFeed`] is a small jitter buffer over that ring:
//!
//! - It waits until the ring holds a little more than the largest input
//!   buffer and the largest output buffer seen (the *target*) before it
//!   starts reading, so the reads ride out the callbacks' uneven timing.
//! - **Underrun:** when a read finds too little, the missing frames are
//!   silence, and the feed waits to refill to the target before reading
//!   again. Each underrun adds a little cushion to the target, so a device
//!   pair that keeps running dry settles at a latency that works.
//! - **Overflow:** when the ring holds well over the target (the input clock
//!   runs a little fast, or the output stalled), the oldest frames are
//!   dropped back down to it. Frames the ring had no room for are dropped
//!   on the input side.
//!
//! Both are counted, in frames, for the status bar. The feed reaches the
//! audio thread inside an [`AudioMessage`] and goes back to the UI thread to
//! be dropped, like a recording's tap, so the ring is never freed there.
//!
//! [`AudioMessage`]: super::commands::AudioMessage

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use rtrb::{Consumer, Producer, RingBuffer};

/// How much audio the ring can hold, in seconds. Far more than the jitter
/// buffer ever keeps: room for the output to stall without losing input.
pub const RING_SECONDS: f32 = 0.5;

/// Counters shared by both ends and the UI.
#[derive(Debug, Default)]
struct InputStats {
    /// Frames of silence read in place of input that hadn't arrived.
    underrun_frames: AtomicU64,
    /// Frames dropped: input with no room in the ring, or trimmed because
    /// the ring ran too far ahead.
    overflow_frames: AtomicU64,
    /// The largest buffer the input device has delivered, in frames at the
    /// output's rate.
    largest_input: AtomicUsize,
    /// The level the feed refills to, in frames: the input's added latency.
    target_frames: AtomicUsize,
    /// Set by the input stream's error callback (unplugged, say).
    failed: AtomicBool,
}

/// Converts a stream of stereo frames from one sample rate to another, a
/// frame at a time, by 4-point cubic Hermite interpolation.
///
/// Clean for voice and guitar. Content right at the top of the band (above
/// the lower rate's Nyquist, when converting down) folds back faintly, as
/// there's no steep anti-aliasing filter.
#[derive(Clone, Debug)]
struct Resampler {
    /// Input frames per output frame.
    step: f64,
    /// Where the next output frame falls: 0 at `history[1]`, 1 at `history[2]`.
    position: f64,
    /// The last four input frames, oldest first.
    history: [[f32; 2]; 4],
}

impl Resampler {
    fn new(input_rate: u32, output_rate: u32) -> Self {
        Self { step: input_rate as f64 / output_rate.max(1) as f64, position: 1.0, history: [[0.0; 2]; 4] }
    }

    /// The most output frames one input frame can complete.
    fn max_burst(&self) -> usize {
        (1.0 / self.step).ceil() as usize + 1
    }

    /// Takes in one input frame, handing each output frame it completes to
    /// `emit`.
    #[inline]
    fn push(&mut self, frame: [f32; 2], mut emit: impl FnMut([f32; 2])) {
        self.history.rotate_left(1);
        self.history[3] = frame;
        self.position -= 1.0;
        while self.position < 1.0 {
            let t = self.position.max(0.0) as f32;
            let [x0, x1, x2, x3] = self.history;
            emit([hermite(x0[0], x1[0], x2[0], x3[0], t), hermite(x0[1], x1[1], x2[1], x3[1], t)]);
            self.position += self.step;
        }
    }
}

/// The 4-point, 3rd-order Hermite curve through `x1` (at 0) and `x2` (at 1).
#[inline]
fn hermite(x0: f32, x1: f32, x2: f32, x3: f32, t: f32) -> f32 {
    let c1 = 0.5 * (x2 - x0);
    let c2 = x0 - 2.5 * x1 + 2.0 * x2 - 0.5 * x3;
    let c3 = 0.5 * (x3 - x0) + 1.5 * (x1 - x2);
    ((c3 * t + c2) * t + c1) * t + x1
}

/// Writes interleaved stereo frames into the ring, as many as fit, counting
/// the rest as dropped. Returns the frames written.
fn write_stereo(producer: &mut Producer<f32>, stats: &InputStats, samples: &[f32]) -> usize {
    let frames = samples.len() / 2;
    let fit = (producer.slots() / 2).min(frames);
    if fit > 0 {
        if let Ok(chunk) = producer.write_chunk_uninit(fit * 2) {
            chunk.fill_from_iter(samples[..fit * 2].iter().copied());
        }
    }
    if fit < frames {
        stats.overflow_frames.fetch_add((frames - fit) as u64, Ordering::Relaxed);
    }
    fit
}

/// The input callback's end: pushes what the device records.
pub struct InputSender {
    producer: Producer<f32>,
    stats: Arc<InputStats>,
    /// Converts the device's rate to the output's, when they differ.
    resampler: Option<Resampler>,
}

impl InputSender {
    /// Pushes one callback's interleaved input of `channels` channels as
    /// stereo: a mono device is heard on both sides, and only the first two
    /// channels of a bigger interface. Frames that don't fit are counted.
    ///
    /// REAL-TIME SAFE: no allocation, locking or waiting.
    pub fn push<T: Copy>(&mut self, data: &[T], channels: usize, to_f32: impl Fn(T) -> f32) {
        if channels == 0 {
            return;
        }
        let stereo = |frame: &[T]| {
            let left = to_f32(frame[0]);
            [left, if channels > 1 { to_f32(frame[1]) } else { left }]
        };
        let Self { producer, stats, resampler } = self;

        let Some(resampler) = resampler.as_mut() else {
            let frames = data.len() / channels;
            stats.largest_input.fetch_max(frames, Ordering::Relaxed);
            let fit = (producer.slots() / 2).min(frames);
            if fit > 0 {
                if let Ok(chunk) = producer.write_chunk_uninit(fit * 2) {
                    chunk.fill_from_iter(data.chunks_exact(channels).take(fit).flat_map(stereo));
                }
            }
            if fit < frames {
                stats.overflow_frames.fetch_add((frames - fit) as u64, Ordering::Relaxed);
            }
            return;
        };

        // Converted a few hundred frames at a time through the stack
        let mut converted = [0.0f32; 512];
        let mut len = 0;
        let mut produced = 0;
        let headroom = 2 * resampler.max_burst();
        for frame in data.chunks_exact(channels) {
            resampler.push(stereo(frame), |out| {
                converted[len] = out[0];
                converted[len + 1] = out[1];
                len += 2;
            });
            if len + headroom > converted.len() {
                write_stereo(producer, stats, &converted[..len]);
                produced += len / 2;
                len = 0;
            }
        }
        write_stereo(producer, stats, &converted[..len]);
        produced += len / 2;
        stats.largest_input.fetch_max(produced, Ordering::Relaxed);
    }

    /// Pushes interleaved `f32` input.
    pub fn push_f32(&mut self, data: &[f32], channels: usize) {
        self.push(data, channels, |s| s);
    }
}

/// The output callback's end: a jitter buffer that hands out input a block
/// at a time.
pub struct InputFeed {
    consumer: Consumer<f32>,
    stats: Arc<InputStats>,
    /// Whether the ring has filled to the target since it last ran dry.
    primed: bool,
    /// Extra frames kept on top of one input and one output buffer, grown
    /// by each underrun.
    cushion: usize,
    /// The largest buffer the output has asked for, in frames. Some drivers
    /// vary their buffer size from one callback to the next.
    largest_output: usize,
}

impl InputFeed {
    /// The fewest frames of cushion, for a little timing jitter.
    const MIN_CUSHION: usize = 32;
    /// The cushion never grows past this, about 20 ms at 48 kHz.
    const MAX_CUSHION: usize = 1024;
    /// How much each underrun adds to the cushion.
    const CUSHION_STEP: usize = 64;

    /// The level reads start from: the largest input buffer, the largest
    /// output buffer, and the cushion.
    fn target(&self) -> usize {
        self.stats.largest_input.load(Ordering::Relaxed) + self.largest_output + self.cushion
    }

    /// Frames waiting in the ring.
    fn level(&self) -> usize {
        self.consumer.slots() / 2
    }

    /// Drops the oldest `frames` frames.
    fn drop_frames(&mut self, frames: usize) {
        if let Ok(chunk) = self.consumer.read_chunk(frames * 2) {
            chunk.commit_all();
        }
    }

    /// Call at the start of an output callback that will read
    /// `output_frames` frames: waits until the ring has filled to the
    /// target, and trims it back to the target if it has run well ahead.
    ///
    /// REAL-TIME SAFE.
    pub fn begin(&mut self, output_frames: usize) {
        self.largest_output = self.largest_output.max(output_frames);
        let target = self.target();
        self.stats.target_frames.store(target, Ordering::Relaxed);
        let level = self.level();
        if !self.primed {
            if level < target {
                return;
            }
            self.primed = true;
        }
        // The level naturally swings between about target - output and
        // target + input. Past that, the input is running ahead
        let ceiling = target + self.stats.largest_input.load(Ordering::Relaxed) + self.largest_output;
        if level > ceiling {
            let excess = level - target;
            self.drop_frames(excess);
            self.stats.overflow_frames.fetch_add(excess as u64, Ordering::Relaxed);
        }
    }

    /// Reads the next `left.len()` frames into `left` and `right` (the same
    /// length). Until the buffer has filled, or past the input it holds,
    /// the frames are silence; running dry counts as an underrun and waits
    /// for the buffer to refill.
    ///
    /// REAL-TIME SAFE.
    pub fn read(&mut self, left: &mut [f32], right: &mut [f32]) {
        let wanted = left.len().min(right.len());
        let mut got = 0;
        if self.primed {
            got = self.level().min(wanted);
            if let Ok(chunk) = self.consumer.read_chunk(got * 2) {
                let (first, second) = chunk.as_slices();
                let samples = first.iter().chain(second);
                for (i, &sample) in samples.enumerate() {
                    if i % 2 == 0 {
                        left[i / 2] = sample;
                    } else {
                        right[i / 2] = sample;
                    }
                }
                chunk.commit_all();
            }
            if got < wanted {
                self.stats.underrun_frames.fetch_add((wanted - got) as u64, Ordering::Relaxed);
                self.primed = false;
                self.cushion = (self.cushion + Self::CUSHION_STEP).min(Self::MAX_CUSHION);
            }
        }
        left[got..wanted].fill(0.0);
        right[got..wanted].fill(0.0);
    }

    /// Drops everything waiting, for a callback that won't read (the
    /// transport is stopped), and waits to refill before reading again.
    ///
    /// REAL-TIME SAFE.
    pub fn discard(&mut self) {
        let level = self.level();
        self.drop_frames(level);
        self.primed = false;
    }
}

/// The UI's view of an open input: its counters and health.
#[derive(Clone)]
pub struct InputMonitor {
    stats: Arc<InputStats>,
}

impl InputMonitor {
    /// Frames of silence played in place of late input, so far.
    pub fn underrun_frames(&self) -> u64 {
        self.stats.underrun_frames.load(Ordering::Relaxed)
    }

    /// Frames of input dropped, so far.
    pub fn overflow_frames(&self) -> u64 {
        self.stats.overflow_frames.load(Ordering::Relaxed)
    }

    /// The jitter buffer's level, in frames: the latency it adds on top of
    /// the devices' own.
    pub fn buffered_frames(&self) -> usize {
        self.stats.target_frames.load(Ordering::Relaxed)
    }

    /// Whether the input stream has reported an error.
    pub fn failed(&self) -> bool {
        self.stats.failed.load(Ordering::Relaxed)
    }

    /// Marks the input as failed, from the stream's error callback.
    pub fn mark_failed(&self) {
        self.stats.failed.store(true, Ordering::Relaxed);
    }
}

/// Creates the three ends of an input whose device runs at the output's
/// `sample_rate`.
pub fn input_channel(sample_rate: u32) -> (InputSender, InputFeed, InputMonitor) {
    input_channel_converting(sample_rate, sample_rate)
}

/// Creates the three ends of an input whose device runs at `input_rate`,
/// converted to the output's `output_rate` if they differ.
pub fn input_channel_converting(input_rate: u32, output_rate: u32) -> (InputSender, InputFeed, InputMonitor) {
    let frames = ((output_rate as f32 * RING_SECONDS) as usize).max(1024);
    let (producer, consumer) = RingBuffer::new(frames * 2);
    let stats = Arc::new(InputStats::default());
    let resampler = (input_rate != output_rate && input_rate > 0).then(|| Resampler::new(input_rate, output_rate));
    (
        InputSender { producer, stats: Arc::clone(&stats), resampler },
        InputFeed {
            consumer,
            stats: Arc::clone(&stats),
            primed: false,
            cushion: InputFeed::MIN_CUSHION,
            largest_output: 0,
        },
        InputMonitor { stats },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Interleaved stereo frames counting up from `start`: left n, right -n.
    fn ramp(start: usize, frames: usize) -> Vec<f32> {
        (start..start + frames).flat_map(|n| [n as f32, -(n as f32)]).collect()
    }

    /// Runs `seconds` of a 48 kHz input pushing `input` frames at a time
    /// against an output reading `output` frames at a time. The input clock
    /// runs `drift` times as fast as the output's. Returns everything read
    /// (left side), and the monitor.
    fn run(input: usize, output: usize, drift: f64, seconds: f64) -> (Vec<f32>, InputMonitor) {
        let (mut sender, mut feed, monitor) = input_channel(48000);
        let mut pushed = 0;
        let mut heard = Vec::new();
        let (mut left, mut right) = (vec![0.0; output], vec![0.0; output]);
        let total_reads = (seconds * 48000.0 / output as f64) as usize;
        let input_period = input as f64 / (48000.0 * drift);
        let mut next_input = 0.0;
        for read in 0..total_reads {
            let now = read as f64 * output as f64 / 48000.0;
            while next_input <= now {
                sender.push_f32(&ramp(pushed, input), 2);
                pushed += input;
                next_input += input_period;
            }
            feed.begin(output);
            feed.read(&mut left, &mut right);
            heard.extend_from_slice(&left);
        }
        (heard, monitor)
    }

    /// The first frame that isn't silence, and that the ramp runs on
    /// unbroken from there to the end.
    fn assert_unbroken(heard: &[f32]) -> usize {
        let start = heard.iter().position(|&s| s != 0.0).expect("input was heard");
        // The first frame pushed is 0, which reads as silence
        let first = heard[start] as usize;
        assert_eq!(first, 1, "reads start at the first frame");
        for (i, pair) in heard[start..].windows(2).enumerate() {
            assert_eq!(pair[1], pair[0] + 1.0, "ramp broke at frame {}", start + i);
        }
        start
    }

    #[test]
    fn test_steady_state_passes_every_frame_in_order() {
        // WASAPI-like input buffers against odd output buffers
        for (input, output) in [(480, 256), (441, 512), (128, 1024), (1024, 64)] {
            let (heard, monitor) = run(input, output, 1.0, 10.0);
            let start = assert_unbroken(&heard);
            assert_eq!(monitor.underrun_frames(), 0, "{input}/{output} ran dry");
            assert_eq!(monitor.overflow_frames(), 0, "{input}/{output} dropped input");
            // It waited for about one of each buffer before starting
            assert!(start <= input + output + 2 * output, "{input}/{output} started after {start} frames");
            assert!(monitor.buffered_frames() <= input + output + InputFeed::MIN_CUSHION);
        }
    }

    #[test]
    fn test_mono_input_is_heard_on_both_sides() {
        let (mut sender, mut feed, _) = input_channel(48000);
        // Several small buffers fill it past the target: one buffer of each
        // side, and the cushion
        for _ in 0..4 {
            sender.push_f32(&[0.5; 128], 1);
        }
        feed.begin(256);
        let (mut left, mut right) = (vec![0.0; 256], vec![0.0; 256]);
        feed.read(&mut left, &mut right);
        assert!(left.iter().all(|&s| s == 0.5));
        assert_eq!(left, right);
    }

    #[test]
    fn test_takes_the_first_two_channels_of_an_interface() {
        let (mut sender, mut feed, _) = input_channel(48000);
        let frames: Vec<f32> = (0..128).flat_map(|_| [0.25, -0.25, 9.0, 9.0]).collect();
        for _ in 0..4 {
            sender.push_f32(&frames, 4);
        }
        feed.begin(128);
        let (mut left, mut right) = (vec![0.0; 128], vec![0.0; 128]);
        feed.read(&mut left, &mut right);
        assert!(left.iter().all(|&s| s == 0.25));
        assert!(right.iter().all(|&s| s == -0.25));
    }

    #[test]
    fn test_converts_integer_samples() {
        let (mut sender, mut feed, _) = input_channel(48000);
        for _ in 0..4 {
            sender.push(&[i16::MIN; 128], 1, |s| s as f32 / 32768.0);
        }
        feed.begin(128);
        let (mut left, mut right) = (vec![0.0; 128], vec![0.0; 128]);
        feed.read(&mut left, &mut right);
        assert!(left.iter().all(|&s| s == -1.0));
    }

    #[test]
    fn test_underrun_fills_silence_counts_it_and_refills() {
        let (mut sender, mut feed, monitor) = input_channel(48000);
        let (mut left, mut right) = (vec![0.0; 256], vec![0.0; 256]);

        // 2 x 480 frames: enough to start, then the input goes quiet
        sender.push_f32(&ramp(1, 480), 2);
        sender.push_f32(&ramp(481, 480), 2);
        let mut heard = Vec::new();
        for _ in 0..5 {
            feed.begin(256);
            feed.read(&mut left, &mut right);
            heard.extend_from_slice(&left);
        }
        // All 960 frames, in order, then silence for the rest
        assert_eq!(&heard[..960], &ramp(1, 960).iter().step_by(2).copied().collect::<Vec<_>>()[..]);
        assert!(heard[960..].iter().all(|&s| s == 0.0));
        // Only the read that ran dry counts; the one after was waiting
        assert_eq!(monitor.underrun_frames(), (4 * 256 - 960) as u64);

        // Running dry raised the target, and nothing is read until the
        // ring is back up to it
        let target = 480 + 256 + InputFeed::MIN_CUSHION + InputFeed::CUSHION_STEP;
        sender.push_f32(&ramp(2000, 480), 2);
        feed.begin(256);
        assert_eq!(monitor.buffered_frames(), target);
        feed.read(&mut left, &mut right);
        assert!(left.iter().all(|&s| s == 0.0), "still refilling");
        assert_eq!(monitor.underrun_frames(), (4 * 256 - 960) as u64, "refilling isn't an underrun");

        sender.push_f32(&ramp(2480, 480), 2);
        feed.begin(256);
        feed.read(&mut left, &mut right);
        assert_eq!(left[0], 2000.0, "reading resumes where the input did");
    }

    #[test]
    fn test_a_slow_input_clock_runs_dry_now_and_then() {
        // Input 0.2% slow: about 96 frames short a second
        let (heard, monitor) = run(480, 256, 0.998, 20.0);
        assert!(monitor.underrun_frames() > 0);
        // Each underrun adds cushion, which is why they get rarer, but
        // they stay brief: well under 1% of the run is silence
        let silent = heard.iter().filter(|&&s| s == 0.0).count();
        assert!(silent < heard.len() / 100, "{silent} silent frames");
        assert_eq!(monitor.overflow_frames(), 0);
    }

    #[test]
    fn test_a_fast_input_clock_is_trimmed_back() {
        // Input 0.5% fast: 240 frames a second too many
        let (heard, monitor) = run(480, 256, 1.005, 20.0);
        let dropped = monitor.overflow_frames();
        assert!(dropped > 0, "the excess was trimmed");
        // Roughly the excess, not more
        assert!(dropped <= 20 * 240 + 2000, "dropped {dropped}");
        assert_eq!(monitor.underrun_frames(), 0);
        let jumps = heard.windows(2).filter(|w| w[1] != w[0] + 1.0).count();
        assert!(jumps > 0, "the trims show as jumps forward");
        assert!(heard.windows(2).all(|w| w[1] >= w[0]), "never backwards");
    }

    #[test]
    fn test_a_full_ring_drops_new_input_and_counts_it() {
        let (mut sender, _feed, monitor) = input_channel(48000);
        let capacity = 24000;
        sender.push_f32(&ramp(0, capacity), 2);
        assert_eq!(monitor.overflow_frames(), 0);
        sender.push_f32(&ramp(capacity, 500), 2);
        assert_eq!(monitor.overflow_frames(), 500);
    }

    #[test]
    fn test_an_output_stall_is_trimmed_to_the_newest_input() {
        let (mut sender, mut feed, monitor) = input_channel(48000);
        let (mut left, mut right) = (vec![0.0; 256], vec![0.0; 256]);
        sender.push_f32(&ramp(1, 480), 2);
        sender.push_f32(&ramp(481, 480), 2);
        feed.begin(256);
        feed.read(&mut left, &mut right);
        // The output stalls for 100 ms while input keeps coming
        for chunk in 0..10 {
            sender.push_f32(&ramp(961 + chunk * 480, 480), 2);
        }
        feed.begin(256);
        feed.read(&mut left, &mut right);
        let target = monitor.buffered_frames();
        assert_eq!(monitor.overflow_frames() as usize, 960 + 4800 - 256 - target);
        // What's read is the newest input, `target` frames behind
        assert_eq!(left[0] as usize, 960 + 4800 - target + 1);
    }

    /// A 1 kHz sine at `rate`, `seconds` long, at amplitude 0.5.
    fn sine(rate: u32, seconds: f64) -> Vec<f32> {
        (0..(seconds * rate as f64) as usize)
            .map(|n| 0.5 * (2.0 * std::f64::consts::PI * 1000.0 * n as f64 / rate as f64).sin() as f32)
            .collect()
    }

    /// Converts mono `input` (pushed `chunk` frames at a time) and returns
    /// the left side of everything that came out.
    fn convert(input: &[f32], input_rate: u32, output_rate: u32, chunk: usize) -> Vec<f32> {
        let (mut sender, mut feed, _) = input_channel_converting(input_rate, output_rate);
        let mut out = Vec::new();
        for piece in input.chunks(chunk) {
            sender.push_f32(piece, 1);
            let level = feed.level();
            let (mut left, mut right) = (vec![0.0; level], vec![0.0; level]);
            feed.primed = true;
            feed.read(&mut left, &mut right);
            out.extend_from_slice(&left);
        }
        out
    }

    #[test]
    fn test_converts_between_rates_keeping_pitch_and_level() {
        for (from, to) in [(48000, 44100), (44100, 48000), (96000, 48000), (16000, 48000)] {
            let out = convert(&sine(from, 1.0), from, to, 480);
            // One second in, one second out (the curve runs two frames behind)
            let expected = to as usize;
            assert!(out.len().abs_diff(expected) <= 3, "{from} -> {to}: {} frames", out.len());

            // Still 1 kHz: count upward zero crossings past the start-up
            let settled = &out[100..];
            let crossings = settled.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f64;
            let hz = crossings / (settled.len() as f64 / to as f64);
            assert!((hz - 1000.0).abs() < 2.0, "{from} -> {to}: {hz} Hz");

            // Still at 0.5, within a whisker
            let peak = settled.iter().fold(0.0f32, |p, s| p.max(s.abs()));
            assert!((peak - 0.5).abs() < 0.01, "{from} -> {to}: peak {peak}");
        }
    }

    #[test]
    fn test_conversion_is_clean() {
        // Compare 48 -> 44.1 kHz against the ideal sine at 44.1 kHz, lined
        // up for the converter's two-frame delay
        let out = convert(&sine(48000, 0.5), 48000, 44100, 480);
        let delay = 2.0 * 44100.0 / 48000.0;
        let error: f64 = out[200..out.len() - 10]
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                let t = (i as f64 + 200.0 - delay) / 44100.0;
                let ideal = 0.5 * (2.0 * std::f64::consts::PI * 1000.0 * t).sin();
                (s as f64 - ideal).powi(2)
            })
            .sum::<f64>()
            / (out.len() - 210) as f64;
        let snr_db = 10.0 * (0.125 / error).log10();
        assert!(snr_db > 60.0, "a 1 kHz tone converts {snr_db:.1} dB clean");
    }

    #[test]
    fn test_conversion_is_seamless_across_buffers() {
        let input = sine(48000, 0.2);
        let whole = convert(&input, 48000, 44100, input.len());
        for chunk in [1, 37, 480, 512] {
            assert_eq!(convert(&input, 48000, 44100, chunk), whole, "pushed {chunk} at a time");
        }
    }

    #[test]
    fn test_converted_input_runs_steady_against_the_output() {
        // A 48 kHz microphone feeding a 44.1 kHz output: 480-frame input
        // buffers every 10 ms against 441-frame output reads every 10 ms
        let (mut sender, mut feed, monitor) = input_channel_converting(48000, 44100);
        let (mut left, mut right) = (vec![0.0; 441], vec![0.0; 441]);
        let input = vec![0.25; 480];
        for _ in 0..1000 {
            sender.push_f32(&input, 1);
            feed.begin(441);
            feed.read(&mut left, &mut right);
        }
        assert_eq!(monitor.underrun_frames(), 0);
        assert_eq!(monitor.overflow_frames(), 0);
        assert!(left.iter().all(|&s| (s - 0.25).abs() < 1e-6));
        assert!(monitor.buffered_frames() < 441 * 3, "{} frames buffered", monitor.buffered_frames());
    }

    #[test]
    fn test_discard_empties_the_ring_and_waits_to_refill() {
        let (mut sender, mut feed, monitor) = input_channel(48000);
        sender.push_f32(&ramp(1, 2000), 2);
        feed.discard();
        sender.push_f32(&ramp(5000, 100), 2);
        feed.begin(256);
        let (mut left, mut right) = (vec![1.0; 256], vec![1.0; 256]);
        feed.read(&mut left, &mut right);
        assert!(left.iter().all(|&s| s == 0.0));
        assert_eq!(monitor.underrun_frames(), 0);
    }
}
