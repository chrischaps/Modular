//! LFO (Low Frequency Oscillator) module.
//!
//! Generates low-frequency control signals for modulation purposes.

use std::f32::consts::TAU;

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    SignalType,
};

/// Waveform shapes for the LFO.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LfoWaveform {
    Sine = 0,
    Triangle = 1,
    Square = 2,
    Saw = 3,
}

impl LfoWaveform {
    /// Convert from parameter value (0-3) to waveform.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => LfoWaveform::Sine,
            1 => LfoWaveform::Triangle,
            2 => LfoWaveform::Square,
            3 => LfoWaveform::Saw,
            _ => LfoWaveform::Sine,
        }
    }
}

/// A low-frequency oscillator for modulation.
///
/// Generates control-rate signals for modulating other module parameters.
/// Output range is -1.0 to 1.0 for bipolar modulation.
///
/// # Ports
///
/// - **Out** (Control, Output): The modulation signal (-1.0 to 1.0).
///
/// # Parameters
///
/// - **Rate** (0.01-20 Hz): Speed of the LFO oscillation.
/// - **Waveform** (0-3): Shape of the waveform (Sine, Triangle, Square, Saw).
pub struct Lfo {
    /// Current phase (0.0 to 1.0).
    phase: f32,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Lfo {
    /// Creates a new LFO.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            sample_rate: 44100.0,
            ports: vec![
                // Output port
                PortDefinition::output("out", "Out", SignalType::Control),
            ],
            parameters: vec![
                // Rate parameter (logarithmic feel, but simple linear for now)
                ParameterDefinition::new(
                    "rate",
                    "Rate",
                    0.01,
                    20.0,
                    1.0, // Default 1 Hz
                    crate::dsp::ParameterDisplay::linear("Hz"),
                ),
                // Waveform selection (0=Sine, 1=Triangle, 2=Square, 3=Saw)
                ParameterDefinition::choice(
                    "waveform",
                    "Waveform",
                    &["Sine", "Triangle", "Square", "Saw"],
                    0, // Default Sine
                ),
            ],
        }
    }

    /// Parameter index constants.
    const PARAM_RATE: usize = 0;
    const PARAM_WAVEFORM: usize = 1;

    /// Port index constants.
    const PORT_OUT: usize = 0;

    /// Generate a sample for the given phase (0.0-1.0) and waveform.
    fn generate_sample(phase: f32, waveform: LfoWaveform) -> f32 {
        match waveform {
            LfoWaveform::Sine => (phase * TAU).sin(),
            LfoWaveform::Triangle => {
                // Triangle: 0->0.25 = 0->1, 0.25->0.75 = 1->-1, 0.75->1 = -1->0
                if phase < 0.25 {
                    phase * 4.0
                } else if phase < 0.75 {
                    1.0 - (phase - 0.25) * 4.0
                } else {
                    -1.0 + (phase - 0.75) * 4.0
                }
            }
            LfoWaveform::Square => {
                if phase < 0.5 { 1.0 } else { -1.0 }
            }
            LfoWaveform::Saw => {
                // Sawtooth: 0->1 = 0->1, then wraps to -1
                2.0 * phase - 1.0
            }
        }
    }
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Lfo {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "mod.lfo",
            name: "LFO",
            category: ModuleCategory::Modulation,
            description: "Low frequency oscillator for modulation",
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
        _inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let rate = params[Self::PARAM_RATE];
        let waveform = LfoWaveform::from_param(params[Self::PARAM_WAVEFORM]);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Generate the waveform sample
            output.samples[i] = Self::generate_sample(self.phase, waveform);

            // Advance phase
            let phase_increment = rate / self.sample_rate;
            self.phase += phase_increment;

            // Wrap phase to [0, 1)
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
    fn test_lfo_info() {
        let lfo = Lfo::new();
        assert_eq!(lfo.info().id, "mod.lfo");
        assert_eq!(lfo.info().name, "LFO");
        assert_eq!(lfo.info().category, ModuleCategory::Modulation);
    }

    #[test]
    fn test_lfo_ports() {
        let lfo = Lfo::new();
        let ports = lfo.ports();

        assert_eq!(ports.len(), 1);
        assert!(ports[0].is_output());
        assert_eq!(ports[0].id, "out");
        assert_eq!(ports[0].signal_type, SignalType::Control);
    }

    #[test]
    fn test_lfo_parameters() {
        let lfo = Lfo::new();
        let params = lfo.parameters();

        assert_eq!(params.len(), 2);

        // Rate parameter
        assert_eq!(params[0].id, "rate");
        assert!((params[0].min - 0.01).abs() < f32::EPSILON);
        assert!((params[0].max - 20.0).abs() < f32::EPSILON);
        assert!((params[0].default - 1.0).abs() < f32::EPSILON);

        // Waveform parameter
        assert_eq!(params[1].id, "waveform");
        assert!((params[1].min - 0.0).abs() < f32::EPSILON);
        assert!((params[1].max - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_lfo_generates_output() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Sine wave at 1 Hz
        lfo.process(&[], &mut outputs, &[1.0, 0.0], &ctx);

        // Output should not be all zeros
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_nonzero, "LFO should produce non-zero output");

        // Output should be within valid control range (-1 to 1)
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_lfo_waveform_conversion() {
        assert_eq!(LfoWaveform::from_param(0.0), LfoWaveform::Sine);
        assert_eq!(LfoWaveform::from_param(1.0), LfoWaveform::Triangle);
        assert_eq!(LfoWaveform::from_param(2.0), LfoWaveform::Square);
        assert_eq!(LfoWaveform::from_param(3.0), LfoWaveform::Saw);
        assert_eq!(LfoWaveform::from_param(99.0), LfoWaveform::Sine); // Out of range defaults to Sine
    }

    #[test]
    fn test_lfo_sine_wave() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // Generate 1 second of sine wave at 1 Hz
        lfo.process(&[], &mut outputs, &[1.0, 0.0], &ctx);

        // First sample should be near 0 (sin(0) = 0)
        assert!(outputs[0].samples[0].abs() < 0.01, "First sample should be near 0");

        // At 1/4 period (11025 samples), should be near 1
        assert!(
            (outputs[0].samples[11025] - 1.0).abs() < 0.01,
            "Sample at 1/4 period should be near 1"
        );
    }

    #[test]
    fn test_lfo_square_wave() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // Generate 1 second of square wave at 1 Hz
        lfo.process(&[], &mut outputs, &[1.0, 2.0], &ctx); // 2.0 = Square

        // First half should be 1.0
        assert!(
            (outputs[0].samples[11025] - 1.0).abs() < 0.01,
            "First half should be 1.0"
        );

        // Second half should be -1.0
        assert!(
            (outputs[0].samples[33075] - (-1.0)).abs() < 0.01,
            "Second half should be -1.0"
        );
    }

    #[test]
    fn test_lfo_reset() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 256);

        // Generate some samples
        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        lfo.process(&[], &mut outputs, &[1.0, 0.0], &ctx);

        // Reset
        lfo.reset();

        // Generate first sample after reset - should be sin(0) = 0
        let mut outputs2 = vec![SignalBuffer::control(1)];
        let ctx2 = ProcessContext::new(44100.0, 1);
        lfo.process(&[], &mut outputs2, &[1.0, 0.0], &ctx2);

        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "First sample after reset should be near 0"
        );
    }

    #[test]
    fn test_lfo_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Lfo>();
    }

    #[test]
    fn test_lfo_default() {
        let lfo = Lfo::default();
        assert_eq!(lfo.info().id, "mod.lfo");
    }

    #[test]
    fn test_lfo_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Lfo>();

        assert!(registry.contains("mod.lfo"));

        let module = registry.create("mod.lfo");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "mod.lfo");
        assert_eq!(module.info().name, "LFO");
        assert_eq!(module.ports().len(), 1);
        assert_eq!(module.parameters().len(), 2);
    }
}
