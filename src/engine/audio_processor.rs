//! Audio Processor
//!
//! Handles audio processing in the audio callback, integrating the AudioGraph
//! with command handling from the UI thread.

use std::time::Instant;

use crate::dsp::{ModuleRegistry, ProcessContext};
use crate::modules::{AdsrEnvelope, Attenuverter, AudioOutput, Chorus, Clock, Compressor, Distortion, KeyboardInput, Lfo, MidiMonitor, MidiNote, MoogLadder, Mixer, Oscilloscope, ParametricEq, Reverb, SampleHold, SineOscillator, StepSequencer, StereoDelay, SvfFilter, Vca};

use super::audio_graph::AudioGraph;
use super::channels::EngineHandle;
use super::commands::{EngineCommand, EngineEvent};

/// Creates a module registry with all built-in modules.
pub fn create_module_registry() -> ModuleRegistry {
    let mut registry = ModuleRegistry::new();
    registry.register::<SineOscillator>();
    registry.register::<SvfFilter>();
    registry.register::<MoogLadder>();
    registry.register::<AdsrEnvelope>();
    registry.register::<Clock>();
    registry.register::<Vca>();
    registry.register::<Attenuverter>();
    registry.register::<AudioOutput>();
    registry.register::<Lfo>();
    registry.register::<KeyboardInput>();
    registry.register::<MidiMonitor>();
    registry.register::<MidiNote>();
    registry.register::<SampleHold>();
    registry.register::<Oscilloscope>();
    registry.register::<StepSequencer>();
    registry.register::<StereoDelay>();
    registry.register::<Reverb>();
    registry.register::<ParametricEq>();
    registry.register::<Distortion>();
    registry.register::<Chorus>();
    registry.register::<Compressor>();
    registry.register::<Mixer>();
    registry
}

