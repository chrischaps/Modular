//! Signal types and buffers for the modular synthesizer.
//!
//! Defines the fundamental data types for audio, control voltage, gate, and MIDI signals.

use egui::Color32;

/// The type of signal flowing through a connection.
///
/// Each signal type has a specific range and purpose:
/// - **Audio**: Sample streams, typically -1.0 to 1.0
/// - **Control**: Modulation CV, 0.0 to 1.0 (unipolar) or -1.0 to 1.0 (bipolar)
/// - **Gate**: On/off triggers, either 0.0 or 1.0
/// - **Midi**: Structured note and control change events
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SignalType {
    Audio,
    Control,
    Gate,
    Midi,
}

impl SignalType {
    /// Returns the color associated with this signal type for UI visualization.
    ///
    /// Colors follow the project specification:
    /// - Audio: Blue
    /// - Control: Orange
    /// - Gate: Green
    /// - Midi: Purple
    pub fn color(&self) -> Color32 {
        match self {
            SignalType::Audio => Color32::from_rgb(66, 135, 245),   // Blue
            SignalType::Control => Color32::from_rgb(245, 158, 66), // Orange
            SignalType::Gate => Color32::from_rgb(66, 245, 135),    // Green
            SignalType::Midi => Color32::from_rgb(178, 102, 255),   // Purple
        }
    }

    /// Checks if a connection from this signal type to another is valid.
    ///
    /// Connection rules:
    /// - Same type to same type: Always allowed
    /// - Audio <-> Control: Allowed (audio-rate modulation)
    /// - Gate -> Control: Allowed (on/off modulation)
    /// - Midi -> Any other type: Not allowed (needs converter module)
    pub fn can_connect_to(&self, target: SignalType) -> bool {
        match (self, target) {
            // Same type always connects
            (a, b) if *a == b => true,

            // Audio and Control are interchangeable
            (SignalType::Audio, SignalType::Control) => true,
            (SignalType::Control, SignalType::Audio) => true,

            // Gate can feed into Control (on/off modulation)
            (SignalType::Gate, SignalType::Control) => true,

            // MIDI requires explicit conversion
            (SignalType::Midi, _) => false,
            (_, SignalType::Midi) => false,

            // All other combinations not allowed
            _ => false,
        }
    }

    /// Returns a human-readable name for the signal type.
    pub fn name(&self) -> &'static str {
        match self {
            SignalType::Audio => "Audio",
            SignalType::Control => "Control",
            SignalType::Gate => "Gate",
            SignalType::Midi => "MIDI",
        }
    }
}

/// A buffer containing signal samples.
///
/// Used to pass data between modules in the audio graph.
/// The buffer is pre-allocated to avoid allocations in the audio thread.
#[derive(Clone, Debug)]
pub struct SignalBuffer {
    /// The sample data. Length matches the audio engine's buffer size.
    pub samples: Vec<f32>,
    /// The type of signal stored in this buffer.
    pub signal_type: SignalType,
}

impl SignalBuffer {
    /// Creates a new signal buffer with the specified size and type.
    ///
    /// The buffer is initialized with zeros.
    pub fn new(size: usize, signal_type: SignalType) -> Self {
        Self {
            samples: vec![0.0; size],
            signal_type,
        }
    }

    /// Creates a new audio signal buffer.
    pub fn audio(size: usize) -> Self {
        Self::new(size, SignalType::Audio)
    }

    /// Creates a new control signal buffer.
    pub fn control(size: usize) -> Self {
        Self::new(size, SignalType::Control)
    }

    /// Creates a new gate signal buffer.
    pub fn gate(size: usize) -> Self {
        Self::new(size, SignalType::Gate)
    }

    /// Clears the buffer, setting all samples to zero.
    pub fn clear(&mut self) {
        self.samples.fill(0.0);
    }

    /// Fills the buffer with a constant value.
    pub fn fill(&mut self, value: f32) {
        self.samples.fill(value);
    }

    /// Returns the number of samples in the buffer.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Resizes the buffer to the specified size.
    ///
    /// New samples are initialized to zero.
    pub fn resize(&mut self, new_size: usize) {
        self.samples.resize(new_size, 0.0);
    }
}

/// A MIDI message type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiMessage {
    /// Note On: (note number 0-127, velocity 0-127)
    NoteOn { note: u8, velocity: u8 },
    /// Note Off: (note number 0-127, velocity 0-127)
    NoteOff { note: u8, velocity: u8 },
    /// Control Change: (controller number 0-127, value 0-127)
    ControlChange { controller: u8, value: u8 },
    /// Pitch Bend: (-8192 to 8191, centered at 0)
    PitchBend { value: i16 },
    /// Channel Aftertouch: (pressure 0-127)
    Aftertouch { pressure: u8 },
    /// Program Change: (program number 0-127)
    ProgramChange { program: u8 },
}

impl MidiMessage {
    /// Converts a Note On message's note number to frequency in Hz.
    ///
    /// Uses standard A4 = 440 Hz tuning.
    pub fn note_to_frequency(note: u8) -> f32 {
        440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
    }

    /// Converts a velocity value (0-127) to a normalized float (0.0-1.0).
    pub fn velocity_to_float(velocity: u8) -> f32 {
        velocity as f32 / 127.0
    }

