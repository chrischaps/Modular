//! ADSR Envelope module.
//!
//! Generates an Attack-Decay-Sustain-Release envelope in response to gate signals.
//! Fundamental for shaping sound amplitude and filter cutoff over time.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Envelope stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeStage {
    /// Envelope is idle (output = 0).
    Idle,
    /// Attack phase: rising from 0 to 1.
    Attack,
    /// Decay phase: falling from 1 to sustain level.
    Decay,
    /// Sustain phase: holding at sustain level while gate is high.
    Sustain,
    /// Release phase: falling from current level to 0.
    Release,
}

/// ADSR Envelope generator.
///
/// Generates a control signal that follows the classic ADSR envelope shape:
/// - **Attack**: Time to rise from 0 to 1
/// - **Decay**: Time to fall from 1 to sustain level
/// - **Sustain**: Level to hold while gate is high
/// - **Release**: Time to fall from current level to 0 after gate goes low
///
/// # Ports
///
/// - **Gate** (Gate, Input): Triggers the envelope (high = note on, low = note off).
/// - **Retrigger** (Gate, Input): Restarts attack from current level when high.
/// - **Out** (Control, Output): The envelope output (0.0 to 1.0).
///
/// # Parameters
///
/// - **Attack** (0.001-10.0s): Attack time, logarithmic scaling.
/// - **Decay** (0.001-10.0s): Decay time, logarithmic scaling.
/// - **Sustain** (0.0-1.0): Sustain level, linear scaling.
/// - **Release** (0.001-10.0s): Release time, logarithmic scaling.
pub struct AdsrEnvelope {
    /// Current envelope stage.
    stage: EnvelopeStage,
    /// Current envelope level (0.0 to 1.0).
    level: f32,
    /// Previous gate state (for edge detection).
    prev_gate: bool,
    /// Previous retrigger state (for edge detection).
    prev_retrigger: bool,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl AdsrEnvelope {
    /// Creates a new ADSR envelope.
    pub fn new() -> Self {
        Self {
            stage: EnvelopeStage::Idle,
            level: 0.0,
            prev_gate: false,
            prev_retrigger: false,
            sample_rate: 44100.0,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("gate", "Gate", SignalType::Gate, 0.0),
                PortDefinition::input_with_default("retrigger", "Retrig", SignalType::Gate, 0.0),
                // Output port
                PortDefinition::output("out", "Out", SignalType::Control),
            ],
            parameters: vec![
                // Attack time (1ms to 10s, logarithmic)
                ParameterDefinition::new(
                    "attack",
                    "Attack",
                    0.001,
                    10.0,
                    0.01, // 10ms default
                    ParameterDisplay::logarithmic("s"),
                ),
                // Decay time (1ms to 10s, logarithmic)
                ParameterDefinition::new(
                    "decay",
                    "Decay",
                    0.001,
                    10.0,
                    0.1, // 100ms default
                    ParameterDisplay::logarithmic("s"),
                ),
                // Sustain level (0 to 1, linear)
                ParameterDefinition::new(
                    "sustain",
                    "Sustain",
                    0.0,
                    1.0,
                    0.7, // 70% default
                    ParameterDisplay::linear(""),
                ),
                // Release time (1ms to 10s, logarithmic)
                ParameterDefinition::new(
                    "release",
                    "Release",
                    0.001,
                    10.0,
                    0.3, // 300ms default
                    ParameterDisplay::logarithmic("s"),
                ),
            ],
        }
    }

    /// Port index constants.
    const PORT_GATE: usize = 0;
    const PORT_RETRIGGER: usize = 1;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_ATTACK: usize = 0;
    const PARAM_DECAY: usize = 1;
    const PARAM_SUSTAIN: usize = 2;
    const PARAM_RELEASE: usize = 3;

    /// Gate threshold for detecting high/low states.
    const GATE_THRESHOLD: f32 = 0.5;

    /// Calculate the exponential coefficient for a given time.
    ///
    /// This creates an RC-style exponential curve that reaches ~99.3%
    /// of the target value in the specified time.
    ///
    /// For attack (rising): level = 1.0 - (1.0 - level) * coeff
    /// For decay/release (falling): level = level * coeff
    #[inline]
    fn calc_coeff(&self, time_seconds: f32) -> f32 {
        if time_seconds <= 0.0 {
            return 0.0;
        }
        // Time constant for ~99.3% completion in the given time
        // exp(-5/time) gives us approximately the right decay rate
        let samples = time_seconds * self.sample_rate;
        if samples <= 1.0 {
            return 0.0;
        }
        (-5.0_f32 / samples).exp()
    }

