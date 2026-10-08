//! Audio Engine
//!
//! Manages the cpal audio stream and interfaces with system audio hardware.
//! The audio callback runs in a separate thread and must be real-time safe.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Host, SampleFormat, SizedSample, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::audio_input::{input_channel_converting, InputFeed, InputMonitor, InputSender};
use super::audio_processor::AudioProcessor;
use super::latency::LatencyGauge;
use crate::dsp::{InputAudio, MidiEvent};

/// Errors that can occur during audio engine operation.
#[derive(Debug, Clone)]
pub enum AudioError {
    /// No audio output device was found.
    NoOutputDevice,
    /// The chosen input device is gone.
    NoInputDevice,
    /// Failed to get device configuration.
    ConfigurationFailed(String),
    /// Failed to create the audio stream.
    StreamCreationFailed(String),
    /// Failed to start/stop playback.
    StreamPlaybackFailed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::NoOutputDevice => write!(f, "No audio output device found"),
            AudioError::NoInputDevice => write!(f, "That input device is gone: choose it again under Input"),
            AudioError::ConfigurationFailed(msg) => {
                write!(f, "Failed to get device configuration: {}", msg)
            }
            AudioError::StreamCreationFailed(msg) => {
                write!(f, "Failed to create audio stream: {}", msg)
            }
            AudioError::StreamPlaybackFailed(msg) => {
                write!(f, "Failed to control audio playback: {}", msg)
            }

        }
    }
}

impl std::error::Error for AudioError {}

/// An input device's stream, while it's open.
struct OpenInput {
    /// Kept alive to keep recording; dropping it closes the device.
    _stream: Stream,
    /// The device's index in [`AudioEngine::enumerate_input_devices`].
    index: usize,
    name: String,
    /// The device's own channel count (the patch hears its first two).
    channels: u16,
    /// The device's own sample rate, converted to the output's if it differs.
    sample_rate: u32,
}

/// Information about an audio output device.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Human-readable device name.
    pub name: String,
    /// Whether this is the default output device.
    pub is_default: bool,
    /// Index in the device list (for selection).
    pub index: usize,
}

/// Shared state between audio callback and main thread.
/// All fields use atomics for lock-free access.
struct AudioState {
    /// Whether the test tone is enabled.
    test_tone_enabled: AtomicBool,
    /// Current phase of the sine wave oscillator (stored as fixed-point).
    /// We store phase * 1_000_000 as u32 to avoid floating-point atomics.
    phase_fixed: AtomicU32,
    /// Counts audio callbacks: while the device is taking audio this keeps
    /// climbing, so the UI can tell a live stream from a stalled one.
    callbacks: AtomicU64,
    /// Set by the stream's error callback (device unplugged, driver reset).
    stream_failed: AtomicBool,
    /// Glitches the output device reported, which the stream recovers from.
    xruns: AtomicU64,
    /// The output device's own delay, from callback to playback.
    output_latency: LatencyGauge,
}

impl AudioState {
    fn new() -> Self {
        Self {
            test_tone_enabled: AtomicBool::new(false),
            phase_fixed: AtomicU32::new(0),
            callbacks: AtomicU64::new(0),
            stream_failed: AtomicBool::new(false),
            xruns: AtomicU64::new(0),
            output_latency: LatencyGauge::default(),
        }
    }
}

/// The main audio engine that manages cpal streams.
pub struct AudioEngine {
    host: Host,
    device: Device,
    config: StreamConfig,
    stream: Option<Stream>,
    state: Arc<AudioState>,
    /// The graph processor driven by the stream, kept here so it survives a
    /// device change (the stream, and its callback's handle, are rebuilt).
    processor: Option<Arc<Mutex<AudioProcessor>>>,
    /// The input device's stream, while one is open.
    input: Option<OpenInput>,
}

impl AudioEngine {
    /// Create a new AudioEngine using the default output device.
    pub fn new() -> Result<Self, AudioError> {
        let host = cpal::default_host();

        let device = host
            .default_output_device()
            .ok_or(AudioError::NoOutputDevice)?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?;

        let sample_rate = supported_config.sample_rate();
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        let state = Arc::new(AudioState::new());

        Ok(Self {
            host,
            device,
            config,
            stream: None,
            state,
            processor: None,
            input: None,
        })
    }

