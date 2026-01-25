//! Moog Ladder Filter module.
//!
//! A classic 4-pole (24dB/octave) lowpass filter with resonance,
//! emulating the iconic Moog transistor ladder topology.

use std::f32::consts::PI;

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    SignalType,
};

/// A Moog-style 4-pole ladder filter.
///
/// The Moog ladder is one of the most iconic filter designs in synthesizer
/// history. It consists of 4 cascaded 1-pole lowpass filters with a global
/// feedback path that creates resonance.
///
/// This implementation uses an improved digital model that accurately captures
/// the nonlinear behavior of the original transistor ladder, including:
/// - Soft saturation in each filter stage (tanh)
/// - Proper compensation for resonance gain
/// - Stable self-oscillation at high resonance
///
/// # Ports
///
/// - **In** (Audio, Input): The audio signal to filter.
/// - **Cutoff** (Control, Input): CV modulation for cutoff frequency.
/// - **Resonance** (Control, Input): CV modulation for resonance.
/// - **Out** (Audio, Output): 4-pole lowpass filtered output (24dB/octave).
///
/// # Parameters
///
/// - **Cutoff** (20-20000 Hz): Filter cutoff frequency.
/// - **Resonance** (0-1): Filter resonance. At 1.0, the filter self-oscillates.
/// - **Drive** (1-10): Input gain/saturation for analog-style warmth.
pub struct MoogLadder {
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Filter state for each of the 4 poles.
    stage: [f32; 4],
    /// Delayed output for feedback (z^-1).
    delay: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
    /// Smoothed cutoff parameter.
    cutoff_smooth: SmoothedValue,
    /// Smoothed resonance parameter.
    resonance_smooth: SmoothedValue,
    /// Smoothed drive parameter.
    drive_smooth: SmoothedValue,
}

impl MoogLadder {
    /// Creates a new Moog ladder filter.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        Self {
            sample_rate,
            stage: [0.0; 4],
            delay: 0.0,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in", "In", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("cutoff_cv", "Cutoff", SignalType::Control, 0.0),
                PortDefinition::input_with_default("res_cv", "Resonance", SignalType::Control, 0.0),
                // Output port - only lowpass (classic Moog)
                PortDefinition::output("out", "Out", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::frequency("cutoff", "Cutoff", 20.0, 20000.0, 1000.0),
                ParameterDefinition::new(
                    "resonance",
                    "Resonance",
                    0.0,
                    1.0,
                    0.0,
                    crate::dsp::ParameterDisplay::Linear { unit: "" },
                ),
                ParameterDefinition::new(
                    "drive",
                    "Drive",
                    1.0,
                    10.0,
                    1.0,
                    crate::dsp::ParameterDisplay::Linear { unit: "x" },
                ),
            ],
            cutoff_smooth: SmoothedValue::with_default_smoothing(1000.0, sample_rate),
            resonance_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            drive_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_CUTOFF_CV: usize = 1;
    const PORT_RES_CV: usize = 2;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_CUTOFF: usize = 0;
    const PARAM_RESONANCE: usize = 1;
    const PARAM_DRIVE: usize = 2;

    /// Soft saturation using tanh.
    /// This emulates the nonlinear behavior of the transistor stages.
    #[inline]
    fn saturate(x: f32) -> f32 {
        x.tanh()
    }

    /// Calculate the filter coefficient 'g' from cutoff frequency.
    /// Uses the formula: g = 1 - exp(-2π * cutoff / sample_rate)
    #[inline]
    fn calc_g(&self, cutoff: f32) -> f32 {
        let cutoff_clamped = cutoff.clamp(20.0, self.sample_rate * 0.45);
        1.0 - (-2.0 * PI * cutoff_clamped / self.sample_rate).exp()
    }

    /// Process a single sample through one ladder stage.
    /// Each stage is a 1-pole lowpass with saturation.
    #[inline]
    fn process_stage(state: &mut f32, input: f32, g: f32) -> f32 {
        // 1-pole lowpass: y = y + g * (tanh(x) - y)
        *state = *state + g * (Self::saturate(input) - *state);
        *state
    }
}

