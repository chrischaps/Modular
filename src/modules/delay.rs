//! Stereo Delay effect module.
//!
//! A versatile delay with feedback, filtering, and ping-pong mode.
//! Features tempo sync options and smooth parameter changes.

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// Maximum delay time in seconds.
const MAX_DELAY_SECONDS: f32 = 2.0;

/// Stereo delay effect with feedback, filtering, and ping-pong mode.
///
/// # Ports
///
/// - **In L** (Audio, Input): Left channel input.
/// - **In R** (Audio, Input): Right channel input (normalled from L).
/// - **Time CV** (Control, Input): Modulates delay time.
/// - **Feedback CV** (Control, Input): Modulates feedback amount.
/// - **Out L** (Audio, Output): Processed left channel.
/// - **Out R** (Audio, Output): Processed right channel.
///
/// # Parameters
///
/// - **Time** (1-2000 ms): Delay time.
/// - **Feedback** (0-100%): Amount of output fed back to input.
/// - **Mix** (0-100%): Wet/dry balance.
/// - **High Cut** (100-20000 Hz): Lowpass filter in feedback path.
/// - **Low Cut** (20-2000 Hz): Highpass filter in feedback path.
/// - **Ping-Pong** (toggle): Alternates repeats between channels.
/// - **Sync** (choice): Tempo sync division.
pub struct StereoDelay {
    /// Sample rate.
    sample_rate: f32,
    /// Left channel delay buffer.
    buffer_left: Vec<f32>,
    /// Right channel delay buffer.
    buffer_right: Vec<f32>,
    /// Write position in the circular buffer.
    write_pos: usize,
    /// Smoothed delay time in samples.
    time_smooth: SmoothedValue,
    /// Smoothed feedback amount.
    feedback_smooth: SmoothedValue,
    /// Smoothed wet/dry mix.
    mix_smooth: SmoothedValue,
    /// Smoothed high cut frequency.
    high_cut_smooth: SmoothedValue,
    /// Smoothed low cut frequency.
    low_cut_smooth: SmoothedValue,
    /// High cut filter state (left).
    high_cut_state_l: f32,
    /// High cut filter state (right).
    high_cut_state_r: f32,
    /// Low cut filter state (left).
    low_cut_state_l: f32,
    /// Low cut filter state (right).
    low_cut_state_r: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl StereoDelay {
    /// Creates a new stereo delay.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let max_samples = (MAX_DELAY_SECONDS * sample_rate) as usize;

        Self {
            sample_rate,
            buffer_left: vec![0.0; max_samples],
            buffer_right: vec![0.0; max_samples],
            write_pos: 0,
            time_smooth: SmoothedValue::new(500.0, 50.0, sample_rate), // 50ms smoothing for time
            feedback_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            mix_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            high_cut_smooth: SmoothedValue::with_default_smoothing(10000.0, sample_rate),
            low_cut_smooth: SmoothedValue::with_default_smoothing(20.0, sample_rate),
            high_cut_state_l: 0.0,
            high_cut_state_r: 0.0,
            low_cut_state_l: 0.0,
            low_cut_state_r: 0.0,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in_l", "In L", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("in_r", "In R", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("time_cv", "Time CV", SignalType::Control, 0.0),
                PortDefinition::input_with_default("feedback_cv", "Feedback CV", SignalType::Control, 0.0),
                // Output ports
                PortDefinition::output("out_l", "Out L", SignalType::Audio),
                PortDefinition::output("out_r", "Out R", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::new(
                    "time",
                    "Time",
                    1.0,
                    2000.0,
                    500.0,
                    ParameterDisplay::Logarithmic { unit: "ms" },
                ),
                ParameterDefinition::normalized("feedback", "Feedback", 0.5),
                ParameterDefinition::normalized("mix", "Mix", 0.5),
                ParameterDefinition::frequency("high_cut", "High Cut", 100.0, 20000.0, 10000.0),
                ParameterDefinition::frequency("low_cut", "Low Cut", 20.0, 2000.0, 20.0),
                ParameterDefinition::toggle("ping_pong", "Ping-Pong", false),
                ParameterDefinition::choice(
                    "sync",
                    "Sync",
                    &["Off", "1/4", "1/8", "1/8T", "1/16", "1/16T", "1/32"],
                    0,
                ),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN_L: usize = 0;
    const PORT_IN_R: usize = 1;
    const PORT_TIME_CV: usize = 2;
    const PORT_FEEDBACK_CV: usize = 3;
    const PORT_OUT_L: usize = 0;
    const PORT_OUT_R: usize = 1;

    /// Parameter index constants.
    const PARAM_TIME: usize = 0;
    const PARAM_FEEDBACK: usize = 1;
    const PARAM_MIX: usize = 2;
    const PARAM_HIGH_CUT: usize = 3;
    const PARAM_LOW_CUT: usize = 4;
    const PARAM_PING_PONG: usize = 5;
    const PARAM_SYNC: usize = 6;

    /// Reads from the delay buffer with linear interpolation.
    #[inline]
    fn read_interpolated(buffer: &[f32], write_pos: usize, delay_samples: f32) -> f32 {
        let buffer_size = buffer.len();
        let int_delay = delay_samples as usize;
        let frac = delay_samples - int_delay as f32;

        // Calculate read positions (circular buffer)
        let read_pos_1 = if write_pos >= int_delay {
            write_pos - int_delay
        } else {
            buffer_size - (int_delay - write_pos)
        };

        let read_pos_2 = if read_pos_1 == 0 {
            buffer_size - 1
        } else {
            read_pos_1 - 1
        };

        // Linear interpolation
        let sample_1 = buffer[read_pos_1];
        let sample_2 = buffer[read_pos_2];
        sample_1 + frac * (sample_2 - sample_1)
    }

    /// Simple one-pole lowpass filter coefficient.
    #[inline]
    fn lowpass_coeff(cutoff: f32, sample_rate: f32) -> f32 {
        let cutoff_clamped = cutoff.clamp(20.0, sample_rate * 0.45);
        let tan = (std::f32::consts::PI * cutoff_clamped / sample_rate).tan();
        tan / (1.0 + tan)
    }

    /// Simple one-pole highpass filter coefficient.
    #[inline]
    fn highpass_coeff(cutoff: f32, sample_rate: f32) -> f32 {
        let cutoff_clamped = cutoff.clamp(20.0, sample_rate * 0.45);
        let tan = (std::f32::consts::PI * cutoff_clamped / sample_rate).tan();
        1.0 / (1.0 + tan)
    }

    /// Soft clip to prevent runaway feedback.
    #[inline]
    fn soft_clip(x: f32) -> f32 {
        x.tanh()
    }

    /// Get sync division multiplier in beats (quarter notes).
    fn sync_to_beats(sync_index: usize) -> Option<f32> {
        match sync_index {
            0 => None,           // Off
            1 => Some(1.0),      // 1/4 = 1 beat
            2 => Some(0.5),      // 1/8 = 0.5 beats
            3 => Some(1.0 / 3.0), // 1/8T = triplet
            4 => Some(0.25),     // 1/16 = 0.25 beats
            5 => Some(1.0 / 6.0), // 1/16T = triplet
            6 => Some(0.125),    // 1/32 = 0.125 beats
            _ => None,
        }
    }
}

impl Default for StereoDelay {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for StereoDelay {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.delay",
            name: "Stereo Delay",
            category: ModuleCategory::Effect,
            description: "Stereo delay with feedback, filtering, and ping-pong mode",
        };
        &INFO
    }

    fn ports(&self) -> &[PortDefinition] {
        &self.ports
    }

    fn parameters(&self) -> &[ParameterDefinition] {
        &self.parameters
    }

    fn prepare(&mut self, sample_rate: f32, _max_block_size: usize) {
        self.sample_rate = sample_rate;

        // Resize buffers if needed
        let max_samples = (MAX_DELAY_SECONDS * sample_rate) as usize;
        if self.buffer_left.len() != max_samples {
            self.buffer_left.resize(max_samples, 0.0);
            self.buffer_right.resize(max_samples, 0.0);
        }

        // Update sample rate for smoothed values
        self.time_smooth.set_sample_rate(sample_rate);
        self.feedback_smooth.set_sample_rate(sample_rate);
        self.mix_smooth.set_sample_rate(sample_rate);
        self.high_cut_smooth.set_sample_rate(sample_rate);
        self.low_cut_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Get parameter values
        let time_ms = params[Self::PARAM_TIME];
        let feedback = params[Self::PARAM_FEEDBACK];
        let mix = params[Self::PARAM_MIX];
        let high_cut = params[Self::PARAM_HIGH_CUT];
        let low_cut = params[Self::PARAM_LOW_CUT];
        let ping_pong = params[Self::PARAM_PING_PONG] > 0.5;
        let sync_index = params[Self::PARAM_SYNC] as usize;

        // Calculate delay time (either from sync or direct)
        let base_time_ms = if let Some(beats) = Self::sync_to_beats(sync_index) {
            // Use tempo from context if available, otherwise assume 120 BPM
            let bpm = context.transport.tempo_bpm.unwrap_or(120.0);
            let ms_per_beat = 60000.0 / bpm;
            (beats * ms_per_beat).clamp(1.0, 2000.0)
        } else {
            time_ms
        };

        // Set smoothing targets
        self.time_smooth.set_target(base_time_ms);
        self.feedback_smooth.set_target(feedback);
        self.mix_smooth.set_target(mix);
        self.high_cut_smooth.set_target(high_cut);
        self.low_cut_smooth.set_target(low_cut);

        // Get input buffers
        let in_left = inputs.get(Self::PORT_IN_L);
        let in_right = inputs.get(Self::PORT_IN_R);
        let time_cv = inputs.get(Self::PORT_TIME_CV);
        let feedback_cv = inputs.get(Self::PORT_FEEDBACK_CV);

        // Split outputs
        let (out_left_slice, out_right_slice) = outputs.split_at_mut(1);
        let out_left = &mut out_left_slice[Self::PORT_OUT_L];
        let out_right = &mut out_right_slice[0];

        let buffer_size = self.buffer_left.len();

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed values
            let time_ms_smoothed = self.time_smooth.next();
            let feedback_smoothed = self.feedback_smooth.next();
            let mix_smoothed = self.mix_smooth.next();
            let high_cut_smoothed = self.high_cut_smooth.next();
            let low_cut_smoothed = self.low_cut_smooth.next();

            // Apply time CV modulation (bipolar, +/- 50% range)
            let time_mod = time_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let modulated_time_ms = (time_ms_smoothed * (1.0 + time_mod * 0.5)).clamp(1.0, 2000.0);

            // Apply feedback CV modulation
            let fb_mod = feedback_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let modulated_feedback = (feedback_smoothed + fb_mod * 0.5).clamp(0.0, 0.95);

            // Convert time to samples
            let delay_samples = (modulated_time_ms * 0.001 * self.sample_rate)
                .clamp(1.0, (buffer_size - 1) as f32);

            // Get dry input samples
            let dry_left = in_left
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Right channel is normalled from left if not connected
            let dry_right = in_right
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .filter(|&s| s.abs() > 0.0001 || in_right.map(|b| !b.samples.is_empty()).unwrap_or(false))
                .unwrap_or(dry_left);

            // Read delayed samples
            let wet_left = Self::read_interpolated(&self.buffer_left, self.write_pos, delay_samples);
            let wet_right = Self::read_interpolated(&self.buffer_right, self.write_pos, delay_samples);

            // Calculate filter coefficients
            let lp_coeff = Self::lowpass_coeff(high_cut_smoothed, self.sample_rate);
            let hp_coeff = Self::highpass_coeff(low_cut_smoothed, self.sample_rate);

            // Apply lowpass to feedback (high cut)
            self.high_cut_state_l += lp_coeff * (wet_left - self.high_cut_state_l);
            self.high_cut_state_r += lp_coeff * (wet_right - self.high_cut_state_r);

            let filtered_left = self.high_cut_state_l;
            let filtered_right = self.high_cut_state_r;

            // Apply highpass to feedback (low cut)
            let hp_filtered_left = hp_coeff * (filtered_left - self.low_cut_state_l);
            self.low_cut_state_l = filtered_left - hp_filtered_left;

            let hp_filtered_right = hp_coeff * (filtered_right - self.low_cut_state_r);
            self.low_cut_state_r = filtered_right - hp_filtered_right;

            // Calculate feedback signals
            let (feedback_left, feedback_right) = if ping_pong {
                // Ping-pong: cross-feed channels
                (
                    Self::soft_clip(hp_filtered_right * modulated_feedback),
                    Self::soft_clip(hp_filtered_left * modulated_feedback),
                )
            } else {
                // Normal: same-channel feedback
                (
                    Self::soft_clip(hp_filtered_left * modulated_feedback),
                    Self::soft_clip(hp_filtered_right * modulated_feedback),
                )
            };

            // Write to delay buffer (input + feedback)
            self.buffer_left[self.write_pos] = dry_left + feedback_left;
            self.buffer_right[self.write_pos] = dry_right + feedback_right;

            // Advance write position
            self.write_pos = (self.write_pos + 1) % buffer_size;

            // Mix dry and wet signals
            let out_l = dry_left * (1.0 - mix_smoothed) + wet_left * mix_smoothed;
            let out_r = dry_right * (1.0 - mix_smoothed) + wet_right * mix_smoothed;

            // Write outputs
            out_left.samples[i] = out_l;
            out_right.samples[i] = out_r;
        }
    }

    fn reset(&mut self) {
        // Clear delay buffers
        self.buffer_left.fill(0.0);
        self.buffer_right.fill(0.0);
        self.write_pos = 0;

        // Clear filter states
        self.high_cut_state_l = 0.0;
        self.high_cut_state_r = 0.0;
        self.low_cut_state_l = 0.0;
        self.low_cut_state_r = 0.0;

        // Reset smoothed values
        self.time_smooth.reset(self.time_smooth.target());
        self.feedback_smooth.reset(self.feedback_smooth.target());
        self.mix_smooth.reset(self.mix_smooth.target());
        self.high_cut_smooth.reset(self.high_cut_smooth.target());
        self.low_cut_smooth.reset(self.low_cut_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_info() {
        let delay = StereoDelay::new();
        assert_eq!(delay.info().id, "fx.delay");
        assert_eq!(delay.info().name, "Stereo Delay");
        assert_eq!(delay.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_delay_ports() {
        let delay = StereoDelay::new();
        let ports = delay.ports();

        assert_eq!(ports.len(), 6);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in_l");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "in_r");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "time_cv");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        assert!(ports[3].is_input());
        assert_eq!(ports[3].id, "feedback_cv");
        assert_eq!(ports[3].signal_type, SignalType::Control);

        // Output ports
        assert!(ports[4].is_output());
        assert_eq!(ports[4].id, "out_l");
        assert_eq!(ports[4].signal_type, SignalType::Audio);

        assert!(ports[5].is_output());
        assert_eq!(ports[5].id, "out_r");
        assert_eq!(ports[5].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_delay_parameters() {
        let delay = StereoDelay::new();
        let params = delay.parameters();

        assert_eq!(params.len(), 7);
        assert_eq!(params[0].id, "time");
        assert_eq!(params[1].id, "feedback");
        assert_eq!(params[2].id, "mix");
        assert_eq!(params[3].id, "high_cut");
        assert_eq!(params[4].id, "low_cut");
        assert_eq!(params[5].id, "ping_pong");
        assert_eq!(params[6].id, "sync");
    }

    #[test]
    fn test_delay_produces_output() {
        let mut delay = StereoDelay::new();
        delay.prepare(44100.0, 256);

        // Create a constant input signal
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let empty_cv = SignalBuffer::control(256);
        let mut outputs = vec![
            SignalBuffer::audio(256), // Out L
            SignalBuffer::audio(256), // Out R
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with 10ms delay, 0% feedback, 50% wet (dry/wet mix)
        delay.process(
            &[&input, &input, &empty_cv, &empty_cv],
            &mut outputs,
            &[10.0, 0.0, 0.5, 10000.0, 20.0, 0.0, 0.0],
            &ctx,
        );

        // Output should have signal (at least 50% dry pass-through)
        let has_output = outputs[0].samples.iter().any(|&s| s.abs() > 0.1);
        assert!(has_output, "Expected output signal");
    }

    #[test]
    fn test_delay_feedback() {
        let mut delay = StereoDelay::new();
        let sample_rate = 44100.0;
        let block_size = 4410;
        delay.prepare(sample_rate, block_size);

        // Use 500ms delay (the default smoothing start point) for reliable timing
        let delay_ms = 500.0;
        let delay_samples = (delay_ms * 0.001 * sample_rate) as usize;

        // First, warm up the delay with silence to let parameters settle
        let silence = SignalBuffer::audio(block_size);
        let empty_cv = SignalBuffer::control(block_size);
        let mut outputs = vec![
            SignalBuffer::audio(block_size),
            SignalBuffer::audio(block_size),
        ];
        let ctx = ProcessContext::new(sample_rate, block_size);

        // Process silence for a few blocks to let smoothing settle
        for _ in 0..10 {
            delay.process(
                &[&silence, &silence, &empty_cv, &empty_cv],
                &mut outputs,
                &[delay_ms, 0.5, 1.0, 20000.0, 20.0, 0.0, 0.0],
                &ctx,
            );
        }

        // Now send an impulse
        let mut input = SignalBuffer::audio(block_size);
        input.samples[0] = 1.0;

        delay.process(
            &[&input, &input, &empty_cv, &empty_cv],
            &mut outputs,
            &[delay_ms, 0.5, 1.0, 20000.0, 20.0, 0.0, 0.0],
            &ctx,
        );

        // Process more blocks until we pass the delay time
        // 500ms = 22050 samples, we need about 5 blocks of 4410 samples
        let mut all_outputs: Vec<f32> = outputs[0].samples.clone();
        for _ in 0..5 {
            delay.process(
                &[&silence, &silence, &empty_cv, &empty_cv],
                &mut outputs,
                &[delay_ms, 0.5, 1.0, 20000.0, 20.0, 0.0, 0.0],
                &ctx,
            );
            all_outputs.extend_from_slice(&outputs[0].samples);
        }

        // Look for the delayed signal somewhere in the output
        // Should appear around sample delay_samples (22050)
        let search_start = delay_samples.saturating_sub(1000);
        let search_end = (delay_samples + 1000).min(all_outputs.len());

        let mut found_signal = false;
        for i in search_start..search_end {
            if all_outputs[i].abs() > 0.1 {
                found_signal = true;
                break;
            }
        }
        assert!(
            found_signal,
            "Expected delayed signal around sample {} (out of {})",
            delay_samples,
            all_outputs.len()
        );
    }

    #[test]
    fn test_delay_reset() {
        let mut delay = StereoDelay::new();
        delay.prepare(44100.0, 256);

        // Fill buffer with some signal
        let mut input = SignalBuffer::audio(256);
        input.fill(1.0);
        let empty_cv = SignalBuffer::control(256);
        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        delay.process(
            &[&input, &input, &empty_cv, &empty_cv],
            &mut outputs,
            &[100.0, 0.5, 0.5, 10000.0, 20.0, 0.0, 0.0],
            &ctx,
        );

        // Reset
        delay.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];

        delay.process(
            &[&silence, &silence, &empty_cv, &empty_cv],
            &mut outputs2,
            &[100.0, 0.5, 1.0, 10000.0, 20.0, 0.0, 0.0],
            &ctx,
        );

        // Output should be near zero (buffers cleared)
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Expected near-zero output after reset, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_delay_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<StereoDelay>();
    }

    #[test]
    fn test_delay_default() {
        let delay = StereoDelay::default();
        assert_eq!(delay.info().id, "fx.delay");
    }

    #[test]
    fn test_sync_to_beats() {
        assert!(StereoDelay::sync_to_beats(0).is_none()); // Off
        assert_eq!(StereoDelay::sync_to_beats(1), Some(1.0)); // 1/4
        assert_eq!(StereoDelay::sync_to_beats(2), Some(0.5)); // 1/8
        assert_eq!(StereoDelay::sync_to_beats(4), Some(0.25)); // 1/16
        assert_eq!(StereoDelay::sync_to_beats(6), Some(0.125)); // 1/32
    }

    #[test]
    fn test_delay_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<StereoDelay>();

        assert!(registry.contains("fx.delay"));

        let module = registry.create("fx.delay");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.delay");
        assert_eq!(module.info().name, "Stereo Delay");
        assert_eq!(module.ports().len(), 6);
        assert_eq!(module.parameters().len(), 7);
    }
}
