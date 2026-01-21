//! Keyboard input module.
//!
//! A virtual keyboard that converts computer keyboard input into gate, pitch CV,
//! and velocity signals for playing the synthesizer.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Key priority modes for handling multiple simultaneous keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyPriority {
    /// Most recently pressed key takes priority.
    Last = 0,
    /// Lowest note playing takes priority.
    Lowest = 1,
    /// Highest note playing takes priority.
    Highest = 2,
}

impl KeyPriority {
    /// Convert from parameter value (0-2) to key priority.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => KeyPriority::Last,
            1 => KeyPriority::Lowest,
            2 => KeyPriority::Highest,
            _ => KeyPriority::Last,
        }
    }
}

/// A virtual keyboard for triggering notes from computer keyboard input.
///
/// Converts QWERTY keyboard presses into musical notes, outputting gate,
/// pitch CV, and velocity signals that can drive oscillators and envelopes.
///
/// # Ports
///
/// **Outputs:**
/// - **Gate** (Gate): High (1.0) when a key is pressed, low (0.0) when released.
/// - **Pitch** (Control): V/Oct pitch CV. 0.0 = C4 (middle C), +1.0 = C5, -1.0 = C3.
/// - **Velocity** (Control): Note velocity (0.0-1.0).
///
/// # Parameters
///
/// - **Note** (0-127): Current MIDI note number (set by UI from keyboard events).
/// - **Gate** (0/1): Current gate state (set by UI from keyboard events).
/// - **Octave** (-2 to +2): Octave shift applied to keyboard input.
/// - **Velocity** (0-1): Fixed velocity value for all notes.
/// - **Priority** (0-2): Key priority mode (Last, Lowest, Highest).
pub struct KeyboardInput {
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
    /// Current output pitch (smoothed to avoid clicks).
    current_pitch: f32,
    /// Current output gate state.
    current_gate: f32,
}

impl KeyboardInput {
    /// Creates a new keyboard input module.
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            ports: vec![
                // Output ports
                PortDefinition::output("gate", "Gate", SignalType::Gate),
                PortDefinition::output("pitch", "Pitch", SignalType::Control),
                PortDefinition::output("velocity", "Velocity", SignalType::Control),
            ],
            parameters: vec![
                // Note: MIDI note number (0-127), set by UI
                // Hidden from normal UI - controlled by keyboard events
                ParameterDefinition::new(
                    "note",
                    "Note",
                    0.0,
                    127.0,
                    60.0, // Default to middle C
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Gate: 0 or 1, set by UI when keys pressed/released
                ParameterDefinition::toggle("gate", "Gate", false),
                // Octave: shift the keyboard up/down by octaves
                ParameterDefinition::new(
                    "octave",
                    "Octave",
                    -2.0,
                    2.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Velocity: fixed velocity for all notes
                ParameterDefinition::normalized("velocity", "Velocity", 1.0),
                // Priority: key priority mode
                ParameterDefinition::choice(
                    "priority",
                    "Priority",
                    &["Last", "Lowest", "Highest"],
                    0,
                ),
            ],
            current_pitch: 0.0,
            current_gate: 0.0,
        }
    }

    /// Port index constants.
    const PORT_GATE: usize = 0;
    const PORT_PITCH: usize = 1;
    const PORT_VELOCITY: usize = 2;

    /// Parameter index constants.
    const PARAM_NOTE: usize = 0;
    const PARAM_GATE: usize = 1;
    const PARAM_OCTAVE: usize = 2;
    const PARAM_VELOCITY: usize = 3;
    #[allow(dead_code)]
    const PARAM_PRIORITY: usize = 4;

    /// Convert MIDI note number to V/Oct pitch CV.
    ///
    /// Middle C (MIDI 60) = 0.0
    /// C5 (MIDI 72) = +1.0
    /// C3 (MIDI 48) = -1.0
    #[inline]
    fn midi_to_voct(midi_note: f32) -> f32 {
        (midi_note - 60.0) / 12.0
    }
}

