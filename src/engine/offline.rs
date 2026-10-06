//! Offline (non-real-time) rendering.
//!
//! Runs an [`AudioGraph`] directly, with no audio device, so patches and
//! modules can be rendered to sample buffers for tests, measurements, and the
//! `render` tool. Uses the same module registry, patch compilation and
//! [`GraphPlan`] swapping as the live app.

use crate::dsp::denormal::DenormalGuard;
use crate::dsp::{MidiEvent, MidiMessage, ProcessContext};
use crate::persistence::{compile_patch, CompiledPatch, Patch, PatchError};

use super::{create_module_registry, AudioGraph, EngineCommand, GraphPlan};

/// Rendered stereo audio.
#[derive(Debug, Default, Clone)]
pub struct StereoBuffer {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Drives an [`AudioGraph`] block by block without an audio device.
pub struct OfflineRenderer {
    graph: AudioGraph,
    plan: Box<GraphPlan>,
    context: ProcessContext<'static>,
    /// Frames processed so far: the renderer's sample clock.
    position: u64,
    /// Queued MIDI as (frame, event), in frame order.
    midi: Vec<(u64, MidiEvent)>,
    /// The current block's MIDI, re-based to the block start.
    block_midi: Vec<MidiEvent>,
}

impl OfflineRenderer {
    /// Creates a renderer with an empty graph.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            graph: AudioGraph::with_registry(sample_rate, block_size, create_module_registry()),
            plan: Box::new(GraphPlan::empty(block_size)),
            context: ProcessContext::new(sample_rate, block_size),
            position: 0,
            midi: Vec::new(),
            block_midi: Vec::new(),
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
        // Keep the running plan in step, as the live engine does
        match command {
            EngineCommand::SetParameter { node_id, param_index, value } => {
                self.plan.set_parameter(node_id, param_index, value);
            }
            EngineCommand::SetBypass { node_id, bypassed } => {
                self.plan.set_bypass(node_id, bypassed);
            }
            _ => {}
        }
        self.graph.handle_command(command)
    }

    /// Mutable access to the underlying graph. Changes take effect from the
    /// next render.
    pub fn graph_mut(&mut self) -> &mut AudioGraph {
        &mut self.graph
    }

    /// Installs a new plan if the graph changed, carrying running modules over.
    fn sync_plan(&mut self) {
        if let Some(mut plan) = self.graph.take_plan() {
            plan.take_over(&mut self.plan);
            self.plan = plan;
        }
    }

    /// Queues a MIDI message to arrive at `frame` on the renderer's sample
    /// clock (see [`position`](Self::position)), as live MIDI would be
    /// placed by the audio thread.
    pub fn queue_midi(&mut self, frame: u64, channel: u8, message: MidiMessage) {
        let at = self.midi.partition_point(|&(queued, _)| queued <= frame);
        self.midi.insert(at, (frame, MidiEvent::new(0, channel, message)));
    }

