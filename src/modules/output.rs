//! Audio output module.
//!
//! This module serves as the final destination in the signal chain,
//! collecting audio and routing it to the system speakers.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    SignalType,
};

/// The audio output module that routes audio to the speakers.
///
/// This is the final destination in an audio graph. It accepts stereo
/// (Left/Right) or mono input and applies volume control and optional
/// limiting before output.
///
/// # Ports
///
/// - **Left** (Audio, Input): Left channel input for stereo output.
/// - **Right** (Audio, Input): Right channel input for stereo output.
/// - **Mono** (Audio, Input): Mono input, routes to both channels.
///
/// # Parameters
///
/// - **Volume** (0.0-1.0): Master volume control, default 0.8.
/// - **Limiter** (Toggle): Soft limiter to prevent harsh clipping, default on.
///
/// # Signal Routing
///
/// - If only Mono is connected, it routes to both Left and Right output.
/// - If Left/Right are connected, Mono is added to both channels.
/// - Output is scaled by Volume and optionally passed through a soft limiter.
pub struct AudioOutput {
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
    /// Internal stereo output buffer for the audio engine to read from.
    /// Index 0 = Left, Index 1 = Right.
    output_buffer: [Vec<f32>; 2],
    /// Peak level for left channel (for metering).
    peak_left: f32,
    /// Peak level for right channel (for metering).
    peak_right: f32,
}

impl AudioOutput {
    /// Creates a new audio output module.
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            ports: vec![
                // Input ports
                PortDefinition::input_with_default("left", "Left", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("right", "Right", SignalType::Audio, 0.0),
                PortDefinition::input_with_default("mono", "Mono", SignalType::Audio, 0.0),
            ],
            parameters: vec![
                ParameterDefinition::normalized("volume", "Volume", 0.8),
                ParameterDefinition::toggle("limiter", "Limiter", true),
            ],
            output_buffer: [Vec::new(), Vec::new()],
            peak_left: 0.0,
            peak_right: 0.0,
        }
    }

    /// Port index constants for clarity.
    const PORT_LEFT: usize = 0;
    const PORT_RIGHT: usize = 1;
    const PORT_MONO: usize = 2;

    /// Parameter index constants.
    const PARAM_VOLUME: usize = 0;
    const PARAM_LIMITER: usize = 1;

    /// Applies a soft clipper using tanh for gentle limiting.
    ///
    /// This provides a smooth saturation curve that prevents harsh
    /// digital clipping while maintaining signal character.
    #[inline]
    fn soft_clip(sample: f32) -> f32 {
        // tanh provides a smooth S-curve that saturates at ±1
        sample.tanh()
    }

    /// Returns the final stereo output buffer.
    ///
    /// This is how the audio engine can access the processed output.
    /// Index 0 = Left channel, Index 1 = Right channel.
    pub fn get_output_buffer(&self) -> &[Vec<f32>; 2] {
        &self.output_buffer
    }

    /// Returns the current peak levels for metering.
    ///
    /// Returns (left_peak, right_peak) in the range 0.0 to 1.0+
    /// (can exceed 1.0 before limiting).
    pub fn get_peak_levels(&self) -> (f32, f32) {
        (self.peak_left, self.peak_right)
    }

    /// Resets the peak meters.
    pub fn reset_peaks(&mut self) {
        self.peak_left = 0.0;
        self.peak_right = 0.0;
    }
}

