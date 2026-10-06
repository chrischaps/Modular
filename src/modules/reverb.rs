//! Stereo Reverb effect module.
//!
//! A Freeverb-style reverb with 8 parallel comb filters and 4 series allpass filters.
//! Provides convincing room/hall ambience with adjustable size, decay, and damping.

use crate::dsp::{
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    context::ProcessContext,
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    connected_input, ParameterDisplay, SignalType,
};

/// Freeverb tuning constants - prime-ish delay times for natural sound.
/// These are the original Freeverb delay times (in samples at 44100 Hz).
const COMB_TUNINGS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_TUNINGS: [usize; 4] = [556, 441, 341, 225];

/// Stereo spread offset (samples) - slight detuning between L/R channels.
const STEREO_SPREAD: usize = 23;

/// Room size multiplier range applied to the Freeverb tunings (Size 0..1).
const MIN_ROOM_SIZE: f32 = 0.5;
const MAX_ROOM_SIZE: f32 = 2.0;

/// How often (in samples) the size-dependent comb lengths and feedback
/// gains are recomputed while Size/Decay are smoothing.
const COEFF_UPDATE_INTERVAL: usize = 32;

/// Maximum pre-delay in seconds.
const MAX_PREDELAY_SECONDS: f32 = 0.1;

/// A simple comb filter with lowpass damping in the feedback path.
///
/// The buffer is allocated at the largest room size; `len` selects how much
/// of it is active, which is how the Size parameter changes the delay time.
struct CombFilter {
    buffer: Vec<f32>,
    len: usize,
    write_pos: usize,
    filter_state: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            len: size,
            write_pos: 0,
            filter_state: 0.0,
        }
    }

    fn resize(&mut self, size: usize) {
        self.buffer.resize(size, 0.0);
        self.set_len(self.len);
    }

    /// Sets the active delay length (clamped to the allocated buffer).
    fn set_len(&mut self, len: usize) {
        self.len = len.clamp(1, self.buffer.len());
        if self.write_pos >= self.len {
            self.write_pos = 0;
        }
    }

    #[inline]
    fn process(&mut self, input: f32, feedback: f32, damping: f32) -> f32 {
        let output = self.buffer[self.write_pos];

        // Lowpass filter in feedback path for damping
        // damping: 0 = no damping (bright), 1 = full damping (dark)
        self.filter_state = output * (1.0 - damping) + self.filter_state * damping;

        // Write input + filtered feedback to buffer
        self.buffer[self.write_pos] = input + self.filter_state * feedback;

        // Advance write position within the active length
        self.write_pos += 1;
        if self.write_pos >= self.len {
            self.write_pos = 0;
        }

        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.filter_state = 0.0;
    }
}

