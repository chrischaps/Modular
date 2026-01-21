//! MIDI Note module.
//!
//! Converts MIDI note events into CV signals (V/Oct pitch, gate, velocity, aftertouch).
//! This provides hardware MIDI input as an alternative to the Keyboard module.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Voice priority modes for handling polyphonic input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoicePriority {
    /// Most recently pressed note takes priority.
    Last = 0,
    /// Lowest note takes priority.
    Low = 1,
    /// Highest note takes priority.
    High = 2,
}

impl VoicePriority {
    /// Convert from parameter value (0-2) to voice priority.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => VoicePriority::Last,
            1 => VoicePriority::Low,
            2 => VoicePriority::High,
            _ => VoicePriority::Last,
        }
    }
}

/// A MIDI Note module that converts MIDI input to CV signals.
///
/// Reads from the global MIDI input and outputs pitch CV, gate, velocity,
/// and aftertouch signals for driving oscillators and envelopes.
///
/// # Ports
///
/// **Outputs:**
/// - **Pitch** (Control): V/Oct pitch CV. 0.0 = C4 (MIDI 60), +1.0 = C5, -1.0 = C3.
/// - **Gate** (Gate): High (1.0) when a note is held, low (0.0) when released.
/// - **Velocity** (Control): Note velocity (0.0-1.0).
/// - **Aftertouch** (Control): Channel pressure (0.0-1.0).
///
/// # Parameters
///
/// - **Note** (0-127): Current MIDI note number (set by MIDI events).
/// - **Gate** (0/1): Current gate state (set by MIDI events).
/// - **Velocity** (0-127): Note velocity (set by MIDI events).
/// - **Aftertouch** (0-127): Channel pressure (set by MIDI events).
/// - **Channel** (0-16): MIDI channel filter (0=Omni, 1-16=specific).
/// - **Octave** (-4 to +4): Octave shift applied to MIDI input.
/// - **Priority** (0-2): Voice priority mode (Last, Low, High).
/// - **Retrigger** (0/1): Retrigger gate on legato notes.
pub struct MidiNote {
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
    /// Current velocity value.
    current_velocity: f32,
    /// Current aftertouch value.
    current_aftertouch: f32,
}

