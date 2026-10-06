//! Poly MIDI module.
//!
//! Turns live MIDI into polyphonic CV: one channel of pitch, gate and
//! velocity per voice, on cables that carry every voice at once. Patched
//! into polyphonic modules (oscillator, filters, envelope, VCA), each held
//! key plays through its own chain, so a chord is several independent
//! voices rather than one note.
//!
//! Like [`MidiNote`](super::MidiNote), it reads MIDI on the audio thread
//! through [`ProcessContext::midi`], so every note starts on its sample.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    MidiEvent, MidiMessage, ParameterDisplay, SignalType, MAX_CHANNELS,
};

use super::MidiNote;

/// CC 64, the sustain pedal.
const SUSTAIN_PEDAL: u8 = 64;
/// CC 120, All Sound Off.
const ALL_SOUND_OFF: u8 = 120;
/// CC 123, All Notes Off.
const ALL_NOTES_OFF: u8 = 123;

/// How quickly pitch bend follows the wheel, as in MIDI Note.
const BEND_SMOOTHING_SECONDS: f32 = 0.005;

/// How a new note picks its voice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allocation {
    /// Each note takes the next free voice in turn, so release tails ring
    /// on while new notes sound elsewhere.
    Rotate = 0,
    /// A note played again goes back to the voice that last played it, as a
    /// piano string is struck again; other notes take the voice that has
    /// been free longest.
    Reuse = 1,
}

impl Allocation {
    /// Converts from the parameter value.
    pub fn from_param(value: f32) -> Self {
        match value.round() as i32 {
            1 => Allocation::Reuse,
            _ => Allocation::Rotate,
        }
    }
}

/// One voice: the note it plays and where its key is.
#[derive(Clone, Copy, Debug)]
struct Voice {
    /// The note the voice plays. Kept after release, so the release tail
    /// stays in tune.
    note: u8,
    /// The note's velocity (0.0-1.0).
    velocity: f32,
    /// The key is down.
    held: bool,
    /// The key is up but the sustain pedal holds the note.
    sustained: bool,
    /// When the note started, in note events: the oldest is stolen first.
    started: u64,
    /// When the voice last fell free, in note events.
    freed: u64,
    /// Keep the gate high for this sample even if the note already ended,
    /// so the shortest note still makes a rising edge.
    hold_gate: bool,
    /// Drop the gate for this sample, so a stolen or restruck voice makes a
    /// fresh rising edge.
    gap: bool,
}

impl Voice {
    const IDLE: Voice = Voice {
        note: 60,
        velocity: 0.0,
        held: false,
        sustained: false,
        started: 0,
        freed: 0,
        hold_gate: false,
        gap: false,
    };

    /// Whether the voice is sounding a note (its gate is up).
    fn busy(&self) -> bool {
        self.held || self.sustained
    }

    /// The gate for the current sample. Clears the one-sample flags.
    fn take_gate(&mut self) -> bool {
        let gate = self.hold_gate || (self.busy() && !self.gap);
        self.hold_gate = false;
        self.gap = false;
        gate
    }
}

/// MIDI to polyphonic CV, with voice allocation and stealing.
///
/// # Ports
///
/// **Outputs:**
/// - **Pitch** (Control, poly): V/Oct per voice, with pitch bend. 0.0 = C4.
/// - **Gate** (Gate, poly): High while the voice's key (or the sustain
///   pedal) holds its note.
/// - **Velocity** (Control, poly): Each voice's note velocity (0.0-1.0).
/// - **Aftertouch** (Control): Channel pressure (0.0-1.0), one channel
///   shared by every voice.
///
/// # Parameters
///
/// - **Channel** (Omni, 1-16): Which MIDI channel to listen to.
/// - **Voices** (1-8): How many channels the cables carry.
/// - **Allocation** (Rotate, Reuse): How a new note picks its voice.
/// - **Octave** (-4 to +4): Octave shift.
/// - **Bend Range** (0-12 semitones): How far the pitch bend wheel bends.
///
/// When every voice is busy, a new note steals one: a voice the sustain
/// pedal is holding if there is one, otherwise the oldest note.
pub struct PolyMidi {
    ports: Vec<PortDefinition>,
    parameters: Vec<ParameterDefinition>,
    voices: [Voice; MAX_CHANNELS],
    /// Where Rotate looks for a free voice next.
    next: usize,
    /// Counts note events, to order voices by age.
    clock: u64,
    /// Whether the sustain pedal is down.
    pedal: bool,
    /// Channel pressure (0.0-1.0).
    aftertouch: f32,
    /// Pitch bend as last received (-1.0 to 1.0).
    bend_target: f32,
    /// Pitch bend as heard, gliding toward `bend_target`.
    bend: f32,
    /// One-pole coefficient for the bend glide.
    bend_coeff: f32,
}