/// A simple allpass filter for diffusion.
struct AllpassFilter {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            write_pos: 0,
        }
    }

    fn resize(&mut self, size: usize) {
        self.buffer.resize(size, 0.0);
        if self.write_pos >= size {
            self.write_pos = 0;
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        const FEEDBACK: f32 = 0.5; // Standard allpass feedback

        let buffered = self.buffer[self.write_pos];
        let output = -input + buffered;

        self.buffer[self.write_pos] = input + buffered * FEEDBACK;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// Simple delay line for pre-delay.
struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
    delay_samples: usize,
}

impl DelayLine {
    fn new(max_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_samples.max(1)],
            write_pos: 0,
            delay_samples: 0,
        }
    }

    fn resize(&mut self, max_samples: usize) {
        let new_size = max_samples.max(1);
        self.buffer.resize(new_size, 0.0);
        if self.write_pos >= new_size {
            self.write_pos = 0;
        }
        if self.delay_samples >= new_size {
            self.delay_samples = new_size - 1;
        }
    }

    fn set_delay(&mut self, samples: usize) {
        self.delay_samples = samples.min(self.buffer.len().saturating_sub(1));
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        self.buffer[self.write_pos] = input;

        let read_pos = if self.write_pos >= self.delay_samples {
            self.write_pos - self.delay_samples
        } else {
            self.buffer.len() - (self.delay_samples - self.write_pos)
        };

        let output = self.buffer[read_pos];
        self.write_pos = (self.write_pos + 1) % self.buffer.len();

        output
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// Stereo Freeverb-style reverb effect.
///
/// # Ports
///
/// - **In L** (Audio, Input): Left channel input.
/// - **In R** (Audio, Input): Right channel input (normalled from L).
/// - **Out L** (Audio, Output): Processed left channel.
/// - **Out R** (Audio, Output): Processed right channel.
///
/// # Parameters
///
/// - **Size** (0-100%): Room size - affects delay times.
/// - **Decay** (0.1-30s): Reverb decay time - controls feedback amount.
/// - **Damping** (0-100%): High frequency absorption in reverb tail.
/// - **Pre-Delay** (0-100ms): Initial delay before reverb starts.
/// - **Mix** (0-100%): Wet/dry balance.
/// - **Width** (0-100%): Stereo width of the reverb.
pub struct Reverb {
    /// Sample rate.
    sample_rate: f32,
    /// Left channel comb filters.
    combs_l: Vec<CombFilter>,
    /// Right channel comb filters (with stereo spread).
    combs_r: Vec<CombFilter>,
    /// Left channel allpass filters.
    allpasses_l: Vec<AllpassFilter>,
    /// Right channel allpass filters.
    allpasses_r: Vec<AllpassFilter>,
    /// Pre-delay line for left channel.
    predelay_l: DelayLine,
    /// Pre-delay line for right channel.
    predelay_r: DelayLine,
    /// Smoothed room size.
    size_smooth: SmoothedValue,
    /// Smoothed decay.
    decay_smooth: SmoothedValue,
    /// Smoothed damping.
    damping_smooth: SmoothedValue,
    /// Smoothed pre-delay time.
    predelay_smooth: SmoothedValue,
    /// Smoothed wet/dry mix.
    mix_smooth: SmoothedValue,
    /// Smoothed stereo width.
    width_smooth: SmoothedValue,
    /// Per-comb feedback gains for the current size/decay (left, right).
    comb_feedback: [[f32; 8]; 2],
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Reverb {
    /// Creates a new reverb.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let scale = sample_rate / 44100.0;

        // Create comb filters with scaled tunings
        let combs_l: Vec<CombFilter> = COMB_TUNINGS
            .iter()
            .map(|&t| CombFilter::new(((t as f32 * scale * MAX_ROOM_SIZE) as usize).max(1)))
            .collect();

        let combs_r: Vec<CombFilter> = COMB_TUNINGS
            .iter()
            .map(|&t| CombFilter::new((((t + STEREO_SPREAD) as f32 * scale * MAX_ROOM_SIZE) as usize).max(1)))
            .collect();

        // Create allpass filters with scaled tunings
        let allpasses_l: Vec<AllpassFilter> = ALLPASS_TUNINGS
            .iter()
            .map(|&t| AllpassFilter::new(((t as f32 * scale) as usize).max(1)))
            .collect();

        let allpasses_r: Vec<AllpassFilter> = ALLPASS_TUNINGS
            .iter()
            .map(|&t| AllpassFilter::new((((t + STEREO_SPREAD) as f32 * scale) as usize).max(1)))
            .collect();

        // Pre-delay lines
        let max_predelay = (MAX_PREDELAY_SECONDS * sample_rate) as usize;
        let predelay_l = DelayLine::new(max_predelay);
        let predelay_r = DelayLine::new(max_predelay);

        Self {
            sample_rate,
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            predelay_l,
            predelay_r,
            size_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            decay_smooth: SmoothedValue::with_default_smoothing(2.0, sample_rate),
            damping_smooth: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            predelay_smooth: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            mix_smooth: SmoothedValue::with_default_smoothing(0.3, sample_rate),
            width_smooth: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            comb_feedback: [[0.0; 8]; 2],
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("in_l", "In L", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("in_r", "In R", SignalType::Audio, 0.0),
                // Output ports
                PortDefinition::output("out_l", "Out L", SignalType::Audio),
                PortDefinition::output("out_r", "Out R", SignalType::Audio),
            ],
            parameters: vec![
                ParameterDefinition::normalized("size", "Size", 0.5),
                ParameterDefinition::new(
                    "decay",
                    "Decay",
                    0.1,
                    30.0,
                    2.0,
                    ParameterDisplay::Logarithmic { unit: "s" },
                ),
                ParameterDefinition::normalized("damping", "Damping", 0.5),
                ParameterDefinition::new(
                    "predelay",
                    "Pre-Delay",
                    0.0,
                    100.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "ms" },
                ),
                ParameterDefinition::normalized("mix", "Mix", 0.3),
                ParameterDefinition::normalized("width", "Width", 1.0),
            ],
        }
    }

    /// Port index constants.
    const PORT_IN_L: usize = 0;
    const PORT_IN_R: usize = 1;
    const PORT_OUT_L: usize = 0;
    const PORT_OUT_R: usize = 1;

    /// Parameter index constants.
    const PARAM_SIZE: usize = 0;
    const PARAM_DECAY: usize = 1;
    const PARAM_DAMPING: usize = 2;
    const PARAM_PREDELAY: usize = 3;
    const PARAM_MIX: usize = 4;
    const PARAM_WIDTH: usize = 5;

    /// Convert decay time to a comb feedback coefficient.
    ///
    /// A signal circulating in a comb of delay `d` loses `g` per trip, so after
    /// `T` seconds it has made `T/d` trips: g^(T/d) = 10^(-60/20) for RT60.
    /// Solving gives g = 10^(-3 * d / T).
    fn decay_to_feedback(decay_seconds: f32, delay_samples: f32, sample_rate: f32) -> f32 {
        let delay_seconds = delay_samples / sample_rate;
        if decay_seconds <= 0.0 || delay_seconds <= 0.0 {
            return 0.0;
        }
        let feedback = 10.0_f32.powf(-3.0 * delay_seconds / decay_seconds);
        feedback.clamp(0.0, 0.998) // Strictly below 1 so the loop always decays
    }

    /// Maps the Size parameter (0..1) to a multiplier on the Freeverb tunings.
    #[inline]
    fn room_scale(size: f32) -> f32 {
        MIN_ROOM_SIZE + size.clamp(0.0, 1.0) * (MAX_ROOM_SIZE - MIN_ROOM_SIZE)
    }

    /// Applies Size to every comb's active length and recomputes the per-comb
    /// feedback so the tail reaches -60 dB after `decay` seconds.
    fn update_combs(&mut self, size: f32, decay: f32) {
        let scale = self.sample_rate / 44100.0 * Self::room_scale(size);
        for idx in 0..COMB_TUNINGS.len() {
            let len_l = (COMB_TUNINGS[idx] as f32 * scale) as usize;
            let len_r = ((COMB_TUNINGS[idx] + STEREO_SPREAD) as f32 * scale) as usize;
            self.combs_l[idx].set_len(len_l);
            self.combs_r[idx].set_len(len_r);
            self.comb_feedback[0][idx] =
                Self::decay_to_feedback(decay, self.combs_l[idx].len as f32, self.sample_rate);
            self.comb_feedback[1][idx] =
                Self::decay_to_feedback(decay, self.combs_r[idx].len as f32, self.sample_rate);
        }
    }

    /// Resize all filters for a new sample rate.
    fn resize_filters(&mut self) {
        let scale = self.sample_rate / 44100.0;

        for (i, comb) in self.combs_l.iter_mut().enumerate() {
            let size = ((COMB_TUNINGS[i] as f32 * scale * MAX_ROOM_SIZE) as usize).max(1);
            comb.resize(size);
        }

        for (i, comb) in self.combs_r.iter_mut().enumerate() {
            let size = (((COMB_TUNINGS[i] + STEREO_SPREAD) as f32 * scale * MAX_ROOM_SIZE) as usize).max(1);
            comb.resize(size);
        }

        for (i, allpass) in self.allpasses_l.iter_mut().enumerate() {
            let size = ((ALLPASS_TUNINGS[i] as f32 * scale) as usize).max(1);
            allpass.resize(size);
        }

        for (i, allpass) in self.allpasses_r.iter_mut().enumerate() {
            let size = (((ALLPASS_TUNINGS[i] + STEREO_SPREAD) as f32 * scale) as usize).max(1);
            allpass.resize(size);
        }

        let max_predelay = (MAX_PREDELAY_SECONDS * self.sample_rate) as usize;
        self.predelay_l.resize(max_predelay);
        self.predelay_r.resize(max_predelay);
    }
}

