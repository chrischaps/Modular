//! Offline (non-real-time) rendering.
//!
//! Runs an [`AudioGraph`] directly, with no audio device, so patches and
//! modules can be rendered to sample buffers for tests, measurements, and the
//! `render` tool. Uses the same module registry and patch compilation as the
//! live app.

use crate::dsp::ProcessContext;
use crate::persistence::{compile_patch, CompiledPatch, Patch, PatchError};

use super::{create_module_registry, AudioGraph, EngineCommand};

/// Rendered stereo audio.
#[derive(Debug, Default, Clone)]
pub struct StereoBuffer {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Drives an [`AudioGraph`] block by block without an audio device.
pub struct OfflineRenderer {
    graph: AudioGraph,
    context: ProcessContext,
}

impl OfflineRenderer {
    /// Creates a renderer with an empty graph.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            graph: AudioGraph::with_registry(sample_rate, block_size, create_module_registry()),
            context: ProcessContext::new(sample_rate, block_size),
        }
    }

    /// Creates a renderer and builds `patch` in it.
    ///
    /// Returns the compiled patch too, for its node-ID map and any warnings.
    pub fn from_patch(
        patch: &Patch,
        sample_rate: f32,
        block_size: usize,
    ) -> Result<(Self, CompiledPatch), PatchError> {
        let compiled = compile_patch(patch)?;
        let mut renderer = Self::new(sample_rate, block_size);
        for command in &compiled.commands {
            renderer.apply(command.clone());
        }
        Ok((renderer, compiled))
    }

    /// Applies an engine command (add module, connect, set parameter, ...).
    pub fn apply(&mut self, command: EngineCommand) -> bool {
        self.graph.handle_command(command)
    }

    /// Mutable access to the underlying graph.
    pub fn graph_mut(&mut self) -> &mut AudioGraph {
        &mut self.graph
    }

    /// The sample rate being rendered at.
    pub fn sample_rate(&self) -> f32 {
        self.context.sample_rate
    }

    /// Renders `frames` frames of the graph's audio output. A graph without an
    /// output module renders silence.
    pub fn render(&mut self, frames: usize) -> StereoBuffer {
        let mut out = StereoBuffer {
            left: Vec::with_capacity(frames),
            right: Vec::with_capacity(frames),
        };

        while out.left.len() < frames {
            self.graph.process(&self.context);

            // Monitoring data is only consumed by the UI; drop it so it
            // doesn't accumulate across blocks.
            self.graph.drain_sampled_input_values();
            self.graph.drain_sampled_output_values();
            self.graph.drain_scope_buffers();

            let wanted = (frames - out.left.len()).min(self.context.block_size);
            match self.graph.get_output() {
                Some((left, right)) => {
                    out.left.extend_from_slice(&left[..wanted]);
                    out.right.extend_from_slice(&right[..wanted]);
                }
                None => {
                    out.left.resize(out.left.len() + wanted, 0.0);
                    out.right.resize(out.right.len() + wanted, 0.0);
                }
            }
        }
        out
    }

    /// Renders `seconds` of audio.
    pub fn render_seconds(&mut self, seconds: f32) -> StereoBuffer {
        let frames = (seconds * self.context.sample_rate).round() as usize;
        self.render(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::{peak, rms, Spectrum};
    use crate::modules::SineOscillator;
    use crate::persistence::{ConnectionData, NodeData, ParameterValue};

    /// Oscillator (default C4, chosen waveform) into the output's Mono input.
    fn osc_patch(waveform: usize) -> Patch {
        let mut patch = Patch::new("osc");
        let mut osc = NodeData::new(1, "osc.sine", (0.0, 0.0));
        osc.parameters = vec![
            ParameterValue::Frequency(SineOscillator::C4_HZ),
            ParameterValue::LinearHz(0.0),
            ParameterValue::Select(waveform),
            ParameterValue::Scalar(0.5),
        ];
        patch.nodes.push(osc);
        patch.nodes.push(NodeData::new(2, "output.audio", (200.0, 0.0)));
        patch.connections.push(ConnectionData::new(1, "Out", 2, "Mono"));
        patch
    }

    #[test]
    fn test_renders_requested_length() {
        let (mut r, _) = OfflineRenderer::from_patch(&osc_patch(0), 48000.0, 256).unwrap();
        // Not a multiple of the block size
        let out = r.render(1000);
        assert_eq!(out.left.len(), 1000);
        assert_eq!(out.right.len(), 1000);
    }

    #[test]
    fn test_patch_plays_middle_c() {
        let sr = 44100.0;
        let (mut r, compiled) = OfflineRenderer::from_patch(&osc_patch(0), sr, 512).unwrap();
        assert!(compiled.warnings.is_empty());

        // Skip the first 0.1 s (parameter smoothing), then measure ~1.5 s
        r.render_seconds(0.1);
        let out = r.render(65536);

        assert!(out.left.iter().all(|s| s.is_finite()));
        assert!(rms(&out.left) > 0.1, "patch should be audible");
        assert_eq!(out.left, out.right, "mono input feeds both channels");

        let f = Spectrum::of(&out.left, sr).dominant_frequency();
        let cents = 1200.0 * (f / SineOscillator::C4_HZ as f64).log2();
        assert!(cents.abs() < 0.5, "expected C4, measured {:.3} Hz ({:+.2} cents)", f, cents);
    }

    #[test]
    fn test_output_stage_keeps_peaks_in_range() {
        // A full-scale saw at volume 0.8 stays within ±1 after the output stage
        let (mut r, _) = OfflineRenderer::from_patch(&osc_patch(1), 48000.0, 256).unwrap();
        let out = r.render_seconds(0.5);
        assert!(peak(&out.left) <= 1.0);
    }

    #[test]
    fn test_graph_without_output_renders_silence() {
        let mut r = OfflineRenderer::new(48000.0, 128);
        let out = r.render(300);
        assert!(out.left.iter().chain(&out.right).all(|&s| s == 0.0));
    }
}