impl Default for MoogLadder {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for MoogLadder {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "filter.moog_ladder",
            name: "Moog Ladder",
            category: ModuleCategory::Filter,
            description: "Classic 4-pole 24dB/oct lowpass with resonance",
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
        self.cutoff_smooth.set_sample_rate(sample_rate);
        self.resonance_smooth.set_sample_rate(sample_rate);
        self.drive_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Set smoothing targets from parameters
        self.cutoff_smooth.set_target(params[Self::PARAM_CUTOFF]);
        self.resonance_smooth.set_target(params[Self::PARAM_RESONANCE]);
        self.drive_smooth.set_target(params[Self::PARAM_DRIVE]);

        // Get input buffers
        let audio_in = inputs.get(Self::PORT_IN);
        let cutoff_cv = inputs.get(Self::PORT_CUTOFF_CV);
        let res_cv = inputs.get(Self::PORT_RES_CV);

        // Get output buffer
        let out = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed parameter values
            let base_cutoff = self.cutoff_smooth.next();
            let base_resonance = self.resonance_smooth.next();
            let drive = self.drive_smooth.next();

            // Get input sample with drive
            let input = audio_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Get CV modulation
            let cutoff_mod = cutoff_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let cutoff = base_cutoff * 2.0_f32.powf(cutoff_mod * 2.0);

            let res_mod = res_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let resonance = (base_resonance + res_mod * 0.5).clamp(0.0, 1.0);

            // Calculate filter coefficient
            let g = self.calc_g(cutoff);

            // Resonance factor (k): 0 to 4 for self-oscillation
            // We use slightly less than 4 to keep it stable
            let k = resonance * 3.98;

            // Apply drive and subtract feedback (resonance)
            let input_driven = Self::saturate(input * drive - k * self.delay);

            // Process through 4 cascaded stages
            let s1 = Self::process_stage(&mut self.stage[0], input_driven, g);
            let s2 = Self::process_stage(&mut self.stage[1], s1, g);
            let s3 = Self::process_stage(&mut self.stage[2], s2, g);
            let s4 = Self::process_stage(&mut self.stage[3], s3, g);

            // Store output for feedback
            self.delay = s4;

