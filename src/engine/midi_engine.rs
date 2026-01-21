//! MIDI Engine
//!
//! Handles MIDI input from hardware controllers and virtual MIDI ports.
//! Uses midir for cross-platform MIDI access and rtrb for lock-free
//! communication with the audio thread.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use midir::{MidiInput, MidiInputConnection, MidiInputPort};
use rtrb::{Consumer, Producer, RingBuffer};

/// Default buffer size for MIDI events.
pub const DEFAULT_MIDI_BUFFER_SIZE: usize = 512;

/// Information about a MIDI input device.
#[derive(Debug, Clone)]
pub struct MidiDeviceInfo {
    /// Human-readable device name.
    pub name: String,
    /// Internal port index.
    pub index: usize,
}

/// MIDI event types received from hardware.
#[derive(Debug, Clone, Copy)]
pub enum MidiEvent {
    /// Note On event.
    NoteOn {
        /// MIDI channel (0-15).
        channel: u8,
        /// Note number (0-127).
        note: u8,
        /// Velocity (0-127).
        velocity: u8,
    },
    /// Note Off event.
    NoteOff {
        /// MIDI channel (0-15).
        channel: u8,
        /// Note number (0-127).
        note: u8,
        /// Velocity (0-127, often ignored).
        velocity: u8,
    },
    /// Control Change (CC) event.
    ControlChange {
        /// MIDI channel (0-15).
        channel: u8,
        /// Controller number (0-127).
        controller: u8,
        /// Controller value (0-127).
        value: u8,
    },
    /// Pitch Bend event.
    PitchBend {
        /// MIDI channel (0-15).
        channel: u8,
        /// Pitch bend value (-8192 to 8191, center = 0).
        value: i16,
    },
    /// Channel Aftertouch (pressure).
    ChannelPressure {
        /// MIDI channel (0-15).
        channel: u8,
        /// Pressure value (0-127).
        pressure: u8,
    },
    /// Polyphonic Aftertouch (per-note pressure).
    PolyPressure {
        /// MIDI channel (0-15).
        channel: u8,
        /// Note number (0-127).
        note: u8,
        /// Pressure value (0-127).
        pressure: u8,
    },
    /// Program Change.
    ProgramChange {
        /// MIDI channel (0-15).
        channel: u8,
        /// Program number (0-127).
        program: u8,
    },
}

impl MidiEvent {
    /// Parse a MIDI event from raw bytes.
    /// Returns None for unsupported or malformed messages.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let status = data[0];
        let channel = status & 0x0F;
        let msg_type = status & 0xF0;

