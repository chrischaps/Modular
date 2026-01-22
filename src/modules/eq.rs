//! 3-Band Parametric EQ module.
//!
//! A versatile equalizer with low shelf, mid parametric, and high shelf bands.
//! Uses biquad filters with RBJ Audio EQ Cookbook formulas.

use std::f32::consts::PI;

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// Biquad filter coefficients.
#[derive(Clone, Copy, Default)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

/// Biquad filter state (delay line).
#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    /// Process a single sample through the biquad filter.
    #[inline]
    fn process(&mut self, input: f32, coeffs: &BiquadCoeffs) -> f32 {
        let output = coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
            - coeffs.a1 * self.y1 - coeffs.a2 * self.y2;

        // Update state
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Reset the filter state.
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// 3-Band Parametric EQ with low shelf, mid parametric, and high shelf.
///
/// # Ports
///
/// - **In** (Audio, Input): Audio signal to equalize.
/// - **Out** (Audio, Output): Equalized audio output.
///
/// # Parameters
///
/// - **Low Freq** (20-500 Hz): Low shelf center frequency.
/// - **Low Gain** (-15 to +15 dB): Low shelf gain.
/// - **Mid Freq** (100-10000 Hz): Mid band center frequency.
/// - **Mid Gain** (-15 to +15 dB): Mid band gain.
/// - **Mid Q** (0.1-10): Mid band Q factor (bandwidth).
/// - **High Freq** (2000-20000 Hz): High shelf center frequency.
/// - **High Gain** (-15 to +15 dB): High shelf gain.
/// - **Output** (-12 to +12 dB): Output gain (makeup).
pub struct ParametricEq {
    /// Sample rate.
    sample_rate: f32,
    /// Low shelf filter coefficients.
    low_coeffs: BiquadCoeffs,
    /// Mid parametric filter coefficients.
    mid_coeffs: BiquadCoeffs,
    /// High shelf filter coefficients.
    high_coeffs: BiquadCoeffs,
    /// Low shelf filter state.
    low_state: BiquadState,
    /// Mid parametric filter state.
    mid_state: BiquadState,
    /// High shelf filter state.
    high_state: BiquadState,
    /// Smoothed low frequency.
    low_freq_smooth: SmoothedValue,
    /// Smoothed low gain.
    low_gain_smooth: SmoothedValue,
    /// Smoothed mid frequency.
    mid_freq_smooth: SmoothedValue,
    /// Smoothed mid gain.
    mid_gain_smooth: SmoothedValue,
    /// Smoothed mid Q.
    mid_q_smooth: SmoothedValue,
    /// Smoothed high frequency.
    high_freq_smooth: SmoothedValue,
    /// Smoothed high gain.
    high_gain_smooth: SmoothedValue,
    /// Smoothed output gain.
    output_gain_smooth: SmoothedValue,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl ParametricEq {
    /// Creates a new parametric EQ.
    pub fn new() -> Self {
        let sample_rate = 44100.0;

        Self {
            sample_rate,
            low_coeffs: BiquadCoeffs::default(),
            mid_coeffs: BiquadCoeffs::default(),
            high_coeffs: BiquadCoeffs::default(),
            low_state: BiquadState::default(),
            mid_state: BiquadState::default(),
            high_state: BiquadState::default(),
            low_freq_smooth: SmoothedValue::with_default_smoothing(100.0, sample_rate),
            low_gain_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            mid_freq_smooth: SmoothedValue::with_default_smoothing(1000.0, sample_rate),
            mid_gain_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            mid_q_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            high_freq_smooth: SmoothedValue::with_default_smoothing(8000.0, sample_rate),
            high_gain_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            output_gain_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            ports: vec![
                PortDefinition::input_with_default("in", "In", SignalType::Audio, 0.0),
                PortDefinition::output("out", "Out", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::new(
                    "low_freq",
                    "Low Freq",
                    20.0,
                    500.0,
                    100.0,
                    ParameterDisplay::Logarithmic { unit: "Hz" },
                ),
                ParameterDefinition::new(
                    "low_gain",
                    "Low Gain",
                    -15.0,
                    15.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::new(
                    "mid_freq",
                    "Mid Freq",
                    100.0,
                    10000.0,
                    1000.0,
                    ParameterDisplay::Logarithmic { unit: "Hz" },
                ),
                ParameterDefinition::new(
                    "mid_gain",
                    "Mid Gain",
                    -15.0,
                    15.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::new(
                    "mid_q",
                    "Mid Q",
                    0.1,
                    10.0,
                    1.0,
                    ParameterDisplay::Logarithmic { unit: "" },
                ),
                ParameterDefinition::new(
                    "high_freq",
                    "High Freq",
                    2000.0,
                    20000.0,
                    8000.0,
                    ParameterDisplay::Logarithmic { unit: "Hz" },
                ),
                ParameterDefinition::new(
                    "high_gain",
                    "High Gain",
                    -15.0,
                    15.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
                ParameterDefinition::new(
                    "output_gain",
                    "Output",
                    -12.0,
                    12.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "dB" },
                ),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN: usize = 0;
    const PORT_OUT: usize = 0;

    /// Parameter index constants.
    const PARAM_LOW_FREQ: usize = 0;
    const PARAM_LOW_GAIN: usize = 1;
    const PARAM_MID_FREQ: usize = 2;
    const PARAM_MID_GAIN: usize = 3;
    const PARAM_MID_Q: usize = 4;
    const PARAM_HIGH_FREQ: usize = 5;
    const PARAM_HIGH_GAIN: usize = 6;
    const PARAM_OUTPUT_GAIN: usize = 7;

    /// Convert dB to linear amplitude.
    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        10.0_f32.powf(db / 20.0)
    }

    /// Calculate low shelf biquad coefficients (RBJ Cookbook).
    fn calc_low_shelf(&self, freq: f32, gain_db: f32) -> BiquadCoeffs {
        if gain_db.abs() < 0.01 {
            // Gain is essentially 0 dB, pass through
            return BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            };
        }

        let a = 10.0_f32.powf(gain_db / 40.0); // sqrt(10^(dB/20))
        let w0 = 2.0 * PI * freq.clamp(20.0, self.sample_rate * 0.45) / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();

        // Slope = 1.0 for a smooth shelf
        let s = 1.0;
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha);

        // Normalize by a0
        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate high shelf biquad coefficients (RBJ Cookbook).
    fn calc_high_shelf(&self, freq: f32, gain_db: f32) -> BiquadCoeffs {
        if gain_db.abs() < 0.01 {
            // Gain is essentially 0 dB, pass through
            return BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            };
        }

        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq.clamp(20.0, self.sample_rate * 0.45) / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();

        let s = 1.0;
        let alpha = sin_w0 / 2.0 * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
        let a2 = (a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha;
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha);

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Calculate peaking (parametric) biquad coefficients (RBJ Cookbook).
    fn calc_peaking(&self, freq: f32, gain_db: f32, q: f32) -> BiquadCoeffs {
        if gain_db.abs() < 0.01 {
            // Gain is essentially 0 dB, pass through
            return BiquadCoeffs {
                b0: 1.0,
                b1: 0.0,
                b2: 0.0,
                a1: 0.0,
                a2: 0.0,
            };
        }

        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq.clamp(20.0, self.sample_rate * 0.45) / self.sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q.max(0.1));

        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha / a;
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0 - alpha * a;

        BiquadCoeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
}

impl Default for ParametricEq {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for ParametricEq {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.eq",
            name: "3-Band EQ",
            category: ModuleCategory::Effect,
            description: "Parametric EQ with low shelf, mid band, and high shelf",
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

        // Update sample rates for smoothed values
        self.low_freq_smooth.set_sample_rate(sample_rate);
        self.low_gain_smooth.set_sample_rate(sample_rate);
        self.mid_freq_smooth.set_sample_rate(sample_rate);
        self.mid_gain_smooth.set_sample_rate(sample_rate);
        self.mid_q_smooth.set_sample_rate(sample_rate);
        self.high_freq_smooth.set_sample_rate(sample_rate);
        self.high_gain_smooth.set_sample_rate(sample_rate);
        self.output_gain_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Set smoothing targets
        self.low_freq_smooth.set_target(params[Self::PARAM_LOW_FREQ]);
        self.low_gain_smooth.set_target(params[Self::PARAM_LOW_GAIN]);
        self.mid_freq_smooth.set_target(params[Self::PARAM_MID_FREQ]);
        self.mid_gain_smooth.set_target(params[Self::PARAM_MID_GAIN]);
        self.mid_q_smooth.set_target(params[Self::PARAM_MID_Q]);
        self.high_freq_smooth.set_target(params[Self::PARAM_HIGH_FREQ]);
        self.high_gain_smooth.set_target(params[Self::PARAM_HIGH_GAIN]);
        self.output_gain_smooth.set_target(params[Self::PARAM_OUTPUT_GAIN]);

        // Get input buffer
        let audio_in = inputs.get(Self::PORT_IN);
        let out = &mut outputs[Self::PORT_OUT];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed parameter values
            let low_freq = self.low_freq_smooth.next();
            let low_gain = self.low_gain_smooth.next();
            let mid_freq = self.mid_freq_smooth.next();
            let mid_gain = self.mid_gain_smooth.next();
            let mid_q = self.mid_q_smooth.next();
            let high_freq = self.high_freq_smooth.next();
            let high_gain = self.high_gain_smooth.next();
            let output_gain_db = self.output_gain_smooth.next();

            // Update filter coefficients (done per-sample for smooth changes)
            self.low_coeffs = self.calc_low_shelf(low_freq, low_gain);
            self.mid_coeffs = self.calc_peaking(mid_freq, mid_gain, mid_q);
            self.high_coeffs = self.calc_high_shelf(high_freq, high_gain);

            // Get input sample
            let input = audio_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Cascade through all three filter bands
            let after_low = self.low_state.process(input, &self.low_coeffs);
            let after_mid = self.mid_state.process(after_low, &self.mid_coeffs);
            let after_high = self.high_state.process(after_mid, &self.high_coeffs);

            // Apply output gain
            let output_gain = Self::db_to_linear(output_gain_db);
            let output = after_high * output_gain;

            // Write output
            out.samples[i] = output;
        }
    }

    fn reset(&mut self) {
        // Reset filter states
        self.low_state.reset();
        self.mid_state.reset();
        self.high_state.reset();

        // Reset smoothed values
        self.low_freq_smooth.reset(self.low_freq_smooth.target());
        self.low_gain_smooth.reset(self.low_gain_smooth.target());
        self.mid_freq_smooth.reset(self.mid_freq_smooth.target());
        self.mid_gain_smooth.reset(self.mid_gain_smooth.target());
        self.mid_q_smooth.reset(self.mid_q_smooth.target());
        self.high_freq_smooth.reset(self.high_freq_smooth.target());
        self.high_gain_smooth.reset(self.high_gain_smooth.target());
        self.output_gain_smooth.reset(self.output_gain_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_info() {
        let eq = ParametricEq::new();
        assert_eq!(eq.info().id, "fx.eq");
        assert_eq!(eq.info().name, "3-Band EQ");
        assert_eq!(eq.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_eq_ports() {
        let eq = ParametricEq::new();
        let ports = eq.ports();

        assert_eq!(ports.len(), 2);

        // Input port
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        // Output port
        assert!(ports[1].is_output());
        assert_eq!(ports[1].id, "out");
        assert_eq!(ports[1].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_eq_parameters() {
        let eq = ParametricEq::new();
        let params = eq.parameters();

        assert_eq!(params.len(), 8);
        assert_eq!(params[0].id, "low_freq");
        assert_eq!(params[1].id, "low_gain");
        assert_eq!(params[2].id, "mid_freq");
        assert_eq!(params[3].id, "mid_gain");
        assert_eq!(params[4].id, "mid_q");
        assert_eq!(params[5].id, "high_freq");
        assert_eq!(params[6].id, "high_gain");
        assert_eq!(params[7].id, "output_gain");
    }

    #[test]
    fn test_eq_passthrough_at_zero_gain() {
        let mut eq = ParametricEq::new();
        eq.prepare(44100.0, 256);

        // Create a test signal
        let mut input = SignalBuffer::audio(256);
        for i in 0..256 {
            input.samples[i] = (i as f32 * 0.1).sin();
        }

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with all gains at 0 dB and output gain at 0 dB
        // Params: low_freq, low_gain, mid_freq, mid_gain, mid_q, high_freq, high_gain, output_gain
        eq.process(
            &[&input],
            &mut outputs,
            &[100.0, 0.0, 1000.0, 0.0, 1.0, 8000.0, 0.0, 0.0],
            &ctx,
        );

        // Output should closely match input (within tolerance for filter settling)
        let skip = 50; // Skip initial transient
        for i in skip..256 {
            let diff = (input.samples[i] - outputs[0].samples[i]).abs();
            assert!(
                diff < 0.01,
                "Sample {} differs: input={}, output={}, diff={}",
                i,
                input.samples[i],
                outputs[0].samples[i],
                diff
            );
        }
    }

    #[test]
    fn test_eq_low_shelf_boost() {
        let mut eq = ParametricEq::new();
        let sample_rate = 44100.0;
        eq.prepare(sample_rate, 4410);

        // Generate low frequency signal (50 Hz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 50.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5;
        }

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Boost low shelf by 12 dB
        eq.process(
            &[&input],
            &mut outputs,
            &[100.0, 12.0, 1000.0, 0.0, 1.0, 8000.0, 0.0, 0.0],
            &ctx,
        );

        // Calculate RMS after filter settles
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let output_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Output should be significantly louder (12 dB = 4x amplitude)
        assert!(
            output_rms > input_rms * 2.0,
            "Low shelf boost should amplify low frequencies: input_rms={}, output_rms={}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_eq_high_shelf_cut() {
        let mut eq = ParametricEq::new();
        let sample_rate = 44100.0;
        eq.prepare(sample_rate, 4410);

        // Generate high frequency signal (10 kHz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 10000.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5;
        }

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Cut high shelf by 12 dB
        eq.process(
            &[&input],
            &mut outputs,
            &[100.0, 0.0, 1000.0, 0.0, 1.0, 5000.0, -12.0, 0.0],
            &ctx,
        );

        // Calculate RMS after filter settles
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let output_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Output should be quieter (12 dB cut = 1/4 amplitude)
        assert!(
            output_rms < input_rms * 0.5,
            "High shelf cut should attenuate high frequencies: input_rms={}, output_rms={}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_eq_mid_parametric() {
        let mut eq = ParametricEq::new();
        let sample_rate = 44100.0;
        eq.prepare(sample_rate, 4410);

        // Generate mid frequency signal (1 kHz)
        let mut input = SignalBuffer::audio(4410);
        let freq = 1000.0;
        for i in 0..4410 {
            input.samples[i] = (2.0 * PI * freq * i as f32 / sample_rate).sin() * 0.5;
        }

        let mut outputs = vec![SignalBuffer::audio(4410)];
        let ctx = ProcessContext::new(sample_rate, 4410);

        // Boost mid band at 1 kHz by 12 dB with narrow Q
        eq.process(
            &[&input],
            &mut outputs,
            &[100.0, 0.0, 1000.0, 12.0, 2.0, 8000.0, 0.0, 0.0],
            &ctx,
        );

        // Calculate RMS after filter settles
        let skip = 441;
        let input_rms: f32 = (input.samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();
        let output_rms: f32 = (outputs[0].samples[skip..].iter().map(|s| s * s).sum::<f32>()
            / (4410 - skip) as f32)
            .sqrt();

        // Output should be louder at the boosted frequency
        assert!(
            output_rms > input_rms * 1.5,
            "Mid boost should amplify at center frequency: input_rms={}, output_rms={}",
            input_rms,
            output_rms
        );
    }

    #[test]
    fn test_eq_output_gain() {
        let mut eq = ParametricEq::new();
        eq.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);

        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process multiple blocks to let parameter smoothing settle
        for _ in 0..20 {
            eq.process(
                &[&input],
                &mut outputs,
                &[100.0, 0.0, 1000.0, 0.0, 1.0, 8000.0, 0.0, 6.0],
                &ctx,
            );
        }

        // Output should be approximately doubled (6 dB ≈ 2x amplitude)
        let skip = 50;
        let output_avg: f32 = outputs[0].samples[skip..].iter().sum::<f32>() / (256 - skip) as f32;
        assert!(
            output_avg > 0.8 && output_avg < 1.2,
            "6 dB output gain should roughly double amplitude: {}",
            output_avg
        );
    }

    #[test]
    fn test_eq_reset() {
        let mut eq = ParametricEq::new();
        eq.prepare(44100.0, 256);

        // Process some signal
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);
        let mut outputs = vec![SignalBuffer::audio(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        eq.process(
            &[&input],
            &mut outputs,
            &[100.0, 12.0, 1000.0, 12.0, 1.0, 8000.0, 12.0, 0.0],
            &ctx,
        );

        // Reset
        eq.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![SignalBuffer::audio(256)];

        eq.process(
            &[&silence],
            &mut outputs2,
            &[100.0, 12.0, 1000.0, 12.0, 1.0, 8000.0, 12.0, 0.0],
            &ctx,
        );

        // First sample should be near zero after reset
        assert!(
            outputs2[0].samples[0].abs() < 0.01,
            "Output should be near zero after reset: {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_eq_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ParametricEq>();
    }

    #[test]
    fn test_eq_default() {
        let eq = ParametricEq::default();
        assert_eq!(eq.info().id, "fx.eq");
    }

    #[test]
    fn test_db_to_linear() {
        // 0 dB = 1.0
        let lin_0 = ParametricEq::db_to_linear(0.0);
        assert!((lin_0 - 1.0).abs() < 0.001);

        // 6 dB ≈ 2.0
        let lin_6 = ParametricEq::db_to_linear(6.0);
        assert!((lin_6 - 2.0).abs() < 0.1);

        // -6 dB ≈ 0.5
        let lin_neg6 = ParametricEq::db_to_linear(-6.0);
        assert!((lin_neg6 - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_eq_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<ParametricEq>();

        assert!(registry.contains("fx.eq"));

        let module = registry.create("fx.eq");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.eq");
        assert_eq!(module.info().name, "3-Band EQ");
        assert_eq!(module.ports().len(), 2);
        assert_eq!(module.parameters().len(), 8);
    }
}
