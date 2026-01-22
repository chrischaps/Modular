//! 2-Channel Summing Mixer module.
//!
//! Combines two audio signals with independent level controls.
//! Essential for mixing multiple sound sources before the output.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// A 2-channel summing mixer.
///
/// This utility module takes two audio inputs and sums them together,
/// with independent level controls for each channel. The mixed signal
/// is then output through a single audio output.
///
/// # Ports
///
/// **Inputs:**
/// - **Ch 1** (Audio): First audio input channel.
/// - **Ch 2** (Audio): Second audio input channel.
///
/// **Outputs:**
/// - **Out** (Audio): The summed audio output.
///
/// # Parameters
///
/// - **Level 1** (0 to 1): Level for channel 1. Default: 1.0 (unity gain).
/// - **Level 2** (0 to 1): Level for channel 2. Default: 1.0 (unity gain).
pub struct Mixer {
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Smoothed level 1 parameter.
    level1_smooth: SmoothedValue,
    /// Smoothed level 2 parameter.
    level2_smooth: SmoothedValue,
}

impl Mixer {
    /// Creates a new Mixer.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        Self {
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("ch1", "Ch 1", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("ch2", "Ch 2", SignalType::Audio, 0.0),
                // Output port
                PortDefinition::output("out", "Out", SignalType::Audio),
            ],
            parameters: vec![
                // Level 1: 0 to 1
                ParameterDefinition::new(
                    "level1",
                    "Level 1",
                    0.0,
                    1.0,
                    1.0, // Default: unity gain
                    ParameterDisplay::linear(""),
                ),
                // Level 2: 0 to 1
                ParameterDefinition::new(
                    "level2",
                    "Level 2",
                    0.0,
                    1.0,
                    1.0, // Default: unity gain
                    ParameterDisplay::linear(""),
                ),
            ],
            sample_rate,
            // Initialize smoothed parameters
            level1_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            level2_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
        }
    }

    /// Port index constants.
    const PORT_CH1: usize = 0;
    const PORT_CH2: usize = 1;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_LEVEL1: usize = 0;
    const PARAM_LEVEL2: usize = 1;
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Mixer {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "util.mixer",
            name: "Mixer",
            category: ModuleCategory::Utility,
            description: "2-channel summing mixer",
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
        // Update sample rate for smoothed parameters
        self.level1_smooth.set_sample_rate(sample_rate);
        self.level2_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Set smoothing targets from parameters
        self.level1_smooth.set_target(params[Self::PARAM_LEVEL1]);
        self.level2_smooth.set_target(params[Self::PARAM_LEVEL2]);

        // Get input buffers
        let ch1 = inputs.get(Self::PORT_CH1);
        let ch2 = inputs.get(Self::PORT_CH2);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed parameter values (per-sample for click-free changes)
            let level1 = self.level1_smooth.next();
            let level2 = self.level2_smooth.next();

            let ch1_value = ch1
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            let ch2_value = ch2
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Sum the channels with their respective levels
            // Soft clip to prevent harsh clipping when summing hot signals
            let mixed = ch1_value * level1 + ch2_value * level2;
            output.samples[i] = soft_clip(mixed);
        }
    }

    fn reset(&mut self) {
        // Reset smoothed parameters to their current targets
        self.level1_smooth.reset(self.level1_smooth.target());
        self.level2_smooth.reset(self.level2_smooth.target());
    }
}

