//! Audio Engine
//!
//! Manages the cpal audio stream and interfaces with system audio hardware.
//! The audio callback runs in a separate thread and must be real-time safe.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleRate, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use super::audio_processor::AudioProcessor;

/// Errors that can occur during audio engine operation.
#[derive(Debug, Clone)]
pub enum AudioError {
    /// No audio output device was found.
    NoOutputDevice,
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
}

impl AudioState {
    fn new() -> Self {
        Self {
            test_tone_enabled: AtomicBool::new(false),
            phase_fixed: AtomicU32::new(0),
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

        let sample_rate = supported_config.sample_rate().0;
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let state = Arc::new(AudioState::new());

        Ok(Self {
            host,
            device,
            config,
            stream: None,
            state,
        })
    }

    /// Get information about all available output devices.
    pub fn enumerate_devices(&self) -> Vec<DeviceInfo> {
        let default_name = self
            .host
            .default_output_device()
            .and_then(|d| d.name().ok());

        self.host
            .output_devices()
            .map(|devices| {
                devices
                    .enumerate()
                    .filter_map(|(index, device)| {
                        device.name().ok().map(|name| DeviceInfo {
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
        self.device.name().unwrap_or_else(|_| "Unknown".to_string())
    }

    /// Select a different output device by index.
    ///
    /// This will stop the current stream if running. Call `start()` to begin
    /// playback on the new device.
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

        let sample_rate = supported_config.sample_rate().0;
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        self.device = device;
        self.config = config;

        // Restart if it was running before
        if was_running {
            self.start()?;
        }

        Ok(())
    }

    /// Get the current stream configuration.
    pub fn config(&self) -> &StreamConfig {
        &self.config
    }

    /// Get the sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
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
        let sample_rate = self.config.sample_rate.0 as f32;
        let channels = self.config.channels as usize;

        // Phase increment per sample for 440Hz
        // phase goes from 0.0 to 1.0
        let phase_increment = 440.0 / sample_rate;

        // Fixed-point scaling factor
        const FIXED_SCALE: f32 = 1_000_000.0;

        let stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // REAL-TIME SAFE: No allocations, no locks, no blocking

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
                    eprintln!("Audio stream error: {}", err);
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

    /// Check if the audio stream is currently running.
    pub fn is_running(&self) -> bool {
        self.stream.is_some()
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

        let channels = self.config.channels as usize;

        // Wrap processor in Mutex for the callback
        // Note: In practice, the Mutex is uncontested since only the audio
        // callback accesses it, so there's no actual blocking.
        let processor = Arc::new(Mutex::new(processor));
        let processor_clone = Arc::clone(&processor);

        let stream = self
            .device
            .build_output_stream(
                &self.config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Lock the processor - this should never block since we're the only user
                    if let Ok(mut proc) = processor_clone.try_lock() {
                        proc.process(data, channels);
                    } else {
                        // Fallback to silence if lock fails (should never happen)
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                move |err| {
                    eprintln!("Audio stream error: {}", err);
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
