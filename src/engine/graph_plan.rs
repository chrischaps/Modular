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

use std::ops::Range;

use crate::dsp::{DspModule, OutputLevels, ProcessContext, SignalBuffer};
use crate::engine::commands::{NodeId, PortIndex};

/// The most input ports a module may have. Inputs are passed to modules as a
/// stack array of buffer references, so this bounds that array.
pub const MAX_INPUTS: usize = 32;

/// Filler for unused entries of the per-module input array.
static EMPTY_BUFFER: SignalBuffer = SignalBuffer::EMPTY;

/// Where a module input reads from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputSource {
    /// An upstream module's output buffer (index into `GraphPlan::outputs`).
    Output(usize),
    /// An unpatched input's stand-in buffer (index into `GraphPlan::defaults`).
    Default(usize),
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

        let Self { nodes, outputs, defaults, .. } = self;
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

            let mut inputs = [&EMPTY_BUFFER; MAX_INPUTS];
            for (slot, source) in inputs.iter_mut().zip(&node.inputs) {
                *slot = match *source {
                    InputSource::Output(index) => &upstream[index],
                    InputSource::Default(index) => &defaults[index],
                };
            }

            module.process(&inputs[..node.inputs.len()], own, &node.params, context);
        }
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
        for buffer in &mut self.outputs {
            buffer.samples.resize(len, 0.0);
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