        match msg_type {
            0x90 => {
                // Note On (velocity 0 = Note Off)
                if data.len() >= 3 {
                    let note = data[1] & 0x7F;
                    let velocity = data[2] & 0x7F;
                    if velocity == 0 {
                        Some(MidiEvent::NoteOff {
                            channel,
                            note,
                            velocity: 0,
                        })
                    } else {
                        Some(MidiEvent::NoteOn {
                            channel,
                            note,
                            velocity,
                        })
                    }
                } else {
                    None
                }
            }
            0x80 => {
                // Note Off
                if data.len() >= 3 {
                    Some(MidiEvent::NoteOff {
                        channel,
                        note: data[1] & 0x7F,
                        velocity: data[2] & 0x7F,
                    })
                } else {
                    None
                }
            }
            0xB0 => {
                // Control Change
                if data.len() >= 3 {
                    Some(MidiEvent::ControlChange {
                        channel,
                        controller: data[1] & 0x7F,
                        value: data[2] & 0x7F,
                    })
                } else {
                    None
                }
            }
            0xE0 => {
                // Pitch Bend
                if data.len() >= 3 {
                    let lsb = data[1] as i16;
                    let msb = data[2] as i16;
                    // Pitch bend is 14-bit, centered at 8192
                    let value = ((msb << 7) | lsb) - 8192;
                    Some(MidiEvent::PitchBend { channel, value })
                } else {
                    None
                }
            }
            0xD0 => {
                // Channel Aftertouch
                if data.len() >= 2 {
                    Some(MidiEvent::ChannelPressure {
                        channel,
                        pressure: data[1] & 0x7F,
                    })
                } else {
                    None
                }
            }
            0xA0 => {
                // Poly Aftertouch
                if data.len() >= 3 {
                    Some(MidiEvent::PolyPressure {
                        channel,
                        note: data[1] & 0x7F,
                        pressure: data[2] & 0x7F,
                    })
                } else {
                    None
                }
            }
            0xC0 => {
                // Program Change
                if data.len() >= 2 {
                    Some(MidiEvent::ProgramChange {
                        channel,
                        program: data[1] & 0x7F,
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get the MIDI channel for this event.
    pub fn channel(&self) -> u8 {
        match self {
            MidiEvent::NoteOn { channel, .. } => *channel,
            MidiEvent::NoteOff { channel, .. } => *channel,
            MidiEvent::ControlChange { channel, .. } => *channel,
            MidiEvent::PitchBend { channel, .. } => *channel,
            MidiEvent::ChannelPressure { channel, .. } => *channel,
            MidiEvent::PolyPressure { channel, .. } => *channel,
            MidiEvent::ProgramChange { channel, .. } => *channel,
        }
    }
}

/// MIDI event with timestamp for sample-accurate playback.
#[derive(Debug, Clone, Copy)]
pub struct TimestampedMidiEvent {
    /// The MIDI event.
    pub event: MidiEvent,
    /// Timestamp in microseconds since connection started.
    pub timestamp_us: u64,
}

/// Error type for MIDI operations.
#[derive(Debug)]
pub enum MidiError {
    /// Failed to initialize MIDI subsystem.
    InitError(String),
    /// Failed to connect to device.
    ConnectionError(String),
    /// Device not found.
    DeviceNotFound,
    /// No MIDI devices available.
    NoDevices,
}

impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::InitError(s) => write!(f, "MIDI init error: {}", s),
            MidiError::ConnectionError(s) => write!(f, "MIDI connection error: {}", s),
            MidiError::DeviceNotFound => write!(f, "MIDI device not found"),
            MidiError::NoDevices => write!(f, "No MIDI devices available"),
        }
    }
}

impl std::error::Error for MidiError {}

/// MIDI engine state shared between threads.
struct MidiState {
    /// Currently available ports (refreshed periodically).
    ports: Vec<MidiInputPort>,
    /// Port names for UI display.
    port_names: Vec<String>,
}

/// MIDI engine for receiving MIDI input.
pub struct MidiEngine {
    /// Cached device list.
    devices: Vec<MidiDeviceInfo>,
    /// Currently selected device index (None = no device).
    selected_device: Option<usize>,
    /// Active MIDI connection.
    connection: Option<MidiInputConnection<()>>,
    /// Producer for sending events to consumers.
    event_producer: Option<Producer<TimestampedMidiEvent>>,
    /// Shared state for device enumeration.
    state: Arc<Mutex<MidiState>>,
    /// Flag to signal device scan thread to stop.
    scan_running: Arc<AtomicBool>,
    /// Handle for the device scan thread.
    scan_thread: Option<thread::JoinHandle<()>>,
}

impl MidiEngine {
    /// Create a new MIDI engine.
    ///
    /// Returns the engine and a consumer for receiving MIDI events.
    pub fn new() -> Result<(Self, Consumer<TimestampedMidiEvent>), MidiError> {
        // Create the event ring buffer
        let (producer, consumer) = RingBuffer::new(DEFAULT_MIDI_BUFFER_SIZE);

        // Initialize MIDI input for port enumeration
        let midi_in = MidiInput::new("Modular Synth")
            .map_err(|e| MidiError::InitError(e.to_string()))?;

        // Get initial port list
        let ports: Vec<MidiInputPort> = midi_in.ports().into_iter().collect();
        let port_names: Vec<String> = ports
            .iter()
            .map(|p| midi_in.port_name(p).unwrap_or_else(|_| "Unknown".to_string()))
            .collect();

        let devices: Vec<MidiDeviceInfo> = port_names
            .iter()
            .enumerate()
            .map(|(i, name)| MidiDeviceInfo {
                name: name.clone(),
                index: i,
            })
            .collect();

        let state = Arc::new(Mutex::new(MidiState { ports, port_names }));

        // Start background thread for device scanning (hot-plug detection)
        let scan_running = Arc::new(AtomicBool::new(true));
        let state_clone = Arc::clone(&state);
        let running_clone = Arc::clone(&scan_running);

        let scan_thread = thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                // Sleep between scans
                thread::sleep(Duration::from_secs(2));

                if !running_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Rescan MIDI ports
                if let Ok(midi_in) = MidiInput::new("Modular Synth Scanner") {
                    let new_ports: Vec<MidiInputPort> = midi_in.ports().into_iter().collect();
                    let new_names: Vec<String> = new_ports
                        .iter()
                        .map(|p| midi_in.port_name(p).unwrap_or_else(|_| "Unknown".to_string()))
                        .collect();

                    if let Ok(mut state) = state_clone.lock() {
                        state.ports = new_ports;
                        state.port_names = new_names;
                    }
                }
            }
        });

