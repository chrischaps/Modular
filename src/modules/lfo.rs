//! LFO (Low Frequency Oscillator) module.
//!
//! Generates low-frequency control signals for modulation purposes.

use std::f32::consts::TAU;

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
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
/// Users can select the waveform shape and synchronize multiple LFOs together.
///
/// # Ports
///
/// **Inputs:**
/// - **Rate** (Control): CV input for rate modulation. Exponential scaling.
/// - **Sync** (Gate): Resets phase to offset on rising edge.
///
/// **Outputs:**
/// - **Out** (Control): The modulation signal.
///
/// # Parameters
///
/// - **Rate** (0.01-100 Hz): Speed of the LFO oscillation. Logarithmic display.
/// - **Waveform** (0-3): Shape of the output (Sine, Triangle, Square, Saw).
/// - **Phase** (0-360°): Phase offset for waveform start point.
/// - **Bipolar** (toggle): When on, output is -1 to +1. When off, output is 0 to +1.
pub struct Lfo {
    /// Current phase (0.0 to 1.0).
    phase: f32,
    /// Previous sync state for edge detection.
    prev_sync: bool,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
    /// Smoothed rate parameter.
    rate_smooth: SmoothedValue,
    /// Smoothed phase offset parameter.
    phase_offset_smooth: SmoothedValue,
}

