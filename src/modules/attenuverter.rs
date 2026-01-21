//! Attenuverter utility module.
//!
//! Scales and optionally inverts control signals.
//! Essential for adjusting modulation depth before sending to destinations.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// An attenuverter for scaling and inverting control signals.
///
/// This utility module takes an input signal and scales it by an amount
/// from -1 to +1. At +1, the signal passes through unchanged. At -1, the
/// signal is inverted. At 0, the output is silent.
///
/// Use this to control modulation depth before sending LFOs or envelopes
/// to oscillator pitch, filter cutoff, or other destinations.
///
/// # Ports
///
/// **Inputs:**
/// - **In** (Control): The input signal to attenuate/invert.
///
/// **Outputs:**
/// - **Out** (Control): The scaled output signal.
///
/// # Parameters
///
/// - **Amount** (-1 to +1): Scaling factor. Positive values preserve polarity,
///   negative values invert the signal. 0 = silence.
/// - **Offset** (-1 to +1): DC offset added after scaling. Useful for shifting
///   unipolar signals to bipolar or vice versa.
pub struct Attenuverter {
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Attenuverter {
    /// Creates a new Attenuverter.
    pub fn new() -> Self {
        Self {
            ports: vec![
                // Input port
                PortDefinition::input_with_default("in", "In", SignalType::Control, 0.0),
                // Output port
                PortDefinition::output("out", "Out", SignalType::Control),
            ],
            parameters: vec![
                // Amount: -1 to +1 (bipolar attenuverter)
                ParameterDefinition::new(
                    "amount",
                    "Amount",
                    -1.0,
                    1.0,
                    1.0, // Default: pass-through
                    ParameterDisplay::linear(""),
                ),
                // Offset: -1 to +1 DC offset
                ParameterDefinition::new(
                    "offset",
                    "Offset",
                    -1.0,
                    1.0,
                    0.0, // Default: no offset
                    ParameterDisplay::linear(""),
                ),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_AMOUNT: usize = 0;
    const PARAM_OFFSET: usize = 1;
}

impl Default for Attenuverter {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Attenuverter {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "util.attenuverter",
            name: "Attenuverter",
            category: ModuleCategory::Utility,
            description: "Scale and invert control signals",
        };
        &INFO
    }

    fn ports(&self) -> &[PortDefinition] {
        &self.ports
    }

    fn parameters(&self) -> &[ParameterDefinition] {
        &self.parameters
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {
        // No preparation needed
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let amount = params[Self::PARAM_AMOUNT];
        let offset = params[Self::PARAM_OFFSET];

        // Get input buffer
        let input = inputs.get(Self::PORT_IN);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            let in_value = input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Apply scaling and offset, clamp to valid control range
            let out_value = (in_value * amount + offset).clamp(-1.0, 1.0);
            output.samples[i] = out_value;
        }
    }

    fn reset(&mut self) {
        // No state to reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attenuverter_info() {
        let att = Attenuverter::new();
        assert_eq!(att.info().id, "util.attenuverter");
        assert_eq!(att.info().name, "Attenuverter");
        assert_eq!(att.info().category, ModuleCategory::Utility);
    }

    #[test]
    fn test_attenuverter_ports() {
        let att = Attenuverter::new();
        let ports = att.ports();

        assert_eq!(ports.len(), 2);

        // Input port
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in");
        assert_eq!(ports[0].signal_type, SignalType::Control);

        // Output port
        assert!(ports[1].is_output());
        assert_eq!(ports[1].id, "out");
        assert_eq!(ports[1].signal_type, SignalType::Control);
    }

    #[test]
    fn test_attenuverter_parameters() {
        let att = Attenuverter::new();
        let params = att.parameters();

        assert_eq!(params.len(), 2);

        // Amount parameter
        assert_eq!(params[0].id, "amount");
        assert!((params[0].min - (-1.0)).abs() < f32::EPSILON);
        assert!((params[0].max - 1.0).abs() < f32::EPSILON);
        assert!((params[0].default - 1.0).abs() < f32::EPSILON);

        // Offset parameter
        assert_eq!(params[1].id, "offset");
        assert!((params[1].min - (-1.0)).abs() < f32::EPSILON);
        assert!((params[1].max - 1.0).abs() < f32::EPSILON);
        assert!((params[1].default - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_attenuverter_passthrough() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create input signal
        let mut input = SignalBuffer::control(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 / 256.0) * 2.0 - 1.0; // -1 to +1
        }

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 1, Offset = 0 (passthrough)
        att.process(&[&input], &mut outputs, &[1.0, 0.0], &ctx);

        // Output should equal input
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - input.samples[i]).abs() < 0.001,
                "Passthrough failed at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_attenuverter_invert() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create input signal
        let mut input = SignalBuffer::control(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 / 256.0) * 2.0 - 1.0; // -1 to +1
        }

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = -1, Offset = 0 (invert)
        att.process(&[&input], &mut outputs, &[-1.0, 0.0], &ctx);

        // Output should be inverted input
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - (-input.samples[i])).abs() < 0.001,
                "Inversion failed at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_attenuverter_half_amplitude() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create full-scale input
        let mut input = SignalBuffer::control(256);
        input.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 0.5, Offset = 0
        att.process(&[&input], &mut outputs, &[0.5, 0.0], &ctx);

        // Output should be 0.5
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 0.5).abs() < 0.001,
                "Half amplitude failed at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_attenuverter_zero_amount() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create input signal
        let mut input = SignalBuffer::control(256);
        input.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 0, Offset = 0 (silence)
        att.process(&[&input], &mut outputs, &[0.0, 0.0], &ctx);

        // Output should be 0
        for i in 0..256 {
            assert!(
                outputs[0].samples[i].abs() < 0.001,
                "Zero amount should produce silence"
            );
        }
    }