        let engine = Self {
            devices,
            selected_device: None,
            connection: None,
            event_producer: Some(producer),
            state,
            scan_running,
            scan_thread: Some(scan_thread),
        };

        Ok((engine, consumer))
    }

    /// Enumerate available MIDI input devices.
    /// This returns a fresh list reflecting any hot-plugged devices.
    pub fn enumerate_devices(&mut self) -> Vec<MidiDeviceInfo> {
        if let Ok(state) = self.state.lock() {
            self.devices = state
                .port_names
                .iter()
                .enumerate()
                .map(|(i, name)| MidiDeviceInfo {
                    name: name.clone(),
                    index: i,
                })
                .collect();
        }
        self.devices.clone()
    }

    /// Get the currently cached device list without rescanning.
    pub fn devices(&self) -> &[MidiDeviceInfo] {
        &self.devices
    }

    /// Get the currently selected device index.
    pub fn selected_device(&self) -> Option<usize> {
        self.selected_device
    }

    /// Connect to a MIDI device by index.
    pub fn connect(&mut self, device_index: usize) -> Result<(), MidiError> {
        // Disconnect existing connection
        self.disconnect();

        // Get the port from our state
        let port = {
            let state = self.state.lock().map_err(|_| {
                MidiError::ConnectionError("Failed to lock state".to_string())
            })?;

            if device_index >= state.ports.len() {
                return Err(MidiError::DeviceNotFound);
            }

            state.ports[device_index].clone()
        };

        // Create a new MIDI input for this connection
        let midi_in = MidiInput::new("Modular Synth Input")
            .map_err(|e| MidiError::InitError(e.to_string()))?;

        // Take the producer for use in the callback
        let producer = self.event_producer.take().ok_or_else(|| {
            MidiError::ConnectionError("Event producer already in use".to_string())
        })?;

        // Wrap producer in Arc<Mutex> for the callback
        let producer = Arc::new(Mutex::new(producer));

        // Connect with callback
        let connection = midi_in
            .connect(
                &port,
                "Modular Synth Input",
                {
                    let producer = Arc::clone(&producer);
                    move |timestamp_us, data, _| {
                        if let Some(event) = MidiEvent::from_bytes(data) {
                            let timestamped = TimestampedMidiEvent {
                                event,
                                timestamp_us,
                            };
                            if let Ok(mut prod) = producer.lock() {
                                // Use lossy push - drop events if buffer is full
                                let _ = prod.push(timestamped);
                            }
                            // Log MIDI events to console for debugging
                            eprintln!("MIDI: {:?}", event);
                        }
                    }
                },
                (),
            )
            .map_err(|e| MidiError::ConnectionError(e.to_string()))?;

        // Store connection and put producer back (wrapped in Arc)
        self.connection = Some(connection);
        // We can't get the producer back out of the callback, so we leave it as None
        // This is fine because we only support one connection at a time
        self.selected_device = Some(device_index);

        eprintln!(
            "MIDI connected to device {}: {}",
            device_index,
            self.devices.get(device_index).map(|d| d.name.as_str()).unwrap_or("Unknown")
        );

        Ok(())
    }

    /// Disconnect from the current MIDI device.
    pub fn disconnect(&mut self) {
        if let Some(connection) = self.connection.take() {
            // Close the connection - this drops it
            connection.close();
            self.selected_device = None;
            eprintln!("MIDI disconnected");
        }
    }

    /// Check if currently connected to a device.
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }
}