impl PolyMidi {
    /// Creates a new Poly MIDI module.
    pub fn new() -> Self {
        let mut module = Self {
            ports: vec![
                PortDefinition::output("pitch", "Pitch", SignalType::Control),
                PortDefinition::output("gate", "Gate", SignalType::Gate),
                PortDefinition::output("velocity", "Velocity", SignalType::Control),
                PortDefinition::output("aftertouch", "Aftertouch", SignalType::Control),
            ],
            parameters: vec![
                ParameterDefinition::choice(
                    "channel",
                    "Channel",
                    &["Omni", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16"],
                    0,
                ),
                ParameterDefinition::new(
                    "voices",
                    "Voices",
                    1.0,
                    MAX_CHANNELS as f32,
                    MAX_CHANNELS as f32,
                    ParameterDisplay::stepped(""),
                ),
                ParameterDefinition::choice("allocation", "Allocation", &["Rotate", "Reuse"], 0),
                ParameterDefinition::new("octave", "Octave", -4.0, 4.0, 0.0, ParameterDisplay::stepped("oct")),
                ParameterDefinition::new("bend_range", "Bend Range", 0.0, 12.0, 2.0, ParameterDisplay::stepped("st")),
            ],
            voices: [Voice::IDLE; MAX_CHANNELS],
            next: 0,
            clock: 0,
            pedal: false,
            aftertouch: 0.0,
            bend_target: 0.0,
            bend: 0.0,
            bend_coeff: 1.0,
        };
        module.prepare(44100.0, 256);
        module
    }

    /// Port index constants.
    const PORT_PITCH: usize = 0;
    const PORT_GATE: usize = 1;
    const PORT_VELOCITY: usize = 2;
    const PORT_AFTERTOUCH: usize = 3;

    /// Parameter index constants.
    pub const PARAM_CHANNEL: usize = 0;
    pub const PARAM_VOICES: usize = 1;
    pub const PARAM_ALLOCATION: usize = 2;
    pub const PARAM_OCTAVE: usize = 3;
    pub const PARAM_BEND_RANGE: usize = 4;

    /// The next tick of the note-event clock.
    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    /// Applies one MIDI event to the first `count` voices.
    fn handle(&mut self, event: &MidiEvent, count: usize, allocation: Allocation) {
        match event.message {
            MidiMessage::NoteOn { note, velocity } if velocity > 0 => {
                self.note_on(note, velocity, count, allocation);
            }
            // Note On at velocity 0 is a Note Off
            MidiMessage::NoteOn { note, .. } | MidiMessage::NoteOff { note, .. } => {
                self.note_off(note, count);
            }
            MidiMessage::PitchBend { value } => {
                self.bend_target = (value as f32 / 8192.0).clamp(-1.0, 1.0);
            }
            MidiMessage::Aftertouch { pressure } => {
                self.aftertouch = pressure as f32 / 127.0;
            }
            MidiMessage::ControlChange { controller: SUSTAIN_PEDAL, value } => {
                self.pedal = value >= 64;
                if !self.pedal {
                    let now = self.tick();
                    for voice in self.voices.iter_mut().filter(|voice| voice.sustained) {
                        voice.sustained = false;
                        voice.freed = now;
                    }
                }
            }
            MidiMessage::ControlChange { controller: ALL_NOTES_OFF | ALL_SOUND_OFF, .. } => {
                self.pedal = false;
                for voice in &mut self.voices {
                    voice.held = false;
                    voice.sustained = false;
                }
            }
            _ => {}
        }
    }

    fn note_on(&mut self, note: u8, velocity: u8, count: usize, allocation: Allocation) {
        let index = self.allocate(note, count, allocation);
        let started = self.tick();
        let voice = &mut self.voices[index];
        if voice.busy() {
            // Stolen or struck again: a gap first, so envelopes restart
            voice.gap = true;
        } else {
            voice.hold_gate = true;
        }
        voice.note = note;
        voice.velocity = velocity as f32 / 127.0;
        voice.held = true;
        voice.sustained = false;
        voice.started = started;
    }

    /// Picks the voice (among the first `count`) to play `note`.
    fn allocate(&mut self, note: u8, count: usize, allocation: Allocation) -> usize {
        let voices = &self.voices[..count];

        // A note still sounding is struck again on its own voice
        if let Some(index) = voices.iter().position(|voice| voice.busy() && voice.note == note) {
            return index;
        }

        let free = match allocation {
            Allocation::Rotate => (0..count).map(|k| (self.next + k) % count).find(|&i| !voices[i].busy()),
            Allocation::Reuse => voices
                .iter()
                .position(|voice| !voice.busy() && voice.note == note)
                .or_else(|| {
                    (0..count).filter(|&i| !voices[i].busy()).min_by_key(|&i| voices[i].freed)
                }),
        };
        if let Some(index) = free {
            self.next = (index + 1) % count;
            return index;
        }

        // Every voice is busy: steal one the pedal is holding, else the
        // oldest note
        (0..count)
            .min_by_key(|&i| (voices[i].held, voices[i].started))
            .unwrap_or(0)
    }

    fn note_off(&mut self, note: u8, count: usize) {
        let now = self.tick();
        let pedal = self.pedal;
        for voice in self.voices[..count].iter_mut().filter(|voice| voice.held && voice.note == note) {
            voice.held = false;
            if pedal {
                voice.sustained = true;
            } else {
                voice.freed = now;
            }
        }
    }
}

impl Default for PolyMidi {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for PolyMidi {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "input.poly_midi",
            name: "Poly MIDI",
            category: ModuleCategory::Source,
            description: "Play chords: MIDI notes as polyphonic pitch, gate and velocity",
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
        self.bend_coeff = 1.0 - (-1.0 / (BEND_SMOOTHING_SECONDS * sample_rate)).exp();
    }

