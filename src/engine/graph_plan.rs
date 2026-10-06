//! Compiled, real-time-ready form of the audio graph.
//!
//! A [`GraphPlan`] is built off the audio thread by
//! [`AudioGraph::compile`](super::AudioGraph::compile). Everything the audio
//! callback needs is resolved up front: the processing order, which buffer
//! feeds each input, every signal buffer at full block capacity, and the
//! monitor taps. Running a block is then plain slice indexing, with no
//! allocation, hashing or searching.
//!
//! Modules already running in the previous plan are not rebuilt. A node that
//! existed before is compiled without a module, and [`GraphPlan::take_over`]
//! moves the live module (with its phase, envelope and delay-line state) from
//! the outgoing plan into the new one. That is a pointer move, so swapping
//! plans on the audio thread is real-time safe. The outgoing plan, holding
//! any removed modules, is handed back to be dropped on another thread.
//!
//! A bypassed filter or effect passes its audio input straight to its output.
//! Switching crossfades over [`BYPASS_FADE_SECONDS`] so nothing clicks, and
//! once fully bypassed the module isn't run at all.
//!
//! Buffers carry up to [`MAX_CHANNELS`](crate::dsp::MAX_CHANNELS) channels.
//! Polyphonic modules set how many each block, and the count flows
//! downstream with the signal. A module that isn't polyphonic hears a
//! polyphonic cable on an audio input as all its channels summed.

use std::ops::Range;

use crate::dsp::{DspModule, OutputLevels, ProcessContext, SignalBuffer};
use crate::engine::commands::{NodeId, PortIndex};

pub use crate::dsp::module_trait::MAX_INPUTS;

/// How long bypassing a module, or bringing it back, crossfades for.
pub const BYPASS_FADE_SECONDS: f32 = 0.02;

/// Filler for unused entries of the per-module input array.
static EMPTY_BUFFER: SignalBuffer = SignalBuffer::EMPTY;

/// Where a module input reads from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputSource {
    /// An upstream module's output buffer (index into `GraphPlan::outputs`).
    Output(usize),
    /// An unpatched input's stand-in buffer (index into `GraphPlan::defaults`).
    Default(usize),
    /// A polyphonic upstream output (index into `GraphPlan::outputs`) heard
    /// by a mono audio input as the sum of its channels, written into a
    /// buffer of its own (index into `GraphPlan::mixdowns`).
    Mixdown { source: usize, mix: usize },
}

/// What a monitored input reports.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MonitorSource {
    /// The first sample of an upstream output buffer.
    Output(usize),
    /// A fixed value: the port's default, as nothing is patched in.
    Constant(f32),
}

/// One module in the plan.
pub(crate) struct PlanNode {
    pub(crate) node_id: NodeId,
    /// `None` until a freshly created module is supplied, or until
    /// [`GraphPlan::take_over`] moves the running module in.
    pub(crate) module: Option<Box<dyn DspModule>>,
    pub(crate) params: Vec<f32>,
    /// One entry per input port, in port order.
    pub(crate) inputs: Vec<InputSource>,
    /// This node's output buffers within `GraphPlan::outputs`.
    pub(crate) outputs: Range<usize>,
    /// Whether the module is bypassed: where the crossfade is heading.
    pub(crate) bypassed: bool,
    /// How much of the module's own output is heard: 1 in the signal path,
    /// 0 fully bypassed, in between while crossfading.
    pub(crate) wet: f32,
    /// For each output, the input (index into `inputs`) it passes while
    /// bypassed. Empty for modules that can't be bypassed.
    pub(crate) dry: Vec<Option<usize>>,
}

/// A monitored input port, reported to the UI for knob animation.
pub(crate) struct InputTap {
    pub(crate) node_id: NodeId,
    pub(crate) input_index: PortIndex,
    pub(crate) source: MonitorSource,
}

/// A monitored output port, reported to the UI for LEDs and cable animation.
pub(crate) struct OutputTap {
    pub(crate) node_id: NodeId,
    pub(crate) output_index: PortIndex,
    pub(crate) buffer: usize,
}