impl Default for Reverb {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Reverb {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.reverb",
            name: "Reverb",
            category: ModuleCategory::Effect,
            description: "Freeverb-style stereo reverb with adjustable size, decay, and damping",
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
        self.resize_filters();

        // Update sample rate for smoothed values
        self.size_smooth.set_sample_rate(sample_rate);
        self.decay_smooth.set_sample_rate(sample_rate);
        self.damping_smooth.set_sample_rate(sample_rate);
        self.predelay_smooth.set_sample_rate(sample_rate);
        self.mix_smooth.set_sample_rate(sample_rate);
        self.width_smooth.set_sample_rate(sample_rate);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Get parameter values
        let size = params[Self::PARAM_SIZE];
        let decay = params[Self::PARAM_DECAY];
        let damping = params[Self::PARAM_DAMPING];
        let predelay_ms = params[Self::PARAM_PREDELAY];
        let mix = params[Self::PARAM_MIX];
        let width = params[Self::PARAM_WIDTH];

        // Set smoothing targets
        self.size_smooth.set_target(size);
        self.decay_smooth.set_target(decay);
        self.damping_smooth.set_target(damping);
        self.predelay_smooth.set_target(predelay_ms);
        self.mix_smooth.set_target(mix);
        self.width_smooth.set_target(width);