impl Default for KeyboardInput {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for KeyboardInput {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "input.keyboard",
            name: "Keyboard",
            category: ModuleCategory::Source,
            description: "Virtual keyboard for playing notes from computer keyboard",
        };
        &INFO
    }

    fn ports(&self) -> &[PortDefinition] {
        &self.ports
    }

    fn parameters(&self) -> &[ParameterDefinition] {
        &self.parameters
    }

    fn prepare(&mut self, sample_rate: f32, _max_block_size: usize) {
        self.sample_rate = sample_rate;
    }

    fn process(
        &mut self,
        _inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Get parameter values
        let note = params[Self::PARAM_NOTE];
        let gate = if params[Self::PARAM_GATE] > 0.5 { 1.0 } else { 0.0 };
        let octave = params[Self::PARAM_OCTAVE];
        let velocity = params[Self::PARAM_VELOCITY];

        // Calculate pitch with octave shift
        let shifted_note = note + (octave * 12.0);
        let target_pitch = Self::midi_to_voct(shifted_note);

        // Fill output buffers
        for i in 0..context.block_size {
            // Gate output - instant transition
            outputs[Self::PORT_GATE].samples[i] = gate;

            // Pitch output - could add glide/portamento here later
            // For now, instant pitch changes when gate is high
            if gate > 0.5 {
                self.current_pitch = target_pitch;
            }
            outputs[Self::PORT_PITCH].samples[i] = self.current_pitch;

            // Velocity output
            outputs[Self::PORT_VELOCITY].samples[i] = velocity;
        }

        self.current_gate = gate;
    }

    fn reset(&mut self) {
        self.current_pitch = 0.0;
        self.current_gate = 0.0;
    }
}

/// Maps a computer keyboard key to a MIDI note number relative to C4.
///
/// Uses a piano-like layout where:
/// - Bottom row (Z, X, C, V, B, N, M, comma, period, slash) = white keys
/// - Middle row (S, D, G, H, J, L, semicolon) = black keys (sharps/flats)
/// - Upper rows (W, E, T, Y, U, O, P) = black keys for higher octave
///
/// Returns None if the key doesn't map to a note.
pub fn key_to_note(key: egui::Key) -> Option<i32> {
    use egui::Key;

    // Bottom row: white keys starting from C
    // Z=C, X=D, C=E, V=F, B=G, N=A, M=B, ,=C+, .=D+, /=E+
    match key {
        // Lower octave - white keys (Z X C V B N M , . /)
        Key::Z => Some(0),   // C
        Key::X => Some(2),   // D
        Key::C => Some(4),   // E
        Key::V => Some(5),   // F
        Key::B => Some(7),   // G
        Key::N => Some(9),   // A
        Key::M => Some(11),  // B
        Key::Comma => Some(12),  // C (next octave)
        Key::Period => Some(14), // D (next octave)
        Key::Slash => Some(16),  // E (next octave)

        // Lower octave - black keys (S D F G H J K L ; ')
        Key::S => Some(1),   // C#
        Key::D => Some(3),   // D#
        // F is skipped (no black key between E and F)
        Key::G => Some(6),   // F#
        Key::H => Some(8),   // G#
        Key::J => Some(10),  // A#
        // K is skipped (no black key between B and C)
        Key::L => Some(13),  // C# (next octave)
        Key::Semicolon => Some(15), // D# (next octave)

        // Upper row for additional black keys (Q W E R T Y U I O P)
        Key::Q => Some(0),   // C (alternative)
        Key::W => Some(1),   // C# (alternative)
        Key::E => Some(3),   // D# (alternative)
        Key::R => Some(4),   // E (alternative)
        Key::T => Some(6),   // F# (alternative)
        Key::Y => Some(8),   // G# (alternative)
        Key::U => Some(10),  // A# (alternative)
        Key::I => Some(11),  // B (alternative)
        Key::O => Some(13),  // C# upper (alternative)
        Key::P => Some(15),  // D# upper (alternative)

        _ => None,
    }
}

