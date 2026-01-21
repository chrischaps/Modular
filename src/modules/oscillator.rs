//! Oscillator modules.
//!
//! This module contains sound source modules that generate audio waveforms.
//! Includes sine, sawtooth, square (with PWM), and triangle waveforms with
//! band-limited synthesis for alias-free sound at all frequencies.

use std::f32::consts::TAU;

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Waveform shapes for the oscillator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OscWaveform {
    Sine = 0,
    Saw = 1,
    Square = 2,
    Triangle = 3,
}

impl OscWaveform {
    /// Convert from parameter value (0-3) to waveform.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => OscWaveform::Sine,
            1 => OscWaveform::Saw,
            2 => OscWaveform::Square,
            3 => OscWaveform::Triangle,
            _ => OscWaveform::Sine,
        }
    }
}

/// A multi-waveform oscillator with FM, V/Oct, and PWM support.
///
/// This is a full-featured VCO (Voltage-Controlled Oscillator) producing
/// sine, sawtooth, square, and triangle waveforms. Band-limited synthesis
/// using PolyBLEP ensures alias-free output at all frequencies.
///
/// # Ports
///
/// **Inputs:**
/// - **V/Oct** (Control): 1V/Octave pitch CV input. Each unit of CV shifts
///   the pitch by one octave (CV of +1 = double frequency, -1 = half frequency).
/// - **FM** (Control): Linear frequency modulation input. The signal is scaled
///   by the FM Depth parameter and added to the frequency in Hz.
/// - **Frequency** (Control): When connected, overrides the Frequency parameter.
/// - **PWM** (Control): Pulse width modulation input for square wave.
///   Modulates around the Pulse Width parameter.
///
/// **Outputs:**
/// - **Out** (Audio): The generated waveform output.
///
/// # Parameters
///
/// - **Frequency** (20-20000 Hz): Base frequency of the oscillator.
/// - **FM Depth** (0-1000 Hz): How much the FM input affects the frequency.
/// - **Waveform** (Sine/Saw/Square/Tri): The waveform shape to generate.
/// - **Pulse Width** (0.1-0.9): Duty cycle for square wave. 0.5 = 50% duty cycle.
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
    /// Creates a new oscillator.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            sample_rate: 44100.0,
            ports: vec![
                // Input ports first (by convention)
                // V/Oct: 1V/Octave pitch CV (exponential scaling)
                PortDefinition::input_with_default("v_oct", "V/Oct", SignalType::Control, 0.0),
                // FM: Linear frequency modulation (scaled by FM Depth)
                PortDefinition::input_with_default("fm", "FM", SignalType::Control, 0.0),
                // Direct frequency input - when connected, overrides the Frequency parameter
                PortDefinition::input_with_default("freq_in", "Frequency", SignalType::Control, 0.0),
                // PWM: Pulse width modulation for square wave
                PortDefinition::input_with_default("pwm", "PWM", SignalType::Control, 0.0),
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
                    ParameterDisplay::linear("Hz"),
                ),
                // Waveform selection
                ParameterDefinition::choice(
                    "waveform",
                    "Waveform",
                    &["Sine", "Saw", "Square", "Tri"],
                    0, // Default Sine
                ),
                // Pulse Width for square wave
                ParameterDefinition::new(
                    "pulse_width",
                    "Pulse Width",
                    0.1,
                    0.9,
                    0.5, // Default 50% duty cycle
                    ParameterDisplay::linear(""),
                ),
            ],
        }
    }

    /// Port index constants for clarity.
    const PORT_V_OCT: usize = 0;
    const PORT_FM: usize = 1;
    const PORT_FREQ_IN: usize = 2;
    const PORT_PWM: usize = 3;
    const PORT_OUT: usize = 0; // First (only) output

    /// Parameter index constants.
    const PARAM_FREQUENCY: usize = 0;
    const PARAM_FM_DEPTH: usize = 1;
    const PARAM_WAVEFORM: usize = 2;
    const PARAM_PULSE_WIDTH: usize = 3;

    /// PolyBLEP (Polynomial Band-Limited Step) correction.
    ///
    /// This smooths out discontinuities in waveforms to reduce aliasing.
    /// `t` is the position relative to the discontinuity (0.0-1.0 phase)
    /// `dt` is the phase increment per sample (frequency / sample_rate)
    ///
    /// Returns a correction value to add to the naive waveform.
    #[inline]
    fn poly_blep(t: f32, dt: f32) -> f32 {
        if dt <= 0.0 {
            return 0.0;
        }

        if t < dt {
            // Just after discontinuity (0)
            let t_normalized = t / dt;
            // 2*t - t^2 - 1 = -(1-t)^2 + something... let's use standard formula
            2.0 * t_normalized - t_normalized * t_normalized - 1.0
        } else if t > 1.0 - dt {
            // Just before discontinuity (1)
            let t_normalized = (t - 1.0) / dt;
            t_normalized * t_normalized + 2.0 * t_normalized + 1.0
        } else {
            0.0
        }
    }

    /// Generate a naive (non-band-limited) sawtooth sample.
    /// Returns value from -1 to +1.
    #[inline]
    fn naive_saw(phase: f32) -> f32 {
        2.0 * phase - 1.0
    }

    /// Generate a band-limited sawtooth sample using PolyBLEP.
    #[inline]
    fn blep_saw(phase: f32, dt: f32) -> f32 {
        let mut sample = Self::naive_saw(phase);
        // Apply PolyBLEP correction at the discontinuity (phase wrap at 1->0)
        sample -= Self::poly_blep(phase, dt);
        sample
    }

    /// Generate a naive (non-band-limited) square sample.
    /// Returns +1 for phase < pulse_width, -1 otherwise.
    #[inline]
    fn naive_square(phase: f32, pulse_width: f32) -> f32 {
        if phase < pulse_width {
            1.0
        } else {
            -1.0
        }
    }

    /// Generate a band-limited square sample using PolyBLEP.
    #[inline]
    fn blep_square(phase: f32, dt: f32, pulse_width: f32) -> f32 {
        let mut sample = Self::naive_square(phase, pulse_width);

        // Apply PolyBLEP at both discontinuities:
        // 1. Rising edge at phase = 0 (wrap from 1)
        sample += Self::poly_blep(phase, dt);
        // 2. Falling edge at phase = pulse_width
        let phase_from_pw = phase - pulse_width;
        let adjusted_phase = if phase_from_pw < 0.0 {
            phase_from_pw + 1.0
        } else {
            phase_from_pw
        };
        sample -= Self::poly_blep(adjusted_phase, dt);

        sample
    }

    /// Generate a triangle sample.
    /// Triangle doesn't have sharp discontinuities, so no anti-aliasing needed.
    /// However, for quality at high frequencies, we integrate a band-limited square.
    /// For simplicity, we use the naive triangle which is smooth enough.
    #[inline]
    fn naive_triangle(phase: f32) -> f32 {
        // Triangle: rises from -1 to 1 in first half, falls from 1 to -1 in second half
        // At phase 0: -1, phase 0.25: 0, phase 0.5: 1, phase 0.75: 0, phase 1: -1
        let value = if phase < 0.5 {
            4.0 * phase - 1.0
        } else {
            3.0 - 4.0 * phase
        };
        value
    }
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
            name: "Oscillator",
            category: ModuleCategory::Source,
            description: "Multi-waveform oscillator with FM and PWM support",
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
        let waveform = OscWaveform::from_param(params[Self::PARAM_WAVEFORM]);
        let base_pulse_width = params[Self::PARAM_PULSE_WIDTH];

        // Get input buffers (may be empty if not connected, use defaults)
        let v_oct_input = inputs.get(Self::PORT_V_OCT);
        let fm_input = inputs.get(Self::PORT_FM);
        let freq_in = inputs.get(Self::PORT_FREQ_IN);
        let pwm_input = inputs.get(Self::PORT_PWM);

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

            // Get V/Oct modulation (1V/Octave: each unit = one octave)
            let v_oct_value = v_oct_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Get FM modulation (linear Hz offset)
            let fm_value = fm_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Calculate final frequency:
            // - Base frequency (from param or freq_in)
            // - V/Oct: exponential scaling (2^cv), so cv=1 doubles freq, cv=-1 halves it
            // - FM: linear Hz offset scaled by FM depth
            let pitched_freq = effective_base_freq * 2.0_f32.powf(v_oct_value);
            let fm_hz = fm_value * fm_depth;
            let final_freq = (pitched_freq + fm_hz).clamp(0.0, 20000.0);

            // Calculate phase increment (dt for PolyBLEP)
            let dt = final_freq / self.sample_rate;

            // Get PWM modulation for square wave
            let pwm_value = pwm_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            // PWM input is -1 to +1, scale to +-0.4 and add to base pulse width
            let pulse_width = (base_pulse_width + pwm_value * 0.4).clamp(0.1, 0.9);

            // Generate waveform sample
            let sample = match waveform {
                OscWaveform::Sine => (self.phase * TAU).sin(),
                OscWaveform::Saw => Self::blep_saw(self.phase, dt),
                OscWaveform::Square => Self::blep_square(self.phase, dt, pulse_width),
                OscWaveform::Triangle => Self::naive_triangle(self.phase),
            };

            output.samples[i] = sample;

            // Advance phase
            self.phase += dt;

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
    fn test_oscillator_info() {
        let osc = SineOscillator::new();
        assert_eq!(osc.info().id, "osc.sine");
        assert_eq!(osc.info().name, "Oscillator");
        assert_eq!(osc.info().category, ModuleCategory::Source);
    }

    #[test]
    fn test_oscillator_ports() {
        let osc = SineOscillator::new();
        let ports = osc.ports();

        assert_eq!(ports.len(), 5); // V/Oct, FM, Frequency, PWM, Out

        // First four are inputs
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "v_oct");
        assert_eq!(ports[0].signal_type, SignalType::Control);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "fm");
        assert_eq!(ports[1].signal_type, SignalType::Control);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "freq_in");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        assert!(ports[3].is_input());
        assert_eq!(ports[3].id, "pwm");
        assert_eq!(ports[3].signal_type, SignalType::Control);

        // Fifth is output
        assert!(ports[4].is_output());
        assert_eq!(ports[4].id, "out");
        assert_eq!(ports[4].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_oscillator_parameters() {
        let osc = SineOscillator::new();
        let params = osc.parameters();

        assert_eq!(params.len(), 4);

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

        // Waveform parameter
        assert_eq!(params[2].id, "waveform");
        assert_eq!(params[2].min, 0.0);
        assert_eq!(params[2].max, 3.0);
        assert_eq!(params[2].default, 0.0);

        // Pulse Width parameter
        assert_eq!(params[3].id, "pulse_width");
        assert_eq!(params[3].min, 0.1);
        assert_eq!(params[3].max, 0.9);
        assert_eq!(params[3].default, 0.5);
    }

    #[test]
    fn test_waveform_conversion() {
        assert_eq!(OscWaveform::from_param(0.0), OscWaveform::Sine);
        assert_eq!(OscWaveform::from_param(1.0), OscWaveform::Saw);
        assert_eq!(OscWaveform::from_param(2.0), OscWaveform::Square);
        assert_eq!(OscWaveform::from_param(3.0), OscWaveform::Triangle);
        assert_eq!(OscWaveform::from_param(99.0), OscWaveform::Sine); // Out of range defaults to Sine
    }

    #[test]
    fn test_oscillator_generates_output() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Empty inputs (no CV, no FM), default waveform (sine)
        let inputs: Vec<&SignalBuffer> = vec![];
        osc.process(&inputs, &mut outputs, &[440.0, 0.0, 0.0, 0.5], &ctx);

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
    fn test_sine_waveform_correct_frequency() {
        let mut osc = SineOscillator::new();
        let sample_rate = 44100.0;
        osc.prepare(sample_rate, 4410);

        // Generate 0.1 second of audio at 440 Hz
        let num_samples = (sample_rate * 0.1) as usize; // 0.1 second = 4410 samples

        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(sample_rate, num_samples);

        // Sine waveform (0)
        osc.process(&[], &mut outputs, &[440.0, 0.0, 0.0, 0.5], &ctx);

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
    fn test_sawtooth_waveform() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 4410);

        let num_samples = 4410;
        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(44100.0, num_samples);

        // Saw waveform (1)
        osc.process(&[], &mut outputs, &[440.0, 0.0, 1.0, 0.5], &ctx);

        // Output should be within valid audio range
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.1 && sample <= 1.1, // Small tolerance for PolyBLEP overshoot
                "Saw sample {} out of range",
                sample
            );
        }

        // Saw should have non-zero output
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_nonzero, "Sawtooth should produce non-zero output");
    }

    #[test]
    fn test_square_waveform() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 4410);

        let num_samples = 4410;
        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(44100.0, num_samples);

        // Square waveform (2) with 50% duty cycle
        osc.process(&[], &mut outputs, &[440.0, 0.0, 2.0, 0.5], &ctx);

        // Most samples should be near +1 or -1 (with some transition samples from PolyBLEP)
        let near_one_count = outputs[0]
            .samples
            .iter()
            .filter(|&&s| s.abs() > 0.9)
            .count();
        assert!(
            near_one_count > num_samples / 2,
            "Square wave should have most samples near +/-1, got {} of {}",
            near_one_count,
            num_samples
        );
    }

    #[test]
    fn test_triangle_waveform() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 4410);

        let num_samples = 4410;
        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(44100.0, num_samples);

        // Triangle waveform (3)
        osc.process(&[], &mut outputs, &[440.0, 0.0, 3.0, 0.5], &ctx);

        // Output should be within valid audio range
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "Triangle sample {} out of range",
                sample
            );
        }

        // Triangle should reach near +1 and -1
        let max = outputs[0].samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = outputs[0].samples.iter().cloned().fold(f32::INFINITY, f32::min);
        assert!(max > 0.9, "Triangle should reach near +1");
        assert!(min < -0.9, "Triangle should reach near -1");
    }

    #[test]
    fn test_pwm_modulation() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 4410);

        let num_samples = 4410;
        let ctx = ProcessContext::new(44100.0, num_samples);

        // Test with narrow pulse width (0.2)
        let mut outputs_narrow = vec![SignalBuffer::audio(num_samples)];
        osc.reset();
        osc.process(&[], &mut outputs_narrow, &[440.0, 0.0, 2.0, 0.2], &ctx);

        // Test with wide pulse width (0.8)
        let mut outputs_wide = vec![SignalBuffer::audio(num_samples)];
        osc.reset();
        osc.process(&[], &mut outputs_wide, &[440.0, 0.0, 2.0, 0.8], &ctx);

        // Count high samples (+0.5 threshold)
        let narrow_highs = outputs_narrow[0].samples.iter().filter(|&&s| s > 0.5).count();
        let wide_highs = outputs_wide[0].samples.iter().filter(|&&s| s > 0.5).count();

        // Wide pulse width should have more high samples
        assert!(
            wide_highs > narrow_highs,
            "Wide pulse should have more high samples: {} vs {}",
            wide_highs,
            narrow_highs
        );
    }

    #[test]
    fn test_pwm_input() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Create PWM modulation input
        let v_oct = SignalBuffer::control(256);
        let fm = SignalBuffer::control(256);
        let freq = SignalBuffer::control(256);
        let mut pwm = SignalBuffer::control(256);
        pwm.fill(0.5); // Modulate pulse width by +0.5 * 0.4 = +0.2

        // Square wave with base pulse width 0.5, modulated to ~0.7
        osc.process(
            &[&v_oct, &fm, &freq, &pwm],
            &mut outputs,
            &[440.0, 0.0, 2.0, 0.5],
            &ctx,
        );

        // Should produce valid output
        for &sample in &outputs[0].samples {
            assert!(sample >= -1.1 && sample <= 1.1, "Sample out of range");
        }
    }

    #[test]
    fn test_oscillator_reset() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 256);

        // Generate some samples to advance phase
        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        osc.process(&[], &mut outputs, &[440.0, 0.0, 0.0, 0.5], &ctx);

        // Reset should bring phase back to 0
        osc.reset();

        // Generate first sample after reset - should start at sin(0) = 0
        let mut outputs2 = vec![SignalBuffer::audio(1)];
        let ctx2 = ProcessContext::new(44100.0, 1);
        osc.process(&[], &mut outputs2, &[440.0, 0.0, 0.0, 0.5], &ctx2);

        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "First sample after reset should be near 0, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_fm_modulation() {
        let mut osc = SineOscillator::new();
        let sample_rate = 44100.0;
        osc.prepare(sample_rate, 256);

        // Create dummy V/Oct input (first input)
        let v_oct_input = SignalBuffer::control(256);

        // Create FM input with constant value (second input)
        let mut fm_input = SignalBuffer::control(256);
        fm_input.fill(1.0); // Max FM

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(sample_rate, 256);

        // With FM depth of 100 Hz and FM input of 1.0, frequency should be 440 + 100 = 540 Hz
        osc.process(
            &[&v_oct_input, &fm_input],
            &mut outputs,
            &[440.0, 100.0, 0.0, 0.5],
            &ctx,
        );

        // Output should be valid
        for &sample in &outputs[0].samples {
            assert!(
                sample >= -1.0 && sample <= 1.0,
                "FM modulated sample out of range"
            );
        }

        // Output should not be all zeros
        let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(
            has_nonzero,
            "FM modulated oscillator should produce non-zero output"
        );
    }

    #[test]
    fn test_v_oct_scaling() {
        let sample_rate = 44100.0;
        let num_samples = 44100; // 1 second

        // Test with V/Oct = +1 (should double frequency, one octave up)
        let mut osc = SineOscillator::new();
        osc.prepare(sample_rate, num_samples);

        let mut v_oct_input = SignalBuffer::control(num_samples);
        v_oct_input.fill(1.0); // +1 octave

        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(sample_rate, num_samples);

        // Base frequency 440 Hz with V/Oct = +1 should give 880 Hz
        osc.process(&[&v_oct_input], &mut outputs, &[440.0, 0.0, 0.0, 0.5], &ctx);

        // Count zero crossings
        let mut zero_crossings = 0;
        for i in 1..num_samples {
            if outputs[0].samples[i - 1] <= 0.0 && outputs[0].samples[i] > 0.0 {
                zero_crossings += 1;
            }
        }

        // At 880 Hz for 1 second, expect ~880 cycles
        assert!(
            (zero_crossings as f32 - 880.0).abs() < 10.0,
            "Expected ~880 cycles with +1 octave, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_v_oct_negative() {
        let sample_rate = 44100.0;
        let num_samples = 44100; // 1 second

        // Test with V/Oct = -1 (should halve frequency, one octave down)
        let mut osc = SineOscillator::new();
        osc.prepare(sample_rate, num_samples);

        let mut v_oct_input = SignalBuffer::control(num_samples);
        v_oct_input.fill(-1.0); // -1 octave

        let mut outputs = vec![SignalBuffer::audio(num_samples)];
        let ctx = ProcessContext::new(sample_rate, num_samples);

        // Base frequency 440 Hz with V/Oct = -1 should give 220 Hz
        osc.process(&[&v_oct_input], &mut outputs, &[440.0, 0.0, 0.0, 0.5], &ctx);

        // Count zero crossings
        let mut zero_crossings = 0;
        for i in 1..num_samples {
            if outputs[0].samples[i - 1] <= 0.0 && outputs[0].samples[i] > 0.0 {
                zero_crossings += 1;
            }
        }

        // At 220 Hz for 1 second, expect ~220 cycles
        assert!(
            (zero_crossings as f32 - 220.0).abs() < 5.0,
            "Expected ~220 cycles with -1 octave, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_poly_blep() {
        // Test PolyBLEP at discontinuity
        let dt = 0.01; // 1% of phase per sample

        // Just after discontinuity (near 0)
        let blep_after = SineOscillator::poly_blep(0.005, dt);
        assert!(blep_after.abs() > 0.0, "PolyBLEP should be non-zero near discontinuity");

        // Just before discontinuity (near 1)
        let blep_before = SineOscillator::poly_blep(0.995, dt);
        assert!(blep_before.abs() > 0.0, "PolyBLEP should be non-zero near discontinuity");

        // Away from discontinuity
        let blep_away = SineOscillator::poly_blep(0.5, dt);
        assert_eq!(blep_away, 0.0, "PolyBLEP should be zero away from discontinuity");
    }

    #[test]
    fn test_all_waveforms_produce_output() {
        let mut osc = SineOscillator::new();
        osc.prepare(44100.0, 4410);
        let ctx = ProcessContext::new(44100.0, 4410);

        for waveform_idx in 0..4 {
            osc.reset();
            let mut outputs = vec![SignalBuffer::audio(4410)];
            osc.process(
                &[],
                &mut outputs,
                &[440.0, 0.0, waveform_idx as f32, 0.5],
                &ctx,
            );

            let has_nonzero = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
            assert!(
                has_nonzero,
                "Waveform {} should produce non-zero output",
                waveform_idx
            );
        }
    }

    #[test]
    fn test_oscillator_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SineOscillator>();
    }

    #[test]
    fn test_oscillator_default() {
        let osc = SineOscillator::default();
        assert_eq!(osc.info().id, "osc.sine");
    }

    #[test]
    fn test_oscillator_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<SineOscillator>();

        assert!(registry.contains("osc.sine"));

        let module = registry.create("osc.sine");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "osc.sine");
        assert_eq!(module.info().name, "Oscillator");
        assert_eq!(module.ports().len(), 5);
        assert_eq!(module.parameters().len(), 4);
    }
}