        // Get input buffers
        let in_left = inputs.get(Self::PORT_IN_L);
        // Right is normalled from left when nothing is plugged into it
        let in_right = connected_input(inputs, Self::PORT_IN_R);

        // Split outputs
        let (out_left_slice, out_right_slice) = outputs.split_at_mut(1);
        let out_left = &mut out_left_slice[Self::PORT_OUT_L];
        let out_right = &mut out_right_slice[0];

        // Process each sample
        for i in 0..context.block_size {
            // Get smoothed values
            let size_smoothed = self.size_smooth.next();
            let decay_smoothed = self.decay_smooth.next();
            let damping_smoothed = self.damping_smooth.next();
            let predelay_ms_smoothed = self.predelay_smooth.next();
            let mix_smoothed = self.mix_smooth.next();
            let width_smoothed = self.width_smooth.next();

            // Calculate pre-delay in samples
            let predelay_samples = (predelay_ms_smoothed * 0.001 * self.sample_rate) as usize;
            self.predelay_l.set_delay(predelay_samples);
            self.predelay_r.set_delay(predelay_samples);

            // Size sets the comb lengths; Decay sets per-comb feedback for RT60
            if i % COEFF_UPDATE_INTERVAL == 0 {
                self.update_combs(size_smoothed, decay_smoothed);
            }

            // Get dry input samples
            let dry_left = in_left
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Right channel normalled from left
            let dry_right = match in_right {
                Some(buf) => buf.samples.get(i).copied().unwrap_or(0.0),
                None => dry_left,
            };

            // Apply pre-delay
            let predelayed_l = self.predelay_l.process(dry_left);
            let predelayed_r = self.predelay_r.process(dry_right);

            // Mix to mono for reverb input (scaled down to prevent clipping)
            let input = (predelayed_l + predelayed_r) * 0.5;

            // Process through parallel comb filters
            let mut sum_l = 0.0;
            let mut sum_r = 0.0;

            for (idx, (comb_l, comb_r)) in self.combs_l.iter_mut().zip(self.combs_r.iter_mut()).enumerate() {
                sum_l += comb_l.process(input, self.comb_feedback[0][idx], damping_smoothed);
                sum_r += comb_r.process(input, self.comb_feedback[1][idx], damping_smoothed);
            }

            // Scale down comb output
            sum_l *= 0.125;
            sum_r *= 0.125;

            // Process through series allpass filters for diffusion
            let mut wet_l = sum_l;
            let mut wet_r = sum_r;

            for allpass in self.allpasses_l.iter_mut() {
                wet_l = allpass.process(wet_l);
            }

            for allpass in self.allpasses_r.iter_mut() {
                wet_r = allpass.process(wet_r);
            }

            // Apply stereo width
            // Width 0 = mono, 1 = full stereo
            let mid = (wet_l + wet_r) * 0.5;
            let side = (wet_l - wet_r) * 0.5;
            wet_l = mid + side * width_smoothed;
            wet_r = mid - side * width_smoothed;

            // Mix wet and dry
            let out_l = dry_left * (1.0 - mix_smoothed) + wet_l * mix_smoothed;
            let out_r = dry_right * (1.0 - mix_smoothed) + wet_r * mix_smoothed;

            // Write outputs
            out_left.samples[i] = out_l;
            out_right.samples[i] = out_r;
        }
    }

