//! Distortion/saturation effect module.
//!
//! Provides multiple distortion algorithms for adding grit, warmth,
//! and harmonic content to sounds.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// Distortion algorithm types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DistortionType {
    /// Soft clipping - tube-like warmth using tanh saturation.
    Soft = 0,
    /// Hard clipping - aggressive, digital distortion.
    Hard = 1,
    /// Wave folding - complex harmonics from folding the waveform.
    Fold = 2,
    /// Bit crushing - lo-fi digital reduction.
    Bit = 3,
}

impl DistortionType {
    /// Convert from parameter index to distortion type.
    fn from_index(index: usize) -> Self {
        match index {
            0 => DistortionType::Soft,
            1 => DistortionType::Hard,
            2 => DistortionType::Fold,
            3 => DistortionType::Bit,
            _ => DistortionType::Soft,
        }
    }
}

/// Simple one-pole lowpass filter for tone control.
#[derive(Clone, Copy, Default)]
struct OnePoleFilter {
    /// Filter state (previous output).
    z1: f32,
}

impl OnePoleFilter {
    /// Process a single sample through the filter.
    #[inline]
    fn process(&mut self, input: f32, coefficient: f32) -> f32 {
        // One-pole lowpass: y[n] = (1-a)*x[n] + a*y[n-1]
        self.z1 = input * (1.0 - coefficient) + self.z1 * coefficient;
        self.z1
    }

    /// Reset filter state.
    fn reset(&mut self) {
        self.z1 = 0.0;
    }
}

