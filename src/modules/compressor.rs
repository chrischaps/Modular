//! Dynamics Compressor effect module.
//!
//! A dynamics compressor for controlling dynamic range with adjustable
//! threshold, ratio, attack, release, knee, makeup gain, and mix controls.
//! Includes optional sidechain input and gain reduction output for metering.

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// Envelope follower for level detection.
struct EnvelopeFollower {
    /// Current envelope level.
    level: f32,
    /// Sample rate for coefficient calculation.
    sample_rate: f32,
}

impl EnvelopeFollower {
    fn new(sample_rate: f32) -> Self {
        Self {
            level: 0.0,
            sample_rate,
        }
    }

    /// Process a sample and return the envelope level.
    /// Uses peak detection with separate attack/release times.
    #[inline]
    fn process(&mut self, input: f32, attack_ms: f32, release_ms: f32) -> f32 {
        let input_abs = input.abs();

        // Calculate coefficients from time constants
        // Using standard exponential envelope follower formula
        let attack_coeff = (-1.0 / (attack_ms * 0.001 * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (release_ms * 0.001 * self.sample_rate)).exp();

        // Choose coefficient based on whether signal is above or below current level
        if input_abs > self.level {
            // Attack: input is louder than current level
            self.level = attack_coeff * self.level + (1.0 - attack_coeff) * input_abs;
        } else {
            // Release: input is quieter than current level
            self.level = release_coeff * self.level + (1.0 - release_coeff) * input_abs;
        }

        self.level
    }

    fn reset(&mut self) {
        self.level = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
}

/// Dynamics compressor with sidechain support and gain reduction output.
///
/// # Ports
///
/// - **In** (Audio, Input): Main audio input signal.
/// - **Sidechain** (Audio, Input): External sidechain input (optional, normalled from In).
/// - **Out** (Audio, Output): Compressed audio output.
/// - **GR** (Control, Output): Gain reduction amount (0 to 1, for metering).
///
/// # Parameters
///
/// - **Threshold** (-60dB to 0dB): Level where compression starts.
/// - **Ratio** (1:1 to 20:1): Amount of compression.
/// - **Attack** (0.1ms to 100ms): How fast compression engages.
/// - **Release** (10ms to 1000ms): How fast compression releases.
/// - **Knee** (0dB to 12dB): Soft/hard knee width.
/// - **Makeup** (0dB to +24dB): Output level boost.
/// - **Mix** (0% to 100%): Parallel compression blend.
pub struct Compressor {
    /// Sample rate.
    sample_rate: f32,
    /// Envelope follower for level detection.
    envelope: EnvelopeFollower,
    /// Smoothed threshold parameter.
    threshold_smooth: SmoothedValue,
    /// Smoothed ratio parameter.
    ratio_smooth: SmoothedValue,
    /// Smoothed attack parameter.
    attack_smooth: SmoothedValue,
    /// Smoothed release parameter.
    release_smooth: SmoothedValue,
    /// Smoothed knee parameter.
    knee_smooth: SmoothedValue,
    /// Smoothed makeup gain parameter.
    makeup_smooth: SmoothedValue,
    /// Smoothed mix parameter.
    mix_smooth: SmoothedValue,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Compressor {
    /// Creates a new dynamics compressor.
    pub fn new() -> Self {
        let sample_rate = 44100.0;

        Self {
            sample_rate,
            envelope: EnvelopeFollower::new(sample_rate),
            threshold_smooth: SmoothedValue::with_default_smoothing(-20.0, sample_rate),
            ratio_smooth: SmoothedValue::with_default_smoothing(4.0, sample_rate),
            attack_smooth: SmoothedValue::with_default_smoothing(10.0, sample_rate),
            release_smooth: SmoothedValue::with_default_smoothing(100.0, sample_rate),
            knee_smooth: SmoothedValue::with_default_smoothing(6.0, sample_rate),
            makeup_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            mix_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in", "In", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("sidechain", "Sidechain", SignalType::Audio, 0.0),
                // Output ports
                PortDefinition::output("out", "Out", SignalType::Audio),
                PortDefinition::output("gr", "GR", SignalType::Control),
            ],
            parameters: vec![
                ParameterDefinition::new(
                    "threshold",
                    "Threshold",
                    -60.0,
                    0.0,
                    -20.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::new(
                    "ratio",
                    "Ratio",
                    1.0,
                    20.0,
                    4.0,
                    ParameterDisplay::Logarithmic { unit: ":1" },
                ),
                ParameterDefinition::new(
                    "attack",
                    "Attack",
                    0.1,
                    100.0,
                    10.0,
                    ParameterDisplay::Logarithmic { unit: "ms" },
                ),
                ParameterDefinition::new(
                    "release",
                    "Release",
                    10.0,
                    1000.0,
                    100.0,
                    ParameterDisplay::Logarithmic { unit: "ms" },
                ),
                ParameterDefinition::new(
                    "knee",
                    "Knee",
                    0.0,
                    12.0,
                    6.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::new(
                    "makeup",
                    "Makeup",
                    0.0,
                    24.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::normalized("mix", "Mix", 1.0),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_SIDECHAIN: usize = 1;
    const PORT_OUT: usize = 0;
    const PORT_GR: usize = 1;

    /// Parameter index constants.
    const PARAM_THRESHOLD: usize = 0;
    const PARAM_RATIO: usize = 1;
    const PARAM_ATTACK: usize = 2;
    const PARAM_RELEASE: usize = 3;
    const PARAM_KNEE: usize = 4;
    const PARAM_MAKEUP: usize = 5;
    const PARAM_MIX: usize = 6;

    /// Compute gain reduction in dB for a given input level in dB.
    /// Uses soft knee algorithm for smooth transition into compression.
    #[inline]
    fn compute_gain_reduction(level_db: f32, threshold: f32, ratio: f32, knee: f32) -> f32 {
        // Distance above threshold
        let over_threshold = level_db - threshold;

        if knee <= 0.0 {
            // Hard knee
            if over_threshold <= 0.0 {
                0.0 // Below threshold, no compression
            } else {
                // Above threshold: reduce by (1 - 1/ratio) * overshoot
                over_threshold * (1.0 - 1.0 / ratio)
            }
        } else {
            // Soft knee
            let half_knee = knee / 2.0;

            if over_threshold <= -half_knee {
                // Below knee region, no compression
                0.0
            } else if over_threshold >= half_knee {
                // Above knee region, full compression
                over_threshold * (1.0 - 1.0 / ratio)
            } else {
                // In knee region - smooth curve
                // Quadratic interpolation through the knee
                let knee_factor = over_threshold + half_knee;
                (1.0 - 1.0 / ratio) * knee_factor * knee_factor / (2.0 * knee)
            }
        }
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Compressor {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.compressor",
            name: "Compressor",
            category: ModuleCategory::Effect,
            description: "Dynamics compressor with sidechain and gain reduction output",
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
        self.envelope.set_sample_rate(sample_rate);

        // Update sample rate for smoothed values
        self.threshold_smooth.set_sample_rate(sample_rate);
        self.ratio_smooth.set_sample_rate(sample_rate);
        self.attack_smooth.set_sample_rate(sample_rate);
        self.release_smooth.set_sample_rate(sample_rate);
        self.knee_smooth.set_sample_rate(sample_rate);
        self.makeup_smooth.set_sample_rate(sample_rate);
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
        let threshold = params[Self::PARAM_THRESHOLD];
        let ratio = params[Self::PARAM_RATIO];
        let attack = params[Self::PARAM_ATTACK];
        let release = params[Self::PARAM_RELEASE];
        let knee = params[Self::PARAM_KNEE];
        let makeup = params[Self::PARAM_MAKEUP];
        let mix = params[Self::PARAM_MIX];

        // Set smoothing targets
        self.threshold_smooth.set_target(threshold);
        self.ratio_smooth.set_target(ratio);
        self.attack_smooth.set_target(attack);
        self.release_smooth.set_target(release);
        self.knee_smooth.set_target(knee);
        self.makeup_smooth.set_target(makeup);
        self.mix_smooth.set_target(mix);

        // Get input buffers
        let input = inputs.get(Self::PORT_IN);
        let sidechain = inputs.get(Self::PORT_SIDECHAIN);

        // Split outputs
        let (out_slice, gr_slice) = outputs.split_at_mut(1);
        let out = &mut out_slice[Self::PORT_OUT];
        let gr_out = &mut gr_slice[0];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed values
            let threshold_db = self.threshold_smooth.next();
            let ratio_val = self.ratio_smooth.next().max(1.0);
            let attack_ms = self.attack_smooth.next().max(0.1);
            let release_ms = self.release_smooth.next().max(10.0);
            let knee_db = self.knee_smooth.next().max(0.0);
            let makeup_db = self.makeup_smooth.next();
            let mix_val = self.mix_smooth.next().clamp(0.0, 1.0);

            // Get input sample
            let dry = input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Get sidechain sample (normalled from input if not connected)
            let sidechain_sample = sidechain
                .and_then(|buf| {
                    let s = buf.samples.get(i).copied().unwrap_or(0.0);
                    // Check if sidechain is actually providing signal
                    if s.abs() > 0.00001 { Some(s) } else { None }
                })
                .unwrap_or(dry);

            // Envelope follow the sidechain signal
            let envelope_level = self.envelope.process(sidechain_sample, attack_ms, release_ms);

            // Convert envelope level to dB (with floor to avoid -inf)
            let level_db = 20.0 * envelope_level.max(0.00001).log10();

            // Compute gain reduction
            let gr_db = Self::compute_gain_reduction(level_db, threshold_db, ratio_val, knee_db);

            // Convert to linear gain (negative dB = less than 1.0)
            let gr_linear = 10.0_f32.powf(-gr_db / 20.0);

            // Apply makeup gain
            let makeup_linear = 10.0_f32.powf(makeup_db / 20.0);

            // Apply compression
            let compressed = dry * gr_linear * makeup_linear;

            // Mix dry and wet (parallel compression)
            let output = dry * (1.0 - mix_val) + compressed * mix_val;

            // Write outputs
            out.samples[i] = output;

            // Output gain reduction as control signal (0 = no GR, 1 = max GR)
            // Normalize GR to 0-1 range: 0dB GR = 0, 60dB GR = 1
            let gr_normalized = (gr_db / 60.0).clamp(0.0, 1.0);
            gr_out.samples[i] = gr_normalized;
        }
    }

    fn reset(&mut self) {
        self.envelope.reset();

        // Reset smoothed values
        self.threshold_smooth.reset(self.threshold_smooth.target());
        self.ratio_smooth.reset(self.ratio_smooth.target());
        self.attack_smooth.reset(self.attack_smooth.target());
        self.release_smooth.reset(self.release_smooth.target());
        self.knee_smooth.reset(self.knee_smooth.target());
        self.makeup_smooth.reset(self.makeup_smooth.target());
        self.mix_smooth.reset(self.mix_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_info() {
        let compressor = Compressor::new();
        assert_eq!(compressor.info().id, "fx.compressor");
        assert_eq!(compressor.info().name, "Compressor");
        assert_eq!(compressor.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_compressor_ports() {
        let compressor = Compressor::new();
        let ports = compressor.ports();

        assert_eq!(ports.len(), 4);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "sidechain");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        // Output ports
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out");
        assert_eq!(ports[2].signal_type, SignalType::Audio);

        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "gr");
        assert_eq!(ports[3].signal_type, SignalType::Control);
    }

    #[test]
    fn test_compressor_parameters() {
        let compressor = Compressor::new();
        let params = compressor.parameters();

        assert_eq!(params.len(), 7);
        assert_eq!(params[0].id, "threshold");
        assert_eq!(params[1].id, "ratio");
        assert_eq!(params[2].id, "attack");
        assert_eq!(params[3].id, "release");
        assert_eq!(params[4].id, "knee");
        assert_eq!(params[5].id, "makeup");
        assert_eq!(params[6].id, "mix");
    }

    #[test]
    fn test_gain_reduction_below_threshold() {
        // Signal below threshold should have no gain reduction
        let gr = Compressor::compute_gain_reduction(-30.0, -20.0, 4.0, 0.0);
        assert_eq!(gr, 0.0);
    }

    #[test]
    fn test_gain_reduction_above_threshold_hard_knee() {
        // Signal 10dB above threshold with 4:1 ratio
        // Should reduce by (1 - 1/4) * 10 = 7.5 dB
        let gr = Compressor::compute_gain_reduction(-10.0, -20.0, 4.0, 0.0);
        assert!((gr - 7.5).abs() < 0.01);
    }

    #[test]
    fn test_gain_reduction_soft_knee() {
        // In the soft knee region, gain reduction should be smooth
        let gr_at_threshold = Compressor::compute_gain_reduction(-20.0, -20.0, 4.0, 6.0);
        let gr_below = Compressor::compute_gain_reduction(-23.0, -20.0, 4.0, 6.0);
        let gr_above = Compressor::compute_gain_reduction(-17.0, -20.0, 4.0, 6.0);

        // At threshold, should have some reduction
        assert!(gr_at_threshold > 0.0);
        // Below knee start, should have no reduction
        assert_eq!(gr_below, 0.0);
        // Above knee end, should have full reduction
        assert!(gr_above > gr_at_threshold);
    }

    #[test]
    fn test_compressor_unity_gain_at_low_levels() {
        let mut compressor = Compressor::new();
        compressor.prepare(44100.0, 256);

        // Create a low-level input signal (should be below threshold)
        let mut input = SignalBuffer::audio(256);
        for sample in input.samples.iter_mut() {
            *sample = 0.01; // Very quiet
        }

        let sidechain = SignalBuffer::audio(256); // Empty sidechain
        let mut outputs = vec![
            SignalBuffer::audio(256), // Out
            SignalBuffer::control(256), // GR
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with default settings (-20dB threshold)
        // Mix = 100% (1.0), makeup = 0dB
        compressor.process(
            &[&input, &sidechain],
            &mut outputs,
            &[-20.0, 4.0, 10.0, 100.0, 6.0, 0.0, 1.0],
            &ctx,
        );

        // At very low levels, output should be close to input (no compression)
        // Allow for some smoothing artifacts
        for i in 100..256 {
            let ratio = outputs[0].samples[i] / input.samples[i];
            assert!(
                (ratio - 1.0).abs() < 0.5,
                "Expected near-unity gain at low levels, got ratio {} at sample {}",
                ratio,
                i
            );
        }
    }

    #[test]
    fn test_compressor_reduces_loud_signals() {
        let mut compressor = Compressor::new();
        compressor.prepare(44100.0, 256);

        // Create a loud input signal (well above threshold)
        let mut input = SignalBuffer::audio(256);
        for sample in input.samples.iter_mut() {
            *sample = 0.9; // Loud signal
        }

        let sidechain = SignalBuffer::audio(256);
        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process multiple blocks to let envelope settle
        for _ in 0..10 {
            compressor.process(
                &[&input, &sidechain],
                &mut outputs,
                &[-20.0, 4.0, 0.1, 10.0, 0.0, 0.0, 1.0], // Fast attack, hard knee, no makeup
                &ctx,
            );
        }

        // After compression, output should be reduced
        let last_output = outputs[0].samples[255].abs();
        let last_input = input.samples[255].abs();
        assert!(
            last_output < last_input,
            "Expected compression to reduce signal, got output {} vs input {}",
            last_output,
            last_input
        );

        // Gain reduction output should show some compression
        assert!(
            outputs[1].samples[255] > 0.0,
            "Expected gain reduction output, got {}",
            outputs[1].samples[255]
        );
    }

    #[test]
    fn test_compressor_makeup_gain() {
        let mut compressor = Compressor::new();
        compressor.prepare(44100.0, 256);

        // Create a loud signal
        let mut input = SignalBuffer::audio(256);
        for sample in input.samples.iter_mut() {
            *sample = 0.5;
        }

        let sidechain = SignalBuffer::audio(256);

        // Process without makeup
        let mut outputs_no_makeup = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        for _ in 0..5 {
            compressor.process(
                &[&input, &sidechain],
                &mut outputs_no_makeup,
                &[-20.0, 4.0, 0.1, 10.0, 0.0, 0.0, 1.0], // No makeup
                &ctx,
            );
        }

        compressor.reset();

        // Process with 12dB makeup
        let mut outputs_with_makeup = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];

        for _ in 0..5 {
            compressor.process(
                &[&input, &sidechain],
                &mut outputs_with_makeup,
                &[-20.0, 4.0, 0.1, 10.0, 0.0, 12.0, 1.0], // 12dB makeup
                &ctx,
            );
        }

        // Output with makeup should be louder
        let level_no_makeup = outputs_no_makeup[0].samples[255].abs();
        let level_with_makeup = outputs_with_makeup[0].samples[255].abs();
        assert!(
            level_with_makeup > level_no_makeup,
            "Expected makeup gain to increase output"
        );
    }

    #[test]
    fn test_compressor_parallel_compression() {
        let mut compressor = Compressor::new();
        compressor.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        for sample in input.samples.iter_mut() {
            *sample = 0.8;
        }

        let sidechain = SignalBuffer::audio(256);
        let ctx = ProcessContext::new(44100.0, 256);

        // 100% wet
        let mut outputs_wet = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];

        for _ in 0..5 {
            compressor.process(
                &[&input, &sidechain],
                &mut outputs_wet,
                &[-20.0, 8.0, 0.1, 10.0, 0.0, 0.0, 1.0], // 100% wet
                &ctx,
            );
        }

        compressor.reset();

        // 50% wet (parallel)
        let mut outputs_parallel = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];

        for _ in 0..5 {
            compressor.process(
                &[&input, &sidechain],
                &mut outputs_parallel,
                &[-20.0, 8.0, 0.1, 10.0, 0.0, 0.0, 0.5], // 50% wet
                &ctx,
            );
        }

        // Parallel should be between dry and fully compressed
        let wet_level = outputs_wet[0].samples[255].abs();
        let parallel_level = outputs_parallel[0].samples[255].abs();
        let dry_level = input.samples[255].abs();

        // Parallel mix should be louder than full compression (due to dry signal)
        assert!(
            parallel_level > wet_level,
            "Parallel mix {} should be louder than full wet {}",
            parallel_level,
            wet_level
        );
        // But quieter than dry (compression still applied to wet portion)
        assert!(
            parallel_level <= dry_level + 0.01,
            "Parallel mix {} shouldn't exceed dry {}",
            parallel_level,
            dry_level
        );
    }

    #[test]
    fn test_compressor_reset() {
        let mut compressor = Compressor::new();
        compressor.prepare(44100.0, 256);

        // Fill with signal to build up envelope
        let mut input = SignalBuffer::audio(256);
        input.fill(0.9);
        let sidechain = SignalBuffer::audio(256);
        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        compressor.process(
            &[&input, &sidechain],
            &mut outputs,
            &[-20.0, 4.0, 0.1, 10.0, 0.0, 0.0, 1.0],
            &ctx,
        );

        // Reset
        compressor.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::control(256),
        ];

        compressor.process(
            &[&silence, &sidechain],
            &mut outputs2,
            &[-20.0, 4.0, 0.1, 10.0, 0.0, 0.0, 1.0],
            &ctx,
        );

        // Output should be near zero
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Expected near-zero output after reset"
        );
    }

    #[test]
    fn test_compressor_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Compressor>();
    }

    #[test]
    fn test_compressor_default() {
        let compressor = Compressor::default();
        assert_eq!(compressor.info().id, "fx.compressor");
    }

    #[test]
    fn test_compressor_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Compressor>();

        assert!(registry.contains("fx.compressor"));

        let module = registry.create("fx.compressor");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.compressor");
        assert_eq!(module.info().name, "Compressor");
        assert_eq!(module.ports().len(), 4);
        assert_eq!(module.parameters().len(), 7);
    }
}