impl Drop for MidiEngine {
    fn drop(&mut self) {
        // Stop the scan thread
        self.scan_running.store(false, Ordering::Relaxed);

        // Disconnect if connected
        self.disconnect();

        // Wait for scan thread to finish
        if let Some(thread) = self.scan_thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_event_from_bytes_note_on() {
        let data = [0x90, 60, 100]; // Note On, channel 0, middle C, velocity 100
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::NoteOn {
            channel,
            note,
            velocity,
        }) = event
        {
            assert_eq!(channel, 0);
            assert_eq!(note, 60);
            assert_eq!(velocity, 100);
        } else {
            panic!("Expected NoteOn event");
        }
    }

    #[test]
    fn test_midi_event_from_bytes_note_off() {
        let data = [0x80, 60, 64]; // Note Off, channel 0, middle C
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::NoteOff { channel, note, .. }) = event {
            assert_eq!(channel, 0);
            assert_eq!(note, 60);
        } else {
            panic!("Expected NoteOff event");
        }
    }

    #[test]
    fn test_midi_event_from_bytes_note_on_zero_velocity() {
        // Note On with velocity 0 should be treated as Note Off
        let data = [0x90, 60, 0];
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        assert!(matches!(event, Some(MidiEvent::NoteOff { .. })));
    }

    #[test]
    fn test_midi_event_from_bytes_control_change() {
        let data = [0xB0, 1, 64]; // CC, channel 0, mod wheel, value 64
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::ControlChange {
            channel,
            controller,
            value,
        }) = event
        {
            assert_eq!(channel, 0);
            assert_eq!(controller, 1);
            assert_eq!(value, 64);
        } else {
            panic!("Expected ControlChange event");
        }
    }

    #[test]
    fn test_midi_event_from_bytes_pitch_bend() {
        // Pitch bend centered (8192 = 0x2000)
        let data = [0xE0, 0x00, 0x40]; // LSB=0, MSB=64 -> 64*128 = 8192 -> value = 0
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::PitchBend { channel, value }) = event {
            assert_eq!(channel, 0);
            assert_eq!(value, 0);
        } else {
            panic!("Expected PitchBend event");
        }
    }

    #[test]
    fn test_midi_event_from_bytes_channel() {
        // Test channel extraction
        let data = [0x95, 60, 100]; // Note On, channel 5
        let event = MidiEvent::from_bytes(&data).unwrap();
        assert_eq!(event.channel(), 5);
    }

    #[test]
    fn test_midi_event_from_bytes_empty() {
        let data: [u8; 0] = [];
        assert!(MidiEvent::from_bytes(&data).is_none());
    }

    #[test]
    fn test_midi_event_from_bytes_incomplete() {
        let data = [0x90, 60]; // Missing velocity byte
        assert!(MidiEvent::from_bytes(&data).is_none());
    }

    #[test]
    fn test_midi_event_from_bytes_program_change() {
        let data = [0xC0, 42]; // Program change, channel 0, program 42
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::ProgramChange { channel, program }) = event {
            assert_eq!(channel, 0);
            assert_eq!(program, 42);
        } else {
            panic!("Expected ProgramChange event");
        }
    }

    #[test]
    fn test_midi_event_from_bytes_channel_pressure() {
        let data = [0xD0, 100]; // Channel pressure, channel 0, pressure 100
        let event = MidiEvent::from_bytes(&data);
        assert!(event.is_some());
        if let Some(MidiEvent::ChannelPressure { channel, pressure }) = event {
            assert_eq!(channel, 0);
            assert_eq!(pressure, 100);
        } else {
            panic!("Expected ChannelPressure event");
        }
    }

    #[test]
    fn test_midi_event_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MidiEvent>();
        assert_send::<TimestampedMidiEvent>();
    }

    #[test]
    fn test_midi_event_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<MidiEvent>();
        assert_copy::<TimestampedMidiEvent>();
    }
}