    fn reset(&mut self) {
        // Clear all filter buffers
        for comb in &mut self.combs_l {
            comb.clear();
        }
        for comb in &mut self.combs_r {
            comb.clear();
        }
        for allpass in &mut self.allpasses_l {
            allpass.clear();
        }
        for allpass in &mut self.allpasses_r {
            allpass.clear();
        }
        self.predelay_l.clear();
        self.predelay_r.clear();

        // Reset smoothed values
        self.size_smooth.reset(self.size_smooth.target());
        self.decay_smooth.reset(self.decay_smooth.target());
        self.damping_smooth.reset(self.damping_smooth.target());
        self.predelay_smooth.reset(self.predelay_smooth.target());
        self.mix_smooth.reset(self.mix_smooth.target());
        self.width_smooth.reset(self.width_smooth.target());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_info() {
        let reverb = Reverb::new();
        assert_eq!(reverb.info().id, "fx.reverb");
        assert_eq!(reverb.info().name, "Reverb");
        assert_eq!(reverb.info().category, ModuleCategory::Effect);
    }

    #[test]
    fn test_reverb_ports() {
        let reverb = Reverb::new();
        let ports = reverb.ports();

        assert_eq!(ports.len(), 4);

        // Input ports
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "in_l");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "in_r");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        // Output ports
        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "out_l");
        assert_eq!(ports[2].signal_type, SignalType::Audio);

        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "out_r");
        assert_eq!(ports[3].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_reverb_parameters() {
        let reverb = Reverb::new();
        let params = reverb.parameters();

        assert_eq!(params.len(), 6);
        assert_eq!(params[0].id, "size");
        assert_eq!(params[1].id, "decay");
        assert_eq!(params[2].id, "damping");
        assert_eq!(params[3].id, "predelay");
        assert_eq!(params[4].id, "mix");
        assert_eq!(params[5].id, "width");
    }

    #[test]
    fn test_reverb_produces_output() {
        let mut reverb = Reverb::new();
        reverb.prepare(44100.0, 256);

        // Create an impulse input
        let mut input = SignalBuffer::audio(256);
        input.samples[0] = 1.0;

        let mut outputs = vec![
            SignalBuffer::audio(256), // Out L
            SignalBuffer::audio(256), // Out R
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Process with default settings
        reverb.process(
            &[&input, &input],
            &mut outputs,
            &[0.5, 2.0, 0.5, 0.0, 0.5, 1.0], // size, decay, damping, predelay, mix, width
            &ctx,
        );

        // Output should have signal (mix of dry and wet)
        let has_output = outputs[0].samples.iter().any(|&s| s.abs() > 0.001);
        assert!(has_output, "Expected output signal");
    }

    #[test]
    fn test_reverb_tail() {
        let mut reverb = Reverb::new();
        let sample_rate = 44100.0;
        let block_size = 4410;
        reverb.prepare(sample_rate, block_size);

        // Create an impulse
        let mut input = SignalBuffer::audio(block_size);
        input.samples[0] = 1.0;

        let silence = SignalBuffer::audio(block_size);
        let mut outputs = vec![
            SignalBuffer::audio(block_size),
            SignalBuffer::audio(block_size),
        ];
        let ctx = ProcessContext::new(sample_rate, block_size);

        // Process impulse
        reverb.process(
            &[&input, &input],
            &mut outputs,
            &[0.5, 2.0, 0.5, 0.0, 1.0, 1.0], // 100% wet to hear reverb tail
            &ctx,
        );

        // Process silence and check reverb tail continues
        reverb.process(
            &[&silence, &silence],
            &mut outputs,
            &[0.5, 2.0, 0.5, 0.0, 1.0, 1.0],
            &ctx,
        );

        // Should still have output (reverb tail)
        let has_tail = outputs[0].samples.iter().any(|&s| s.abs() > 0.0001);
        assert!(has_tail, "Expected reverb tail to continue after input stops");
    }

    #[test]
    fn test_reverb_reset() {
        let mut reverb = Reverb::new();
        reverb.prepare(44100.0, 256);

        // Fill reverb with signal
        let mut input = SignalBuffer::audio(256);
        input.fill(0.5);
        let mut outputs = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        reverb.process(
            &[&input, &input],
            &mut outputs,
            &[0.5, 2.0, 0.5, 0.0, 1.0, 1.0],
            &ctx,
        );

        // Reset
        reverb.reset();

        // Process silence
        let silence = SignalBuffer::audio(256);
        let mut outputs2 = vec![
            SignalBuffer::audio(256),
            SignalBuffer::audio(256),
        ];

        reverb.process(
            &[&silence, &silence],
            &mut outputs2,
            &[0.5, 2.0, 0.5, 0.0, 1.0, 1.0],
            &ctx,
        );

        // Output should be near zero after reset
        assert!(
            outputs2[0].samples[0].abs() < 0.001,
            "Expected near-zero output after reset, got {}",
            outputs2[0].samples[0]
        );
    }

    #[test]
    fn test_reverb_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Reverb>();
    }

    #[test]
    fn test_reverb_default() {
        let reverb = Reverb::default();
        assert_eq!(reverb.info().id, "fx.reverb");
    }

    #[test]
    fn test_decay_to_feedback() {
        // Very short decay should give low feedback
        let short_feedback = Reverb::decay_to_feedback(0.1, 1000.0, 44100.0);

        // Longer decay should give higher feedback
        let long_feedback = Reverb::decay_to_feedback(10.0, 1000.0, 44100.0);

        assert!(short_feedback < long_feedback, "Longer decay should give higher feedback");
        assert!(long_feedback < 1.0, "Feedback should stay below unity");
        assert!(short_feedback >= 0.0, "Feedback should be non-negative");

        // RT60 identity: g^(T/d) must equal -60 dB
        let (decay, delay, sr) = (2.0, 1400.0, 44100.0);
        let g = Reverb::decay_to_feedback(decay, delay, sr);
        let trips = decay / (delay / sr);
        let level_db = 20.0 * g.powf(trips).log10();
        assert!((level_db + 60.0).abs() < 0.5, "Expected -60 dB after decay, got {}", level_db);
    }

    /// Renders the wet impulse response (damping 0, mix 100%, no pre-delay).
    fn impulse_response(size: f32, decay: f32, seconds: f32) -> Vec<f32> {
        let sample_rate = 44100.0;
        let block = 512;
        let mut reverb = Reverb::new();
        reverb.prepare(sample_rate, block);
        let params = [size, decay, 0.0, 0.0, 1.0, 1.0];
        reverb.size_smooth.reset(size);
        reverb.decay_smooth.reset(decay);
        reverb.damping_smooth.reset(0.0);
        reverb.mix_smooth.reset(1.0);

        let total = (seconds * sample_rate) as usize;
        let ctx = ProcessContext::new(sample_rate, block);
        let mut response = Vec::with_capacity(total);
        let mut first = true;
        while response.len() < total {
            let mut input = SignalBuffer::audio(block);
            if first {
                input.samples[0] = 1.0;
                first = false;
            }
            let mut outputs = vec![SignalBuffer::audio(block), SignalBuffer::audio(block)];
            reverb.process(&[&input], &mut outputs, &params, &ctx);
            response.extend_from_slice(&outputs[0].samples);
        }
        response
    }

    #[test]
    fn test_reverb_size_changes_echo_time() {
        let first_echo = |size: f32| {
            impulse_response(size, 2.0, 0.2)
                .iter()
                .position(|s| s.abs() > 1e-6)
                .expect("reverb should produce an echo")
        };
        let small = first_echo(0.0);
        let large = first_echo(1.0);
        // Shortest comb is 1116 samples at 1x; Size spans 0.5x..2x.
        assert_eq!(small, 558, "Size 0 first echo");
        assert_eq!(large, 2232, "Size 1 first echo");
    }

    #[test]
    fn test_reverb_decay_matches_rt60() {
        // With Decay = 1 s the tail should fall ~30 dB between 0.2 s and 0.7 s.
        let ir = impulse_response(0.5, 1.0, 0.8);
        let rms_at = |t: f32| {
            let start = (t * 44100.0) as usize;
            let w = &ir[start..start + 2205];
            (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt()
        };
        let drop_db = 20.0 * (rms_at(0.2) / rms_at(0.7)).log10();
        assert!(
            (drop_db - 30.0).abs() < 6.0,
            "Expected ~30 dB decay over 0.5 s at RT60 = 1 s, got {:.1} dB",
            drop_db
        );
    }

    #[test]
    fn test_reverb_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Reverb>();

        assert!(registry.contains("fx.reverb"));

        let module = registry.create("fx.reverb");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "fx.reverb");
        assert_eq!(module.info().name, "Reverb");
        assert_eq!(module.ports().len(), 4);
        assert_eq!(module.parameters().len(), 6);
    }
}
