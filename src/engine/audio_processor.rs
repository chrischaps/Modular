//! Audio Processor
//!
//! Handles audio processing in the audio callback: runs the compiled
//! [`GraphPlan`] and applies plans and parameter changes from the UI thread.

use std::time::Instant;

use rtrb::Consumer;

use crate::dsp::denormal::DenormalGuard;
use crate::dsp::{ModuleRegistry, Poly, ProcessContext};
use crate::modules::{AdsrEnvelope, Attenuverter, AudioOutput, Chorus, Clock, Compressor, Distortion, KeyboardInput, LadderFilter, Lfo, MidiMonitor, MidiNote, Mixer, Oscilloscope, PolyMidi, ParametricEq, Reverb, SampleHold, Oscillator, StepSequencer, StereoDelay, SvfFilter, Vca};

use super::channels::EngineHandle;
use super::commands::{AudioMessage, EngineEvent, ScopeFrame};
use super::graph_plan::GraphPlan;
use super::midi_engine::TimestampedMidiEvent;
use super::midi_scheduler::{take_chunk, MidiScheduler};

/// Creates a module registry with all built-in modules.
///
/// Registering a module here is all it takes to make it available in the
/// editor: node templates are generated from the registry. Registration
/// order is the order modules appear in the add-node menu.
///
/// Modules wrapped in [`Poly`] are polyphonic: they run one voice per
/// channel of a polyphonic cable.
pub fn create_module_registry() -> ModuleRegistry {
    let mut registry = ModuleRegistry::new();
    registry.register::<Poly<Oscillator>>();
    registry.register::<KeyboardInput>();
    registry.register::<MidiNote>();
    registry.register::<PolyMidi>();
    registry.register::<Poly<SvfFilter>>();
    registry.register::<Poly<LadderFilter>>();
    registry.register::<Poly<AdsrEnvelope>>();
    registry.register::<Lfo>();
    registry.register::<Clock>();
    registry.register::<Poly<Vca>>();
    registry.register::<Poly<Attenuverter>>();
    registry.register::<Mixer>();
    registry.register::<Poly<SampleHold>>();
    registry.register::<Oscilloscope>();
    registry.register::<StepSequencer>();
    registry.register::<StereoDelay>();
    registry.register::<Reverb>();
    registry.register::<ParametricEq>();
    registry.register::<Distortion>();
    registry.register::<Chorus>();
    registry.register::<Compressor>();
    registry.register::<MidiMonitor>();
    registry.register::<AudioOutput>();
    registry
}

/// Audio processor that runs in the audio callback.
///
/// This struct is moved into the audio callback closure and handles
/// all audio processing, including:
/// - Receiving compiled plans and parameter changes from the UI thread
/// - Running the current plan to generate samples
/// - Extracting output from the AudioOutput module
///
/// Nothing here allocates once constructed: graph changes arrive fully
/// built, and replaced plans are handed back to the UI thread to drop.
pub struct AudioProcessor {
    /// The graph currently playing.
    plan: Box<GraphPlan>,
    /// Handle for receiving messages from the UI thread.
    engine_handle: EngineHandle,
    /// Live MIDI input, placed at sample offsets for each callback.
    midi: MidiScheduler,
    /// Current sample rate.
    sample_rate: f32,
    /// Whether audio processing is active.
    is_playing: bool,
    /// Frame counter for throttling CPU load events.
    frame_counter: u32,
    /// Running average of CPU load (0.0-100.0).
    cpu_load_avg: f32,
}

impl AudioProcessor {
    /// Creates a new audio processor.
    ///
    /// # Arguments
    /// * `sample_rate` - The audio sample rate in Hz
    /// * `block_size` - The largest block the graph processes at once; device
    ///   buffers larger than this are processed in several blocks
    /// * `engine_handle` - Handle for receiving messages from the UI
    pub fn new(sample_rate: f32, block_size: usize, engine_handle: EngineHandle) -> Self {
        // Tell the UI side what to prepare new modules for
        engine_handle.set_audio_config(sample_rate, block_size);

        Self {
            plan: Box::new(GraphPlan::empty(block_size)),
            engine_handle,
            midi: MidiScheduler::new(),
            sample_rate,
            is_playing: false,
            frame_counter: 0,
            cpu_load_avg: 0.0,
        }
    }