/// A compiled audio graph, ready to run on the audio thread.
pub struct GraphPlan {
    /// Modules in processing (topological) order.
    pub(crate) nodes: Vec<PlanNode>,
    /// Every output port's buffer, grouped by node in processing order, so a
    /// node's inputs always lie before its own outputs.
    pub(crate) outputs: Vec<SignalBuffer>,
    /// Stand-ins for unpatched inputs, holding the port's default value.
    pub(crate) defaults: Vec<SignalBuffer>,
    /// The value each `defaults` buffer is filled with.
    pub(crate) default_values: Vec<f32>,
    /// Sums of polyphonic cables, for mono audio inputs.
    pub(crate) mixdowns: Vec<SignalBuffer>,
    pub(crate) input_taps: Vec<InputTap>,
    pub(crate) output_taps: Vec<OutputTap>,
    /// The number of samples every buffer currently holds.
    pub(crate) block_len: usize,
    /// The capacity every buffer was allocated with.
    pub(crate) max_block_size: usize,
}

impl GraphPlan {
    /// A plan with no modules.
    pub fn empty(max_block_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            outputs: Vec::new(),
            defaults: Vec::new(),
            default_values: Vec::new(),
            mixdowns: Vec::new(),
            input_taps: Vec::new(),
            output_taps: Vec::new(),
            block_len: max_block_size,
            max_block_size,
        }
    }

    /// The largest block [`process`](Self::process) accepts.
    pub fn max_block_size(&self) -> usize {
        self.max_block_size
    }

    /// The number of modules in the plan.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the plan has no modules.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Node IDs in processing order.
    pub fn processing_order(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.iter().map(|node| node.node_id)
    }

    /// Moves the running module of every node this plan shares with
    /// `previous` into this plan. Modules this plan doesn't use stay in
    /// `previous`, to be dropped with it.
    ///
    /// REAL-TIME SAFE: moves boxes, never allocates.
    pub fn take_over(&mut self, previous: &mut GraphPlan) {
        for node in self.nodes.iter_mut().filter(|node| node.module.is_none()) {
            if let Some(old) = previous.nodes.iter_mut().find(|old| old.node_id == node.node_id) {
                node.module = old.module.take();
                // A crossfade in progress carries on where it was
                node.wet = old.wet;
            }
        }
    }

    /// Sets a parameter on a module. Returns false if the node or parameter
    /// doesn't exist in this plan.
    ///
    /// REAL-TIME SAFE.
    pub fn set_parameter(&mut self, node_id: NodeId, param_index: usize, value: f32) -> bool {
        self.nodes
            .iter_mut()
            .find(|node| node.node_id == node_id)
            .and_then(|node| node.params.get_mut(param_index))
            .map(|param| *param = value)
            .is_some()
    }

    /// Bypasses a module or brings it back, crossfading over the next
    /// [`BYPASS_FADE_SECONDS`]. Returns false if the node doesn't exist in
    /// this plan or can't be bypassed.
    ///
    /// REAL-TIME SAFE.
    pub fn set_bypass(&mut self, node_id: NodeId, bypassed: bool) -> bool {
        self.nodes
            .iter_mut()
            .find(|node| node.node_id == node_id && !node.dry.is_empty())
            .map(|node| node.bypassed = bypassed)
            .is_some()
    }

    /// Resets the internal state of every module (oscillator phases, filter
    /// memory, delay and reverb tails) so playback restarts from silence.
    pub fn reset_modules(&mut self) {
        for module in self.nodes.iter_mut().filter_map(|node| node.module.as_mut()) {
            module.reset();
        }
    }

    /// Re-prepares every module for a new sample rate.
    ///
    /// Not real-time safe: modules may reallocate. Call with the stream stopped.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        for module in self.nodes.iter_mut().filter_map(|node| node.module.as_mut()) {
            module.prepare(sample_rate, self.max_block_size);
            module.reset();
        }
    }

    /// Processes one block through every module in order.
    ///
    /// `context.block_size` must not exceed [`max_block_size`](Self::max_block_size);
    /// larger blocks are clamped. Callers with bigger device buffers process
    /// them in chunks.
    ///
    /// REAL-TIME SAFE: no allocation, locking or searching.
    pub fn process(&mut self, context: &ProcessContext) {
        debug_assert!(context.block_size <= self.max_block_size);
        self.set_block_len(context.block_size.min(self.max_block_size));

        // Every module in the block sees the same tempo, whatever its place
        // in the processing order
        let mut context = *context;
        if let Some(bpm) = self.tempo_bpm() {
            context.transport.tempo_bpm = Some(bpm);
        }
        let context = &context;
        let fade_step = 1.0 / (BYPASS_FADE_SECONDS * context.sample_rate).max(1.0);

        let Self { nodes, outputs, defaults, mixdowns, .. } = self;
        for node in nodes.iter_mut() {
            // Inputs come from earlier nodes, whose buffers all precede ours
            let (upstream, rest) = outputs.split_at_mut(node.outputs.start);
            let own = &mut rest[..node.outputs.len()];
            for buffer in own.iter_mut() {
                buffer.clear();
            }

            let Some(module) = node.module.as_mut() else {
                continue;
            };

            for source in &node.inputs {
                if let InputSource::Mixdown { source, mix } = *source {
                    if upstream[source].channels() > 1 {
                        mixdowns[mix].mix_down(&upstream[source]);
                    }
                }
            }

            let mut inputs = [&EMPTY_BUFFER; MAX_INPUTS];
            for (slot, source) in inputs.iter_mut().zip(&node.inputs) {
                *slot = match *source {
                    InputSource::Output(index) => &upstream[index],
                    InputSource::Default(index) => &defaults[index],
                    // One channel needs no summing: read it where it is
                    InputSource::Mixdown { source, .. } if upstream[source].channels() == 1 => {
                        &upstream[source]
                    }
                    InputSource::Mixdown { mix, .. } => &mixdowns[mix],
                };
            }

            let inputs = &inputs[..node.inputs.len()];

            if node.bypassed && node.wet == 0.0 {
                // Fully bypassed: the module rests while its input passes by
                pass_dry(own, &node.dry, inputs);
                continue;
            }
            if !node.bypassed && node.wet == 0.0 {
                // Coming back from a full bypass. Start from silence rather
                // than replay whatever the delay lines held when it left
                module.reset();
            }

            module.process(inputs, own, &node.params, context);

            let target = if node.bypassed { 0.0 } else { 1.0 };
            if node.wet != target {
                node.wet = crossfade(own, &node.dry, inputs, node.wet, target, fade_step);
            }
        }
    }

    /// How many channels a node's output (counted among its outputs) carried
    /// in the last block, or `None` if there is no such output.
    pub fn output_channels(&self, node_id: NodeId, output_index: usize) -> Option<usize> {
        let node = self.nodes.iter().find(|node| node.node_id == node_id)?;
        let buffer = node.outputs.clone().nth(output_index)?;
        Some(self.outputs[buffer].channels())
    }

    /// Whether a node is bypassed, or `None` if it isn't in this plan.
    pub fn is_bypassed(&self, node_id: NodeId) -> Option<bool> {
        self.nodes.iter().find(|node| node.node_id == node_id).map(|node| node.bypassed)
    }

    /// The patch tempo: set by the first tempo source (a Clock) in processing
    /// order, or `None` if the patch has none.
    ///
    /// REAL-TIME SAFE.
    pub fn tempo_bpm(&self) -> Option<f32> {
        self.nodes.iter().find_map(|node| {
            node.module.as_ref().and_then(|module| module.tempo_bpm(&node.params))
        })
    }

    /// Sets every buffer's length to `len`, within its allocated capacity.
    fn set_block_len(&mut self, len: usize) {
        if len == self.block_len {
            return;
        }
        for buffer in self.outputs.iter_mut().chain(&mut self.mixdowns) {
            buffer.set_len(len);
        }
        for (buffer, &value) in self.defaults.iter_mut().zip(&self.default_values) {
            buffer.samples.resize(len, value);
        }
        self.block_len = len;
    }

    /// The values at monitored inputs after the last block: the first sample
    /// of the patched signal, or the port default when nothing is patched.
    pub fn input_values(&self) -> impl Iterator<Item = (NodeId, PortIndex, f32)> + '_ {
        self.input_taps.iter().map(|tap| {
            let value = match tap.source {
                MonitorSource::Output(index) => {
                    self.outputs[index].samples.first().copied().unwrap_or(0.0)
                }
                MonitorSource::Constant(value) => value,
            };
            (tap.node_id, tap.input_index, value)
        })
    }

    /// The values at monitored outputs after the last block: the sample with
    /// the largest magnitude, sign preserved, so bipolar signals such as LFOs
    /// can animate cables in reverse.
    pub fn output_values(&self) -> impl Iterator<Item = (NodeId, PortIndex, f32)> + '_ {
        self.output_taps.iter().map(|tap| {
            let value = self.outputs[tap.buffer].samples.iter().fold(0.0_f32, |acc, &sample| {
                if sample.abs() > acc.abs() {
                    sample
                } else {
                    acc
                }
            });
            (tap.node_id, tap.output_index, value)
        })
    }

    /// The first module in processing order that produces the final audio
    /// output, if any.
    pub fn output_module(&self) -> Option<&dyn DspModule> {
        self.nodes
            .iter()
            .filter_map(|node| node.module.as_deref())
            .find(|module| module.get_audio_output().is_some())
    }

    /// Takes the output module's meter readings since the last call.
    pub fn take_output_levels(&mut self) -> Option<OutputLevels> {
        self.nodes
            .iter_mut()
            .filter_map(|node| node.module.as_deref_mut())
            .find(|module| module.get_audio_output().is_some())
            .and_then(|module| module.take_output_levels())
    }

    /// The final stereo output, as (left, right). The slices span the
    /// module's full buffer; the first `block_size` samples are this block.
    pub fn audio_output(&self) -> Option<(&[f32], &[f32])> {
        self.output_module().and_then(|module| module.get_audio_output())
    }

    /// Calls `f` for each oscilloscope with a new capture ready.
    ///
    /// REAL-TIME SAFE: captures are lent, not copied.
    pub fn take_scope_captures(&mut self, mut f: impl FnMut(NodeId, &[f32], &[f32], bool)) {
        for node in &mut self.nodes {
            if let Some(module) = node.module.as_mut() {
                if let Some((channel1, channel2, triggered)) = module.take_scope_data() {
                    f(node.node_id, channel1, channel2, triggered);
                }
            }
        }
    }
}