    #[test]
    fn test_attenuverter_offset() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create zero input
        let input = SignalBuffer::control(256);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 1, Offset = 0.5
        att.process(&[&input], &mut outputs, &[1.0, 0.5], &ctx);

        // Output should be 0.5
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 0.5).abs() < 0.001,
                "Offset failed at sample {}",
                i
            );
        }
    }

    #[test]
    fn test_attenuverter_bipolar_to_unipolar() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create bipolar LFO-like signal (-1 to +1)
        let mut input = SignalBuffer::control(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 / 256.0) * 2.0 - 1.0;
        }

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 0.5, Offset = 0.5 -> converts -1..+1 to 0..+1
        att.process(&[&input], &mut outputs, &[0.5, 0.5], &ctx);

        // Output should be in 0..+1 range
        for i in 0..256 {
            assert!(
                outputs[0].samples[i] >= 0.0 && outputs[0].samples[i] <= 1.0,
                "Bipolar to unipolar conversion failed at sample {}: {}",
                i,
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_attenuverter_output_clamping() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        // Create full-scale input
        let mut input = SignalBuffer::control(256);
        input.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Amount = 1, Offset = 1 -> would be 2.0, but should clamp to 1.0
        att.process(&[&input], &mut outputs, &[1.0, 1.0], &ctx);

        // Output should be clamped to 1.0
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 1.0).abs() < 0.001,
                "Clamping failed at sample {}: {}",
                i,
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_attenuverter_no_input() {
        let mut att = Attenuverter::new();
        att.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // No input connected, Amount = 1, Offset = 0.25
        att.process(&[], &mut outputs, &[1.0, 0.25], &ctx);

        // Output should be just the offset
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 0.25).abs() < 0.001,
                "No-input offset failed"
            );
        }
    }

    #[test]
    fn test_attenuverter_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Attenuverter>();
    }

    #[test]
    fn test_attenuverter_default() {
        let att = Attenuverter::default();
        assert_eq!(att.info().id, "util.attenuverter");
    }

    #[test]
    fn test_attenuverter_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Attenuverter>();

        assert!(registry.contains("util.attenuverter"));

        let module = registry.create("util.attenuverter");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "util.attenuverter");
        assert_eq!(module.info().name, "Attenuverter");
        assert_eq!(module.ports().len(), 2); // 1 input + 1 output
        assert_eq!(module.parameters().len(), 2); // Amount, Offset
    }
}