impl Lfo {
    /// Creates a new LFO.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        Self {
            phase: 0.0,
            prev_sync: false,
            sample_rate,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("rate_cv", "Rate", SignalType::Control, 0.0),
                PortDefinition::input_with_default("sync", "Sync", SignalType::Gate, 0.0),
                // Single output port
                PortDefinition::output("out", "Out", SignalType::Control),
            ],
            parameters: vec![
                // Rate parameter (logarithmic for musical response)
                ParameterDefinition::new(
                    "rate",
                    "Rate",
                    0.01,
                    100.0,
                    1.0, // Default 1 Hz
                    ParameterDisplay::logarithmic("Hz"),
                ),
                // Waveform selection
                ParameterDefinition::choice(
                    "waveform",
                    "Waveform",
                    &["Sine", "Triangle", "Square", "Saw"],
                    0, // Default Sine
                ),
                // Phase offset (0-360 degrees)
                ParameterDefinition::new(
                    "phase",
                    "Phase",
                    0.0,
                    360.0,
                    0.0,
                    ParameterDisplay::linear("°"),
                ),
                // Bipolar toggle
                ParameterDefinition::toggle("bipolar", "Bipolar", true),
            ],
            // Initialize smoothed parameters
            rate_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            phase_offset_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
        }
    }

    /// Port index constants.
    const PORT_RATE_CV: usize = 0;
    const PORT_SYNC: usize = 1;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_RATE: usize = 0;
    const PARAM_WAVEFORM: usize = 1;
    const PARAM_PHASE: usize = 2;
    const PARAM_BIPOLAR: usize = 3;

    /// Sync threshold for detecting high/low states.
    const SYNC_THRESHOLD: f32 = 0.5;

    /// Generate a sample for the given phase (0.0-1.0) and waveform.
    /// Returns value in bipolar form (-1 to +1).
    fn generate_sample(phase: f32, waveform: LfoWaveform) -> f32 {
        match waveform {
            LfoWaveform::Sine => (phase * TAU).sin(),
            LfoWaveform::Triangle => {
                // Triangle: 0->0.25 ramps 0->1, 0.25->0.75 ramps 1->-1, 0.75->1 ramps -1->0
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
                // Sawtooth: linear ramp from -1 to +1
                2.0 * phase - 1.0
            }
        }
    }

    /// Convert bipolar signal (-1 to +1) to unipolar (0 to +1).
    #[inline]
    fn to_unipolar(value: f32) -> f32 {
        (value + 1.0) * 0.5
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
        // Update sample rate for smoothed parameters
        self.rate_smooth.set_sample_rate(sample_rate);
        self.phase_offset_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Set smoothing targets from parameters
        self.rate_smooth.set_target(params[Self::PARAM_RATE]);
        self.phase_offset_smooth.set_target(params[Self::PARAM_PHASE] / 360.0); // Convert degrees to 0-1

        // Discrete parameters don't need smoothing
        let waveform = LfoWaveform::from_param(params[Self::PARAM_WAVEFORM]);
        let is_bipolar = params[Self::PARAM_BIPOLAR] > 0.5;

        // Get input buffers
        let rate_cv = inputs.get(Self::PORT_RATE_CV);
        let sync_in = inputs.get(Self::PORT_SYNC);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed parameter values (per-sample for click-free changes)
            let base_rate = self.rate_smooth.next();
            let phase_offset = self.phase_offset_smooth.next();

            // Check for sync reset (rising edge detection)
            let sync_value = sync_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let sync_high = sync_value > Self::SYNC_THRESHOLD;
            let sync_rising = sync_high && !self.prev_sync;
            self.prev_sync = sync_high;

            // Reset phase on sync rising edge
            if sync_rising {
                self.phase = 0.0;
            }

            // Calculate effective phase with offset
            let effective_phase = (self.phase + phase_offset).fract();

            // Generate the waveform sample
            let sample = Self::generate_sample(effective_phase, waveform);

            // Apply bipolar/unipolar scaling
            output.samples[i] = if is_bipolar {
                sample
            } else {
                Self::to_unipolar(sample)
            };

            // Get rate CV modulation
            let rate_cv_value = rate_cv
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Rate CV scales the rate multiplicatively
            // CV of 0 = base rate, CV of +1 = double rate, CV of -1 = half rate
            let rate_multiplier = 2.0_f32.powf(rate_cv_value);
            let final_rate = (base_rate * rate_multiplier).clamp(0.001, 1000.0);

            // Advance phase
            let phase_increment = final_rate / self.sample_rate;
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
        self.prev_sync = false;
        // Reset smoothed parameters to their current targets
        self.rate_smooth.reset(self.rate_smooth.target());
        self.phase_offset_smooth.reset(self.phase_offset_smooth.target());
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

        // 2 inputs + 1 output = 3 ports
        assert_eq!(ports.len(), 3);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "rate_cv");
        assert_eq!(ports[0].signal_type, SignalType::Control);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "sync");
        assert_eq!(ports[1].signal_type, SignalType::Gate);

        // Output port
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out");
        assert_eq!(ports[2].signal_type, SignalType::Control);
    }

    #[test]
    fn test_lfo_parameters() {
        let lfo = Lfo::new();
        let params = lfo.parameters();

        assert_eq!(params.len(), 4);

        // Rate parameter
        assert_eq!(params[0].id, "rate");
        assert!((params[0].min - 0.01).abs() < f32::EPSILON);
        assert!((params[0].max - 100.0).abs() < f32::EPSILON);
        assert!((params[0].default - 1.0).abs() < f32::EPSILON);

        // Waveform parameter
        assert_eq!(params[1].id, "waveform");
        assert!((params[1].min - 0.0).abs() < f32::EPSILON);
        assert!((params[1].max - 3.0).abs() < f32::EPSILON);

        // Phase parameter
        assert_eq!(params[2].id, "phase");
        assert!((params[2].min - 0.0).abs() < f32::EPSILON);
        assert!((params[2].max - 360.0).abs() < f32::EPSILON);

        // Bipolar parameter
        assert_eq!(params[3].id, "bipolar");
        assert!((params[3].default - 1.0).abs() < f32::EPSILON); // Default on
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
    fn test_lfo_generates_output() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // 1 Hz, Sine, 0° phase, bipolar
        lfo.process(&[], &mut outputs, &[1.0, 0.0, 0.0, 1.0], &ctx);

        // Output should have non-zero values
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_nonzero, "LFO should produce non-zero signal");
    }

    #[test]
    fn test_lfo_bipolar_range() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // 1 Hz, Sine, 0° phase, bipolar
        lfo.process(&[], &mut outputs, &[1.0, 0.0, 0.0, 1.0], &ctx);

        // Output should be within -1 to +1
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Bipolar sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_lfo_unipolar_range() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // 1 Hz, Sine, 0° phase, unipolar (bipolar = 0)
        lfo.process(&[], &mut outputs, &[1.0, 0.0, 0.0, 0.0], &ctx);

        // Output should be within 0 to +1
        for &sample in &outputs[0].samples {
            assert!(
                sample >= 0.0 && sample <= 1.0,
                "Unipolar sample {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_lfo_sync_reset() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 1000);

        // Run LFO to advance phase
        let mut outputs = vec![SignalBuffer::control(1000)];
        let ctx = ProcessContext::new(44100.0, 1000);
        lfo.process(&[], &mut outputs, &[1.0, 0.0, 0.0, 1.0], &ctx);

        // Now send a sync pulse
        let mut sync = SignalBuffer::control(100);
        sync.samples[50] = 1.0; // Rising edge at sample 50

        let rate_cv = SignalBuffer::control(100);
        let mut outputs2 = vec![SignalBuffer::control(100)];
        let ctx2 = ProcessContext::new(44100.0, 100);
        lfo.process(&[&rate_cv, &sync], &mut outputs2, &[1.0, 0.0, 0.0, 1.0], &ctx2);

        // After sync, sine should be near 0 (sin(0) = 0)
        assert!(
            outputs2[0].samples[51].abs() < 0.1,
            "Sine should be near 0 after sync reset, got {}",
            outputs2[0].samples[51]
        );
    }

    #[test]
    fn test_lfo_phase_offset() {
        let mut lfo1 = Lfo::new();
        let mut lfo2 = Lfo::new();
        lfo1.prepare(44100.0, 256);
        lfo2.prepare(44100.0, 256);

        let mut outputs1 = vec![SignalBuffer::control(256)];
        let mut outputs2 = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process multiple times to let parameter smoothing settle
        // Use very slow rate (0.01 Hz) so phase doesn't advance much
        for _ in 0..20 {
            lfo1.reset();
            lfo1.process(&[], &mut outputs1, &[0.01, 0.0, 0.0, 1.0], &ctx);
        }
        for _ in 0..20 {
            lfo2.reset();
            lfo2.process(&[], &mut outputs2, &[0.01, 0.0, 90.0, 1.0], &ctx);
        }

        // With slow rate and reset, check that outputs show offset effect
        // After smoothing settles, sine with 0° offset should start near 0
        // and sine with 90° offset should start near 1
        assert!(
            outputs1[0].samples[200].abs() < 0.2,
            "Sine at 0° should be near 0, got {}",
            outputs1[0].samples[200]
        );

        // With 90° offset, sine should be near 1
        assert!(
            outputs2[0].samples[200] > 0.8,
            "Sine at 90° should be near 1, got {}",
            outputs2[0].samples[200]
        );
    }

    #[test]
    fn test_lfo_rate_cv_modulation() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        // Create rate CV input with +1 value (should double the rate)
        let mut rate_cv = SignalBuffer::control(44100);
        rate_cv.fill(1.0);

        let sync = SignalBuffer::control(44100);
        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // Base rate 1 Hz with +1 CV = 2 Hz
        lfo.process(&[&rate_cv, &sync], &mut outputs, &[1.0, 0.0, 0.0, 1.0], &ctx);

        // Count zero crossings of sine to verify doubled rate
        let mut zero_crossings = 0;
        for i in 1..44100 {
            if outputs[0].samples[i - 1] <= 0.0 && outputs[0].samples[i] > 0.0 {
                zero_crossings += 1;
            }
        }

        // At 2 Hz for 1 second, expect ~2 cycles
        assert!(
            zero_crossings >= 1 && zero_crossings <= 3,
            "Expected ~2 zero crossings for 2 Hz, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_lfo_waveform_shapes() {
        // Test that waveform generation produces correct shapes

        // Sine at key phases
        assert!((Lfo::generate_sample(0.0, LfoWaveform::Sine) - 0.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.25, LfoWaveform::Sine) - 1.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.5, LfoWaveform::Sine) - 0.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.75, LfoWaveform::Sine) - (-1.0)).abs() < 0.01);

        // Triangle at key phases
        assert!((Lfo::generate_sample(0.0, LfoWaveform::Triangle) - 0.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.25, LfoWaveform::Triangle) - 1.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.5, LfoWaveform::Triangle) - 0.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.75, LfoWaveform::Triangle) - (-1.0)).abs() < 0.01);

        // Square at key phases
        assert!((Lfo::generate_sample(0.0, LfoWaveform::Square) - 1.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.25, LfoWaveform::Square) - 1.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.5, LfoWaveform::Square) - (-1.0)).abs() < 0.01);
        assert!((Lfo::generate_sample(0.75, LfoWaveform::Square) - (-1.0)).abs() < 0.01);

        // Saw at key phases
        assert!((Lfo::generate_sample(0.0, LfoWaveform::Saw) - (-1.0)).abs() < 0.01);
        assert!((Lfo::generate_sample(0.25, LfoWaveform::Saw) - (-0.5)).abs() < 0.01);
        assert!((Lfo::generate_sample(0.5, LfoWaveform::Saw) - 0.0).abs() < 0.01);
        assert!((Lfo::generate_sample(0.75, LfoWaveform::Saw) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_lfo_different_waveforms() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        // Test each waveform produces different output
        let mut sine_out = vec![SignalBuffer::control(44100)];
        let mut tri_out = vec![SignalBuffer::control(44100)];
        let mut sq_out = vec![SignalBuffer::control(44100)];
        let mut saw_out = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        lfo.reset();
        lfo.process(&[], &mut sine_out, &[1.0, 0.0, 0.0, 1.0], &ctx);
        lfo.reset();
        lfo.process(&[], &mut tri_out, &[1.0, 1.0, 0.0, 1.0], &ctx);
        lfo.reset();
        lfo.process(&[], &mut sq_out, &[1.0, 2.0, 0.0, 1.0], &ctx);
        lfo.reset();
        lfo.process(&[], &mut saw_out, &[1.0, 3.0, 0.0, 1.0], &ctx);

        // Verify waveforms are different by comparing at quarter period
        let quarter = 11025; // 1/4 second at 44100 Hz

        // At quarter period: sine=1, tri=1, square=1, saw=-0.5
        assert!((sine_out[0].samples[quarter] - 1.0).abs() < 0.1);
        assert!((tri_out[0].samples[quarter] - 1.0).abs() < 0.1);
        assert!((sq_out[0].samples[quarter] - 1.0).abs() < 0.1);
        assert!((saw_out[0].samples[quarter] - (-0.5)).abs() < 0.1);
    }

    #[test]
    fn test_lfo_reset() {
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 256);

        // Generate some samples
        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        lfo.process(&[], &mut outputs, &[1.0, 0.0, 0.0, 1.0], &ctx);

        // Reset
        lfo.reset();

        // Generate first sample after reset - sine should be near 0
        let mut outputs2 = vec![SignalBuffer::control(1)];
        let ctx2 = ProcessContext::new(44100.0, 1);
        lfo.process(&[], &mut outputs2, &[1.0, 0.0, 0.0, 1.0], &ctx2);

        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Sine after reset should be near 0"
        );
    }

    #[test]
    fn test_lfo_high_rate() {
        // Test that LFO works correctly at higher rates (100 Hz)
        let mut lfo = Lfo::new();
        lfo.prepare(44100.0, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(44100.0, 44100);

        // 100 Hz rate
        lfo.process(&[], &mut outputs, &[100.0, 0.0, 0.0, 1.0], &ctx);

        // Count zero crossings - should be ~100 for 100 Hz
        let mut zero_crossings = 0;
        for i in 1..44100 {
            if outputs[0].samples[i - 1] <= 0.0 && outputs[0].samples[i] > 0.0 {
                zero_crossings += 1;
            }
        }

        // Allow some tolerance
        assert!(
            zero_crossings >= 95 && zero_crossings <= 105,
            "Expected ~100 cycles at 100 Hz, got {}",
            zero_crossings
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
        assert_eq!(module.ports().len(), 3); // 2 inputs + 1 output
        assert_eq!(module.parameters().len(), 4); // Rate, Waveform, Phase, Bipolar
    }
}