impl Default for AudioOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for AudioOutput {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "output.audio",
            name: "Audio Output",
            category: ModuleCategory::Output,
            description: "Master stereo output with volume and limiting",
        };
        &INFO
    }

    fn ports(&self) -> &[PortDefinition] {
        &self.ports
    }

    fn parameters(&self) -> &[ParameterDefinition] {
        &self.parameters
    }

    fn prepare(&mut self, sample_rate: f32, max_block_size: usize) {
        self.sample_rate = sample_rate;
        // Pre-allocate output buffers
        self.output_buffer[0].resize(max_block_size, 0.0);
        self.output_buffer[1].resize(max_block_size, 0.0);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        _outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let volume = params[Self::PARAM_VOLUME];
        let limiter_enabled = params[Self::PARAM_LIMITER] > 0.5;

        // Get input buffers (may be empty if not connected)
        let left_input = inputs.get(Self::PORT_LEFT);
        let right_input = inputs.get(Self::PORT_RIGHT);
        let mono_input = inputs.get(Self::PORT_MONO);

        // Reset peak meters for this block
        let mut block_peak_left: f32 = 0.0;
        let mut block_peak_right: f32 = 0.0;

        // Process each sample
        for i in 0..context.block_size {
            // Get input samples
            let left_sample = left_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            let right_sample = right_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            let mono_sample = mono_input
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);

            // Mix: L/R inputs plus mono added to both channels
            let mut out_left = left_sample + mono_sample;
            let mut out_right = right_sample + mono_sample;

            // Apply volume
            out_left *= volume;
            out_right *= volume;

            // Track peak levels before limiting (for accurate metering)
            block_peak_left = block_peak_left.max(out_left.abs());
            block_peak_right = block_peak_right.max(out_right.abs());

            // Apply soft limiter if enabled
            if limiter_enabled {
                out_left = Self::soft_clip(out_left);
                out_right = Self::soft_clip(out_right);
            }

            // Store in output buffer (ensure we don't exceed capacity)
            if i < self.output_buffer[0].len() {
                self.output_buffer[0][i] = out_left;
                self.output_buffer[1][i] = out_right;
            }
        }

        // Update peak meters (with slow decay for visual smoothness)
        const DECAY_FACTOR: f32 = 0.95;
        self.peak_left = self.peak_left.max(block_peak_left) * DECAY_FACTOR;
        self.peak_right = self.peak_right.max(block_peak_right) * DECAY_FACTOR;
    }

    fn reset(&mut self) {
        // Clear output buffers
        for buf in &mut self.output_buffer {
            buf.fill(0.0);
        }
        self.peak_left = 0.0;
        self.peak_right = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_output_info() {
        let output = AudioOutput::new();
        assert_eq!(output.info().id, "output.audio");
        assert_eq!(output.info().name, "Audio Output");
        assert_eq!(output.info().category, ModuleCategory::Output);
    }

    #[test]
    fn test_audio_output_ports() {
        let output = AudioOutput::new();
        let ports = output.ports();

        assert_eq!(ports.len(), 3);

        // All are audio inputs
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "left");
        assert_eq!(ports[0].signal_type, SignalType::Audio);

        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "right");
        assert_eq!(ports[1].signal_type, SignalType::Audio);

        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "mono");
        assert_eq!(ports[2].signal_type, SignalType::Audio);
    }

    #[test]
    fn test_audio_output_parameters() {
        let output = AudioOutput::new();
        let params = output.parameters();

        assert_eq!(params.len(), 2);

        // Volume parameter
        assert_eq!(params[0].id, "volume");
        assert_eq!(params[0].min, 0.0);
        assert_eq!(params[0].max, 1.0);
        assert_eq!(params[0].default, 0.8);

        // Limiter parameter (toggle)
        assert_eq!(params[1].id, "limiter");
        assert_eq!(params[1].min, 0.0);
        assert_eq!(params[1].max, 1.0);
        assert_eq!(params[1].default, 1.0); // true = 1.0
    }

    #[test]
    fn test_mono_routes_to_both_channels() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Create mono input signal
        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(0.5);

        // Empty left/right inputs
        let left_input = SignalBuffer::audio(256);
        let right_input = SignalBuffer::audio(256);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter off
        output.process(
            &[&left_input, &right_input, &mono_input],
            &mut [],
            &[1.0, 0.0],
            &ctx,
        );

        let out_buf = output.get_output_buffer();

        // Both channels should have the mono signal
        for i in 0..256 {
            assert!(
                (out_buf[0][i] - 0.5).abs() < 0.001,
                "Left channel sample {} should be 0.5, got {}",
                i,
                out_buf[0][i]
            );
            assert!(
                (out_buf[1][i] - 0.5).abs() < 0.001,
                "Right channel sample {} should be 0.5, got {}",
                i,
                out_buf[1][i]
            );
        }
    }

    #[test]
    fn test_stereo_inputs() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Create stereo inputs
        let mut left_input = SignalBuffer::audio(256);
        let mut right_input = SignalBuffer::audio(256);
        left_input.fill(0.3);
        right_input.fill(0.7);

        // No mono input
        let mono_input = SignalBuffer::audio(256);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter off
        output.process(
            &[&left_input, &right_input, &mono_input],
            &mut [],
            &[1.0, 0.0],
            &ctx,
        );

        let out_buf = output.get_output_buffer();

        // Channels should have their respective inputs
        for i in 0..256 {
            assert!(
                (out_buf[0][i] - 0.3).abs() < 0.001,
                "Left channel sample {} should be 0.3, got {}",
                i,
                out_buf[0][i]
            );
            assert!(
                (out_buf[1][i] - 0.7).abs() < 0.001,
                "Right channel sample {} should be 0.7, got {}",
                i,
                out_buf[1][i]
            );
        }
    }

    #[test]
    fn test_mono_added_to_stereo() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Create all inputs
        let mut left_input = SignalBuffer::audio(256);
        let mut right_input = SignalBuffer::audio(256);
        let mut mono_input = SignalBuffer::audio(256);
        left_input.fill(0.2);
        right_input.fill(0.3);
        mono_input.fill(0.1);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter off
        output.process(
            &[&left_input, &right_input, &mono_input],
            &mut [],
            &[1.0, 0.0],
            &ctx,
        );

        let out_buf = output.get_output_buffer();

        // L = 0.2 + 0.1 = 0.3, R = 0.3 + 0.1 = 0.4
        for i in 0..256 {
            assert!(
                (out_buf[0][i] - 0.3).abs() < 0.001,
                "Left channel sample {} should be 0.3, got {}",
                i,
                out_buf[0][i]
            );
            assert!(
                (out_buf[1][i] - 0.4).abs() < 0.001,
                "Right channel sample {} should be 0.4, got {}",
                i,
                out_buf[1][i]
            );
        }
    }

    #[test]
    fn test_volume_scales_output() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(1.0);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 0.5, limiter off
        output.process(&[&SignalBuffer::audio(256), &SignalBuffer::audio(256), &mono_input], &mut [], &[0.5, 0.0], &ctx);

        let out_buf = output.get_output_buffer();

        for i in 0..256 {
            assert!(
                (out_buf[0][i] - 0.5).abs() < 0.001,
                "Left channel should be scaled to 0.5"
            );
        }
    }

    #[test]
    fn test_limiter_prevents_clipping() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Create very loud input that would clip
        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(5.0); // Way over 1.0

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter ON
        output.process(&[&SignalBuffer::audio(256), &SignalBuffer::audio(256), &mono_input], &mut [], &[1.0, 1.0], &ctx);

        let out_buf = output.get_output_buffer();

        // Output should be limited (tanh(5.0) ≈ 0.9999)
        for i in 0..256 {
            assert!(
                out_buf[0][i].abs() <= 1.0,
                "Limiter should keep output within -1 to 1, got {}",
                out_buf[0][i]
            );
            assert!(
                out_buf[0][i] > 0.99,
                "Limiter should be close to 1.0 for loud input, got {}",
                out_buf[0][i]
            );
        }
    }

    #[test]
    fn test_limiter_off_allows_clipping() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Create loud input
        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(2.0);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter OFF
        output.process(&[&SignalBuffer::audio(256), &SignalBuffer::audio(256), &mono_input], &mut [], &[1.0, 0.0], &ctx);

        let out_buf = output.get_output_buffer();

        // Output should NOT be limited
        for i in 0..256 {
            assert!(
                (out_buf[0][i] - 2.0).abs() < 0.001,
                "With limiter off, output should pass through as-is"
            );
        }
    }

    #[test]
    fn test_soft_clip_function() {
        // tanh provides smooth saturation
        assert!((AudioOutput::soft_clip(0.0) - 0.0).abs() < 0.001);
        assert!((AudioOutput::soft_clip(0.5) - 0.5_f32.tanh()).abs() < 0.001);
        assert!((AudioOutput::soft_clip(1.0) - 1.0_f32.tanh()).abs() < 0.001);
        assert!((AudioOutput::soft_clip(5.0) - 5.0_f32.tanh()).abs() < 0.001);

        // Negative values
        assert!((AudioOutput::soft_clip(-0.5) - (-0.5_f32).tanh()).abs() < 0.001);
    }

    #[test]
    fn test_peak_metering() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(0.8);

        let ctx = ProcessContext::new(44100.0, 256);

        // Process with volume = 1.0, limiter off
        output.process(&[&SignalBuffer::audio(256), &SignalBuffer::audio(256), &mono_input], &mut [], &[1.0, 0.0], &ctx);

        let (left_peak, right_peak) = output.get_peak_levels();

        // Peak should be around 0.8 (with some decay applied)
        assert!(
            left_peak > 0.7 && left_peak <= 0.8,
            "Peak should be close to 0.8, got {}",
            left_peak
        );
        assert!(
            right_peak > 0.7 && right_peak <= 0.8,
            "Peak should be close to 0.8, got {}",
            right_peak
        );
    }

    #[test]
    fn test_reset() {
        let mut output = AudioOutput::new();
        output.prepare(44100.0, 256);

        // Process some audio to fill buffers and set peaks
        let mut mono_input = SignalBuffer::audio(256);
        mono_input.fill(0.8);
        let ctx = ProcessContext::new(44100.0, 256);
        output.process(&[&SignalBuffer::audio(256), &SignalBuffer::audio(256), &mono_input], &mut [], &[1.0, 0.0], &ctx);

        // Reset
        output.reset();

        // Buffers should be cleared
        let out_buf = output.get_output_buffer();
        for sample in &out_buf[0] {
            assert_eq!(*sample, 0.0);
        }

        // Peaks should be reset
        let (left_peak, right_peak) = output.get_peak_levels();
        assert_eq!(left_peak, 0.0);
        assert_eq!(right_peak, 0.0);
    }

    #[test]
    fn test_audio_output_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<AudioOutput>();
    }

    #[test]
    fn test_audio_output_default() {
        let output = AudioOutput::default();
        assert_eq!(output.info().id, "output.audio");
    }

    #[test]
    fn test_audio_output_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<AudioOutput>();

        assert!(registry.contains("output.audio"));

        let module = registry.create("output.audio");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "output.audio");
        assert_eq!(module.info().name, "Audio Output");
        assert_eq!(module.ports().len(), 3);
        assert_eq!(module.parameters().len(), 2);
    }
}