/// Copies each bypassed output's input straight through, every channel of
/// it. Outputs with nothing to pass stay silent.
fn pass_dry(outputs: &mut [SignalBuffer], dry: &[Option<usize>], inputs: &[&SignalBuffer]) {
    for (output, source) in outputs.iter_mut().zip(dry) {
        if let Some(input) = source.and_then(|index| inputs.get(index)) {
            let channels = output.set_channels(input.channels());
            for channel in 0..channels {
                let input = &input.voice(channel).samples;
                for (out, &sample) in output.channel_mut(channel).iter_mut().zip(input) {
                    *out = sample;
                }
            }
        }
    }
}

/// Blends the module's outputs with its dry inputs, moving the wet amount
/// from `from` toward `to` by `step` per sample. Returns where it got to.
fn crossfade(
    outputs: &mut [SignalBuffer],
    dry: &[Option<usize>],
    inputs: &[&SignalBuffer],
    from: f32,
    to: f32,
    step: f32,
) -> f32 {
    let advance = |wet: f32| if to > wet { (wet + step).min(to) } else { (wet - step).max(to) };
    let mut reached = from;
    for (output, source) in outputs.iter_mut().zip(dry) {
        let input = source.and_then(|index| inputs.get(index));
        // As many channels as either side carries
        let wanted = output.channels().max(input.map_or(1, |buffer| buffer.channels()));
        let channels = output.set_channels(wanted);
        for channel in 0..channels {
            let input = input.map(|buffer| &buffer.voice(channel).samples);
            let mut wet = from;
            for (i, sample) in output.channel_mut(channel).iter_mut().enumerate() {
                wet = advance(wet);
                let dry = input.and_then(|samples| samples.get(i)).copied().unwrap_or(0.0);
                *sample = dry + wet * (*sample - dry);
            }
            reached = wet;
        }
    }
    reached
}