    /// Feeds live MIDI from the MIDI engine to the graph. Events are placed
    /// at the sample matching when they arrived, one callback later.
    pub fn set_midi_input(&mut self, input: Consumer<TimestampedMidiEvent>) {
        self.midi.set_input(input);
    }

    /// How often to send CPU load events (in audio callbacks).
    /// At 44100Hz with 256 sample blocks, this is about 172 callbacks/sec.
    /// Sending every 8 callbacks gives ~21Hz update rate.
    const CPU_REPORT_INTERVAL: u32 = 8;

    /// Smoothing factor for CPU load averaging (0-1, higher = more responsive).
    const CPU_SMOOTHING: f32 = 0.3;

    /// Processes a block of audio.
    ///
    /// This is called from the cpal audio callback. It:
    /// 1. Applies pending messages from the UI (new plans, parameters, play/stop)
    /// 2. Places the MIDI that arrived since the last callback in this buffer
    /// 3. If playing, runs the graph, in chunks of at most the plan's block size
    /// 4. Writes the output module's audio to the output buffer
    ///
    /// REAL-TIME SAFE: no allocation, locking or blocking.
    ///
    /// # Arguments
    /// * `output` - The output buffer to fill with audio samples
    /// * `channels` - Number of output channels (typically 2 for stereo)
    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        // Decaying filter and reverb tails must not fall into slow denormal
        // arithmetic; restored when the callback returns
        let _denormals = DenormalGuard::new();

        // Start timing for CPU measurement; also the moment MIDI is placed against
        let start_time = Instant::now();

        // Process pending messages from UI
        self.process_messages();

        // Clear output buffer
        output.fill(0.0);

        if !self.is_playing || channels == 0 {
            // Notes played while stopped shouldn't all sound at once on Play
            self.midi.skip(start_time);
            // Reset CPU load when not playing
            self.cpu_load_avg = 0.0;
            return;
        }

        let num_frames = output.len() / channels;
        let mut midi = self.midi.collect(start_time, num_frames);

        // Run the graph over the device buffer in plan-sized blocks, each
        // with the MIDI that falls inside it
        let block = self.plan.max_block_size().max(1);
        for (index, chunk) in output.chunks_mut(block * channels).enumerate() {
            let frames = chunk.len() / channels;
            let start = index * block;
            let chunk_midi = take_chunk(&mut midi, start, start + frames);
            let context = ProcessContext::new(self.sample_rate, frames).with_midi(chunk_midi);
            self.plan.process(&context);
            Self::write_output(&self.plan, chunk, channels, frames);
        }

        self.send_monitor_values();
        self.send_scope_captures();
        self.send_output_level();

        // Calculate CPU load
        let elapsed = start_time.elapsed();
        let available_time = num_frames as f64 / self.sample_rate as f64;
        let cpu_percent = (elapsed.as_secs_f64() / available_time * 100.0) as f32;

        // Smooth the CPU load value using exponential moving average
        self.cpu_load_avg = Self::CPU_SMOOTHING * cpu_percent
            + (1.0 - Self::CPU_SMOOTHING) * self.cpu_load_avg;

