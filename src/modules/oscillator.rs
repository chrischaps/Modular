//! Oscillator modules.
//!
//! This module contains sound source modules that generate audio waveforms.

use std::f32::consts::TAU;

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    SignalType,
};

/// A sine wave oscillator with frequency modulation support.
///
/// This is a fundamental building block for synthesis, producing a pure
/// sine wave that can be frequency-modulated via the FM input.
///
/// # Ports
///
/// - **Frequency** (Control, Input): CV input for frequency control. When connected,
///   this signal is added to the base frequency parameter.
/// - **FM** (Control, Input): Frequency modulation input. The signal is scaled by
///   the FM Depth parameter and added to the frequency.
/// - **Out** (Audio, Output): The generated sine wave output.
///
/// # Parameters
///
/// - **Frequency** (20-20000 Hz): Base frequency of the oscillator.
/// - **FM Depth** (0-1000 Hz): How much the FM input affects the frequency.
pub struct SineOscillator {
    /// Current phase accumulator (0.0 to 1.0).
    phase: f32,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl SineOscillator {
    /// Creates a new sine oscillator.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            sample_rate: 44100.0,
            ports: vec![
                // Input ports first (by convention)
                // These map to UI inputs: Add Freq (0), FM (1), Frequency (2)
                PortDefinition::input_with_default("freq_cv", "Add Freq", SignalType::Control, 0.0),
                PortDefinition::input_with_default("fm", "FM", SignalType::Control, 0.0),
                // Direct frequency input - when connected, overrides the Frequency parameter
                // Expects Control signal in range that maps to 20-20000 Hz
                PortDefinition::input_with_default("freq_in", "Frequency", SignalType::Control, 0.0),
                // Output port
                PortDefinition::output("out", "Out", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::frequency("frequency", "Frequency", 20.0, 20000.0, 440.0),
                ParameterDefinition::new(
                    "fm_depth",
                    "FM Depth",
                    0.0,
                    1000.0,
                    0.0,
                    crate::dsp::ParameterDisplay::linear("Hz"),
                ),
            ],
        }
    }

    /// Port index constants for clarity.
    const PORT_FREQ_CV: usize = 0;
    const PORT_FM: usize = 1;
    const PORT_FREQ_IN: usize = 2;
    const PORT_OUT: usize = 0; // First (only) output

    /// Parameter index constants.
    const PARAM_FREQUENCY: usize = 0;
    const PARAM_FM_DEPTH: usize = 1;
}

impl Default for SineOscillator {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for SineOscillator {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "osc.sine",
            name: "Sine Oscillator",
            category: ModuleCategory::Source,
            description: "A pure sine wave oscillator with FM support",
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
        let base_freq = params[Self::PARAM_FREQUENCY];
        let fm_depth = params[Self::PARAM_FM_DEPTH];

        // Get input buffers (may be empty if not connected, use defaults)
        let freq_cv = inputs.get(Self::PORT_FREQ_CV);
        let fm_input = inputs.get(Self::PORT_FM);
        let freq_in = inputs.get(Self::PORT_FREQ_IN);

        // Check if freq_in is connected (has non-zero signal)
        // If connected, it overrides the base frequency parameter
        let freq_in_connected = freq_in
            .map(|buf| buf.samples.iter().any(|&s| s.abs() > f32::EPSILON))
            .unwrap_or(false);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Determine base frequency: either from freq_in (if connected) or parameter
            let effective_base_freq = if freq_in_connected {
                // freq_in is a Control signal (-1 to 1), map to frequency range (20-20000 Hz)
                // Using logarithmic mapping for musical response
                let control_val = freq_in
                    .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                    .unwrap_or(0.0);
                // Map -1..1 to 0..1, then to log frequency range
                let normalized = (control_val + 1.0) * 0.5; // 0..1
                let min_freq = 20.0_f32;
                let max_freq = 20000.0_f32;
                // Logarithmic interpolation for musical scaling
                min_freq * (max_freq / min_freq).powf(normalized)
            } else {
                base_freq
            };

            // Get CV modulation for frequency (additive offset)
            let freq_cv_value = freq_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Get FM modulation
            let fm_value = fm_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Calculate final frequency:
            // - Base frequency (from param or freq_in)
            // - CV input adds directly to frequency (scaled)
            // - FM input scaled by FM depth
            let freq_cv_hz = freq_cv_value * 1000.0; // CV -1..1 maps to -1000..1000 Hz offset
            let fm_hz = fm_value * fm_depth;
            let final_freq = (effective_base_freq + freq_cv_hz + fm_hz).max(0.0);

            // Generate sine wave sample
            output.samples[i] = (self.phase * TAU).sin();

            // Advance phase
            let phase_increment = final_freq / self.sample_rate;
            self.phase += phase_increment;