/// Converts a relative note (from key_to_note) to an absolute MIDI note number.
///
/// Base octave 0 means the keyboard starts at C4 (MIDI 60).
pub fn relative_to_midi(relative_note: i32, octave_shift: i32) -> u8 {
    let base_midi = 60; // C4
    let midi_note = base_midi + relative_note + (octave_shift * 12);
    midi_note.clamp(0, 127) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_info() {
        let kbd = KeyboardInput::new();
        assert_eq!(kbd.info().id, "input.keyboard");
        assert_eq!(kbd.info().name, "Keyboard");
        assert_eq!(kbd.info().category, ModuleCategory::Source);
    }

    #[test]
    fn test_keyboard_ports() {
        let kbd = KeyboardInput::new();
        let ports = kbd.ports();

        assert_eq!(ports.len(), 3);

        // All are outputs
        assert!(ports[0].is_output());
        assert_eq!(ports[0].id, "gate");
        assert_eq!(ports[0].signal_type, SignalType::Gate);

        assert!(ports[1].is_output());
        assert_eq!(ports[1].id, "pitch");
        assert_eq!(ports[1].signal_type, SignalType::Control);

        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "velocity");
        assert_eq!(ports[2].signal_type, SignalType::Control);
    }

    #[test]
    fn test_keyboard_parameters() {
        let kbd = KeyboardInput::new();
        let params = kbd.parameters();

        assert_eq!(params.len(), 5);

        assert_eq!(params[0].id, "note");
        assert_eq!(params[1].id, "gate");
        assert_eq!(params[2].id, "octave");
        assert_eq!(params[3].id, "velocity");
        assert_eq!(params[4].id, "priority");
    }

    #[test]
    fn test_midi_to_voct() {
        // Middle C (60) = 0.0
        assert!((KeyboardInput::midi_to_voct(60.0) - 0.0).abs() < f32::EPSILON);

        // C5 (72) = +1.0
        assert!((KeyboardInput::midi_to_voct(72.0) - 1.0).abs() < f32::EPSILON);

        // C3 (48) = -1.0
        assert!((KeyboardInput::midi_to_voct(48.0) - (-1.0)).abs() < f32::EPSILON);

        // A4 (69) = 0.75 (9 semitones above C4)
        assert!((KeyboardInput::midi_to_voct(69.0) - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_key_priority_conversion() {
        assert_eq!(KeyPriority::from_param(0.0), KeyPriority::Last);
        assert_eq!(KeyPriority::from_param(1.0), KeyPriority::Lowest);
        assert_eq!(KeyPriority::from_param(2.0), KeyPriority::Highest);
        assert_eq!(KeyPriority::from_param(99.0), KeyPriority::Last); // Out of range
    }

    #[test]
    fn test_key_mapping() {
        use egui::Key;

        // White keys
        assert_eq!(key_to_note(Key::Z), Some(0));  // C
        assert_eq!(key_to_note(Key::X), Some(2));  // D
        assert_eq!(key_to_note(Key::C), Some(4));  // E
        assert_eq!(key_to_note(Key::V), Some(5));  // F
        assert_eq!(key_to_note(Key::B), Some(7));  // G
        assert_eq!(key_to_note(Key::N), Some(9));  // A
        assert_eq!(key_to_note(Key::M), Some(11)); // B

        // Black keys
        assert_eq!(key_to_note(Key::S), Some(1));  // C#
        assert_eq!(key_to_note(Key::D), Some(3));  // D#
        assert_eq!(key_to_note(Key::G), Some(6));  // F#
        assert_eq!(key_to_note(Key::H), Some(8));  // G#
        assert_eq!(key_to_note(Key::J), Some(10)); // A#

        // Non-note keys
        assert_eq!(key_to_note(Key::Space), None);
        assert_eq!(key_to_note(Key::Escape), None);
    }

    #[test]
    fn test_relative_to_midi() {
        // C at base octave = C4 = 60
        assert_eq!(relative_to_midi(0, 0), 60);

        // D at base octave = D4 = 62
        assert_eq!(relative_to_midi(2, 0), 62);

        // C one octave up = C5 = 72
        assert_eq!(relative_to_midi(0, 1), 72);

        // C one octave down = C3 = 48
        assert_eq!(relative_to_midi(0, -1), 48);

        // Clamping
        assert_eq!(relative_to_midi(0, -10), 0);  // Can't go below 0
        assert_eq!(relative_to_midi(12, 10), 127); // Can't go above 127
    }

    #[test]
    fn test_keyboard_generates_output() {
        let mut kbd = KeyboardInput::new();
        kbd.prepare(44100.0, 256);

        let mut outputs = vec![
            SignalBuffer::gate(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Test with gate on, note 60, octave 0, velocity 1.0
        kbd.process(&[], &mut outputs, &[60.0, 1.0, 0.0, 1.0, 0.0], &ctx);

        // Gate should be 1.0
        assert!((outputs[0].samples[0] - 1.0).abs() < f32::EPSILON);

        // Pitch should be 0.0 (middle C)
        assert!((outputs[1].samples[0] - 0.0).abs() < f32::EPSILON);

        // Velocity should be 1.0
        assert!((outputs[2].samples[0] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyboard_octave_shift() {
        let mut kbd = KeyboardInput::new();
        kbd.prepare(44100.0, 256);

        let mut outputs = vec![
            SignalBuffer::gate(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Note 60 (C4) with octave +1 should output pitch +1.0 (C5)
        kbd.process(&[], &mut outputs, &[60.0, 1.0, 1.0, 1.0, 0.0], &ctx);
        assert!((outputs[1].samples[0] - 1.0).abs() < f32::EPSILON);

        // Note 60 (C4) with octave -1 should output pitch -1.0 (C3)
        kbd.reset();
        kbd.process(&[], &mut outputs, &[60.0, 1.0, -1.0, 1.0, 0.0], &ctx);
        assert!((outputs[1].samples[0] - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_keyboard_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<KeyboardInput>();
    }

    #[test]
    fn test_keyboard_default() {
        let kbd = KeyboardInput::default();
        assert_eq!(kbd.info().id, "input.keyboard");
    }
}
