//! Processing context for DSP modules.
//!
//! Provides runtime information that modules need during audio processing.

use super::signal::MidiEvent;

/// Transport state information for synchronization.
///
/// Provides tempo and playback state for modules that need to sync
/// to a timeline (e.g., tempo-synced LFOs, sequencers).
#[derive(Clone, Copy, Debug, Default)]
pub struct TransportState {
    /// Whether playback is currently active.
    pub playing: bool,
    /// Current position in samples from the start.
    pub sample_position: u64,
    /// Current tempo in beats per minute (if available).
    pub tempo_bpm: Option<f32>,
    /// Time signature numerator (beats per bar).
    pub time_sig_numerator: u8,
    /// Time signature denominator (beat unit).
    pub time_sig_denominator: u8,
}

impl TransportState {
    /// Creates a new transport state with default values.
    ///
    /// Defaults to stopped, at position 0, with no tempo and 4/4 time.
    pub fn new() -> Self {
        Self {
            playing: false,
            sample_position: 0,
            tempo_bpm: None,
            time_sig_numerator: 4,
            time_sig_denominator: 4,
        }
    }

    /// Creates a transport state that is playing at the given tempo.
    pub fn playing_at(tempo_bpm: f32) -> Self {
        Self {
            playing: true,
            sample_position: 0,
            tempo_bpm: Some(tempo_bpm),
            time_sig_numerator: 4,
            time_sig_denominator: 4,
        }
    }

    /// Returns the current position in beats (if tempo is known).
    pub fn position_in_beats(&self, sample_rate: f32) -> Option<f64> {
        self.tempo_bpm.map(|bpm| {
            let seconds = self.sample_position as f64 / sample_rate as f64;
            let beats_per_second = bpm as f64 / 60.0;
            seconds * beats_per_second
        })
    }

    /// Returns the current position in bars (if tempo is known).
    pub fn position_in_bars(&self, sample_rate: f32) -> Option<f64> {
        self.position_in_beats(sample_rate)
            .map(|beats| beats / self.time_sig_numerator as f64)
    }

    /// Returns the duration of one beat in samples (if tempo is known).
    pub fn samples_per_beat(&self, sample_rate: f32) -> Option<f32> {
        self.tempo_bpm
            .map(|bpm| sample_rate * 60.0 / bpm)
    }
}

/// Live audio from the input device (a microphone, guitar or line source)
/// for one block. Each side is `block_size` samples long, or empty when no
/// input is open, as in the offline renderer.
#[derive(Clone, Copy, Debug, Default)]
pub struct InputAudio<'a> {
    pub left: &'a [f32],
    pub right: &'a [f32],
}

impl<'a> InputAudio<'a> {
    /// The `frames` frames from `start`, or as many of them as there are.
    pub fn slice(&self, start: usize, frames: usize) -> InputAudio<'a> {
        let part = |side: &'a [f32]| {
            let start = start.min(side.len());
            &side[start..(start + frames).min(side.len())]
        };
        InputAudio { left: part(self.left), right: part(self.right) }
    }

    /// The sample at `index` on each side, or silence past the end.
    #[inline]
    pub fn frame(&self, index: usize) -> (f32, f32) {
        (
            self.left.get(index).copied().unwrap_or(0.0),
            self.right.get(index).copied().unwrap_or(0.0),
        )
    }
}

/// Context provided to modules during audio processing.
///
/// Contains all the runtime information a module needs to process audio,
/// including sample rate, buffer size, transport state, and the MIDI that
/// arrived for this block.
#[derive(Clone, Copy, Debug)]
pub struct ProcessContext<'a> {
    /// The audio sample rate in Hz (e.g., 44100, 48000).
    pub sample_rate: f32,
    /// The number of samples in the current processing block.
    pub block_size: usize,
    /// Current transport/timeline state.
    pub transport: TransportState,
    /// Incoming MIDI for this block, in time order. Each event's
    /// `sample_offset` is the sample within the block it lands on.
    pub midi: &'a [MidiEvent],
    /// Audio from the input device for this block.
    pub input: InputAudio<'a>,
}

