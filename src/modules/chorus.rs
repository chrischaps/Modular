//! Stereo Chorus effect module.
//!
//! A multi-voice chorus/flanger with stereo spread.
//! Uses LFO-modulated delay lines to create movement and width.

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

use std::f32::consts::TAU;

/// Maximum delay time in seconds (for buffer allocation).
const MAX_DELAY_SECONDS: f32 = 0.05; // 50ms max for chorus

/// Single chorus voice with its own LFO phase.
struct ChorusVoice {
    /// Delay line buffer.
    delay_line: Vec<f32>,
    /// Current write position in circular buffer.
    write_pos: usize,
    /// LFO phase (0.0 to 1.0).
    lfo_phase: f32,
    /// Stereo pan position (-1 = left, +1 = right).
    pan: f32,
}

impl ChorusVoice {
    fn new(max_samples: usize, initial_phase: f32, pan: f32) -> Self {
        Self {
            delay_line: vec![0.0; max_samples],
            write_pos: 0,
            lfo_phase: initial_phase,
            pan,
        }
    }

    /// Process one sample through this voice.
    /// Returns the delayed/modulated sample.
    fn process(
        &mut self,
        input: f32,
        rate_hz: f32,
        depth: f32,
        base_delay_samples: f32,
        feedback: f32,
        sample_rate: f32,
    ) -> f32 {
        // Advance LFO
        self.lfo_phase += rate_hz / sample_rate;
        if self.lfo_phase >= 1.0 {
            self.lfo_phase -= 1.0;
        }

        // Calculate LFO value (sine wave, -1 to +1)
        let lfo = (self.lfo_phase * TAU).sin();

        // Calculate modulated delay in samples
        // depth is 0-1, scales the modulation amount (up to +/- base_delay)
        let modulation = lfo * depth * base_delay_samples;
        let delay_samples = (base_delay_samples + modulation).max(1.0);

        // Read from delay line with linear interpolation
        let delayed = self.read_interpolated(delay_samples);

        // Write input + feedback to delay line
        let buffer_len = self.delay_line.len();
        self.delay_line[self.write_pos] = input + delayed * feedback;

        // Advance write position
        self.write_pos = (self.write_pos + 1) % buffer_len;

        delayed
    }

    /// Read from delay buffer with linear interpolation.
    #[inline]
    fn read_interpolated(&self, delay_samples: f32) -> f32 {
        let buffer_size = self.delay_line.len();
        let int_delay = delay_samples as usize;
        let frac = delay_samples - int_delay as f32;

        // Calculate read positions (circular buffer)
        let read_pos_1 = if self.write_pos >= int_delay {
            self.write_pos - int_delay
        } else {
            buffer_size - (int_delay - self.write_pos)
        };

        let read_pos_2 = if read_pos_1 == 0 {
            buffer_size - 1
        } else {
            read_pos_1 - 1
        };

        // Linear interpolation
        let sample_1 = self.delay_line[read_pos_1];
        let sample_2 = self.delay_line[read_pos_2];
        sample_1 + frac * (sample_2 - sample_1)
    }

    fn reset(&mut self) {
        self.delay_line.fill(0.0);
        self.write_pos = 0;
        // Keep LFO phase for continuity
    }
}