    /// Converts a control value (0-127) to a normalized float (0.0-1.0).
    pub fn cc_to_float(value: u8) -> f32 {
        value as f32 / 127.0
    }
}

/// A MIDI event with timing information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MidiEvent {
    /// The sample offset within the current buffer where this event occurs.
    pub sample_offset: u32,
    /// The MIDI channel (0-15).
    pub channel: u8,
    /// The MIDI message.
    pub message: MidiMessage,
}

impl MidiEvent {
    /// Creates a new MIDI event.
    pub fn new(sample_offset: u32, channel: u8, message: MidiMessage) -> Self {
        Self {
            sample_offset,
            channel,
            message,
        }
    }

    /// Creates a Note On event.
    pub fn note_on(sample_offset: u32, channel: u8, note: u8, velocity: u8) -> Self {
        Self::new(
            sample_offset,
            channel,
            MidiMessage::NoteOn { note, velocity },
        )
    }

    /// Creates a Note Off event.
    pub fn note_off(sample_offset: u32, channel: u8, note: u8, velocity: u8) -> Self {
        Self::new(
            sample_offset,
            channel,
            MidiMessage::NoteOff { note, velocity },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_type_connections() {
        assert!(SignalType::Audio.can_connect_to(SignalType::Audio));
        assert!(SignalType::Control.can_connect_to(SignalType::Control));
        assert!(SignalType::Gate.can_connect_to(SignalType::Gate));
        assert!(SignalType::Midi.can_connect_to(SignalType::Midi));
    }

    #[test]
    fn test_audio_control_connections() {
        assert!(SignalType::Audio.can_connect_to(SignalType::Control));
        assert!(SignalType::Control.can_connect_to(SignalType::Audio));
    }

    #[test]
    fn test_gate_to_control_connection() {
        assert!(SignalType::Gate.can_connect_to(SignalType::Control));
    }

    #[test]
    fn test_midi_isolation() {
        // MIDI can only connect to MIDI
        assert!(!SignalType::Midi.can_connect_to(SignalType::Audio));
        assert!(!SignalType::Midi.can_connect_to(SignalType::Control));
        assert!(!SignalType::Midi.can_connect_to(SignalType::Gate));

        // Nothing can connect to MIDI except MIDI
        assert!(!SignalType::Audio.can_connect_to(SignalType::Midi));
        assert!(!SignalType::Control.can_connect_to(SignalType::Midi));
        assert!(!SignalType::Gate.can_connect_to(SignalType::Midi));
    }

    #[test]
    fn test_invalid_connections() {
        assert!(!SignalType::Gate.can_connect_to(SignalType::Audio));
        assert!(!SignalType::Audio.can_connect_to(SignalType::Gate));
        assert!(!SignalType::Control.can_connect_to(SignalType::Gate));
    }

    #[test]
    fn test_signal_type_colors() {
        // Just verify colors are distinct
        let colors = [
            SignalType::Audio.color(),
            SignalType::Control.color(),
            SignalType::Gate.color(),
            SignalType::Midi.color(),
        ];
        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Signal type colors should be unique");
            }
        }
    }

    #[test]
    fn test_signal_buffer_creation() {
        let buffer = SignalBuffer::audio(256);
        assert_eq!(buffer.len(), 256);
        assert_eq!(buffer.signal_type, SignalType::Audio);
        assert!(buffer.samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_signal_buffer_fill_and_clear() {
        let mut buffer = SignalBuffer::control(128);

        buffer.fill(0.5);
        assert!(buffer.samples.iter().all(|&s| s == 0.5));

        buffer.clear();
        assert!(buffer.samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_signal_buffer_resize() {
        let mut buffer = SignalBuffer::gate(64);
        assert_eq!(buffer.len(), 64);

        buffer.resize(128);
        assert_eq!(buffer.len(), 128);

        buffer.resize(32);
        assert_eq!(buffer.len(), 32);
    }

    #[test]
    fn test_midi_note_to_frequency() {
        // A4 = 440 Hz
        let a4_freq = MidiMessage::note_to_frequency(69);
        assert!((a4_freq - 440.0).abs() < 0.001);

        // A3 = 220 Hz (one octave below)
        let a3_freq = MidiMessage::note_to_frequency(57);
        assert!((a3_freq - 220.0).abs() < 0.001);

        // A5 = 880 Hz (one octave above)
        let a5_freq = MidiMessage::note_to_frequency(81);
        assert!((a5_freq - 880.0).abs() < 0.001);
    }

    #[test]
    fn test_midi_velocity_conversion() {
        assert!((MidiMessage::velocity_to_float(0) - 0.0).abs() < 0.001);
        assert!((MidiMessage::velocity_to_float(127) - 1.0).abs() < 0.001);
        assert!((MidiMessage::velocity_to_float(64) - 0.504).abs() < 0.01);
    }

    #[test]
    fn test_midi_event_creation() {
        let event = MidiEvent::note_on(100, 0, 60, 100);
        assert_eq!(event.sample_offset, 100);
        assert_eq!(event.channel, 0);
        assert_eq!(
            event.message,
            MidiMessage::NoteOn {
                note: 60,
                velocity: 100
            }
        );
    }
}