        // Send CPU load event at regular intervals (to avoid flooding UI)
        self.frame_counter += 1;
        if self.frame_counter >= Self::CPU_REPORT_INTERVAL {
            self.frame_counter = 0;
            self.engine_handle.send_event_lossy(EngineEvent::CpuLoad(self.cpu_load_avg));
        }
    }

    /// Sends monitored input and output values to the UI thread, for knob
    /// animation and LED indicators.
    fn send_monitor_values(&mut self) {
        let Self { plan, engine_handle, .. } = self;
        for (node_id, input_index, value) in plan.input_values() {
            engine_handle.send_event_lossy(EngineEvent::InputValue { node_id, input_index, value });
        }
        for (node_id, output_index, value) in plan.output_values() {
            engine_handle.send_event_lossy(EngineEvent::OutputValue { node_id, output_index, value });
        }
    }

    /// Sends oscilloscope captures to the UI thread for waveform display.
    fn send_scope_captures(&mut self) {
        let Self { plan, engine_handle, .. } = self;
        plan.take_scope_captures(|node_id, channel1, channel2, triggered| {
            engine_handle.send_scope_frame_lossy(ScopeFrame::new(node_id, channel1, channel2, triggered));
        });
    }

    /// Sends this callback's output levels to the UI for metering.
    fn send_output_level(&mut self) {
        if let Some(levels) = self.plan.take_output_levels() {
            self.engine_handle.send_event_lossy(EngineEvent::OutputLevel(levels));
        }
    }

    /// Applies all pending messages from the UI thread.
    fn process_messages(&mut self) {
        while let Some(message) = self.engine_handle.recv_message() {
            match message {
                AudioMessage::InstallPlan(mut plan) => {
                    // Running modules move into the new plan; the old one,
                    // with any removed modules, goes back to be dropped
                    plan.take_over(&mut self.plan);
                    let retired = std::mem::replace(&mut self.plan, plan);
                    self.engine_handle.retire_plan(retired);
                }
                AudioMessage::SetParameter { node_id, param_index, value } => {
                    self.plan.set_parameter(node_id, param_index, value);
                }
                AudioMessage::SetBypass { node_id, bypassed } => {
                    self.plan.set_bypass(node_id, bypassed);
                }
                AudioMessage::SetPlaying(playing) => {
                    if self.is_playing && !playing {
                        // Clear tails so pressing Play again starts from silence
                        self.plan.reset_modules();
                    }
                    self.is_playing = playing;
                    let event = if playing {
                        EngineEvent::Started
                    } else {
                        EngineEvent::Stopped
                    };
                    self.engine_handle.send_event_lossy(event);
                }
            }
        }
    }

    /// Writes the output module's audio for one block into `output`
    /// (interleaved), duplicating to any channels beyond stereo.
    fn write_output(plan: &GraphPlan, output: &mut [f32], channels: usize, frames: usize) {
        let Some((left, right)) = plan.audio_output() else {
            return;
        };

        for (i, frame) in output.chunks_mut(channels).take(frames).enumerate() {
            let l = left.get(i).copied().unwrap_or(0.0);
            let r = right.get(i).copied().unwrap_or(0.0);

            frame[0] = l;
            if channels >= 2 {
                frame[1] = r;
            }
            // For more than 2 channels, duplicate to additional channels
            for ch in frame.iter_mut().skip(2) {
                *ch = (l + r) * 0.5;
            }
        }
    }

    /// Returns whether audio processing is currently active.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// The plan currently playing.
    pub fn plan(&self) -> &GraphPlan {
        &self.plan
    }

    /// Changes the sample rate, re-preparing every module. Only call this
    /// while no audio stream is running the processor.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        // Install anything already compiled for the old rate first, so it
        // gets re-prepared below along with everything else
        self.process_messages();

        self.sample_rate = sample_rate;
        self.plan.set_sample_rate(sample_rate);
        self.engine_handle.set_audio_config(sample_rate, self.plan.max_block_size());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{EngineChannels, EngineCommand, UiHandle};

    fn processor() -> (UiHandle, AudioProcessor) {
        let (ui, engine) = EngineChannels::with_defaults().split();
        (ui, AudioProcessor::new(44100.0, 256, engine))
    }

    /// Oscillator into the output module's Mono input, playing.
    fn playing_patch(ui: &mut UiHandle) {
        ui.send_command(EngineCommand::AddModule { node_id: 1, module_id: "osc.sine" });
        ui.send_command(EngineCommand::AddModule { node_id: 2, module_id: "output.audio" });
        ui.send_command(EngineCommand::Connect { from_node: 1, from_port: 5, to_node: 2, to_port: 2 });
        ui.send_command(EngineCommand::SetPlaying(true));
        ui.flush();
    }

    #[test]
    fn test_create_module_registry() {
        let registry = create_module_registry();
        assert!(registry.contains("osc.sine"));
        assert!(registry.contains("filter.svf"));
        assert!(registry.contains("filter.ladder"));
        assert!(registry.contains("mod.adsr"));
        assert!(registry.contains("util.clock"));
        assert!(registry.contains("util.vca"));
        assert!(registry.contains("util.attenuverter"));
        assert!(registry.contains("output.audio"));
        assert!(registry.contains("mod.lfo"));
        assert!(registry.contains("input.keyboard"));
        assert!(registry.contains("util.midi_monitor"));
        assert!(registry.contains("input.midi_note"));
        assert!(registry.contains("input.poly_midi"));
        assert!(registry.contains("util.sample_hold"));
        assert!(registry.contains("util.oscilloscope"));
        assert!(registry.contains("seq.step"));
        assert!(registry.contains("fx.delay"));
        assert!(registry.contains("fx.reverb"));
        assert!(registry.contains("fx.eq"));
        assert!(registry.contains("fx.distortion"));
        assert!(registry.contains("fx.chorus"));
        assert!(registry.contains("fx.compressor"));
        assert!(registry.contains("util.mixer"));
        assert_eq!(registry.len(), 23);
    }

    #[test]
    fn test_audio_processor_creation() {
        let (_ui, processor) = processor();
        assert!(!processor.is_playing());
    }

    #[test]
    fn test_audio_processor_silence_when_stopped() {
        let (_ui, mut processor) = processor();

        let mut output = vec![1.0; 512]; // Fill with non-zero
        processor.process(&mut output, 2);

        // Should be silence when not playing
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_audio_processor_responds_to_play_command() {
        let (mut ui, mut processor) = processor();

        // Send play command
        ui.send_command(EngineCommand::SetPlaying(true));

        // Process to handle the command
        let mut output = vec![0.0; 512];
        processor.process(&mut output, 2);

        assert!(processor.is_playing());

        // Check for Started event
        let event = ui.recv_event();
        assert!(matches!(event, Some(EngineEvent::Started)));
    }

    #[test]
    fn test_plays_a_patch_built_on_the_ui_side() {
        let (mut ui, mut processor) = processor();
        playing_patch(&mut ui);

        let mut output = vec![0.0; 1024];
        processor.process(&mut output, 2);

        assert_eq!(processor.plan().len(), 2);
        assert!(output.iter().any(|&s| s.abs() > 0.01), "patch should be audible");
    }

    #[test]
    fn test_device_buffers_larger_than_a_block_are_chunked() {
        let (mut ui, mut processor) = processor();
        playing_patch(&mut ui);

        // 441 frames: one full 256-frame block and a shorter one
        let mut output = vec![0.0; 441 * 2];
        processor.process(&mut output, 2);

        let tail = &output[256 * 2..];
        assert!(tail.iter().any(|&s| s.abs() > 0.01), "second chunk was rendered");
    }

    #[test]
    fn test_retired_plans_go_back_to_ui() {
        let (mut ui, mut processor) = processor();
        playing_patch(&mut ui);
        let mut output = vec![0.0; 512];
        processor.process(&mut output, 2);

        // Remove the oscillator: the new plan drops it, the old plan carries
        // it back to the UI thread
        ui.send_command(EngineCommand::RemoveModule { node_id: 1 });
        ui.flush();
        processor.process(&mut output, 2);
        assert_eq!(processor.plan().len(), 1);

        // The output limiter's 1 ms lookahead still holds the last of it, and
        // the DC blocker settles with a slow sub-audio tail. Neither moves
        // like a 261 Hz oscillator (about 0.02 per sample at full scale).
        processor.process(&mut output, 2);
        let left: Vec<f32> = output.iter().step_by(2).copied().collect();
        assert!(left.windows(2).all(|w| (w[1] - w[0]).abs() < 1e-3), "oscillator is gone");
        assert!(ui.flush());
    }
}