    /// Threshold for considering the envelope "close enough" to target.
    const LEVEL_THRESHOLD: f32 = 0.0001;
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for AdsrEnvelope {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "mod.adsr",
            name: "ADSR Envelope",
            category: ModuleCategory::Modulation,
            description: "Attack-Decay-Sustain-Release envelope generator",
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
        let attack_time = params[Self::PARAM_ATTACK];
        let decay_time = params[Self::PARAM_DECAY];
        let sustain_level = params[Self::PARAM_SUSTAIN].clamp(0.0, 1.0);
        let release_time = params[Self::PARAM_RELEASE];

        // Get input buffers
        let gate_in = inputs.get(Self::PORT_GATE);
        let retrigger_in = inputs.get(Self::PORT_RETRIGGER);

        // Get output buffer
        let output = &mut outputs[Self::PORT_OUT];

        // Pre-calculate coefficients
        let attack_coeff = self.calc_coeff(attack_time);
        let decay_coeff = self.calc_coeff(decay_time);
        let release_coeff = self.calc_coeff(release_time);

        // Process each sample
        for i in 0..context.block_size {
            // Get gate state
            let gate_value = gate_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let gate_high = gate_value > Self::GATE_THRESHOLD;

            // Get retrigger state
            let retrigger_value = retrigger_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let retrigger_high = retrigger_value > Self::GATE_THRESHOLD;

            // Detect gate rising edge (note on)
            let gate_rising = gate_high && !self.prev_gate;

            // Detect retrigger rising edge
            let retrigger_rising = retrigger_high && !self.prev_retrigger;

            // State machine transitions
            match self.stage {
                EnvelopeStage::Idle => {
                    if gate_rising {
                        self.stage = EnvelopeStage::Attack;
                    }
                }
                EnvelopeStage::Attack => {
                    if !gate_high {
                        // Gate went low during attack
                        self.stage = EnvelopeStage::Release;
                    } else if retrigger_rising {
                        // Retrigger: restart attack from current level
                        // (already in attack, just continue)
                    } else {
                        // Exponential rise toward 1.0
                        self.level = 1.0 - (1.0 - self.level) * attack_coeff;

                        // Transition to decay when we reach the top
                        if self.level >= 1.0 - Self::LEVEL_THRESHOLD {
                            self.level = 1.0;
                            self.stage = EnvelopeStage::Decay;
                        }
                    }
                }
                EnvelopeStage::Decay => {
                    if !gate_high {
                        // Gate went low during decay
                        self.stage = EnvelopeStage::Release;
                    } else if retrigger_rising {
                        // Retrigger: restart attack from current level
                        self.stage = EnvelopeStage::Attack;
                    } else {
                        // Exponential fall toward sustain level
                        self.level = sustain_level + (self.level - sustain_level) * decay_coeff;

                        // Transition to sustain when we reach the sustain level
                        if (self.level - sustain_level).abs() < Self::LEVEL_THRESHOLD {
                            self.level = sustain_level;
                            self.stage = EnvelopeStage::Sustain;
                        }
                    }
                }
                EnvelopeStage::Sustain => {
                    if !gate_high {
                        // Gate went low
                        self.stage = EnvelopeStage::Release;
                    } else if retrigger_rising {
                        // Retrigger: restart attack from sustain level
                        self.stage = EnvelopeStage::Attack;
                    } else {
                        // Hold at sustain level
                        self.level = sustain_level;
                    }
                }
                EnvelopeStage::Release => {
                    if gate_rising {
                        // New note on during release
                        self.stage = EnvelopeStage::Attack;
                    } else if retrigger_rising && gate_high {
                        // Retrigger while gate is still high
                        self.stage = EnvelopeStage::Attack;
                    } else {
                        // Exponential fall toward 0
                        self.level *= release_coeff;

                        // Transition to idle when we reach zero
                        if self.level < Self::LEVEL_THRESHOLD {
                            self.level = 0.0;
                            self.stage = EnvelopeStage::Idle;
                        }
                    }
                }
            }

            // Handle gate rising edge transitioning from Idle
            if gate_rising && self.stage == EnvelopeStage::Idle {
                self.stage = EnvelopeStage::Attack;
            }

            // Update previous states for edge detection
            self.prev_gate = gate_high;
            self.prev_retrigger = retrigger_high;

            // Write output
            output.samples[i] = self.level;
        }
    }

    fn reset(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level = 0.0;
        self.prev_gate = false;
        self.prev_retrigger = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adsr_info() {
        let env = AdsrEnvelope::new();
        assert_eq!(env.info().id, "mod.adsr");
        assert_eq!(env.info().name, "ADSR Envelope");
        assert_eq!(env.info().category, ModuleCategory::Modulation);
    }

    #[test]
    fn test_adsr_ports() {
        let env = AdsrEnvelope::new();
        let ports = env.ports();

        assert_eq!(ports.len(), 3);

        // Gate input
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "gate");
        assert_eq!(ports[0].signal_type, SignalType::Gate);

        // Retrigger input
        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "retrigger");
        assert_eq!(ports[1].signal_type, SignalType::Gate);