    fn process(
        &mut self,
        _inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let channel = params[Self::PARAM_CHANNEL];
        let allocation = Allocation::from_param(params[Self::PARAM_ALLOCATION]);
        let octave = params[Self::PARAM_OCTAVE].round();
        let bend_range = params[Self::PARAM_BEND_RANGE];

        // As many voices as the knob asks for and the cables have room for
        let room = outputs[..Self::PORT_AFTERTOUCH].iter().map(|output| output.max_channels()).min().unwrap_or(1);
        let count = (params[Self::PARAM_VOICES].round() as usize).clamp(1, room);
        for output in &mut outputs[..Self::PORT_AFTERTOUCH] {
            output.set_channels(count);
        }
        // Voices turned off let go of their notes
        for voice in &mut self.voices[count..] {
            *voice = Voice { note: voice.note, ..Voice::IDLE };
        }
        self.next %= count;

        let mut events = context
            .midi
            .iter()
            .filter(|event| MidiNote::listens_to(channel, event))
            .peekable();

        for i in 0..context.block_size {
            // Everything that lands on this sample, in order
            while let Some(event) = events.next_if(|event| event.sample_offset as usize <= i) {
                self.handle(event, count, allocation);
            }

            self.bend += (self.bend_target - self.bend) * self.bend_coeff;
            let shift = octave * 12.0 + self.bend * bend_range;

            for (index, voice) in self.voices[..count].iter_mut().enumerate() {
                let gate = voice.take_gate();
                outputs[Self::PORT_PITCH].channel_mut(index)[i] = MidiNote::midi_to_voct(voice.note as f32 + shift);
                outputs[Self::PORT_GATE].channel_mut(index)[i] = if gate { 1.0 } else { 0.0 };
                outputs[Self::PORT_VELOCITY].channel_mut(index)[i] = voice.velocity;
            }
            outputs[Self::PORT_AFTERTOUCH].samples[i] = self.aftertouch;
        }

        // Anything placed past the block still counts, from the next one
        for event in events {
            self.handle(event, count, allocation);
        }
    }