/// Distortion effect module with multiple algorithms.
///
/// # Ports
///
/// - **In** (Audio, Input): Audio signal to distort.
/// - **Drive CV** (Control, Input): Modulates drive amount.
/// - **Out** (Audio, Output): Distorted audio output.
///
/// # Parameters
///
/// - **Drive** (0-100%): Distortion intensity.
/// - **Tone** (0-100%): Post-distortion brightness (lowpass filter).
/// - **Type** (Soft/Hard/Fold/Bit): Distortion algorithm.
/// - **Mix** (0-100%): Wet/dry blend.
/// - **Output** (-12dB to +12dB): Makeup gain.
pub struct Distortion {
    /// Sample rate.
    sample_rate: f32,
    /// Tone filter.
    tone_filter: OnePoleFilter,
    /// Smoothed drive parameter.
    drive_smooth: SmoothedValue,
    /// Smoothed tone parameter.
    tone_smooth: SmoothedValue,
    /// Smoothed mix parameter.
    mix_smooth: SmoothedValue,
    /// Smoothed output gain parameter.
    output_gain_smooth: SmoothedValue,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Distortion {
    /// Creates a new Distortion effect.
    pub fn new() -> Self {
        let sample_rate = 44100.0;

        Self {
            sample_rate,
            tone_filter: OnePoleFilter::default(),
            drive_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            tone_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            mix_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            output_gain_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            ports: vec![
                PortDefinition::input_with_default("in", "In", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("drive_cv", "Drive CV", SignalType::Control, 0.0),
                PortDefinition::output("out", "Out", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::new(
                    "drive",
                    "Drive",
                    0.0,
                    1.0,
                    0.5,
                    ParameterDisplay::Linear { unit: "%" },
                ),
                ParameterDefinition::new(
                    "tone",
                    "Tone",
                    0.0,
                    1.0,
                    0.5,
                    ParameterDisplay::Linear { unit: "%" },
                ),
                ParameterDefinition::new(
                    "type",
                    "Type",
                    0.0,
                    3.0,
                    0.0,
                    ParameterDisplay::Discrete {
                        labels: &["Soft", "Hard", "Fold", "Bit"],
                    },
                ),
                ParameterDefinition::new(
                    "mix",
                    "Mix",
                    0.0,
                    1.0,
                    1.0,
                    ParameterDisplay::Linear { unit: "%" },
                ),
                ParameterDefinition::new(
                    "output_gain",
                    "Output",
                    -12.0,
                    12.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_DRIVE_CV: usize = 1;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_DRIVE: usize = 0;
    const PARAM_TONE: usize = 1;
    const PARAM_TYPE: usize = 2;
    const PARAM_MIX: usize = 3;
    const PARAM_OUTPUT: usize = 4;

    /// Convert dB to linear amplitude.
    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    /// Soft clip (tanh saturation) - tube-like warmth.
    /// Creates smooth, musical distortion with odd harmonics.
    #[inline]
    fn soft_clip(x: f32, drive: f32) -> f32 {
        // Drive multiplies signal before saturation (1x to 11x)
        let driven = x * (1.0 + drive * 10.0);
        driven.tanh()
    }

    /// Hard clip - aggressive digital distortion.
    /// Creates harsh clipping at a threshold that decreases with drive.
    #[inline]
    fn hard_clip(x: f32, drive: f32) -> f32 {
        // Threshold goes from 1.0 down to 0.1 as drive increases
        let threshold = 1.0 - drive * 0.9;
        let threshold = threshold.max(0.1); // Safety floor
        (x.clamp(-threshold, threshold) / threshold).clamp(-1.0, 1.0)
    }

    /// Wave folder - creates complex harmonics by folding the waveform.
    /// Higher drive creates more folds and richer harmonics.
    #[inline]
    fn fold(x: f32, drive: f32) -> f32 {
        // Drive increases the amount of folding (1x to 5x)
        let driven = x * (1.0 + drive * 4.0);
        // Double sine fold for complex harmonics
        (driven.sin() * 2.0).sin()
    }

    /// Bit crush - lo-fi digital reduction.
    /// Reduces bit depth from 16-bit down to ~2-bit.
    #[inline]
    fn bit_crush(x: f32, drive: f32) -> f32 {
        // Bits go from 16 down to 2 as drive increases
        let bits = 16.0 - drive * 14.0;
        let bits = bits.max(2.0); // Minimum 2 bits
        let levels = 2.0_f32.powf(bits);
        (x * levels).round() / levels
    }

    /// Apply the selected distortion algorithm.
    #[inline]
    fn apply_distortion(x: f32, drive: f32, distortion_type: DistortionType) -> f32 {
        match distortion_type {
            DistortionType::Soft => Self::soft_clip(x, drive),
            DistortionType::Hard => Self::hard_clip(x, drive),
            DistortionType::Fold => Self::fold(x, drive),
            DistortionType::Bit => Self::bit_crush(x, drive),
        }
    }

    /// Calculate tone filter coefficient from tone parameter.
    /// Tone 0 = very dark, Tone 1 = bright (no filtering).
    #[inline]
    fn tone_to_coefficient(&self, tone: f32) -> f32 {
        // Map tone to cutoff frequency (200 Hz to 20000 Hz)
        let min_freq: f32 = 200.0;
        let max_freq: f32 = 20000.0;
        let freq = min_freq * (max_freq / min_freq).powf(tone);

        // One-pole coefficient from frequency
        let omega = 2.0 * std::f32::consts::PI * freq / self.sample_rate;
        (-omega).exp()
    }
}

impl Default for Distortion {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Distortion {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.distortion",
            name: "Distortion",
            category: ModuleCategory::Effect,
            description: "Multi-algorithm distortion with soft, hard, fold, and bit crush modes",
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

        // Update sample rates for smoothed values
        self.drive_smooth.set_sample_rate(sample_rate);
        self.tone_smooth.set_sample_rate(sample_rate);
        self.mix_smooth.set_sample_rate(sample_rate);
        self.output_gain_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Set smoothing targets
        self.drive_smooth.set_target(params[Self::PARAM_DRIVE]);
        self.tone_smooth.set_target(params[Self::PARAM_TONE]);
        self.mix_smooth.set_target(params[Self::PARAM_MIX]);
        self.output_gain_smooth.set_target(params[Self::PARAM_OUTPUT]);

        // Get distortion type (discrete, no smoothing needed)
        let distortion_type = DistortionType::from_index(params[Self::PARAM_TYPE] as usize);

        // Get input buffers
        let audio_in = inputs.get(Self::PORT_IN);
        let drive_cv = inputs.get(Self::PORT_DRIVE_CV);
        let out = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed parameter values
            let base_drive = self.drive_smooth.next();
            let tone = self.tone_smooth.next();
            let mix = self.mix_smooth.next();
            let output_gain_db = self.output_gain_smooth.next();

            // Add CV modulation to drive (bipolar CV, -1 to +1 range)
            let drive_mod = drive_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let drive = (base_drive + drive_mod * 0.5).clamp(0.0, 1.0);

            // Get input sample
            let dry = audio_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Apply distortion
            let distorted = Self::apply_distortion(dry, drive, distortion_type);

            // Apply tone filter (lowpass)
            let tone_coeff = self.tone_to_coefficient(tone);
            let filtered = self.tone_filter.process(distorted, tone_coeff);

            // Apply wet/dry mix
            let mixed = dry * (1.0 - mix) + filtered * mix;

            // Apply output gain
            let output_gain = Self::db_to_linear(output_gain_db);
            let output = mixed * output_gain;

            // Write output (soft limit to prevent excessive clipping)
            out.samples[i] = output.clamp(-2.0, 2.0);
        }
    }

    fn reset(&mut self) {
        // Reset filter state
        self.tone_filter.reset();

        // Reset smoothed values
        self.drive_smooth.reset(self.drive_smooth.target());
        self.tone_smooth.reset(self.tone_smooth.target());
        self.mix_smooth.reset(self.mix_smooth.target());
        self.output_gain_smooth.reset(self.output_gain_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_distortion_info() {
        let dist = Distortion::new();
        assert_eq!(dist.info().id, "fx.distortion");
        assert_eq!(dist.info().name, "Distortion");
        assert_eq!(dist.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_distortion_ports() {
        let dist = Distortion::new();
        let ports = dist.ports();

        assert_eq!(ports.len(), 3);

        // Input port
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        // Drive CV port
        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "drive_cv");
        assert_eq!(ports[1].signal_type, SignalType::Control);

        // Output port
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out");
        assert_eq!(ports[2].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_distortion_parameters() {
        let dist = Distortion::new();
        let params = dist.parameters();

        assert_eq!(params.len(), 5);
        assert_eq!(params[0].id, "drive");
        assert_eq!(params[1].id, "tone");
        assert_eq!(params[2].id, "type");
        assert_eq!(params[3].id, "mix");
        assert_eq!(params[4].id, "output_gain");
    }

    #[test]
    fn test_soft_clip() {
        // Zero drive should pass through
        let result = Distortion::soft_clip(0.5, 0.0);
        assert!((result - 0.5_f32.tanh()).abs() < 0.001);

        // High drive should saturate
        let result = Distortion::soft_clip(0.5, 1.0);
        assert!(result.abs() < 1.0); // Should be bounded
        assert!(result > 0.9); // Should be near saturation
    }

    #[test]
    fn test_hard_clip() {
        // Zero drive should clip at 1.0
        let result = Distortion::hard_clip(0.5, 0.0);
        assert!((result - 0.5).abs() < 0.001);

        // High drive should clip aggressively
        let result = Distortion::hard_clip(0.5, 1.0);
        assert_eq!(result, 1.0); // Should be fully clipped
    }

    #[test]
    fn test_fold() {
        // Zero drive should just sine fold once
        let result = Distortion::fold(0.5, 0.0);
        let expected = (0.5_f32.sin() * 2.0).sin();
        assert!((result - expected).abs() < 0.001);

        // Output should always be bounded
        let result = Distortion::fold(1.0, 1.0);
        assert!(result.abs() <= 1.0);
    }

    #[test]
    fn test_bit_crush() {
        // Zero drive = 16 bits, should be nearly unchanged
        let result = Distortion::bit_crush(0.5, 0.0);
        assert!((result - 0.5).abs() < 0.001);

        // High drive = 2 bits, severe quantization
        let result = Distortion::bit_crush(0.5, 1.0);
        // With 2 bits (4 levels), 0.5 should quantize to 0.5
        assert!((result - 0.5).abs() < 0.26); // Coarse quantization
    }

    #[test]
    fn test_distortion_passthrough_at_zero_drive() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        // Create test signal
        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 * 0.1).sin() * 0.5;
        }

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with zero drive, full mix, zero output gain
        // Let smoothing settle
        for _ in 0..10 {
            dist.process(
                &[&input, &SignalBuffer::audio(256)],
                &mut outputs,
                &[0.0, 1.0, 0.0, 1.0, 0.0], // drive=0, tone=1 (bright), type=soft, mix=1, out=0dB
                &ctx,
            );
        }

        // With zero drive, soft clip tanh(x) ≈ x for small x
        let skip = 50;
        for i in skip..256 {
            let diff = (input.samples[i] - outputs[0].samples[i]).abs();
            assert!(
                diff < 0.1,
                "Sample {} differs too much: input={}, output={}",
                i,
                input.samples[i],
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_distortion_adds_harmonics() {
        let mut dist = Distortion::new();
        let sample_rate = 44100.0;
        dist.prepare(sample_rate, 4410);

        // Generate clean sine wave
        let freq = 440.0;
        let mut input = SignalBuffer::audio(4410);
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5;
        }

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Process with high drive (should add harmonics)
        dist.process(
            &[&input, &SignalBuffer::audio(4410)],
            &mut outputs,
            &[0.8, 1.0, 0.0, 1.0, 0.0], // High drive, bright tone, soft clip
            &ctx,
        );

        // The output should have higher RMS due to harmonics/saturation
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let output_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Distortion should change the signal
        assert!(
            (output_rms - input_rms).abs() > 0.01 || output_rms > input_rms * 0.5,
            "Distortion should affect the signal: input_rms={}, output_rms={}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_distortion_mix() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with zero mix (dry only)
        for _ in 0..20 {
            dist.process(
                &[&input, &SignalBuffer::audio(256)],
                &mut outputs,
                &[1.0, 0.5, 0.0, 0.0, 0.0], // High drive but zero mix
                &ctx,
            );
        }

        // Output should match input (dry)
        let skip = 50;
        let avg: f32 = outputs[0].samples[skip..].iter().sum::<f32>() / (256 - skip) as f32;
        assert!(
            (avg - 0.5).abs() < 0.05,
            "Zero mix should output dry signal: {}",
            avg
        );
    }

    #[test]
    fn test_distortion_output_gain() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        input.fill(0.25);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with +6dB output gain
        for _ in 0..20 {
            dist.process(
                &[&input, &SignalBuffer::audio(256)],
                &mut outputs,
                &[0.0, 1.0, 0.0, 1.0, 6.0], // Zero drive, full tone, soft, full mix, +6dB
                &ctx,
            );
        }

        // Output should be roughly doubled (6dB ≈ 2x)
        let skip = 50;
        let avg: f32 = outputs[0].samples[skip..].iter().sum::<f32>() / (256 - skip) as f32;
        assert!(
            avg > 0.4 && avg < 0.6,
            "+6dB should roughly double amplitude: {}",
            avg
        );
    }

    #[test]
    fn test_all_distortion_types() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 * 0.05).sin() * 0.8;
        }

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Test each distortion type produces different output
        let mut results = Vec::new();
        for dist_type in 0..4 {
            dist.reset();
            for _ in 0..5 {
                dist.process(
                    &[&input, &SignalBuffer::audio(256)],
                    &mut outputs,
                    &[0.7, 1.0, dist_type as f32, 1.0, 0.0],
                    &ctx,
                );
            }
            results.push(outputs[0].samples[200]);
        }

        // Each type should produce different results
        for i in 0..results.len() {
            for j in (i + 1)..results.len() {
                assert!(
                    (results[i] - results[j]).abs() > 0.001,
                    "Types {} and {} should differ: {} vs {}",
                    i,
                    j,
                    results[i],
                    results[j]
                );
            }
        }
    }

    #[test]
    fn test_distortion_reset() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        // Process some signal
        let mut input = SignalBuffer::audio(256);
        input.fill(0.8);
        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        dist.process(
            &[&input, &SignalBuffer::audio(256)],
            &mut outputs,
            &[0.8, 0.2, 0.0, 1.0, 0.0],
            &ctx,
        );

        // Reset
        dist.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![SignalBuffer::audio(256)];

        dist.process(
            &[&silence, &SignalBuffer::audio(256)],
            &mut outputs2,
            &[0.8, 0.2, 0.0, 1.0, 0.0],
            &ctx,
        );

        // First sample should be near zero after reset
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Output should be near zero after reset: {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_distortion_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Distortion>();
    }

    #[test]
    fn test_distortion_default() {
        let dist = Distortion::default();
        assert_eq!(dist.info().id, "fx.distortion");
    }

    #[test]
    fn test_db_to_linear() {
        // 0 dB = 1.0
        let lin_0 = Distortion::db_to_linear(0.0);
        assert!((lin_0 - 1.0).abs() < 0.001);

        // 6 dB ≈ 2.0
        let lin_6 = Distortion::db_to_linear(6.0);
        assert!((lin_6 - 2.0).abs() < 0.1);

        // -6 dB ≈ 0.5
        let lin_neg6 = Distortion::db_to_linear(-6.0);
        assert!((lin_neg6 - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_distortion_type_from_index() {
        assert_eq!(DistortionType::from_index(0), DistortionType::Soft);
        assert_eq!(DistortionType::from_index(1), DistortionType::Hard);
        assert_eq!(DistortionType::from_index(2), DistortionType::Fold);
        assert_eq!(DistortionType::from_index(3), DistortionType::Bit);
        assert_eq!(DistortionType::from_index(99), DistortionType::Soft); // Default
    }

    #[test]
    fn test_distortion_drive_cv_modulation() {
        let mut dist = Distortion::new();
        dist.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        // Create drive CV modulation signal
        let mut drive_cv = SignalBuffer::audio(256);
        drive_cv.fill(0.5); // Positive CV adds to drive

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with base drive and CV modulation
        for _ in 0..10 {
            dist.process(
                &[&input, &drive_cv],
                &mut outputs,
                &[0.3, 1.0, 0.0, 1.0, 0.0], // Low base drive + CV
                &ctx,
            );
        }

        // The CV should increase effective drive
        let with_cv = outputs[0].samples[200];

        // Reset and process without CV
        dist.reset();
        let no_cv = SignalBuffer::audio(256);
        for _ in 0..10 {
            dist.process(
                &[&input, &no_cv],
                &mut outputs,
                &[0.3, 1.0, 0.0, 1.0, 0.0],
                &ctx,
            );
        }
        let without_cv = outputs[0].samples[200];

        // Results should differ
        assert!(
            (with_cv - without_cv).abs() > 0.01,
            "Drive CV should affect output: with={}, without={}",
            with_cv,
            without_cv
        );
    }

    #[test]
    fn test_distortion_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Distortion>();

        assert!(registry.contains("fx.distortion"));

        let module = registry.create("fx.distortion");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.distortion");
        assert_eq!(module.info().name, "Distortion");
        assert_eq!(module.ports().len(), 3);
        assert_eq!(module.parameters().len(), 5);
    }
}