    /// Get information about all available output devices.
    pub fn enumerate_devices(&self) -> Vec<DeviceInfo> {
        let default_name = self
            .host
            .default_output_device()
            .and_then(|d| device_name(&d));

        self.host
            .output_devices()
            .map(|devices| {
                devices
                    .enumerate()
                    .filter_map(|(index, device)| {
                        device_name(&device).map(|name| DeviceInfo {
                            is_default: Some(&name) == default_name.as_ref(),
                            name,
                            index,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the name of the currently selected device.
    pub fn current_device_name(&self) -> String {
        device_name(&self.device).unwrap_or_else(|| "Unknown".to_string())
    }

    /// Select a different output device by index.
    ///
    /// If a stream was running it is rebuilt on the new device. When the
    /// engine is driving an `AudioProcessor`, the same processor (and so the
    /// whole patch) moves to the new device, re-prepared at its sample rate.
    pub fn select_device(&mut self, index: usize) -> Result<(), AudioError> {
        // Stop current stream if running
        let was_running = self.is_running();
        if was_running {
            self.stop()?;
        }

        // Find the device by index
        let device = self
            .host
            .output_devices()
            .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?
            .nth(index)
            .ok_or(AudioError::NoOutputDevice)?;

        // Get configuration for new device
        let supported_config = device
            .default_output_config()
            .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?;

        let sample_rate = supported_config.sample_rate();
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate,
            buffer_size: cpal::BufferSize::Default,
        };

        self.device = device;
        self.config = config;

        // Restart if it was running before
        if was_running {
            match self.processor.clone() {
                Some(processor) => {
                    // The old stream is gone, so this lock is uncontended
                    if let Ok(mut proc) = processor.lock() {
                        proc.set_sample_rate(sample_rate as f32);
                    }
                    self.build_processor_stream(processor)?;
                }
                None => self.start()?,
            }
        }

        Ok(())
    }

    /// Get information about all available input devices.
    pub fn enumerate_input_devices(&self) -> Vec<DeviceInfo> {
        let default_name = self.host.default_input_device().and_then(|d| device_name(&d));
        self.host
            .input_devices()
            .map(|devices| {
                devices
                    .enumerate()
                    .filter_map(|(index, device)| {
                        device_name(&device).map(|name| DeviceInfo {
                            is_default: Some(&name) == default_name.as_ref(),
                            name,
                            index,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Opens input device `index`, closing any input already open. Returns
    /// the feed to hand to the audio thread (see
    /// [`UiHandle::connect_input`](super::UiHandle::connect_input)) and a
    /// monitor for the UI.
    ///
    /// The device runs at the output's sample rate if it can. If it can't
    /// (Windows often sets a microphone to 48 kHz and speakers to 44.1 kHz),
    /// it runs at its own and the input is converted on the way in.
    pub fn open_input(&mut self, index: usize) -> Result<(InputFeed, InputMonitor), AudioError> {
        self.close_input();
        let device = self
            .host
            .input_devices()
            .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?
            .nth(index)
            .ok_or(AudioError::NoInputDevice)?;
        let name = device_name(&device).unwrap_or_else(|| "Unknown".to_string());

        let rate = self.config.sample_rate;
        let supported: Vec<_> = device
            .supported_input_configs()
            .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?
            .collect();
        // The richest format the device offers at our rate, in stereo if it can
        let format_rank = |format: SampleFormat| match format {
            SampleFormat::F32 => Some(0),
            SampleFormat::I32 => Some(1),
            SampleFormat::I16 => Some(2),
            SampleFormat::U16 => Some(3),
            _ => None,
        };
        let channel_rank = |channels: u16| match channels {
            2 => 0,
            1 => 1,
            n => n,
        };
        let at_output_rate = supported
            .iter()
            .filter(|range| range.min_sample_rate() <= rate && rate <= range.max_sample_rate())
            .filter_map(|range| format_rank(range.sample_format()).map(|rank| (rank, channel_rank(range.channels()), range)))
            .min_by_key(|&(rank, channels, _)| (rank, channels))
            .map(|(_, _, range)| range.clone().with_sample_rate(rate));
        // Otherwise the device's own settings, converted on the way in
        let supported_config = match at_output_rate {
            Some(config) => config,
            None => device
                .default_input_config()
                .map_err(|e| AudioError::ConfigurationFailed(e.to_string()))?,
        };
        if format_rank(supported_config.sample_format()).is_none() {
            return Err(AudioError::ConfigurationFailed(format!(
                "{} records {:?} samples, which Modular can't read",
                name,
                supported_config.sample_format()
            )));
        }

        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        let (sender, feed, monitor) = input_channel_converting(config.sample_rate, rate);
        let stream = match supported_config.sample_format() {
            SampleFormat::I32 => build_input_stream::<i32>(&device, &config, sender, monitor.clone()),
            SampleFormat::I16 => build_input_stream::<i16>(&device, &config, sender, monitor.clone()),
            SampleFormat::U16 => build_input_stream::<u16>(&device, &config, sender, monitor.clone()),
            _ => build_input_stream::<f32>(&device, &config, sender, monitor.clone()),
        }?;
        stream.play().map_err(|e| AudioError::StreamPlaybackFailed(e.to_string()))?;

        self.input = Some(OpenInput {
            _stream: stream,
            index,
            name,
            channels: config.channels,
            sample_rate: config.sample_rate,
        });
        Ok((feed, monitor))
    }

    /// Closes the input device, if one is open. Its feed stays with the
    /// audio thread, silent, until disconnected.
    pub fn close_input(&mut self) {
        self.input = None;
    }

    /// The open input device's index in
    /// [`enumerate_input_devices`](Self::enumerate_input_devices).
    pub fn input_index(&self) -> Option<usize> {
        self.input.as_ref().map(|input| input.index)
    }

    /// The open input device's name.
    pub fn input_name(&self) -> Option<&str> {
        self.input.as_ref().map(|input| input.name.as_str())
    }

    /// The open input device's channel count.
    pub fn input_channels(&self) -> Option<u16> {
        self.input.as_ref().map(|input| input.channels)
    }

    /// The open input device's sample rate. When it differs from the
    /// output's, the input is converted.
    pub fn input_sample_rate(&self) -> Option<u32> {
        self.input.as_ref().map(|input| input.sample_rate)
    }

    /// Get the current stream configuration.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// Get the sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    /// Get the number of output channels.
    pub fn channels(&self) -> u16 {
        self.config.channels
    }

    /// Enable or disable the test tone (440Hz sine wave).
    pub fn set_test_tone(&self, enabled: bool) {
        self.state.test_tone_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Check if the test tone is enabled.
    pub fn test_tone_enabled(&self) -> bool {
        self.state.test_tone_enabled.load(Ordering::Relaxed)
    }

    /// Start the audio stream.
    pub fn start(&mut self) -> Result<(), AudioError> {
        if self.stream.is_some() {
            return Ok(());
        }

        let state = Arc::clone(&self.state);
        let error_state = Arc::clone(&self.state);
        self.state.stream_failed.store(false, Ordering::Relaxed);
        let sample_rate = self.config.sample_rate as f32;
        let channels = self.config.channels as usize;

        // Phase increment per sample for 440Hz
        // phase goes from 0.0 to 1.0
        let phase_increment = 440.0 / sample_rate;

        // Fixed-point scaling factor
        const FIXED_SCALE: f32 = 1_000_000.0;

        let stream = self
            .device
            .build_output_stream(
                self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // REAL-TIME SAFE: No allocations, no locks, no blocking
                    state.callbacks.fetch_add(1, Ordering::Relaxed);

                    let test_tone = state.test_tone_enabled.load(Ordering::Relaxed);

                    if test_tone {
                        // Get current phase from atomic (convert from fixed-point)
                        let mut phase =
                            state.phase_fixed.load(Ordering::Relaxed) as f32 / FIXED_SCALE;

                        for frame in data.chunks_mut(channels) {
                            // Generate sine wave sample
                            let sample = (phase * 2.0 * std::f32::consts::PI).sin() * 0.3;

                            // Write to all channels
                            for sample_out in frame.iter_mut() {
                                *sample_out = sample;
                            }

                            // Advance phase
                            phase += phase_increment;
                            if phase >= 1.0 {
                                phase -= 1.0;
                            }
                        }

                        // Store phase back (convert to fixed-point)
                        state
                            .phase_fixed
                            .store((phase * FIXED_SCALE) as u32, Ordering::Relaxed);
                    } else {
                        // Output silence
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                move |err| {
                    if is_glitch(&err) {
                        error_state.xruns.fetch_add(1, Ordering::Relaxed);
                    } else {
                        eprintln!("Audio stream error: {}", err);
                        error_state.stream_failed.store(true, Ordering::Relaxed);
                    }
                },
                None,
            )
            .map_err(|e| AudioError::StreamCreationFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamPlaybackFailed(e.to_string()))?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Stop the audio stream.
    pub fn stop(&mut self) -> Result<(), AudioError> {
        if let Some(stream) = self.stream.take() {
            stream
                .pause()
                .map_err(|e| AudioError::StreamPlaybackFailed(e.to_string()))?;
        }
        // Reset phase when stopping
        self.state.phase_fixed.store(0, Ordering::Relaxed);
        Ok(())
    }

    /// Check if an audio stream has been started (it may since have failed:
    /// see [`stream_failed`](Self::stream_failed) and
    /// [`callback_count`](Self::callback_count)).
    pub fn is_running(&self) -> bool {
        self.stream.is_some()
    }

    /// How many times the device has asked for audio. It stops climbing when
    /// the device stops taking audio.
    pub fn callback_count(&self) -> u64 {
        self.state.callbacks.load(Ordering::Relaxed)
    }

    /// The output device's own delay, from callback to playback, or `None`
    /// if it doesn't report timestamps (or the graph stream hasn't run yet).
    pub fn output_latency(&self) -> Option<Duration> {
        self.state.output_latency.get()
    }

    /// Glitches the output device has reported since the engine started.
    pub fn output_xruns(&self) -> u64 {
        self.state.xruns.load(Ordering::Relaxed)
    }

    /// Whether the stream has reported an error since it was started.
    pub fn stream_failed(&self) -> bool {
        self.state.stream_failed.load(Ordering::Relaxed)
    }

    /// Start the audio stream with an AudioProcessor for graph-based synthesis.
    ///
    /// The AudioProcessor is moved into the audio callback where it processes
    /// the audio graph and produces output. The processor is wrapped in a Mutex
    /// to allow safe access from the audio callback.
    ///
    /// Note: This method is preferred over `start()` for actual synthesis.
    /// The test tone (`start()`) is only for basic audio testing.
    pub fn start_with_processor(&mut self, processor: AudioProcessor) -> Result<(), AudioError> {
        if self.stream.is_some() {
            return Ok(());
        }

        // Wrap processor in Mutex for the callback
        // Note: In practice, the Mutex is uncontested since only the audio
        // callback accesses it, so there's no actual blocking.
        let processor = Arc::new(Mutex::new(processor));
        self.processor = Some(Arc::clone(&processor));
        self.build_processor_stream(processor)
    }

    /// Stops the device stream but keeps its processor, re-prepared at
    /// `sample_rate`, for [`render_offline`](Self::render_offline) to drive
    /// by hand.
    pub fn go_offline(&mut self, sample_rate: f32) -> Result<(), AudioError> {
        self.stop()?;
        if let Some(Ok(mut processor)) = self.processor.as_ref().map(|p| p.lock()) {
            processor.set_sample_rate(sample_rate);
        }
        Ok(())
    }

    /// Renders one buffer through the processor with the given MIDI and
    /// audio input, after [`go_offline`](Self::go_offline). Returns false
    /// with no processor.
    pub fn render_offline(&self, output: &mut [f32], channels: usize, midi: &mut [MidiEvent], input: InputAudio<'_>) -> bool {
        match self.processor.as_ref().map(|p| p.lock()) {
            Some(Ok(mut processor)) => {
                processor.process_offline(output, channels, midi, input);
                true
            }
            _ => false,
        }
    }

    /// Builds and starts a stream on the current device that runs `processor`.
    fn build_processor_stream(
        &mut self,
        processor: Arc<Mutex<AudioProcessor>>,
    ) -> Result<(), AudioError> {
        let channels = self.config.channels as usize;
        let state = Arc::clone(&self.state);
        let error_state = Arc::clone(&self.state);
        self.state.stream_failed.store(false, Ordering::Relaxed);
        self.state.output_latency.clear();

        let stream = self
            .device
            .build_output_stream(
                self.config,
                move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    state.callbacks.fetch_add(1, Ordering::Relaxed);
                    let time = info.timestamp();
                    state.output_latency.record(time.playback.checked_duration_since(time.callback));
                    // REAL-TIME SAFE: the lock is only ever taken here, or by
                    // select_device while no stream is running, so try_lock
                    // never fails in practice. If it did, output silence
                    // rather than wait.
                    match processor.try_lock() {
                        Ok(mut proc) => proc.process(data, channels),
                        Err(_) => data.fill(0.0),
                    }
                },
                move |err| {
                    if is_glitch(&err) {
                        error_state.xruns.fetch_add(1, Ordering::Relaxed);
                    } else {
                        eprintln!("Audio stream error: {}", err);
                        error_state.stream_failed.store(true, Ordering::Relaxed);
                    }
                },
                None,
            )
            .map_err(|e| AudioError::StreamCreationFailed(e.to_string()))?;

        stream
            .play()
            .map_err(|e| AudioError::StreamPlaybackFailed(e.to_string()))?;

        self.stream = Some(stream);
        Ok(())
    }
}

/// Whether a stream error is a passing glitch the stream carries on from
/// (it dropped or repeated a buffer, or the OS wouldn't raise the audio
/// thread's priority) rather than the end of the stream.
fn is_glitch(err: &cpal::Error) -> bool {
    matches!(err.kind(), cpal::ErrorKind::Xrun | cpal::ErrorKind::RealtimeDenied)
}

/// A device's human-readable name, or `None` if it can't say.
fn device_name(device: &Device) -> Option<String> {
    device.description().ok().map(|description| description.name().to_string())
}

/// Builds a stream that pushes device `device`'s input, in samples of type
/// `T`, into `sender`.
fn build_input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    mut sender: InputSender,
    monitor: InputMonitor,
) -> Result<Stream, AudioError>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
            *config,
            move |data: &[T], info: &cpal::InputCallbackInfo| {
                // REAL-TIME SAFE: a copy into the ring and an atomic store
                let time = info.timestamp();
                sender.record_latency(time.callback.checked_duration_since(time.capture));
                sender.push(data, channels, |sample| sample.to_sample::<f32>());
            },
            move |err| {
                if is_glitch(&err) {
                    monitor.mark_xrun();
                } else {
                    eprintln!("Audio input error: {}", err);
                    monitor.mark_failed();
                }
            },
            None,
        )
        .map_err(|e| AudioError::StreamCreationFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_error_display() {
        let err = AudioError::NoOutputDevice;
        assert_eq!(err.to_string(), "No audio output device found");

        let err = AudioError::StreamCreationFailed("test error".to_string());
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_device_info() {
        let info = DeviceInfo {
            name: "Test Device".to_string(),
            is_default: true,
            index: 0,
        };
        assert_eq!(info.name, "Test Device");
        assert!(info.is_default);
        assert_eq!(info.index, 0);
    }

    // Note: Hardware-dependent tests are difficult to run in CI
    // The following tests require actual audio hardware:
    //
    // #[test]
    // fn test_engine_creation() {
    //     let engine = AudioEngine::new();
    //     assert!(engine.is_ok());
    // }
    //
    // #[test]
    // fn test_start_stop() {
    //     let mut engine = AudioEngine::new().unwrap();
    //     assert!(engine.start().is_ok());
    //     assert!(engine.is_running());
    //     assert!(engine.stop().is_ok());
    //     assert!(!engine.is_running());
    // }
}