    fn reset(&mut self) {
        self.voices = [Voice::IDLE; MAX_CHANNELS];
        self.next = 0;
        self.clock = 0;
        self.pedal = false;
        self.aftertouch = 0.0;
        self.bend_target = 0.0;
        self.bend = 0.0;
    }

    fn polyphonic(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;
    const BLOCK: usize = 256;

    /// Default parameters: Omni, 8 voices, Rotate, octave 0, ±2 st.
    const DEFAULTS: [f32; 5] = [0.0, 8.0, 0.0, 0.0, 2.0];

    fn params(voices: usize, allocation: Allocation) -> [f32; 5] {
        [0.0, voices as f32, allocation as i32 as f32, 0.0, 2.0]
    }

    fn module() -> PolyMidi {
        let mut module = PolyMidi::new();
        module.prepare(SR, BLOCK);
        module
    }

    fn run(module: &mut PolyMidi, params: &[f32], midi: &[MidiEvent]) -> Vec<SignalBuffer> {
        let mut out = vec![
            SignalBuffer::polyphonic(BLOCK, SignalType::Control),
            SignalBuffer::polyphonic(BLOCK, SignalType::Gate),
            SignalBuffer::polyphonic(BLOCK, SignalType::Control),
            SignalBuffer::polyphonic(BLOCK, SignalType::Control),
        ];
        let context = ProcessContext::new(SR, BLOCK).with_midi(midi);
        module.process(&[], &mut out, params, &context);
        out
    }

    fn on(at: u32, note: u8) -> MidiEvent {
        MidiEvent::note_on(at, 0, note, 100)
    }

    fn off(at: u32, note: u8) -> MidiEvent {
        MidiEvent::note_off(at, 0, note, 0)
    }

    fn cc(at: u32, controller: u8, value: u8) -> MidiEvent {
        MidiEvent::new(at, 0, MidiMessage::ControlChange { controller, value })
    }

    /// The note each voice plays at sample `i`, or `None` if its gate is low.
    fn sounding(out: &[SignalBuffer], i: usize) -> Vec<Option<u8>> {
        (0..out[PolyMidi::PORT_GATE].channels())
            .map(|v| {
                let gate = out[PolyMidi::PORT_GATE].voice(v).samples[i] > 0.5;
                let note = (out[PolyMidi::PORT_PITCH].voice(v).samples[i] * 12.0 + 60.0).round() as u8;
                gate.then_some(note)
            })
            .collect()
    }

    fn rising_edges(gate: &[f32]) -> Vec<usize> {
        let mut previous = 0.0;
        let mut edges = Vec::new();
        for (i, &g) in gate.iter().enumerate() {
            if g > 0.5 && previous <= 0.5 {
                edges.push(i);
            }
            previous = g;
        }
        edges
    }

    #[test]
    fn test_poly_midi_ports_and_parameters() {
        let module = PolyMidi::new();
        assert_eq!(module.info().id, "input.poly_midi");
        assert!(module.polyphonic());
        let ports: Vec<&str> = module.ports().iter().map(|p| p.id).collect();
        assert_eq!(ports, ["pitch", "gate", "velocity", "aftertouch"]);
        let defaults: Vec<f32> = module.parameters().iter().map(|p| p.default).collect();
        assert_eq!(defaults, DEFAULTS);
    }

    #[test]
    fn test_chord_spreads_across_voices() {
        let mut m = module();
        let out = run(&mut m, &DEFAULTS, &[on(0, 60), on(0, 64), on(0, 67), on(0, 72)]);
        assert_eq!(out[PolyMidi::PORT_PITCH].channels(), 8);
        assert_eq!(out[PolyMidi::PORT_AFTERTOUCH].channels(), 1);
        assert_eq!(
            sounding(&out, 0),
            [Some(60), Some(64), Some(67), Some(72), None, None, None, None]
        );
    }

    #[test]
    fn test_releasing_one_note_leaves_the_others() {
        let mut m = module();
        let out = run(&mut m, &DEFAULTS, &[on(0, 60), on(0, 64), on(0, 67), off(100, 64)]);
        assert_eq!(sounding(&out, 99)[..3], [Some(60), Some(64), Some(67)]);
        assert_eq!(sounding(&out, 100)[..3], [Some(60), None, Some(67)]);
        // The released voice keeps its pitch for the release tail
        assert_eq!(out[PolyMidi::PORT_PITCH].voice(1).samples[200], 4.0 / 12.0);
    }

    #[test]
    fn test_rotate_moves_on_to_the_next_voice() {
        let mut m = module();
        let p = params(4, Allocation::Rotate);
        let out = run(&mut m, &p, &[on(0, 60), off(10, 60), on(20, 60), off(30, 60), on(40, 62)]);
        assert_eq!(sounding(&out, 5), [Some(60), None, None, None]);
        assert_eq!(sounding(&out, 25), [None, Some(60), None, None], "same note, next voice");
        assert_eq!(sounding(&out, 45), [None, None, Some(62), None]);
    }

    #[test]
    fn test_reuse_returns_a_note_to_its_voice() {
        let mut m = module();
        let p = params(4, Allocation::Reuse);
        let out = run(
            &mut m,
            &p,
            &[on(0, 60), on(0, 62), off(10, 60), off(10, 62), on(20, 62), on(30, 65)],
        );
        assert_eq!(sounding(&out, 5), [Some(60), Some(62), None, None]);
        assert_eq!(sounding(&out, 25), [None, Some(62), None, None], "62 back on its voice");
        // A new note takes the voice free longest: 3 and 4 never played
        assert_eq!(sounding(&out, 35)[2..], [Some(65), None]);
    }

    #[test]
    fn test_full_voices_steal_the_oldest_note() {
        let mut m = module();
        let p = params(2, Allocation::Rotate);
        let out = run(&mut m, &p, &[on(0, 60), on(10, 64), on(20, 67)]);
        assert_eq!(sounding(&out, 15), [Some(60), Some(64)]);
        // The stolen voice drops for one sample, then plays the new note
        assert_eq!(sounding(&out, 20), [None, Some(64)]);
        assert_eq!(sounding(&out, 21), [Some(67), Some(64)]);
        assert_eq!(rising_edges(&out[PolyMidi::PORT_GATE].voice(0).samples), vec![0, 21]);
    }

    #[test]
    fn test_sustain_pedal_holds_notes_and_is_stolen_first() {
        let mut m = module();
        let p = params(2, Allocation::Rotate);
        let out = run(
            &mut m,
            &p,
            &[cc(0, SUSTAIN_PEDAL, 127), on(0, 60), off(10, 60), on(20, 64), on(30, 67)],
        );
        assert_eq!(sounding(&out, 15), [Some(60), None], "the pedal holds 60");
        // Both voices busy: 67 takes the pedalled 60 rather than the held 64
        assert_eq!(sounding(&out, 31), [Some(67), Some(64)]);

        let out = run(&mut m, &p, &[off(0, 67), cc(50, SUSTAIN_PEDAL, 0)]);
        assert_eq!(sounding(&out, 25), [Some(67), Some(64)]);
        assert_eq!(sounding(&out, 50), [None, Some(64)], "pedal up lets 67 go");
    }

    #[test]
    fn test_restriking_a_held_note_retriggers_its_voice() {
        let mut m = module();
        let out = run(&mut m, &DEFAULTS, &[cc(0, SUSTAIN_PEDAL, 127), on(0, 60), off(10, 60), on(20, 60)]);
        assert_eq!(rising_edges(&out[PolyMidi::PORT_GATE].voice(0).samples), vec![0, 21]);
        assert!(sounding(&out, 30)[1..].iter().all(Option::is_none), "no second voice");
    }

    #[test]
    fn test_velocity_per_voice() {
        let mut m = module();
        let out = run(
            &mut m,
            &DEFAULTS,
            &[MidiEvent::note_on(0, 0, 60, 127), MidiEvent::note_on(0, 0, 64, 0x40)],
        );
        let velocity = &out[PolyMidi::PORT_VELOCITY];
        assert_eq!(velocity.voice(0).samples[0], 1.0);
        assert!((velocity.voice(1).samples[0] - 64.0 / 127.0).abs() < 1e-6);
    }

    #[test]
    fn test_shortest_note_still_triggers() {
        let mut m = module();
        let out = run(&mut m, &DEFAULTS, &[on(10, 60), off(10, 60)]);
        assert_eq!(rising_edges(&out[PolyMidi::PORT_GATE].samples), vec![10]);
        assert_eq!(out[PolyMidi::PORT_GATE].samples[11], 0.0);
    }

    #[test]
    fn test_pitch_bend_bends_every_voice() {
        let mut m = module();
        run(&mut m, &DEFAULTS, &[on(0, 60), on(0, 67), MidiEvent::new(0, 0, MidiMessage::PitchBend { value: 8191 })]);
        for _ in 0..20 {
            run(&mut m, &DEFAULTS, &[]);
        }
        let out = run(&mut m, &DEFAULTS, &[]);
        let pitch = &out[PolyMidi::PORT_PITCH];
        assert!((pitch.voice(0).samples[BLOCK - 1] * 12.0 - 2.0).abs() < 0.01);
        assert!((pitch.voice(1).samples[BLOCK - 1] * 12.0 - 9.0).abs() < 0.01);
    }

    #[test]
    fn test_fewer_voices_release_the_rest() {
        let mut m = module();
        run(&mut m, &DEFAULTS, &[on(0, 60), on(0, 64), on(0, 67)]);
        let out = run(&mut m, &params(2, Allocation::Rotate), &[]);
        assert_eq!(out[PolyMidi::PORT_GATE].channels(), 2);

        // Back to 8: voice 3's note was let go, not left hanging
        let out = run(&mut m, &DEFAULTS, &[]);
        assert_eq!(sounding(&out, 0)[..3], [Some(60), Some(64), None]);
    }

    #[test]
    fn test_channel_filter_and_all_notes_off() {
        let mut m = module();
        let ch1_only = [1.0, 8.0, 0.0, 0.0, 2.0];
        let out = run(&mut m, &ch1_only, &[MidiEvent::note_on(0, 1, 60, 100), on(0, 64)]);
        assert_eq!(sounding(&out, 0)[..2], [Some(64), None]);

        let out = run(&mut m, &ch1_only, &[cc(10, ALL_NOTES_OFF, 0)]);
        assert!(sounding(&out, 10).iter().all(Option::is_none));
    }

    #[test]
    fn test_mono_outputs_play_one_voice() {
        // Without room for more channels, it plays like a mono MIDI module
        let mut m = module();
        let mut out = vec![
            SignalBuffer::control(BLOCK),
            SignalBuffer::gate(BLOCK),
            SignalBuffer::control(BLOCK),
            SignalBuffer::control(BLOCK),
        ];
        let midi = [on(0, 60), on(10, 64)];
        let context = ProcessContext::new(SR, BLOCK).with_midi(&midi);
        m.process(&[], &mut out, &DEFAULTS, &context);
        assert_eq!(sounding(&out, 20), [Some(64)]);
    }

    #[test]
    fn test_reset_releases_everything() {
        let mut m = module();
        run(&mut m, &DEFAULTS, &[on(0, 60), on(0, 64)]);
        m.reset();
        let out = run(&mut m, &DEFAULTS, &[]);
        assert!(sounding(&out, 0).iter().all(Option::is_none));
    }
}