            // Wrap phase to [0, 1) to prevent floating point precision issues
            self.phase = self.phase.fract();
            if self.phase < 0.0 {
                self.phase += 1.0;
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_oscillator_info() {
        let osc = SineOscillator::new();
        assert_eq!(osc.info().id, "osc.sine");
        assert_eq!(osc.info().name, "Sine Oscillator");
        assert_eq!(osc.info().category, ModuleCategory::Source);
    }

    #[test]
    fn test_sine_oscillator_ports() {
        let osc = SineOscillator::new();
        let ports = osc.ports();

        assert_eq!(ports.len(), 4);

        // First three are inputs
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "freq_cv");
        assert_eq!(ports[0].signal_type, SignalType::Control);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "fm");
        assert_eq!(ports[1].signal_type, SignalType::Control);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "freq_in");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        // Fourth is output
        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "out");
        assert_eq!(ports[3].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_sine_oscillator_parameters() {
        let osc = SineOscillator::new();
        let params = osc.parameters();

        assert_eq!(params.len(), 2);

        // Frequency parameter
        assert_eq!(params[0].id, "frequency");
        assert_eq!(params[0].min, 20.0);
        assert_eq!(params[0].max, 20000.0);
        assert_eq!(params[0].default, 440.0);

        // FM Depth parameter
        assert_eq!(params[1].id, "fm_depth");
        assert_eq!(params[1].min, 0.0);
        assert_eq!(params[1].max, 1000.0);
        assert_eq!(params[1].default, 0.0);
    }

    #[test]
    fn test_sine_oscillator_generates_output() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Empty inputs (no CV, no FM)
        let inputs: Vec<&SignalBuffer> = vec![];
        osc.process(&inputs, &mut outputs, &[440.0, 0.0], &ctx);

        // Output should not be all zeros
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_nonzero, "Oscillator should produce non-zero output");

        // Output should be within valid audio range
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_sine_oscillator_correct_frequency() {
        let mut osc = SineOscillator::new();
        let sample_rate = 44100.0;
        osc.prepare(sample_rate, 4410);

        // Generate 0.1 second of audio at 440 Hz
        let num_samples = (sample_rate * 0.1) as usize; // 0.1 second = 4410 samples

        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(sample_rate, num_samples);

        osc.process(&[], &mut outputs, &[440.0, 0.0], &ctx);

        // Count zero crossings (going positive)
        let mut zero_crossings = 0;
        for i in 1..num_samples {
            if outputs[0].samples[i - 1] <= 0.0 && outputs[0].samples[i] > 0.0 {
                zero_crossings += 1;
            }
        }

        // At 440 Hz for 0.1 seconds, we expect ~44 cycles
        let expected_cycles = 440.0 * 0.1;
        let tolerance = 2; // Allow some tolerance for phase
        assert!(
            (zero_crossings as f32 - expected_cycles).abs() < tolerance as f32,
            "Expected ~{} zero crossings, got {}",
            expected_cycles,
            zero_crossings
        );
    }

    #[test]
    fn test_sine_oscillator_reset() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 256);

        // Generate some samples to advance phase
        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        osc.process(&[], &mut outputs, &[440.0, 0.0], &ctx);

        // Reset should bring phase back to 0
        osc.reset();

        // Generate first sample after reset - should start at sin(0) = 0
        let mut outputs2 = vec![SignalBuffer::audio(1)];
        let ctx2 = ProcessContext::new(44100.0, 1);
        osc.process(&[], &mut outputs2, &[440.0, 0.0], &ctx2);

        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "First sample after reset should be near 0, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_sine_oscillator_fm_modulation() {
        let mut osc = SineOscillator::new();
        let sample_rate = 44100.0;
        osc.prepare(sample_rate, 256);

        // Create dummy freq_cv input (first input)
        let freq_cv_input = SignalBuffer::control(256);

        // Create FM input with constant value (second input)
        let mut fm_input = SignalBuffer::control(256);
        fm_input.fill(1.0); // Max FM

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(sample_rate, 256);

        // With FM depth of 100 Hz and FM input of 1.0, frequency should be 440 + 100 = 540 Hz
        osc.process(&[&freq_cv_input, &fm_input], &mut outputs, &[440.0, 100.0], &ctx);

        // Output should be valid
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "FM modulated sample out of range"
            );
        }

        // Output should not be all zeros
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_nonzero, "FM modulated oscillator should produce non-zero output");
    }

    #[test]
    fn test_sine_oscillator_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SineOscillator>();
    }

    #[test]
    fn test_sine_oscillator_default() {
        let osc = SineOscillator::default();
        assert_eq!(osc.info().id, "osc.sine");
    }

    #[test]
    fn test_sine_oscillator_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<SineOscillator>();

        assert!(registry.contains("osc.sine"));

        let module = registry.create("osc.sine");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "osc.sine");
        assert_eq!(module.info().name, "Sine Oscillator");
        assert_eq!(module.ports().len(), 4);
        assert_eq!(module.parameters().len(), 2);
    }
}