impl<'a> ProcessContext<'a> {
    /// Creates a new process context.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            sample_rate,
            block_size,
            transport: TransportState::new(),
            midi: &[],
            input: InputAudio::default(),
        }
    }

    /// Creates a process context with transport information.
    pub fn with_transport(sample_rate: f32, block_size: usize, transport: TransportState) -> Self {
        Self {
            sample_rate,
            block_size,
            transport,
            midi: &[],
            input: InputAudio::default(),
        }
    }

    /// The same context, carrying `midi` as this block's MIDI input.
    pub fn with_midi<'b>(self, midi: &'b [MidiEvent]) -> ProcessContext<'b>
    where
        'a: 'b,
    {
        ProcessContext { midi, ..self }
    }

    /// The same context, carrying `input` as this block's audio input.
    pub fn with_input<'b>(self, input: InputAudio<'b>) -> ProcessContext<'b>
    where
        'a: 'b,
    {
        ProcessContext { input, ..self }
    }

    /// Returns the duration of the current block in seconds.
    pub fn block_duration(&self) -> f32 {
        self.block_size as f32 / self.sample_rate
    }

    /// Converts a duration in seconds to samples.
    pub fn seconds_to_samples(&self, seconds: f32) -> usize {
        (seconds * self.sample_rate).round() as usize
    }

    /// Converts a sample count to seconds.
    pub fn samples_to_seconds(&self, samples: usize) -> f32 {
        samples as f32 / self.sample_rate
    }

    /// Converts a frequency in Hz to radians per sample.
    ///
    /// Useful for oscillator phase increments.
    pub fn frequency_to_radians(&self, frequency: f32) -> f32 {
        2.0 * std::f32::consts::PI * frequency / self.sample_rate
    }

    /// Returns the Nyquist frequency (half the sample rate).
    pub fn nyquist(&self) -> f32 {
        self.sample_rate / 2.0
    }
}

impl Default for ProcessContext<'_> {
    fn default() -> Self {
        Self::new(44100.0, 256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_state_default() {
        let transport = TransportState::new();
        assert!(!transport.playing);
        assert_eq!(transport.sample_position, 0);
        assert_eq!(transport.tempo_bpm, None);
        assert_eq!(transport.time_sig_numerator, 4);
        assert_eq!(transport.time_sig_denominator, 4);
    }

    #[test]
    fn test_transport_state_playing() {
        let transport = TransportState::playing_at(120.0);
        assert!(transport.playing);
        assert_eq!(transport.tempo_bpm, Some(120.0));
    }

    #[test]
    fn test_transport_position_in_beats() {
        let mut transport = TransportState::playing_at(120.0); // 2 beats per second
        let sample_rate = 48000.0;

        // At sample 0, position is 0 beats
        assert_eq!(transport.position_in_beats(sample_rate), Some(0.0));

        // After 1 second (48000 samples) at 120 BPM, we should be at beat 2
        transport.sample_position = 48000;
        let beats = transport.position_in_beats(sample_rate).unwrap();
        assert!((beats - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_transport_samples_per_beat() {
        let transport = TransportState::playing_at(120.0);
        let sample_rate = 48000.0;

        // 120 BPM = 2 beats/sec, so 48000/2 = 24000 samples/beat
        let spb = transport.samples_per_beat(sample_rate).unwrap();
        assert!((spb - 24000.0).abs() < 0.1);
    }

    #[test]
    fn test_process_context_creation() {
        let ctx = ProcessContext::new(44100.0, 256);
        assert_eq!(ctx.sample_rate, 44100.0);
        assert_eq!(ctx.block_size, 256);
    }

    #[test]
    fn test_process_context_block_duration() {
        let ctx = ProcessContext::new(44100.0, 441);
        // 441 samples at 44100 Hz = 0.01 seconds = 10ms
        assert!((ctx.block_duration() - 0.01).abs() < 0.0001);
    }

    #[test]
    fn test_process_context_time_conversions() {
        let ctx = ProcessContext::new(48000.0, 256);

        // 1 second = 48000 samples
        assert_eq!(ctx.seconds_to_samples(1.0), 48000);

        // 48000 samples = 1 second
        assert!((ctx.samples_to_seconds(48000) - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_process_context_frequency_to_radians() {
        let ctx = ProcessContext::new(48000.0, 256);

        // 1 Hz = 2π/48000 radians per sample
        let rad = ctx.frequency_to_radians(1.0);
        let expected = 2.0 * std::f32::consts::PI / 48000.0;
        assert!((rad - expected).abs() < 0.0000001);
    }

    #[test]
    fn test_process_context_nyquist() {
        let ctx = ProcessContext::new(44100.0, 256);
        assert_eq!(ctx.nyquist(), 22050.0);
    }

    #[test]
    fn test_process_context_default() {
        let ctx = ProcessContext::default();
        assert_eq!(ctx.sample_rate, 44100.0);
        assert_eq!(ctx.block_size, 256);
    }
}