/// Audio processor that runs in the audio callback.
///
/// This struct is moved into the audio callback closure and handles
/// all audio processing, including:
/// - Receiving and processing commands from the UI thread
/// - Running the audio graph to generate samples
/// - Extracting output from the AudioOutput module
pub struct AudioProcessor {
    /// The audio processing graph.
    graph: AudioGraph,
    /// Handle for receiving commands from the UI thread.
    engine_handle: EngineHandle,
    /// Processing context (sample rate, block size).
    context: ProcessContext,
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
    /// * `block_size` - The maximum number of samples per processing block
    /// * `engine_handle` - Handle for receiving commands from the UI
    pub fn new(sample_rate: f32, block_size: usize, engine_handle: EngineHandle) -> Self {
        let registry = create_module_registry();
        let graph = AudioGraph::with_registry(sample_rate, block_size, registry);
        let context = ProcessContext::new(sample_rate, block_size);

        Self {
            graph,
            engine_handle,
            context,
            is_playing: false,
            frame_counter: 0,
            cpu_load_avg: 0.0,
        }
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
    /// 1. Processes any pending commands from the UI
    /// 2. If playing, processes the audio graph
    /// 3. Extracts audio from the output module and writes to the output buffer
    ///
    /// # Arguments
    /// * `output` - The output buffer to fill with audio samples
    /// * `channels` - Number of output channels (typically 2 for stereo)
    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        // Process pending commands from UI
        self.process_commands();

        // Clear output buffer
        for sample in output.iter_mut() {
            *sample = 0.0;
        }

        if !self.is_playing {
            // Reset CPU load when not playing
            self.cpu_load_avg = 0.0;
            return;
        }

        // Start timing for CPU measurement
        let start_time = Instant::now();

        // Calculate number of frames in this callback
        let num_frames = output.len() / channels;

        // Update context and graph block size if different
        if num_frames != self.context.block_size {
            self.context = ProcessContext::new(self.context.sample_rate, num_frames);
            // Resize audio graph buffers to match new block size
            self.graph.set_block_size(num_frames);
        }

        // Process the audio graph
        self.graph.process(&self.context);

        // Send monitored input values to UI for knob animation
        self.send_input_values();

        // Send monitored output values to UI for LED indicators
        self.send_output_values();

        // Send oscilloscope buffer data to UI for waveform display
        self.send_scope_buffers();

        // Extract output from AudioOutput modules and write to output buffer
        self.extract_output(output, channels, num_frames);

        // Calculate CPU load
        let elapsed = start_time.elapsed();
        let available_time = num_frames as f64 / self.context.sample_rate as f64;
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

    /// Sends monitored input values to the UI thread.
    fn send_input_values(&mut self) {
        for (node_id, input_index, value) in self.graph.drain_sampled_input_values() {
            self.engine_handle.send_event_lossy(EngineEvent::InputValue {
                node_id,
                input_index,
                value,
            });
        }
    }

    /// Sends monitored output values to the UI thread.
    fn send_output_values(&mut self) {
        for (node_id, output_index, value) in self.graph.drain_sampled_output_values() {
            self.engine_handle.send_event_lossy(EngineEvent::OutputValue {
                node_id,
                output_index,
                value,
            });
        }
    }

    /// Sends oscilloscope buffer data to the UI thread.
    fn send_scope_buffers(&mut self) {
        for (node_id, channel1, channel2, triggered) in self.graph.drain_scope_buffers() {
            self.engine_handle.send_event_lossy(EngineEvent::ScopeBuffer {
                node_id,
                channel1: channel1.into_boxed_slice(),
                channel2: channel2.into_boxed_slice(),
                triggered,
            });
        }
    }

    /// Processes all pending commands from the UI thread.
    fn process_commands(&mut self) {
        // Collect commands first to avoid borrow issues
        let mut commands = Vec::new();
        while let Some(cmd) = self.engine_handle.recv_command() {
            commands.push(cmd);
        }

        // Process collected commands
        for cmd in commands {
            match cmd {
                EngineCommand::SetPlaying(playing) => {
                    self.is_playing = playing;
                    let event = if playing {
                        EngineEvent::Started
                    } else {
                        EngineEvent::Stopped
                    };
                    self.engine_handle.send_event_lossy(event);
                }
                other => {
                    // Delegate graph-related commands to the audio graph
                    self.graph.handle_command(other);
                }
            }
        }
    }

    /// Extracts audio from AudioOutput modules and writes to the output buffer.
    fn extract_output(&mut self, output: &mut [f32], channels: usize, num_frames: usize) {
        // Find the AudioOutput module(s) and extract their output
        // For now, we support a single output module

        for node_id in self.graph.processing_order().to_vec() {
            if let Some(module) = self.graph.get_module(node_id) {
                // Check if this module provides audio output
                if let Some((left, right)) = module.get_audio_output() {
                    // Write to output (interleaved stereo)
                    for (i, frame) in output.chunks_mut(channels).enumerate() {
                        if i < num_frames {
                            let l = left.get(i).copied().unwrap_or(0.0);
                            let r = right.get(i).copied().unwrap_or(0.0);

                            if channels >= 1 {
                                frame[0] = l;
                            }
                            if channels >= 2 {
                                frame[1] = r;
                            }
                            // For more than 2 channels, duplicate to additional channels
                            for ch in frame.iter_mut().skip(2) {
                                *ch = (l + r) * 0.5;
                            }
                        }
                    }

                    // Send output levels to UI for metering
                    if let Some((peak_l, peak_r)) = module.get_peak_levels() {
                        self.engine_handle.send_event_lossy(EngineEvent::OutputLevel {
                            left: peak_l,
                            right: peak_r,
                        });
                    }

                    break; // Only process first output module
                }
            }
        }
    }

    /// Returns whether audio processing is currently active.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::EngineChannels;

    #[test]
    fn test_create_module_registry() {
        let registry = create_module_registry();
        assert!(registry.contains("osc.sine"));
        assert!(registry.contains("filter.svf"));
        assert!(registry.contains("mod.adsr"));
        assert!(registry.contains("util.clock"));
        assert!(registry.contains("util.vca"));
        assert!(registry.contains("util.attenuverter"));
        assert!(registry.contains("output.audio"));
        assert!(registry.contains("mod.lfo"));
        assert!(registry.contains("input.keyboard"));
        assert!(registry.contains("util.midi_monitor"));
        assert!(registry.contains("input.midi_note"));
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
        assert_eq!(registry.len(), 21);
    }

    #[test]
    fn test_audio_processor_creation() {
        let channels = EngineChannels::with_defaults();
        let (_ui, engine) = channels.split();

        let processor = AudioProcessor::new(44100.0, 256, engine);
        assert!(!processor.is_playing());
    }

    #[test]
    fn test_audio_processor_silence_when_stopped() {
        let channels = EngineChannels::with_defaults();
        let (_ui, engine) = channels.split();

        let mut processor = AudioProcessor::new(44100.0, 256, engine);

        let mut output = vec![1.0; 512]; // Fill with non-zero
        processor.process(&mut output, 2);

        // Should be silence when not playing
        assert!(output.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_audio_processor_responds_to_play_command() {
        let channels = EngineChannels::with_defaults();
        let (mut ui, engine) = channels.split();

        let mut processor = AudioProcessor::new(44100.0, 256, engine);

        // Send play command
        ui.send_command(EngineCommand::SetPlaying(true)).unwrap();

        // Process to handle the command
        let mut output = vec![0.0; 512];
        processor.process(&mut output, 2);

        assert!(processor.is_playing());

        // Check for Started event
        let event = ui.recv_event();
        assert!(matches!(event, Some(EngineEvent::Started)));
    }
}