/// Stereo chorus effect with multiple voices and stereo spread.
///
/// # Ports
///
/// - **In L** (Audio, Input): Left channel input.
/// - **In R** (Audio, Input): Right channel input (normalled from L).
/// - **Rate CV** (Control, Input): Modulates LFO rate.
/// - **Depth CV** (Control, Input): Modulates modulation depth.
/// - **Out L** (Audio, Output): Processed left channel.
/// - **Out R** (Audio, Output): Processed right channel.
///
/// # Parameters
///
/// - **Rate** (0.1-10 Hz): LFO speed.
/// - **Depth** (0-100%): Modulation depth.
/// - **Delay** (1-30 ms): Base delay time.
/// - **Feedback** (-50% to +50%): For flanger effect.
/// - **Voices** (1-4): Number of chorus voices.
/// - **Mix** (0-100%): Wet/dry blend.
pub struct Chorus {
    /// Sample rate.
    sample_rate: f32,
    /// Chorus voices (up to 4).
    voices: Vec<ChorusVoice>,
    /// Smoothed rate parameter.
    rate_smooth: SmoothedValue,
    /// Smoothed depth parameter.
    depth_smooth: SmoothedValue,
    /// Smoothed delay parameter.
    delay_smooth: SmoothedValue,
    /// Smoothed feedback parameter.
    feedback_smooth: SmoothedValue,
    /// Smoothed mix parameter.
    mix_smooth: SmoothedValue,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Chorus {
    /// Creates a new stereo chorus.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let max_samples = (MAX_DELAY_SECONDS * sample_rate) as usize;

        // Create 4 voices with different LFO phases and pan positions
        // Phase spread: 0°, 90°, 180°, 270°
        // Pan spread: L, R, L, R (alternating)
        let voices = vec![
            ChorusVoice::new(max_samples, 0.0, -1.0),   // Voice 1: phase 0°, left
            ChorusVoice::new(max_samples, 0.25, 1.0),  // Voice 2: phase 90°, right
            ChorusVoice::new(max_samples, 0.5, -0.5),  // Voice 3: phase 180°, left-center
            ChorusVoice::new(max_samples, 0.75, 0.5),  // Voice 4: phase 270°, right-center
        ];

        Self {
            sample_rate,
            voices,
            rate_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            depth_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            delay_smooth: SmoothedValue::new(10.0, 20.0, sample_rate), // 20ms smoothing for delay
            feedback_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            mix_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in_l", "In L", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("in_r", "In R", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("rate_cv", "Rate CV", SignalType::Control, 0.0),
                PortDefinition::input_with_default("depth_cv", "Depth CV", SignalType::Control, 0.0),
                // Output ports
                PortDefinition::output("out_l", "Out L", SignalType::Audio),
                PortDefinition::output("out_r", "Out R", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::new(
                    "rate",
                    "Rate",
                    0.1,
                    10.0,
                    1.0,
                    ParameterDisplay::Logarithmic { unit: "Hz" },
                ),
                ParameterDefinition::normalized("depth", "Depth", 0.5),
                ParameterDefinition::new(
                    "delay",
                    "Delay",
                    1.0,
                    30.0,
                    10.0,
                    ParameterDisplay::Logarithmic { unit: "ms" },
                ),
                ParameterDefinition::new(
                    "feedback",
                    "Feedback",
                    -0.5,
                    0.5,
                    0.0,
                    ParameterDisplay::Linear { unit: "" },
                ),
                ParameterDefinition::choice(
                    "voices",
                    "Voices",
                    &["1", "2", "3", "4"],
                    1, // Default: 2 voices
                ),
                ParameterDefinition::normalized("mix", "Mix", 0.5),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN_L: usize = 0;
    const PORT_IN_R: usize = 1;
    const PORT_RATE_CV: usize = 2;
    const PORT_DEPTH_CV: usize = 3;
    const PORT_OUT_L: usize = 0;
    const PORT_OUT_R: usize = 1;

    /// Parameter index constants.
    const PARAM_RATE: usize = 0;
    const PARAM_DEPTH: usize = 1;
    const PARAM_DELAY: usize = 2;
    const PARAM_FEEDBACK: usize = 3;
    const PARAM_VOICES: usize = 4;
    const PARAM_MIX: usize = 5;
}

impl Default for Chorus {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Chorus {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.chorus",
            name: "Chorus",
            category: ModuleCategory::Effect,
            description: "Stereo chorus/flanger with multiple voices",
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

        // Resize voice buffers if needed
        let max_samples = (MAX_DELAY_SECONDS * sample_rate) as usize;
        for voice in &mut self.voices {
            if voice.delay_line.len() != max_samples {
                voice.delay_line.resize(max_samples, 0.0);
            }
        }

        // Update sample rate for smoothed values
        self.rate_smooth.set_sample_rate(sample_rate);
        self.depth_smooth.set_sample_rate(sample_rate);
        self.delay_smooth.set_sample_rate(sample_rate);
        self.feedback_smooth.set_sample_rate(sample_rate);
        self.mix_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Get parameter values
        let rate = params[Self::PARAM_RATE];
        let depth = params[Self::PARAM_DEPTH];
        let delay_ms = params[Self::PARAM_DELAY];
        let feedback = params[Self::PARAM_FEEDBACK];
        let num_voices = (params[Self::PARAM_VOICES] as usize + 1).clamp(1, 4);
        let mix = params[Self::PARAM_MIX];

        // Set smoothing targets
        self.rate_smooth.set_target(rate);
        self.depth_smooth.set_target(depth);
        self.delay_smooth.set_target(delay_ms);
        self.feedback_smooth.set_target(feedback);
        self.mix_smooth.set_target(mix);

        // Get input buffers
        let in_left = inputs.get(Self::PORT_IN_L);
        let in_right = inputs.get(Self::PORT_IN_R);
        let rate_cv = inputs.get(Self::PORT_RATE_CV);
        let depth_cv = inputs.get(Self::PORT_DEPTH_CV);

        // Split outputs
        let (out_left_slice, out_right_slice) = outputs.split_at_mut(1);
        let out_left = &mut out_left_slice[Self::PORT_OUT_L];
        let out_right = &mut out_right_slice[0];

        let max_delay_samples = (MAX_DELAY_SECONDS * self.sample_rate) as f32;

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed values
            let rate_smoothed = self.rate_smooth.next();
            let depth_smoothed = self.depth_smooth.next();
            let delay_ms_smoothed = self.delay_smooth.next();
            let feedback_smoothed = self.feedback_smooth.next();
            let mix_smoothed = self.mix_smooth.next();

            // Apply rate CV modulation (bipolar, +/- 50% range)
            let rate_mod = rate_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let modulated_rate = (rate_smoothed * (1.0 + rate_mod * 0.5)).clamp(0.1, 10.0);

            // Apply depth CV modulation
            let depth_mod = depth_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let modulated_depth = (depth_smoothed + depth_mod * 0.5).clamp(0.0, 1.0);

            // Convert delay to samples
            let base_delay_samples = (delay_ms_smoothed * 0.001 * self.sample_rate)
                .clamp(1.0, max_delay_samples - 1.0);

            // Get dry input samples
            let dry_left = in_left
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Right channel normalled from left if not connected/silent
            let dry_right = in_right
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .filter(|&s| s.abs() > 0.0001 || in_right.map(|b| !b.samples.is_empty()).unwrap_or(false))
                .unwrap_or(dry_left);

            // Process through active voices and accumulate wet signal
            let mut wet_left = 0.0;
            let mut wet_right = 0.0;

            for voice_idx in 0..num_voices {
                let voice = &mut self.voices[voice_idx];

                // Mix left and right inputs for this voice
                let input = (dry_left + dry_right) * 0.5;

                let wet = voice.process(
                    input,
                    modulated_rate,
                    modulated_depth,
                    base_delay_samples,
                    feedback_smoothed,
                    self.sample_rate,
                );

                // Apply stereo panning with equal power
                let pan_norm = (voice.pan + 1.0) * 0.5; // Convert -1..1 to 0..1
                let left_gain = (1.0 - pan_norm).sqrt();
                let right_gain = pan_norm.sqrt();

                wet_left += wet * left_gain;
                wet_right += wet * right_gain;
            }

            // Normalize wet signal by number of voices
            let voice_scale = 1.0 / (num_voices as f32).sqrt();
            wet_left *= voice_scale;
            wet_right *= voice_scale;

            // Mix dry and wet signals
            let out_l = dry_left * (1.0 - mix_smoothed) + wet_left * mix_smoothed;
            let out_r = dry_right * (1.0 - mix_smoothed) + wet_right * mix_smoothed;

            // Write outputs
            out_left.samples[i] = out_l;
            out_right.samples[i] = out_r;
        }
    }

    fn reset(&mut self) {
        // Reset all voices
        for voice in &mut self.voices {
            voice.reset();
        }

        // Reset smoothed values
        self.rate_smooth.reset(self.rate_smooth.target());
        self.depth_smooth.reset(self.depth_smooth.target());
        self.delay_smooth.reset(self.delay_smooth.target());
        self.feedback_smooth.reset(self.feedback_smooth.target());
        self.mix_smooth.reset(self.mix_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chorus_info() {
        let chorus = Chorus::new();
        assert_eq!(chorus.info().id, "fx.chorus");
        assert_eq!(chorus.info().name, "Chorus");
        assert_eq!(chorus.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_chorus_ports() {
        let chorus = Chorus::new();
        let ports = chorus.ports();

        assert_eq!(ports.len(), 6);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in_l");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "in_r");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "rate_cv");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        assert!(ports[3].is_input());
        assert_eq!(ports[3].id, "depth_cv");
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
    fn test_chorus_parameters() {
        let chorus = Chorus::new();
        let params = chorus.parameters();

        assert_eq!(params.len(), 6);
        assert_eq!(params[0].id, "rate");
        assert_eq!(params[1].id, "depth");
        assert_eq!(params[2].id, "delay");
        assert_eq!(params[3].id, "feedback");
        assert_eq!(params[4].id, "voices");
        assert_eq!(params[5].id, "mix");
    }

    #[test]
    fn test_chorus_produces_output() {
        let mut chorus = Chorus::new();
        chorus.prepare(44100.0, 256);

        // Create a constant input signal
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let empty_cv = SignalBuffer::control(256);
        let mut outputs = vec![
            SignalBuffer::audio(256), // Out L
            SignalBuffer::audio(256), // Out R
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with default settings: 1Hz rate, 50% depth, 10ms delay, 0 feedback, 2 voices, 50% mix
        chorus.process(
            &[&input, &input, &empty_cv, &empty_cv],
            &mut outputs,
            &[1.0, 0.5, 10.0, 0.0, 1.0, 0.5],
            &ctx,
        );

        // Output should have signal
        let has_output = outputs[0].samples.iter().any(|&s| s.abs() > 0.1);
        assert!(has_output, "Expected output signal");
    }

    #[test]
    fn test_chorus_stereo_spread() {
        let mut chorus = Chorus::new();
        let block_size = 1024;
        chorus.prepare(44100.0, block_size);

        // Create a test tone
        let mut input = SignalBuffer::audio(block_size);
        for (i, sample) in input.samples.iter_mut().enumerate() {
            *sample = (i as f32 * 0.1).sin();
        }

        let empty_cv = SignalBuffer::control(block_size);
        let mut outputs = vec![
            SignalBuffer::audio(block_size),
            SignalBuffer::audio(block_size),
        ];
        let ctx = ProcessContext::new(44100.0, block_size);

        // Process multiple blocks to let delay lines fill up
        for _ in 0..5 {
            chorus.process(
                &[&input, &input, &empty_cv, &empty_cv],
                &mut outputs,
                &[1.0, 0.5, 10.0, 0.0, 1.0, 1.0], // 100% wet
                &ctx,
            );
        }

        // Left and right should be different due to stereo spread
        let mut diff_count = 0;
        for i in 0..block_size {
            if (outputs[0].samples[i] - outputs[1].samples[i]).abs() > 0.001 {
                diff_count += 1;
            }
        }
        assert!(diff_count > 100, "Expected stereo difference, got {} diffs", diff_count);
    }

    #[test]
    fn test_chorus_reset() {
        let mut chorus = Chorus::new();
        chorus.prepare(44100.0, 256);

        // Fill buffer with signal
        let mut input = SignalBuffer::audio(256);
        input.fill(1.0);
        let empty_cv = SignalBuffer::control(256);
        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        chorus.process(
            &[&input, &input, &empty_cv, &empty_cv],
            &mut outputs,
            &[1.0, 0.5, 10.0, 0.5, 1.0, 1.0],
            &ctx,
        );

        // Reset
        chorus.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];

        chorus.process(
            &[&silence, &silence, &empty_cv, &empty_cv],
            &mut outputs2,
            &[1.0, 0.5, 10.0, 0.0, 1.0, 1.0], // No feedback
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
    fn test_chorus_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Chorus>();
    }

    #[test]
    fn test_chorus_default() {
        let chorus = Chorus::default();
        assert_eq!(chorus.info().id, "fx.chorus");
    }

    #[test]
    fn test_chorus_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Chorus>();

        assert!(registry.contains("fx.chorus"));

        let module = registry.create("fx.chorus");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.chorus");
        assert_eq!(module.info().name, "Chorus");
        assert_eq!(module.ports().len(), 6);
        assert_eq!(module.parameters().len(), 6);
    }
}