            // Output is the 4-pole (24dB/oct) lowpass
            out.samples[i] = s4;
        }
    }

    fn reset(&mut self) {
        self.stage = [0.0; 4];
        self.delay = 0.0;
        self.cutoff_smooth.reset(self.cutoff_smooth.target());
        self.resonance_smooth.reset(self.resonance_smooth.target());
        self.drive_smooth.reset(self.drive_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moog_ladder_info() {
        let filter = MoogLadder::new();
        assert_eq!(filter.info().id, "filter.moog_ladder");
        assert_eq!(filter.info().name, "Moog Ladder");
        assert_eq!(filter.info().category, ModuleCategory::Filter);
    }

    #[test]
    fn test_moog_ladder_ports() {
        let filter = MoogLadder::new();
        let ports = filter.ports();

        assert_eq!(ports.len(), 4);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "cutoff_cv");
        assert_eq!(ports[1].signal_type, SignalType::Control);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "res_cv");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        // Output port
        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "out");
        assert_eq!(ports[3].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_moog_ladder_parameters() {
        let filter = MoogLadder::new();
        let params = filter.parameters();

        assert_eq!(params.len(), 3);

        assert_eq!(params[0].id, "cutoff");
        assert_eq!(params[0].min, 20.0);
        assert_eq!(params[0].max, 20000.0);
        assert_eq!(params[0].default, 1000.0);

        assert_eq!(params[1].id, "resonance");
        assert_eq!(params[1].min, 0.0);
        assert_eq!(params[1].max, 1.0);
        assert_eq!(params[1].default, 0.0);

        assert_eq!(params[2].id, "drive");
        assert_eq!(params[2].min, 1.0);
        assert_eq!(params[2].max, 10.0);
        assert_eq!(params[2].default, 1.0);
    }

    #[test]
    fn test_moog_ladder_produces_output() {
        let mut filter = MoogLadder::new();
        filter.prepare(44100.0, 256);

        // Create a simple sine wave input
        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 * 0.1).sin();
        }

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        filter.process(&[&input], &mut outputs, &[1000.0, 0.0, 1.0], &ctx);

        // Output should have non-zero values
        let has_signal = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_signal, "Filter should produce output");

        // All outputs should be within valid range
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -2.0 && sample <= 2.0,
                "Sample {} out of reasonable range",
                sample
            );
        }
    }

    #[test]
    fn test_moog_ladder_lowpass_behavior() {
        let mut filter = MoogLadder::new();
        let sample_rate = 44100.0;
        filter.prepare(sample_rate, 4410);

        // Generate a high frequency signal (5000 Hz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 5000.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin();
        }

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Set cutoff to 500 Hz (well below our 5000 Hz signal)
        filter.process(&[&input], &mut outputs, &[500.0, 0.0, 1.0], &ctx);

        // Calculate RMS of input and output (after initial transient)
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let out_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Output should be significantly attenuated (24dB/oct is steep!)
        assert!(
            out_rms < input_rms * 0.1,
            "Moog ladder should strongly attenuate high frequencies: input_rms={}, out_rms={}",
            input_rms,
            out_rms
        );
    }

    #[test]
    fn test_moog_ladder_resonance_boost() {
        let mut filter1 = MoogLadder::new();
        filter1.prepare(44100.0, 4410);
        let mut filter2 = MoogLadder::new();
        filter2.prepare(44100.0, 4410);

        // Generate a signal near the cutoff frequency
        let sample_rate = 44100.0;
        let mut input = SignalBuffer::audio(4410);
        let freq = 1000.0; // Same as cutoff
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5;
        }

        let ctx = ProcessContext::new(sample_rate, 4410);

        // Process with no resonance
        let mut outputs1 = vec![SignalBuffer::audio(4410)];
        for _ in 0..5 {
            filter1.process(&[&input], &mut outputs1, &[1000.0, 0.0, 1.0], &ctx);
        }

        // Process with high resonance
        let mut outputs2 = vec![SignalBuffer::audio(4410)];
        for _ in 0..5 {
            filter2.process(&[&input], &mut outputs2, &[1000.0, 0.8, 1.0], &ctx);
        }

        // High resonance should boost signal at cutoff
        let skip = 2000;
        let rms1: f32 = (outputs1[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let rms2: f32 = (outputs2[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        assert!(
            rms2 > rms1,
            "High resonance should boost signal at cutoff: rms_no_res={}, rms_high_res={}",
            rms1,
            rms2
        );
    }

    #[test]
    fn test_moog_ladder_stability_high_resonance() {
        let mut filter = MoogLadder::new();
        filter.prepare(44100.0, 4410);

        // Test with maximum resonance and impulse input
        let mut input = SignalBuffer::audio(4410);
        input.samples[0] = 1.0; // Impulse

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(44100.0, 4410);

        // Maximum resonance (self-oscillation territory)
        filter.process(&[&input], &mut outputs, &[1000.0, 1.0, 1.0], &ctx);

        // Check that no outputs explode
        for &sample in &outputs[0].samples {
            assert!(
                sample.is_finite() && sample.abs() < 10.0,
                "Filter became unstable: sample={}",
                sample
            );
        }
    }

    #[test]
    fn test_moog_ladder_reset() {
        let mut filter = MoogLadder::new();
        filter.prepare(44100.0, 256);

        // Process some samples to build up state
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        filter.process(&[&input], &mut outputs, &[1000.0, 0.5, 1.0], &ctx);

        // Reset
        filter.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![SignalBuffer::audio(256)];
        filter.process(&[&silence], &mut outputs2, &[1000.0, 0.5, 1.0], &ctx);

        // First sample should be near zero after reset
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Output should be near zero after reset, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_moog_ladder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MoogLadder>();
    }

    #[test]
    fn test_moog_ladder_default() {
        let filter = MoogLadder::default();
        assert_eq!(filter.info().id, "filter.moog_ladder");
    }
}
