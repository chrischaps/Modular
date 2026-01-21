//! State Variable Filter module.
//!
//! A versatile filter topology providing simultaneous lowpass, highpass,
//! and bandpass outputs from a single algorithm.

use std::f32::consts::PI;

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    SignalType,
};

/// A State Variable Filter with multiple simultaneous outputs.
///
/// The SVF is a classic filter topology that provides lowpass, highpass,
/// and bandpass outputs simultaneously. This implementation uses the
/// Chamberlin algorithm with improvements for stability at high resonance.
///
/// # Ports
///
/// - **In** (Audio, Input): The audio signal to filter.
/// - **Cutoff** (Control, Input): CV modulation for cutoff frequency.
/// - **Resonance** (Control, Input): CV modulation for resonance.
/// - **LowPass** (Audio, Output): Lowpass filtered output.
/// - **HighPass** (Audio, Output): Highpass filtered output.
/// - **BandPass** (Audio, Output): Bandpass filtered output.
///
/// # Parameters
///
/// - **Cutoff** (20-20000 Hz): Filter cutoff frequency.
/// - **Resonance** (0-1): Filter resonance/Q. Higher values add emphasis at cutoff.
/// - **Drive** (1-10): Input gain/saturation for analog-style warmth.
pub struct SvfFilter {
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Filter state: lowpass output.
    low: f32,
    /// Filter state: bandpass output.
    band: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl SvfFilter {
    /// Creates a new SVF filter.
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            low: 0.0,
            band: 0.0,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in", "In", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("cutoff_cv", "Cutoff", SignalType::Control, 0.0),
                PortDefinition::input_with_default("res_cv", "Resonance", SignalType::Control, 0.0),
                // Output ports
                PortDefinition::output("lowpass", "LowPass", SignalType::Audio),
                PortDefinition::output("highpass", "HighPass", SignalType::Audio),
                PortDefinition::output("bandpass", "BandPass", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::frequency("cutoff", "Cutoff", 20.0, 20000.0, 1000.0),
                ParameterDefinition::new(
                    "resonance",
                    "Resonance",
                    0.0,
                    1.0,
                    0.5,
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
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_CUTOFF_CV: usize = 1;
    const PORT_RES_CV: usize = 2;
    const PORT_LOWPASS: usize = 0;
    const PORT_HIGHPASS: usize = 1;
    const PORT_BANDPASS: usize = 2;

    /// Parameter index constants.
    const PARAM_CUTOFF: usize = 0;
    const PARAM_RESONANCE: usize = 1;
    const PARAM_DRIVE: usize = 2;

    /// Soft clip function using tanh for smooth saturation.
    /// This helps stabilize the filter at high resonance.
    #[inline]
    fn soft_clip(x: f32) -> f32 {
        x.tanh()
    }

    /// Calculate the filter coefficient 'f' from cutoff frequency.
    /// Uses the formula: f = 2 * sin(pi * cutoff / sample_rate)
    /// Clamped to prevent instability at high frequencies.
    #[inline]
    fn calc_f(&self, cutoff: f32) -> f32 {
        let cutoff_clamped = cutoff.clamp(20.0, self.sample_rate * 0.45);
        let f = 2.0 * (PI * cutoff_clamped / self.sample_rate).sin();
        // Clamp f to prevent instability (max ~0.9 for stable operation)
        f.clamp(0.0, 0.9)
    }

    /// Calculate the damping coefficient 'q' from resonance.
    /// Resonance 0-1 maps to damping 2-0.1 (higher resonance = lower damping).
    #[inline]
    fn calc_q(resonance: f32) -> f32 {
        let res_clamped = resonance.clamp(0.0, 0.99);
        // Map resonance 0-1 to q 2-0.1
        // At resonance=0, q=2 (low Q, no emphasis)
        // At resonance=1, q=0.1 (high Q, strong emphasis/self-oscillation)
        2.0 - res_clamped * 1.9
    }
}

impl Default for SvfFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for SvfFilter {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "filter.svf",
            name: "State Variable Filter",
            category: ModuleCategory::Filter,
            description: "Multi-mode filter with LP, HP, and BP outputs",
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
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let base_cutoff = params[Self::PARAM_CUTOFF];
        let base_resonance = params[Self::PARAM_RESONANCE];
        // Map drive from 0-1 (UI) to 1-10 (DSP) so signal always passes through
        let drive = 1.0 + params[Self::PARAM_DRIVE] * 9.0;

        // Get input buffers
        let audio_in = inputs.get(Self::PORT_IN);
        let cutoff_cv = inputs.get(Self::PORT_CUTOFF_CV);
        let res_cv = inputs.get(Self::PORT_RES_CV);

        // Get output buffers
        let (lp_out, rest) = outputs.split_at_mut(1);
        let lp_out = &mut lp_out[Self::PORT_LOWPASS];
        let (hp_out, bp_out) = rest.split_at_mut(1);
        let hp_out = &mut hp_out[0];
        let bp_out = &mut bp_out[0];

        // Process each sample
        for i in 0..context.block_size {
            // Get input sample with drive
            let input = audio_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let input = Self::soft_clip(input * drive);

            // Get CV modulation
            // Cutoff CV: -1 to 1 maps to exponential frequency scaling (2 octaves each direction)
            let cutoff_mod = cutoff_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Apply exponential CV scaling: 2^(cv*2) gives us 4 octaves of range
            let cutoff = base_cutoff * 2.0_f32.powf(cutoff_mod * 2.0);

            // Resonance CV: adds directly to base resonance
            let res_mod = res_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let resonance = (base_resonance + res_mod * 0.5).clamp(0.0, 1.0);

            // Calculate filter coefficients
            let f = self.calc_f(cutoff);
            let q = Self::calc_q(resonance);

            // Chamberlin SVF algorithm (two integrator topology)
            // low = low + f * band
            // high = input - low - q * band
            // band = f * high + band

            self.low = self.low + f * self.band;
            let high = input - self.low - q * self.band;
            self.band = f * high + self.band;

            // Apply soft clipping to internal states to prevent runaway at high resonance
            self.low = Self::soft_clip(self.low);
            self.band = Self::soft_clip(self.band);

            // Write outputs
            lp_out.samples[i] = self.low;
            hp_out.samples[i] = high;
            bp_out.samples[i] = self.band;
        }
    }

    fn reset(&mut self) {
        self.low = 0.0;
        self.band = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svf_filter_info() {
        let filter = SvfFilter::new();
        assert_eq!(filter.info().id, "filter.svf");
        assert_eq!(filter.info().name, "State Variable Filter");
        assert_eq!(filter.info().category, ModuleCategory::Filter);
    }

    #[test]
    fn test_svf_filter_ports() {
        let filter = SvfFilter::new();
        let ports = filter.ports();

        assert_eq!(ports.len(), 6);

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

        // Output ports
        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "lowpass");
        assert_eq!(ports[3].signal_type, SignalType::Audio);

        assert!(ports[4].is_output());
        assert_eq!(ports[4].id, "highpass");
        assert_eq!(ports[4].signal_type, SignalType::Audio);

        assert!(ports[5].is_output());
        assert_eq!(ports[5].id, "bandpass");
        assert_eq!(ports[5].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_svf_filter_parameters() {
        let filter = SvfFilter::new();
        let params = filter.parameters();

        assert_eq!(params.len(), 3);

        // Cutoff parameter
        assert_eq!(params[0].id, "cutoff");
        assert_eq!(params[0].min, 20.0);
        assert_eq!(params[0].max, 20000.0);
        assert_eq!(params[0].default, 1000.0);

        // Resonance parameter
        assert_eq!(params[1].id, "resonance");
        assert_eq!(params[1].min, 0.0);
        assert_eq!(params[1].max, 1.0);
        assert_eq!(params[1].default, 0.5);

        // Drive parameter
        assert_eq!(params[2].id, "drive");
        assert_eq!(params[2].min, 1.0);
        assert_eq!(params[2].max, 10.0);
        assert_eq!(params[2].default, 1.0);
    }

    #[test]
    fn test_svf_filter_produces_output() {
        let mut filter = SvfFilter::new();
        filter.prepare(44100.0, 256);

        // Create a simple sine wave input
        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 * 0.1).sin();
        }

        let mut outputs = vec![
            SignalBuffer::audio(256), // LP
            SignalBuffer::audio(256), // HP
            SignalBuffer::audio(256), // BP
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        filter.process(&[&input], &mut outputs, &[1000.0, 0.5, 0.0], &ctx);

        // All outputs should have non-zero values
        let lp_has_signal = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        let hp_has_signal = outputs[1].samples.iter().any(|&s| s.abs() > 0.001);
        let bp_has_signal = outputs[2].samples.iter().any(|&s| s.abs() > 0.001);

        assert!(lp_has_signal, "Lowpass should produce output");
        assert!(hp_has_signal, "Highpass should produce output");
        assert!(bp_has_signal, "Bandpass should produce output");

        // All outputs should be within valid range
        for output in &outputs {
            for &sample in &output.samples {
                assert!(
                    sample >= -2.0 && sample <= 2.0,
                    "Sample {} out of reasonable range",
                    sample
                );
            }
        }
    }

    #[test]
    fn test_svf_filter_lowpass_attenuates_high_freq() {
        let mut filter = SvfFilter::new();
        let sample_rate = 44100.0;
        filter.prepare(sample_rate, 4410);

        // Generate a high frequency signal (5000 Hz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 5000.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin();
        }

        let mut outputs = vec![
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
        ];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Set cutoff to 500 Hz (well below our 5000 Hz signal)
        filter.process(&[&input], &mut outputs, &[500.0, 0.5, 0.0], &ctx);

        // Calculate RMS of input and lowpass output (after initial transient)
        let skip = 441; // Skip first ~10ms for filter settling
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let lp_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Lowpass should significantly attenuate the high frequency
        assert!(
            lp_rms < input_rms * 0.5,
            "Lowpass should attenuate high frequencies: input_rms={}, lp_rms={}",
            input_rms,
            lp_rms
        );
    }

    #[test]
    fn test_svf_filter_highpass_attenuates_low_freq() {
        let mut filter = SvfFilter::new();
        let sample_rate = 44100.0;
        filter.prepare(sample_rate, 4410);

        // Generate a low frequency signal (100 Hz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 100.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin();
        }

        let mut outputs = vec![
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
        ];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Set cutoff to 2000 Hz (well above our 100 Hz signal)
        filter.process(&[&input], &mut outputs, &[2000.0, 0.5, 0.0], &ctx);

        // Calculate RMS of input and highpass output (after initial transient)
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let hp_rms: f32 = (outputs[1].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Highpass should significantly attenuate the low frequency
        assert!(
            hp_rms < input_rms * 0.5,
            "Highpass should attenuate low frequencies: input_rms={}, hp_rms={}",
            input_rms,
            hp_rms
        );
    }

    #[test]
    fn test_svf_filter_reset() {
        let mut filter = SvfFilter::new();
        filter.prepare(44100.0, 256);

        // Process some samples to build up state
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);
        filter.process(&[&input], &mut outputs, &[1000.0, 0.5, 0.0], &ctx);

        // Reset should clear internal state
        filter.reset();

        // Process silence - output should be near zero
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx2 = ProcessContext::new(44100.0, 256);
        filter.process(&[&silence], &mut outputs2, &[1000.0, 0.5, 0.0], &ctx2);

        // First output sample should be near zero after reset
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Lowpass should be near zero after reset, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_svf_filter_stability_high_resonance() {
        let mut filter = SvfFilter::new();
        filter.prepare(44100.0, 4410);

        // Test with high resonance (0.95) and varying input
        let mut input = SignalBuffer::audio(4410);
        for i in 0..4410 {
            input.samples[i] = if i % 100 < 50 { 1.0 } else { -1.0 };
        }

        let mut outputs = vec![
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
            SignalBuffer::audio(4410),
        ];
        let ctx = ProcessContext::new(44100.0, 4410);

        // High resonance, should still be stable
        filter.process(&[&input], &mut outputs, &[1000.0, 0.95, 1.0], &ctx);

        // Check that no outputs explode (all within reasonable bounds)
        for output in &outputs {
            for &sample in &output.samples {
                assert!(
                    sample.is_finite() && sample.abs() < 10.0,
                    "Filter became unstable: sample={}",
                    sample
                );
            }
        }
    }

    #[test]
    fn test_svf_filter_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SvfFilter>();
    }

    #[test]
    fn test_svf_filter_default() {
        let filter = SvfFilter::default();
        assert_eq!(filter.info().id, "filter.svf");
    }

    #[test]
    fn test_svf_filter_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<SvfFilter>();

        assert!(registry.contains("filter.svf"));

        let module = registry.create("filter.svf");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "filter.svf");
        assert_eq!(module.info().name, "State Variable Filter");
        assert_eq!(module.ports().len(), 6);
        assert_eq!(module.parameters().len(), 3);
    }

    #[test]
    fn test_calc_f_coefficient() {
        let filter = SvfFilter::new();

        // At low frequencies, f should be small
        let f_low = filter.calc_f(100.0);
        assert!(f_low > 0.0 && f_low < 0.1, "f at 100Hz should be small: {}", f_low);

        // At high frequencies, f should be larger but clamped
        let f_high = filter.calc_f(10000.0);
        assert!(f_high > f_low, "f should increase with frequency");
        assert!(f_high <= 0.9, "f should be clamped for stability: {}", f_high);
    }

    #[test]
    fn test_calc_q_coefficient() {
        // Low resonance = high damping (low Q)
        let q_low = SvfFilter::calc_q(0.0);
        assert!((q_low - 2.0).abs() < 0.01, "q at res=0 should be 2.0: {}", q_low);

        // High resonance = low damping (high Q)
        let q_high = SvfFilter::calc_q(0.99);
        assert!(q_high < 0.2, "q at res=0.99 should be low: {}", q_high);
    }

    #[test]
    fn test_svf_filter_drive() {
        let mut filter = SvfFilter::new();
        filter.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = 0.5;
        }

        // Process with drive = 0.0 (UI value) -> actual drive = 1.0 (unity)
        let mut outputs1 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);
        filter.process(&[&input], &mut outputs1, &[1000.0, 0.5, 0.0], &ctx);

        filter.reset();

        // Process with drive = 0.5 (UI value) -> actual drive = 5.5
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        filter.process(&[&input], &mut outputs2, &[1000.0, 0.5, 0.5], &ctx);

        // With higher drive, the output should be louder (until saturation)
        let rms1: f32 = (outputs1[0].samples.iter().map(|s| s * s).sum::<f32>() / 256.0).sqrt();
        let rms2: f32 = (outputs2[0].samples.iter().map(|s| s * s).sum::<f32>() / 256.0).sqrt();

        assert!(
            rms2 > rms1,
            "Higher drive should produce louder output: rms1={}, rms2={}",
            rms1,
            rms2
        );
    }
}