/// Soft clipping function to prevent harsh digital clipping.
/// Uses tanh-style saturation for natural-sounding limiting.
#[inline]
fn soft_clip(x: f32) -> f32 {
    if x.abs() <= 1.0 {
        x
    } else {
        x.signum() * (1.0 + (1.0 - (x.abs() - 1.0).min(1.0) * 0.5))
            .min(1.5)
            .max(1.0)
            * (2.0 / 3.0)
            + x.signum() * (1.0 / 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_info() {
        let mixer = Mixer::new();
        assert_eq!(mixer.info().id, "util.mixer");
        assert_eq!(mixer.info().name, "Mixer");
        assert_eq!(mixer.info().category, ModuleCategory::Utility);
    }

    #[test]
    fn test_mixer_ports() {
        let mixer = Mixer::new();
        let ports = mixer.ports();

        assert_eq!(ports.len(), 3);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "ch1");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "ch2");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        // Output port
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out");
        assert_eq!(ports[2].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_mixer_parameters() {
        let mixer = Mixer::new();
        let params = mixer.parameters();

        assert_eq!(params.len(), 2);

        // Level 1 parameter
        assert_eq!(params[0].id, "level1");
        assert!((params[0].min - 0.0).abs() < f32::EPSILON);
        assert!((params[0].max - 1.0).abs() < f32::EPSILON);
        assert!((params[0].default - 1.0).abs() < f32::EPSILON);

        // Level 2 parameter
        assert_eq!(params[1].id, "level2");
        assert!((params[1].min - 0.0).abs() < f32::EPSILON);
        assert!((params[1].max - 1.0).abs() < f32::EPSILON);
        assert!((params[1].default - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_mixer_summing() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, 256);

        // Create input signals
        let mut ch1 = SignalBuffer::audio(256);
        let mut ch2 = SignalBuffer::audio(256);
        ch1.fill(0.5);
        ch2.fill(0.3);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with unity gain on both channels
        mixer.process(&[&ch1, &ch2], &mut outputs, &[1.0, 1.0], &ctx);

        // Output should be sum of inputs (0.5 + 0.3 = 0.8)
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 0.8).abs() < 0.01,
                "Summing failed at sample {}: got {}",
                i,
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_mixer_level_control() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, 256);

        // Create input signals
        let mut ch1 = SignalBuffer::audio(256);
        let mut ch2 = SignalBuffer::audio(256);
        ch1.fill(1.0);
        ch2.fill(1.0);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process multiple times to let parameter smoothing settle
        for _ in 0..20 {
            mixer.process(&[&ch1, &ch2], &mut outputs, &[0.5, 0.25], &ctx);
        }

        // Output should be 0.5 + 0.25 = 0.75 (check last samples after smoothing)
        for i in 200..256 {
            assert!(
                (outputs[0].samples[i] - 0.75).abs() < 0.01,
                "Level control failed at sample {}: got {}",
                i,
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_mixer_silence_when_muted() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, 256);

        // Create input signals
        let mut ch1 = SignalBuffer::audio(256);
        let mut ch2 = SignalBuffer::audio(256);
        ch1.fill(1.0);
        ch2.fill(1.0);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process multiple times to let parameter smoothing settle
        for _ in 0..20 {
            mixer.process(&[&ch1, &ch2], &mut outputs, &[0.0, 0.0], &ctx);
        }

        // Output should be silent (check last samples after smoothing)
        for i in 200..256 {
            assert!(
                outputs[0].samples[i].abs() < 0.01,
                "Should be silent with both levels at 0, got {}",
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_mixer_single_input() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, 256);

        // Create only channel 1 input
        let mut ch1 = SignalBuffer::audio(256);
        ch1.fill(0.7);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with only ch1 connected
        mixer.process(&[&ch1], &mut outputs, &[1.0, 1.0], &ctx);

        // Output should be just ch1
        for i in 0..256 {
            assert!(
                (outputs[0].samples[i] - 0.7).abs() < 0.01,
                "Single input failed at sample {}: got {}",
                i,
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_mixer_no_inputs() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with no inputs
        mixer.process(&[], &mut outputs, &[1.0, 1.0], &ctx);

        // Output should be silent
        for i in 0..256 {
            assert!(
                outputs[0].samples[i].abs() < f32::EPSILON,
                "No-input should be silent, got {}",
                outputs[0].samples[i]
            );
        }
    }

    #[test]
    fn test_mixer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Mixer>();
    }

    #[test]
    fn test_mixer_default() {
        let mixer = Mixer::default();
        assert_eq!(mixer.info().id, "util.mixer");
    }

    #[test]
    fn test_mixer_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Mixer>();

        assert!(registry.contains("util.mixer"));

        let module = registry.create("util.mixer");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "util.mixer");
        assert_eq!(module.info().name, "Mixer");
        assert_eq!(module.ports().len(), 3); // 2 inputs + 1 output
        assert_eq!(module.parameters().len(), 2); // Level 1, Level 2
    }
}