impl MidiNote {
    /// Creates a new MIDI Note module.
    pub fn new() -> Self {
        Self {
            sample_rate: 44100.0,
            ports: vec![
                // Output ports
                PortDefinition::output("pitch", "Pitch", SignalType::Control),
                PortDefinition::output("gate", "Gate", SignalType::Gate),
                PortDefinition::output("velocity", "Velocity", SignalType::Control),
                PortDefinition::output("aftertouch", "Aftertouch", SignalType::Control),
            ],
            parameters: vec![
                // Note: MIDI note number (0-127), set by MIDI events
                // Hidden from normal UI - controlled by MIDI input
                ParameterDefinition::new(
                    "note",
                    "Note",
                    0.0,
                    127.0,
                    60.0, // Default to middle C
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Gate: 0 or 1, set by MIDI events
                ParameterDefinition::toggle("gate", "Gate", false),
                // Velocity: 0-127, set by MIDI events
                ParameterDefinition::new(
                    "velocity",
                    "Velocity",
                    0.0,
                    127.0,
                    100.0,
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Aftertouch: 0-127, set by MIDI events
                ParameterDefinition::new(
                    "aftertouch",
                    "Aftertouch",
                    0.0,
                    127.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Channel: MIDI channel filter (0=Omni, 1-16=specific)
                ParameterDefinition::choice(
                    "channel",
                    "Channel",
                    &["Omni", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16"],
                    0,
                ),
                // Octave: shift the notes up/down by octaves
                ParameterDefinition::new(
                    "octave",
                    "Octave",
                    -4.0,
                    4.0,
                    0.0,
                    ParameterDisplay::Linear { unit: "" },
                ),
                // Priority: voice priority mode
                ParameterDefinition::choice(
                    "priority",
                    "Priority",
                    &["Last", "Low", "High"],
                    0,
                ),
                // Retrigger: retrigger gate on legato notes
                ParameterDefinition::toggle("retrigger", "Retrigger", false),
            ],
            current_pitch: 0.0,
            current_gate: 0.0,
            current_velocity: 0.0,
            current_aftertouch: 0.0,
        }
    }

    /// Port index constants.
    const PORT_PITCH: usize = 0;
    const PORT_GATE: usize = 1;
    const PORT_VELOCITY: usize = 2;
    const PORT_AFTERTOUCH: usize = 3;

    /// Parameter index constants.
    pub const PARAM_NOTE: usize = 0;
    pub const PARAM_GATE: usize = 1;
    pub const PARAM_VELOCITY: usize = 2;
    pub const PARAM_AFTERTOUCH: usize = 3;
    pub const PARAM_CHANNEL: usize = 4;
    pub const PARAM_OCTAVE: usize = 5;
    #[allow(dead_code)]
    pub const PARAM_PRIORITY: usize = 6;
    #[allow(dead_code)]
    pub const PARAM_RETRIGGER: usize = 7;

    /// Convert MIDI note number to V/Oct pitch CV.
    ///
    /// Middle C (MIDI 60) = 0.0
    /// C5 (MIDI 72) = +1.0
    /// C3 (MIDI 48) = -1.0
    #[inline]
    pub fn midi_to_voct(midi_note: f32) -> f32 {
        (midi_note - 60.0) / 12.0
    }
}

impl Default for MidiNote {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for MidiNote {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "input.midi_note",
            name: "MIDI Note",
            category: ModuleCategory::Source,
            description: "Convert MIDI note events to CV signals",
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
        let velocity_raw = params[Self::PARAM_VELOCITY];
        let aftertouch_raw = params[Self::PARAM_AFTERTOUCH];
        let octave = params[Self::PARAM_OCTAVE];

        // Normalize velocity and aftertouch from 0-127 to 0.0-1.0
        let velocity = velocity_raw / 127.0;
        let aftertouch = aftertouch_raw / 127.0;

        // Calculate pitch with octave shift
        let shifted_note = note + (octave * 12.0);
        let target_pitch = Self::midi_to_voct(shifted_note);

        // Fill output buffers
        for i in 0..context.block_size {
            // Gate output - instant transition
            outputs[Self::PORT_GATE].samples[i] = gate;

            // Pitch output - update when gate is high
            if gate > 0.5 {
                self.current_pitch = target_pitch;
            }
            outputs[Self::PORT_PITCH].samples[i] = self.current_pitch;

            // Velocity output - update to current velocity
            self.current_velocity = velocity;
            outputs[Self::PORT_VELOCITY].samples[i] = self.current_velocity;

            // Aftertouch output
            self.current_aftertouch = aftertouch;
            outputs[Self::PORT_AFTERTOUCH].samples[i] = self.current_aftertouch;
        }

        self.current_gate = gate;
    }

    fn reset(&mut self) {
        self.current_pitch = 0.0;
        self.current_gate = 0.0;
        self.current_velocity = 0.0;
        self.current_aftertouch = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_note_info() {
        let module = MidiNote::new();
        assert_eq!(module.info().id, "input.midi_note");
        assert_eq!(module.info().name, "MIDI Note");
        assert_eq!(module.info().category, ModuleCategory::Source);
    }

    #[test]
    fn test_midi_note_ports() {
        let module = MidiNote::new();
        let ports = module.ports();

        assert_eq!(ports.len(), 4);

        // All are outputs
        assert!(ports[0].is_output());
        assert_eq!(ports[0].id, "pitch");
        assert_eq!(ports[0].signal_type, SignalType::Control);

        assert!(ports[1].is_output());
        assert_eq!(ports[1].id, "gate");
        assert_eq!(ports[1].signal_type, SignalType::Gate);

        assert!(ports[2].is_output());
        assert_eq!(ports[2].id, "velocity");
        assert_eq!(ports[2].signal_type, SignalType::Control);

        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "aftertouch");
        assert_eq!(ports[3].signal_type, SignalType::Control);
    }

    #[test]
    fn test_midi_note_parameters() {
        let module = MidiNote::new();
        let params = module.parameters();

        assert_eq!(params.len(), 8);

        assert_eq!(params[0].id, "note");
        assert_eq!(params[1].id, "gate");
        assert_eq!(params[2].id, "velocity");
        assert_eq!(params[3].id, "aftertouch");
        assert_eq!(params[4].id, "channel");
        assert_eq!(params[5].id, "octave");
        assert_eq!(params[6].id, "priority");
        assert_eq!(params[7].id, "retrigger");
    }

    #[test]
    fn test_midi_to_voct() {
        // Middle C (60) = 0.0
        assert!((MidiNote::midi_to_voct(60.0) - 0.0).abs() < f32::EPSILON);

        // C5 (72) = +1.0
        assert!((MidiNote::midi_to_voct(72.0) - 1.0).abs() < f32::EPSILON);

        // C3 (48) = -1.0
        assert!((MidiNote::midi_to_voct(48.0) - (-1.0)).abs() < f32::EPSILON);

        // A4 (69) = 0.75 (9 semitones above C4)
        assert!((MidiNote::midi_to_voct(69.0) - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_voice_priority_conversion() {
        assert_eq!(VoicePriority::from_param(0.0), VoicePriority::Last);
        assert_eq!(VoicePriority::from_param(1.0), VoicePriority::Low);
        assert_eq!(VoicePriority::from_param(2.0), VoicePriority::High);
        assert_eq!(VoicePriority::from_param(99.0), VoicePriority::Last); // Out of range
    }

    #[test]
    fn test_midi_note_generates_output() {
        let mut module = MidiNote::new();
        module.prepare(44100.0, 256);

        let mut outputs = vec![
            SignalBuffer::control(256), // Pitch
            SignalBuffer::gate(256),    // Gate
            SignalBuffer::control(256), // Velocity
            SignalBuffer::control(256), // Aftertouch
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Test with note 60, gate on, velocity 100, aftertouch 0
        // Params: note, gate, velocity, aftertouch, channel, octave, priority, retrigger
        module.process(&[], &mut outputs, &[60.0, 1.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0], &ctx);

        // Pitch should be 0.0 (middle C)
        assert!((outputs[0].samples[0] - 0.0).abs() < f32::EPSILON);

        // Gate should be 1.0
        assert!((outputs[1].samples[0] - 1.0).abs() < f32::EPSILON);

        // Velocity should be ~0.787 (100/127)
        assert!((outputs[2].samples[0] - (100.0 / 127.0)).abs() < 0.01);

        // Aftertouch should be 0.0
        assert!((outputs[3].samples[0] - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_midi_note_octave_shift() {
        let mut module = MidiNote::new();
        module.prepare(44100.0, 256);

        let mut outputs = vec![
            SignalBuffer::control(256),
            SignalBuffer::gate(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Note 60 (C4) with octave +1 should output pitch +1.0 (C5)
        module.process(&[], &mut outputs, &[60.0, 1.0, 100.0, 0.0, 0.0, 1.0, 0.0, 0.0], &ctx);
        assert!((outputs[0].samples[0] - 1.0).abs() < f32::EPSILON);

        // Note 60 (C4) with octave -1 should output pitch -1.0 (C3)
        module.reset();
        module.process(&[], &mut outputs, &[60.0, 1.0, 100.0, 0.0, 0.0, -1.0, 0.0, 0.0], &ctx);
        assert!((outputs[0].samples[0] - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_midi_note_velocity_normalization() {
        let mut module = MidiNote::new();
        module.prepare(44100.0, 256);

        let mut outputs = vec![
            SignalBuffer::control(256),
            SignalBuffer::gate(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Velocity 127 should output 1.0
        module.process(&[], &mut outputs, &[60.0, 1.0, 127.0, 0.0, 0.0, 0.0, 0.0, 0.0], &ctx);
        assert!((outputs[2].samples[0] - 1.0).abs() < f32::EPSILON);

        // Velocity 0 should output 0.0
        module.reset();
        module.process(&[], &mut outputs, &[60.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], &ctx);
        assert!(outputs[2].samples[0].abs() < f32::EPSILON);
    }

    #[test]
    fn test_midi_note_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MidiNote>();
    }

    #[test]
    fn test_midi_note_default() {
        let module = MidiNote::default();
        assert_eq!(module.info().id, "input.midi_note");
    }
}