    /// Frames processed so far. Renders run in whole blocks, so this can be
    /// ahead of the frames returned when a render isn't a whole number of
    /// blocks.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Moves the queued MIDI that falls in the next block into `block_midi`.
    fn take_block_midi(&mut self) {
        let end = self.position + self.context.block_size as u64;
        let count = self.midi.partition_point(|&(frame, _)| frame < end);
        self.block_midi.clear();
        for (frame, mut event) in self.midi.drain(..count) {
            event.sample_offset = frame.saturating_sub(self.position) as u32;
            self.block_midi.push(event);
        }
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

        // Same floating-point mode as the live audio callback
        let _denormals = DenormalGuard::new();

        self.sync_plan();
        while out.left.len() < frames {
            self.take_block_midi();
            self.plan.process(&self.context.with_midi(&self.block_midi));
            self.position += self.context.block_size as u64;

            let wanted = (frames - out.left.len()).min(self.context.block_size);
            match self.plan.audio_output() {
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
    use crate::modules::Oscillator;
    use crate::persistence::{ConnectionData, NamedParameter, NodeData, ParameterValue};

    /// Oscillator (default C4, chosen waveform) into the output's Mono input.
    fn osc_patch(waveform: usize) -> Patch {
        let mut patch = Patch::new("osc");
        let mut osc = NodeData::new(1, "osc.sine", (0.0, 0.0));
        osc.parameters = vec![
            NamedParameter::new("Octave", ParameterValue::Number(0.0)),
            NamedParameter::new("FM Depth", ParameterValue::Number(0.0)),
            NamedParameter::new("Waveform", ParameterValue::Select(waveform)),
            NamedParameter::new("Pulse Width", ParameterValue::LinearRange(0.5)),
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
        let cents = 1200.0 * (f / Oscillator::C4_HZ as f64).log2();
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
    fn test_unpatched_right_input_is_normalled_end_to_end() {
        // osc -> Delay "In L" only; both delay outputs feed the output module.
        // With nothing in "In R", the delay must output the same on both sides.
        let mut patch = osc_patch(1);
        patch.connections.clear();
        patch.nodes.push(NodeData::new(3, "fx.delay", (100.0, 0.0)));
        patch.connections.push(ConnectionData::new(1, "Out", 3, "In L"));
        patch.connections.push(ConnectionData::new(3, "Out L", 2, "Left"));
        patch.connections.push(ConnectionData::new(3, "Out R", 2, "Right"));

        let (mut r, compiled) = OfflineRenderer::from_patch(&patch, 48000.0, 256).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
        let out = r.render_seconds(0.5);
        assert!(rms(&out.right) > 0.05, "right channel should carry the mono source");
        assert_eq!(out.left, out.right);
    }

    #[test]
    fn test_editing_between_renders_keeps_module_state() {
        // Rendering in two halves, with an unrelated edit in between, must
        // match rendering in one go: the oscillator keeps its phase
        let whole = OfflineRenderer::from_patch(&osc_patch(0), 48000.0, 256).unwrap().0.render(2048);

        let (mut r, _) = OfflineRenderer::from_patch(&osc_patch(0), 48000.0, 256).unwrap();
        let mut split = r.render(1024);
        r.apply(EngineCommand::AddModule { node_id: 99, module_id: "mod.lfo" });
        let second = r.render(1024);
        split.left.extend(second.left);

        assert_eq!(split.left, whole.left);
    }

    /// Rising edges through `threshold`, in seconds.
    fn onsets(samples: &[f32], threshold: f32, sample_rate: f32) -> Vec<f32> {
        samples
            .windows(2)
            .enumerate()
            .filter(|(_, w)| w[0] < threshold && w[1] >= threshold)
            .map(|(i, _)| (i + 1) as f32 / sample_rate)
            .collect()
    }

    #[test]
    fn test_delay_sync_follows_clock_tempo() {
        // Clock (one short pulse per whole note) -> Delay synced to 1/4,
        // half dry and half wet. Each pulse is followed by its echo exactly
        // one beat later, at whatever tempo the Clock is set to.
        const CLOCK: u64 = 1;
        const DELAY: u64 = 2;
        const OUT: u64 = 3;
        let sr = 48000.0;
        let mut r = OfflineRenderer::new(sr, 256);
        for command in [
            EngineCommand::AddModule { node_id: CLOCK, module_id: "util.clock" },
            EngineCommand::AddModule { node_id: DELAY, module_id: "fx.delay" },
            EngineCommand::AddModule { node_id: OUT, module_id: "output.audio" },
            EngineCommand::Connect { from_node: CLOCK, from_port: 1, to_node: DELAY, to_port: 0 },
            EngineCommand::Connect { from_node: DELAY, from_port: 4, to_node: OUT, to_port: 2 },
            EngineCommand::SetParameter { node_id: CLOCK, param_index: 0, value: 120.0 },
            EngineCommand::SetParameter { node_id: CLOCK, param_index: 1, value: 2.0 }, // gate %
            EngineCommand::SetParameter { node_id: CLOCK, param_index: 2, value: 0.0 }, // whole
            EngineCommand::SetParameter { node_id: DELAY, param_index: 1, value: 0.0 }, // feedback
            EngineCommand::SetParameter { node_id: DELAY, param_index: 6, value: 1.0 }, // 1/4
        ] {
            r.apply(command);
        }

        // 120 BPM: pulses at 0 s and 2 s, echoes half a second after each
        let at_120 = onsets(&r.render_seconds(3.0).left, 0.15, sr);
        assert_eq!(at_120.len(), 4, "pulses and echoes at 120 BPM: {at_120:?}");
        for pair in at_120.chunks(2) {
            let beat = pair[1] - pair[0];
            assert!((beat - 0.5).abs() < 0.002, "echo after {beat} s at 120 BPM");
        }

        // Halfway through the clock's cycle, slow it to 80 BPM: the next pulse
        // comes 1.5 s later, and its echo a beat (0.75 s) after that
        r.apply(EngineCommand::SetParameter { node_id: CLOCK, param_index: 0, value: 80.0 });
        let at_80 = onsets(&r.render_seconds(2.5).left, 0.15, sr);
        assert_eq!(at_80.len(), 2, "pulse and echo at 80 BPM: {at_80:?}");
        let beat = at_80[1] - at_80[0];
        assert!((beat - 0.75).abs() < 0.002, "echo after {beat} s at 80 BPM");
    }

    /// Oscillator (saw) into a delay's left input, both delay outputs to the
    /// output module, with the delay bypassed or not.
    fn osc_through_delay(bypassed: bool) -> Patch {
        let mut patch = osc_patch(1);
        patch.connections.clear();
        let mut delay = NodeData::new(3, "fx.delay", (100.0, 0.0));
        delay.parameters = vec![NamedParameter::new("Feedback", ParameterValue::Number(0.9))];
        delay.bypassed = bypassed;
        patch.nodes.push(delay);
        patch.connections.push(ConnectionData::new(1, "Out", 3, "In L"));
        patch.connections.push(ConnectionData::new(3, "Out L", 2, "Left"));
        patch.connections.push(ConnectionData::new(3, "Out R", 2, "Right"));
        patch
    }

    #[test]
    fn test_bypassed_effect_passes_its_input_untouched() {
        let dry = OfflineRenderer::from_patch(&osc_patch(1), 48000.0, 256).unwrap().0.render(4096);

        let (mut r, compiled) = OfflineRenderer::from_patch(&osc_through_delay(true), 48000.0, 256).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
        let out = r.render(4096);

        // Only the left input is patched, and bypass normals it to both sides
        // just as the running delay would
        assert_eq!(out.left, dry.left);
        assert_eq!(out.right, dry.left);
    }

    #[test]
    fn test_effect_comes_back_from_bypass_without_old_echoes() {
        let (mut r, compiled) = OfflineRenderer::from_patch(&osc_through_delay(false), 48000.0, 256).unwrap();
        let delay = compiled.node_ids[&3];

        // Fill the delay line with echoes, then bypass and pull the cable
        r.render_seconds(0.5);
        r.apply(EngineCommand::SetBypass { node_id: delay, bypassed: true });
        r.apply(EngineCommand::Disconnect { node_id: delay, port: 0, is_input: true });
        let bypassed = r.render_seconds(0.3);
        let tail = &bypassed.left[bypassed.left.len() - 1024..];
        assert!(peak(tail) < 1e-4, "nothing patched in, nothing passes: {}", peak(tail));

        // Switched back in, the echoes it held when bypassed are gone
        r.apply(EngineCommand::SetBypass { node_id: delay, bypassed: false });
        let back = r.render_seconds(1.0);
        assert!(peak(&back.left) < 1e-6, "old echoes came back: peak {}", peak(&back.left));
    }

    #[test]
    fn test_modules_that_cant_bypass_ignore_it() {
        let whole = OfflineRenderer::from_patch(&osc_patch(0), 48000.0, 256).unwrap().0.render(2048);

        let (mut r, compiled) = OfflineRenderer::from_patch(&osc_patch(0), 48000.0, 256).unwrap();
        r.apply(EngineCommand::SetBypass { node_id: compiled.node_ids[&1], bypassed: true });
        assert_eq!(r.render(2048).left, whole.left);
    }

    #[test]
    fn test_graph_without_output_renders_silence() {
        let mut r = OfflineRenderer::new(48000.0, 128);
        let out = r.render(300);
        assert!(out.left.iter().chain(&out.right).all(|&s| s == 0.0));
    }

    /// MIDI Note playing an oscillator (chosen waveform) through a VCA
    /// opened by its gate.
    fn midi_voice_patch(waveform: usize) -> Patch {
        let mut patch = osc_patch(waveform);
        patch.connections.clear();
        patch.nodes.push(NodeData::new(3, "input.midi_note", (-200.0, 0.0)));
        patch.nodes.push(NodeData::new(4, "util.vca", (100.0, 0.0)));
        patch.connections.push(ConnectionData::new(3, "Pitch", 1, "V/Oct"));
        patch.connections.push(ConnectionData::new(3, "Gate", 4, "CV"));
        patch.connections.push(ConnectionData::new(1, "Out", 4, "In"));
        patch.connections.push(ConnectionData::new(4, "Out", 2, "Mono"));
        patch
    }

    #[test]
    fn test_midi_sixteenths_land_on_their_samples() {
        // 16th notes at 120 BPM (6000 samples apart), starting mid-block so
        // every note falls inside a block rather than on its edge
        let sr = 48000.0;
        let (mut r, compiled) = OfflineRenderer::from_patch(&midi_voice_patch(2), sr, 256).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);
        let first = 1037;
        for n in 0..8 {
            let on = first + n * 6000;
            r.queue_midi(on, 0, MidiMessage::NoteOn { note: 60, velocity: 100 });
            r.queue_midi(on + 3000, 0, MidiMessage::NoteOff { note: 60, velocity: 0 });
        }
        let out = r.render(first as usize + 8 * 6000);

        // A note starts where the square wave appears after silence
        let loud = |s: f32| s.abs() > 0.05;
        let onsets: Vec<usize> = (32..out.left.len())
            .filter(|&i| loud(out.left[i]) && out.left[i - 32..i].iter().all(|&s| !loud(s)))
            .collect();
        assert_eq!(onsets.len(), 8, "onsets at {onsets:?}");

        // The output stage's lookahead delays every note equally
        let latency = onsets[0] - first as usize;
        assert!(latency <= 64, "latency {latency}");
        for (n, &onset) in onsets.iter().enumerate() {
            let expected = first as usize + n * 6000 + latency;
            assert!(onset.abs_diff(expected) <= 1, "note {n} at {onset}, expected {expected}");
        }
    }

    /// Poly MIDI playing a polyphonic voice: sine oscillator through a VCA
    /// opened by an envelope, one of each per note, summed at the output.
    fn poly_voice_patch() -> Patch {
        let mut patch = osc_patch(0);
        patch.connections.clear();
        patch.nodes.push(NodeData::new(3, "input.poly_midi", (-200.0, 0.0)));
        let mut env = NodeData::new(4, "mod.adsr", (0.0, 200.0));
        env.parameters = vec![
            NamedParameter::new("Attack", ParameterValue::Number(0.002)),
            NamedParameter::new("Sustain", ParameterValue::Number(1.0)),
            NamedParameter::new("Release", ParameterValue::Number(0.01)),
        ];
        patch.nodes.push(env);
        // Quiet enough that four voices stay clear of the output limiter
        let mut vca = NodeData::new(5, "util.vca", (100.0, 0.0));
        vca.parameters = vec![NamedParameter::new("Level", ParameterValue::Number(0.2))];
        patch.nodes.push(vca);
        patch.connections.push(ConnectionData::new(3, "Pitch", 1, "V/Oct"));
        patch.connections.push(ConnectionData::new(3, "Gate", 4, "Gate"));
        patch.connections.push(ConnectionData::new(4, "Out", 5, "CV"));
        patch.connections.push(ConnectionData::new(1, "Out", 5, "In"));
        patch.connections.push(ConnectionData::new(5, "Out", 2, "Mono"));
        patch
    }

    #[test]
    fn test_poly_chord_plays_independent_voices() {
        let sr = 48000.0;
        let (mut r, compiled) = OfflineRenderer::from_patch(&poly_voice_patch(), sr, 256).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);

        // A C major chord, its notes let go one at a time
        let chord = [60_u8, 64, 67, 72];
        let quarter = (sr / 4.0) as u64;
        for (n, &note) in chord.iter().enumerate() {
            r.queue_midi(0, 0, MidiMessage::NoteOn { note, velocity: 100 });
            if n < 3 {
                r.queue_midi((n as u64 + 1) * quarter, 0, MidiMessage::NoteOff { note, velocity: 0 });
            }
        }
        let out = r.render_seconds(1.0);

        // Each quarter second, the notes still held ring and the rest are gone
        let hz = |note: u8| 440.0 * 2f64.powf((note as f64 - 69.0) / 12.0);
        for quarter_index in 0..4 {
            let start = quarter_index * quarter as usize + 2400;
            let spectrum = Spectrum::of(&out.left[start..start + 8192], sr);
            let level = |f: f64| {
                let bin = (f / spectrum.bin_hz).round() as usize;
                spectrum.magnitudes[bin - 2..=bin + 2].iter().copied().fold(0.0, f64::max)
            };
            for (n, &note) in chord.iter().enumerate() {
                let magnitude = level(hz(note));
                if n >= quarter_index {
                    assert!(magnitude > 0.05, "quarter {quarter_index}: note {note} should ring, {magnitude:.4}");
                } else {
                    assert!(magnitude < 0.002, "quarter {quarter_index}: note {note} should be released, {magnitude:.4}");
                }
            }
        }
    }

    #[test]
    fn test_midi_pitch_bend_bends_the_oscillator() {
        let sr = 48000.0;
        let (mut r, _) = OfflineRenderer::from_patch(&midi_voice_patch(0), sr, 256).unwrap();
        // A4, bent fully up: two semitones (the default range) to B4
        r.queue_midi(0, 0, MidiMessage::NoteOn { note: 69, velocity: 100 });
        r.queue_midi(0, 0, MidiMessage::PitchBend { value: 8191 });
        r.render_seconds(0.1);
        let out = r.render(65536);

        let f = Spectrum::of(&out.left, sr).dominant_frequency();
        let b4 = 440.0 * 2f64.powf(2.0 / 12.0);
        let cents = 1200.0 * (f / b4).log2();
        assert!(cents.abs() < 1.0, "expected B4, measured {:.2} Hz ({:+.2} cents)", f, cents);
    }
}