        // Output
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out");
        assert_eq!(ports[2].signal_type, SignalType::Control);
    }

    #[test]
    fn test_adsr_parameters() {
        let env = AdsrEnvelope::new();
        let params = env.parameters();

        assert_eq!(params.len(), 4);

        // Attack
        assert_eq!(params[0].id, "attack");
        assert_eq!(params[0].min, 0.001);
        assert_eq!(params[0].max, 10.0);
        assert!((params[0].default - 0.01).abs() < f32::EPSILON);

        // Decay
        assert_eq!(params[1].id, "decay");
        assert_eq!(params[1].min, 0.001);
        assert_eq!(params[1].max, 10.0);
        assert!((params[1].default - 0.1).abs() < f32::EPSILON);

        // Sustain
        assert_eq!(params[2].id, "sustain");
        assert_eq!(params[2].min, 0.0);
        assert_eq!(params[2].max, 1.0);
        assert!((params[2].default - 0.7).abs() < f32::EPSILON);

        // Release
        assert_eq!(params[3].id, "release");
        assert_eq!(params[3].min, 0.001);
        assert_eq!(params[3].max, 10.0);
        assert!((params[3].default - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_adsr_idle_output_zero() {
        let mut env = AdsrEnvelope::new();
        env.prepare(44100.0, 256);

        // No gate input, should output zeros
        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        env.process(&[], &mut outputs, &[0.01, 0.1, 0.7, 0.3], &ctx);

        // Output should be all zeros
        assert!(outputs[0].samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_adsr_attack_phase() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 4410);

        // Create a gate signal that's high
        let mut gate = SignalBuffer::control(4410);
        gate.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Short attack (10ms), longer decay
        env.process(&[&gate], &mut outputs, &[0.01, 1.0, 0.7, 0.3], &ctx);

        // First sample should be small (just started attack)
        assert!(outputs[0].samples[0] < 0.5, "Initial attack should be low");

        // After ~10ms (441 samples), should be close to 1.0
        assert!(
            outputs[0].samples[441] > 0.9,
            "Should reach near-peak after attack time"
        );

        // Should eventually reach 1.0 then decay to sustain
        let max_level = outputs[0].samples.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            max_level > 0.99,
            "Should reach peak during attack, got {}",
            max_level
        );
    }

    #[test]
    fn test_adsr_sustain_hold() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 44100);

        // Gate high for 1 second
        let mut gate = SignalBuffer::control(44100);
        gate.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // Very fast attack/decay so we reach sustain quickly
        env.process(&[&gate], &mut outputs, &[0.001, 0.001, 0.5, 0.3], &ctx);

        // After attack and decay, should be at sustain level (0.5)
        // Check last portion of the buffer
        let end_samples = &outputs[0].samples[40000..];
        for &sample in end_samples {
            assert!(
                (sample - 0.5).abs() < 0.01,
                "Should hold at sustain level, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_adsr_release_phase() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;

        // First, trigger the envelope to build up some level
        env.prepare(sample_rate, 4410);

        let mut gate_on = SignalBuffer::control(4410);
        gate_on.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Fast attack/decay to reach sustain
        env.process(&[&gate_on], &mut outputs, &[0.001, 0.001, 0.7, 0.1], &ctx);

        // Now release (gate low)
        let gate_off = SignalBuffer::control(4410);
        let mut outputs2 = vec![SignalBuffer::control(4410)];

        env.process(&[&gate_off], &mut outputs2, &[0.001, 0.001, 0.7, 0.1], &ctx);

        // First sample should be near sustain level
        assert!(
            outputs2[0].samples[0] > 0.5,
            "Release should start from sustain level"
        );

        // Should decay toward zero
        assert!(
            outputs2[0].samples[4409] < outputs2[0].samples[0],
            "Release should decrease the level"
        );
    }

    #[test]
    fn test_adsr_gate_off_during_attack() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 4410);

        // Gate that goes low partway through attack
        let mut gate = SignalBuffer::control(4410);
        for i in 0..1000 {
            gate.samples[i] = 1.0;
        }
        // Rest is 0.0 (low)

        let mut outputs = vec![SignalBuffer::control(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Long attack so we're definitely still in attack when gate goes low
        env.process(&[&gate], &mut outputs, &[0.5, 0.1, 0.7, 0.1], &ctx);

        // After gate goes low, level should start decreasing (release)
        let level_at_gate_off = outputs[0].samples[1000];
        let level_later = outputs[0].samples[4000];
        assert!(
            level_later < level_at_gate_off,
            "Level should decrease after gate off"
        );
    }

    #[test]
    fn test_adsr_retrigger() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 8820);

        // Gate stays high
        let mut gate = SignalBuffer::control(8820);
        gate.fill(1.0);

        // Retrigger pulse at halfway point
        let mut retrigger = SignalBuffer::control(8820);
        for i in 4400..4410 {
            retrigger.samples[i] = 1.0;
        }

        let mut outputs = vec![SignalBuffer::control(8820)];
        let ctx = ProcessContext::new(sample_rate, 8820);

        // Moderate attack/decay so we reach sustain before retrigger
        env.process(
            &[&gate, &retrigger],
            &mut outputs,
            &[0.01, 0.05, 0.5, 0.3],
            &ctx,
        );

        // Before retrigger, should be at or near sustain
        let level_before = outputs[0].samples[4300];

        // After retrigger, should rise toward 1.0 again
        // Check a bit after the retrigger point
        let level_after = outputs[0].samples[4600];
        assert!(
            level_after > level_before,
            "Retrigger should cause level to rise from sustain"
        );
    }

    #[test]
    fn test_adsr_reset() {
        let mut env = AdsrEnvelope::new();
        env.prepare(44100.0, 256);

        // Build up some state
        let mut gate = SignalBuffer::control(256);
        gate.fill(1.0);
        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        env.process(&[&gate], &mut outputs, &[0.001, 0.1, 0.7, 0.3], &ctx);

        // Reset
        env.reset();

        // Should be back to idle
        assert_eq!(env.stage, EnvelopeStage::Idle);
        assert_eq!(env.level, 0.0);

        // Process without gate - should output zeros
        let mut outputs2 = vec![SignalBuffer::control(256)];
        env.process(&[], &mut outputs2, &[0.01, 0.1, 0.7, 0.3], &ctx);
        assert!(outputs2[0].samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_adsr_output_range() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 44100);

        // Gate on then off
        let mut gate = SignalBuffer::control(44100);
        for i in 0..22050 {
            gate.samples[i] = 1.0;
        }

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        env.process(&[&gate], &mut outputs, &[0.01, 0.1, 0.7, 0.3], &ctx);

        // All output values should be in valid range [0, 1]
        for &sample in &outputs[0].samples {
            assert!(
                sample >= 0.0 && sample <= 1.0,
                "Output {} out of range",
                sample
            );
        }
    }

    #[test]
    fn test_adsr_exponential_curves() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 4410);

        let mut gate = SignalBuffer::control(4410);
        gate.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Moderate attack time
        env.process(&[&gate], &mut outputs, &[0.05, 0.5, 0.5, 0.3], &ctx);

        // Check that attack curve is concave (exponential rise)
        // The rate of increase should slow down as we approach 1.0
        let early_increase = outputs[0].samples[100] - outputs[0].samples[0];
        let later_increase = outputs[0].samples[1000] - outputs[0].samples[900];

        // Early increase should be larger (faster initial rise)
        assert!(
            early_increase > later_increase,
            "Attack should have exponential curve (early: {}, later: {})",
            early_increase,
            later_increase
        );
    }

    #[test]
    fn test_adsr_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AdsrEnvelope>();
    }

    #[test]
    fn test_adsr_default() {
        let env = AdsrEnvelope::default();
        assert_eq!(env.info().id, "mod.adsr");
    }

    #[test]
    fn test_adsr_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<AdsrEnvelope>();

        assert!(registry.contains("mod.adsr"));

        let module = registry.create("mod.adsr");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "mod.adsr");
        assert_eq!(module.info().name, "ADSR Envelope");
        assert_eq!(module.ports().len(), 3);
        assert_eq!(module.parameters().len(), 4);
    }

    #[test]
    fn test_adsr_zero_sustain() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 44100);

        let mut gate = SignalBuffer::control(44100);
        gate.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // Zero sustain level
        env.process(&[&gate], &mut outputs, &[0.01, 0.01, 0.0, 0.3], &ctx);

        // After attack and decay, should be at zero
        let end_samples = &outputs[0].samples[40000..];
        for &sample in end_samples {
            assert!(
                sample < 0.01,
                "Should hold at zero sustain, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_adsr_full_sustain() {
        let mut env = AdsrEnvelope::new();
        let sample_rate = 44100.0;
        env.prepare(sample_rate, 44100);

        let mut gate = SignalBuffer::control(44100);
        gate.fill(1.0);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // Full sustain level (1.0)
        env.process(&[&gate], &mut outputs, &[0.01, 0.01, 1.0, 0.3], &ctx);

        // After attack, should stay at 1.0 (no decay needed)
        let end_samples = &outputs[0].samples[40000..];
        for &sample in end_samples {
            assert!(
                sample > 0.99,
                "Should hold at full sustain, got {}",
                sample
            );
        }
    }
}
