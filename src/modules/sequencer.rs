//! Step Sequencer module.
//!
//! A 16-step sequencer with per-step pitch, gate, and velocity.
//! Advances on clock input, outputs CV/Gate signals for driving oscillators and envelopes.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Maximum number of steps in the sequencer.
pub const MAX_STEPS: usize = 16;

// Static parameter IDs and names for each step (must be 'static for ParameterDefinition)
static STEP_PITCH_IDS: [&str; MAX_STEPS] = [
    "step_1_pitch", "step_2_pitch", "step_3_pitch", "step_4_pitch",
    "step_5_pitch", "step_6_pitch", "step_7_pitch", "step_8_pitch",
    "step_9_pitch", "step_10_pitch", "step_11_pitch", "step_12_pitch",
    "step_13_pitch", "step_14_pitch", "step_15_pitch", "step_16_pitch",
];

static STEP_PITCH_NAMES: [&str; MAX_STEPS] = [
    "Step 1 Pitch", "Step 2 Pitch", "Step 3 Pitch", "Step 4 Pitch",
    "Step 5 Pitch", "Step 6 Pitch", "Step 7 Pitch", "Step 8 Pitch",
    "Step 9 Pitch", "Step 10 Pitch", "Step 11 Pitch", "Step 12 Pitch",
    "Step 13 Pitch", "Step 14 Pitch", "Step 15 Pitch", "Step 16 Pitch",
];

static STEP_GATE_IDS: [&str; MAX_STEPS] = [
    "step_1_gate", "step_2_gate", "step_3_gate", "step_4_gate",
    "step_5_gate", "step_6_gate", "step_7_gate", "step_8_gate",
    "step_9_gate", "step_10_gate", "step_11_gate", "step_12_gate",
    "step_13_gate", "step_14_gate", "step_15_gate", "step_16_gate",
];

static STEP_GATE_NAMES: [&str; MAX_STEPS] = [
    "Step 1 Gate", "Step 2 Gate", "Step 3 Gate", "Step 4 Gate",
    "Step 5 Gate", "Step 6 Gate", "Step 7 Gate", "Step 8 Gate",
    "Step 9 Gate", "Step 10 Gate", "Step 11 Gate", "Step 12 Gate",
    "Step 13 Gate", "Step 14 Gate", "Step 15 Gate", "Step 16 Gate",
];

static STEP_VELOCITY_IDS: [&str; MAX_STEPS] = [
    "step_1_velocity", "step_2_velocity", "step_3_velocity", "step_4_velocity",
    "step_5_velocity", "step_6_velocity", "step_7_velocity", "step_8_velocity",
    "step_9_velocity", "step_10_velocity", "step_11_velocity", "step_12_velocity",
    "step_13_velocity", "step_14_velocity", "step_15_velocity", "step_16_velocity",
];

static STEP_VELOCITY_NAMES: [&str; MAX_STEPS] = [
    "Step 1 Velocity", "Step 2 Velocity", "Step 3 Velocity", "Step 4 Velocity",
    "Step 5 Velocity", "Step 6 Velocity", "Step 7 Velocity", "Step 8 Velocity",
    "Step 9 Velocity", "Step 10 Velocity", "Step 11 Velocity", "Step 12 Velocity",
    "Step 13 Velocity", "Step 14 Velocity", "Step 15 Velocity", "Step 16 Velocity",
];

static STEP_TIE_IDS: [&str; MAX_STEPS] = [
    "step_1_tie", "step_2_tie", "step_3_tie", "step_4_tie",
    "step_5_tie", "step_6_tie", "step_7_tie", "step_8_tie",
    "step_9_tie", "step_10_tie", "step_11_tie", "step_12_tie",
    "step_13_tie", "step_14_tie", "step_15_tie", "step_16_tie",
];

static STEP_TIE_NAMES: [&str; MAX_STEPS] = [
    "Step 1 Tie", "Step 2 Tie", "Step 3 Tie", "Step 4 Tie",
    "Step 5 Tie", "Step 6 Tie", "Step 7 Tie", "Step 8 Tie",
    "Step 9 Tie", "Step 10 Tie", "Step 11 Tie", "Step 12 Tie",
    "Step 13 Tie", "Step 14 Tie", "Step 15 Tie", "Step 16 Tie",
];

/// What Gate Length is a share of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateMode {
    /// The time between the last two clock pulses.
    Step = 0,
    /// A fixed 100 ms, whatever the tempo. Patches saved before Gate Mode
    /// existed load with this.
    Fixed = 1,
}

impl GateMode {
    /// Convert from parameter value (0-1) to mode.
    pub fn from_param(value: f32) -> Self {
        if value >= 0.5 { GateMode::Fixed } else { GateMode::Step }
    }
}

/// Direction modes for sequence playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceDirection {
    /// Play steps in order: 1, 2, 3, ..., N, 1, 2, 3, ...
    Forward = 0,
    /// Play steps in reverse: N, N-1, ..., 2, 1, N, N-1, ...
    Backward = 1,
    /// Bounce back and forth: 1, 2, ..., N, N-1, ..., 2, 1, 2, ...
    PingPong = 2,
    /// Random step selection
    Random = 3,
}

impl SequenceDirection {
    /// Convert from parameter value (0-3) to direction.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => SequenceDirection::Forward,
            1 => SequenceDirection::Backward,
            2 => SequenceDirection::PingPong,
            3 => SequenceDirection::Random,
            _ => SequenceDirection::Forward,
        }
    }

    /// The step a pattern starts from: the last one when playing backward,
    /// otherwise the first.
    fn start_step(self, num_steps: usize) -> usize {
        match self {
            SequenceDirection::Backward => num_steps - 1,
            _ => 0,
        }
    }
}

/// Convert a MIDI note number (0-127) to V/Oct control signal.
/// C4 (note 60) = 0V, each semitone = 1/12 V
fn note_to_voct(note: u8) -> f32 {
    (note as f32 - 60.0) / 12.0
}

/// Convert a note number to a note name for display.
pub fn note_to_name(note: u8) -> String {
    const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (note / 12) as i32 - 1;
    let name = NOTES[(note % 12) as usize];
    format!("{}{}", name, octave)
}

/// A step sequencer module with 16 steps.
///
/// Outputs pitch CV, gate, and velocity for each step, advancing on clock input.
///
/// # Ports
///
/// **Inputs:**
/// - **Clock** (Gate): Advances to the next step on rising edge.
/// - **Reset** (Gate): Returns to step 1 on rising edge.
/// - **Run** (Gate): Enables/disables sequencer advancement.
///
/// **Outputs:**
/// - **Pitch** (Control): V/Oct pitch CV from current step.
/// - **Gate** (Gate): Gate output for current step.
/// - **Velocity** (Control): Velocity (0-1) from current step.
/// - **Step** (Control): Current step as 0-1 value (for visualization).
/// - **EOC** (Gate): End-of-cycle trigger pulse.
///
/// # Parameters
///
/// - **Steps** (1-16): Number of active steps in the sequence.
/// - **Direction** (0-3): Playback direction (Forward, Backward, PingPong, Random).
/// - **Gate Length** (1-100%): Gate duration as a share of the step (or of
///   100 ms in the Fixed gate mode). At 100% of the step the gate holds until
///   the next clock.
/// - **Step 1-16 Pitch** (0-127): MIDI note number for each step.
/// - **Step 1-16 Gate** (0/1): Gate on/off for each step.
/// - **Step 1-16 Velocity** (0-127): Velocity for each step.
/// - **Gate Mode** (Step / 100 ms): What Gate Length is a share of.
/// - **Step 1-16 Tie** (0/1): Holds the step's gate into the next step,
///   which then continues the note instead of starting a new one.
///
/// In the Step gate mode a note always starts with a rising edge: when a new
/// note begins while the gate is still high, the gate drops for one sample
/// first so envelopes retrigger. Only a tie carries the gate across unbroken.
pub struct StepSequencer {
    /// Current step index (0-based).
    current_step: usize,
    /// Direction for ping-pong mode (+1 or -1).
    ping_pong_direction: i32,
    /// Set by a reset (and at the start): the next clock plays the start step
    /// instead of advancing past it.
    reset_pending: bool,
    /// Previous clock state for edge detection.
    prev_clock: bool,
    /// Previous reset state for edge detection.
    prev_reset: bool,
    /// Gate timer (samples remaining in gate).
    gate_timer: usize,
    /// The note playing is tied into the next step.
    tied: bool,
    /// The gate output was high on the last sample.
    gate_high: bool,
    /// Samples since the last clock edge, or `None` before the first (and
    /// after a reset, so a stopped clock's gap isn't taken for a step).
    since_clock: Option<usize>,
    /// The time between the last two clock edges, once known.
    step_samples: Option<usize>,
    /// The two steps measured before that, newest first.
    earlier_steps: [Option<usize>; 2],
    /// EOC timer (samples remaining in EOC pulse).
    eoc_timer: usize,
    /// Simple PRNG state for random mode.
    random_state: u32,
    /// Sample rate from prepare().
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl StepSequencer {
    /// Creates a new StepSequencer with default values.
    pub fn new() -> Self {
        let ports = vec![
            // Input ports
            PortDefinition::input_with_default("clock", "Clock", SignalType::Gate, 0.0).describe("Each rising edge advances to the next step; patch a Clock gate here"),
            PortDefinition::input_with_default("reset", "Reset", SignalType::Gate, 0.0).describe("A rising edge jumps back to step 1"),
            PortDefinition::input_with_default("run", "Run", SignalType::Gate, 1.0).describe("Steps only advance while high; runs when unpatched"),
            // Output ports
            PortDefinition::output("pitch", "Pitch", SignalType::Control).describe("Pitch of the current step as V/Oct; patch into an oscillator"),
            PortDefinition::output("gate", "Gate", SignalType::Gate).describe("Pulses on each clock when the step's gate is on; patch into an envelope"),
            PortDefinition::output("velocity", "Velocity", SignalType::Control).describe("Velocity of the current step, 0 to 1"),
            PortDefinition::output("step_out", "Step", SignalType::Control).describe("Current step position as a 0 to 1 ramp"),
            PortDefinition::output("eoc", "EOC", SignalType::Gate).describe("Short pulse when the sequence reaches its end"),
        ];

        let mut parameters = vec![
            // Global sequencer parameters
            ParameterDefinition::new(
                "steps",
                "Steps",
                1.0,
                16.0,
                8.0,
                ParameterDisplay::linear(""),
            ).describe("How many steps play before the sequence loops"),
            ParameterDefinition::choice(
                "direction",
                "Direction",
                &["Fwd", "Bwd", "P-P", "Rnd"],
                0,
            ).describe("Playback order: forward, backward, ping-pong or random"),
            ParameterDefinition::new(
                "gate_length",
                "Gate Length",
                1.0,
                100.0,
                50.0,
                ParameterDisplay::linear("%"),
            ).describe("How long each gate stays high, as a share of the step; 100% holds it until the next clock"),
        ];

        // Add per-step parameters: pitch, gate, velocity for each of 16 steps
        for i in 0..MAX_STEPS {
            // Pitch: MIDI note number (default to C4 = 60)
            parameters.push(ParameterDefinition::new(
                STEP_PITCH_IDS[i],
                STEP_PITCH_NAMES[i],
                0.0,
                127.0,
                60.0,
                ParameterDisplay::linear(""),
            ).describe("Note for this step as a MIDI number; 60 is middle C"));

            // Gate: on/off toggle (default on)
            parameters.push(ParameterDefinition::toggle(
                STEP_GATE_IDS[i],
                STEP_GATE_NAMES[i],
                true,
            ).describe("Plays this step when on; silent when off"));

            // Velocity: 0-127 (default 100)
            parameters.push(ParameterDefinition::new(
                STEP_VELOCITY_IDS[i],
                STEP_VELOCITY_NAMES[i],
                0.0,
                127.0,
                100.0,
                ParameterDisplay::linear(""),
            ).describe("Velocity for this step, 0 to 127"));
        }

        // Added after the per-step parameters, so older parameter indices
        // stay where they were
        parameters.push(ParameterDefinition::choice(
            "gate_mode",
            "Gate Mode",
            &["Step", "100 ms"],
            GateMode::Step as usize,
        ).describe("What Gate Length is a share of: the time between clock pulses, or a fixed 100 ms"));
        for i in 0..MAX_STEPS {
            parameters.push(ParameterDefinition::toggle(
                STEP_TIE_IDS[i],
                STEP_TIE_NAMES[i],
                false,
            ).describe("Holds this step's note into the next step, which continues it without a new attack"));
        }

        Self {
            current_step: 0,
            ping_pong_direction: 1,
            reset_pending: true,
            prev_clock: false,
            prev_reset: false,
            gate_timer: 0,
            tied: false,
            gate_high: false,
            since_clock: None,
            step_samples: None,
            earlier_steps: [None; 2],
            eoc_timer: 0,
            random_state: 12345, // Seed for PRNG
            sample_rate: 44100.0,
            ports,
            parameters,
        }
    }

    /// Port index constants.
    const PORT_CLOCK: usize = 0;
    const PORT_RESET: usize = 1;
    const PORT_RUN: usize = 2;
    const PORT_PITCH: usize = 0;
    const PORT_GATE: usize = 1;
    const PORT_VELOCITY: usize = 2;
    const PORT_STEP: usize = 3;
    const PORT_EOC: usize = 4;

    /// Parameter index constants for global params.
    const PARAM_STEPS: usize = 0;
    const PARAM_DIRECTION: usize = 1;
    const PARAM_GATE_LENGTH: usize = 2;

    /// Get parameter index for step pitch (0-indexed step).
    const fn step_pitch_param(step: usize) -> usize {
        3 + step * 3
    }

    /// Get parameter index for step gate (0-indexed step).
    const fn step_gate_param(step: usize) -> usize {
        3 + step * 3 + 1
    }

    /// Get parameter index for step velocity (0-indexed step).
    const fn step_velocity_param(step: usize) -> usize {
        3 + step * 3 + 2
    }

    /// Gate Mode comes after the per-step pitch, gate and velocity.
    const PARAM_GATE_MODE: usize = 3 + MAX_STEPS * 3;

    /// Get parameter index for step tie (0-indexed step).
    const fn step_tie_param(step: usize) -> usize {
        Self::PARAM_GATE_MODE + 1 + step
    }

    /// How long the step starting now will last, in samples: usually the
    /// last step measured. A swung clock's steps alternate long and short,
    /// though, so when the step before last matches the last, the rhythm
    /// repeats every two steps and the coming step is the one before last.
    /// A steady clock gives the same answer either way.
    fn coming_step(&self) -> Option<usize> {
        let last = self.step_samples?;
        match self.earlier_steps {
            [Some(before), Some(third)] if last.abs_diff(third) <= last / 32 + 1 => Some(before),
            _ => Some(last),
        }
    }

    /// How long a new note's gate stays high, in samples.
    ///
    /// Gate Length is a share of the coming step (see `coming_step`), or of
    /// 100 ms in the Fixed mode and until two clock edges have been seen. A
    /// gate held until the next clock (a tie, or 100% of the step) still
    /// ends after two steps, so a clock that stops doesn't leave a note
    /// hanging.
    fn gate_samples(&self, mode: GateMode, gate_length: f32, tied: bool) -> usize {
        let fixed = self.sample_rate * 0.1;
        let step = self.coming_step();
        let held = 2 * step.unwrap_or(fixed as usize);
        if tied {
            return held;
        }
        let samples = match (mode, step) {
            (GateMode::Step, Some(_)) if gate_length >= 1.0 => held,
            (GateMode::Step, Some(step)) => (step as f32 * gate_length) as usize,
            _ => (fixed * gate_length) as usize,
        };
        samples.max(1)
    }

    /// Gate threshold for edge detection.
    const GATE_THRESHOLD: f32 = 0.5;

    /// EOC pulse duration in samples (approx 1ms at 44100 Hz).
    const EOC_PULSE_SAMPLES: usize = 44;

    /// Simple xorshift PRNG for random mode.
    fn next_random(&mut self) -> u32 {
        let mut x = self.random_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.random_state = x;
        x
    }

    /// Advance to the next step based on direction mode.
    fn advance_step(&mut self, num_steps: usize, direction: SequenceDirection) -> bool {
        let was_at_end;

        match direction {
            SequenceDirection::Forward => {
                was_at_end = self.current_step >= num_steps - 1;
                self.current_step = (self.current_step + 1) % num_steps;
            }
            SequenceDirection::Backward => {
                was_at_end = self.current_step == 0;
                if self.current_step == 0 {
                    self.current_step = num_steps - 1;
                } else {
                    self.current_step -= 1;
                }
            }
            SequenceDirection::PingPong => {
                let next = self.current_step as i32 + self.ping_pong_direction;

                if next >= num_steps as i32 {
                    // Hit end, reverse direction
                    self.ping_pong_direction = -1;
                    self.current_step = if num_steps > 1 { num_steps - 2 } else { 0 };
                    was_at_end = true;
                } else if next < 0 {
                    // Hit start, reverse direction
                    self.ping_pong_direction = 1;
                    self.current_step = if num_steps > 1 { 1 } else { 0 };
                    was_at_end = true;
                } else {
                    self.current_step = next as usize;
                    was_at_end = false;
                }
            }
            SequenceDirection::Random => {
                was_at_end = false; // No EOC in random mode
                self.current_step = (self.next_random() as usize) % num_steps;
            }
        }

        was_at_end
    }

    /// Get the current step's data.
    pub fn current_step(&self) -> usize {
        self.current_step
    }
}

impl Default for StepSequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for StepSequencer {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "seq.step",
            name: "Step Sequencer",
            category: ModuleCategory::Utility,
            description: "16-step sequencer with pitch, gate, and velocity per step",
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
        // A step measured at another rate is the wrong number of samples
        self.since_clock = None;
        self.step_samples = None;
        self.earlier_steps = [None; 2];
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Get global parameters
        let num_steps = (params[Self::PARAM_STEPS] as usize).clamp(1, MAX_STEPS);
        let direction = SequenceDirection::from_param(params[Self::PARAM_DIRECTION]);
        let gate_length_percent = params[Self::PARAM_GATE_LENGTH] / 100.0;
        let gate_mode = GateMode::from_param(params[Self::PARAM_GATE_MODE]);

        // Get input buffers
        let clock_in = inputs.get(Self::PORT_CLOCK);
        let reset_in = inputs.get(Self::PORT_RESET);
        let run_in = inputs.get(Self::PORT_RUN);

        // Process each sample
        for i in 0..context.block_size {
            // Get input values
            let clock_value = clock_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let reset_value = reset_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let run_value = run_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(1.0))
                .unwrap_or(1.0);

            // Edge detection
            let clock_high = clock_value > Self::GATE_THRESHOLD;
            let clock_rising = clock_high && !self.prev_clock;
            self.prev_clock = clock_high;

            let reset_high = reset_value > Self::GATE_THRESHOLD;
            let reset_rising = reset_high && !self.prev_reset;
            self.prev_reset = reset_high;

            let is_running = run_value > Self::GATE_THRESHOLD;

            // Handle reset
            if reset_rising {
                self.current_step = direction.start_step(num_steps);
                self.ping_pong_direction = 1;
                self.reset_pending = true;
                self.gate_timer = 0;
                self.tied = false;
                self.since_clock = None;
            }

            // Every clock edge measures the step, running or not
            if clock_rising {
                if let Some(samples) = self.since_clock {
                    self.earlier_steps = [self.step_samples, self.earlier_steps[0]];
                    self.step_samples = Some(samples.max(1));
                }
                self.since_clock = Some(0);
            }
            let mut retrigger = false;

            // Handle clock advance. The first clock after a reset plays the
            // start step rather than moving past it, so a reset on the
            // downbeat puts step 1 on the downbeat
            if clock_rising && is_running {
                let hit_end = if self.reset_pending {
                    self.reset_pending = false;
                    self.current_step = direction.start_step(num_steps);
                    false
                } else {
                    self.advance_step(num_steps, direction)
                };

                // A step that plays starts a note, or continues the last one
                // if that was tied. A rest ends the note
                let step_gate = params[Self::step_gate_param(self.current_step)] > 0.5;
                if step_gate {
                    let tie = params[Self::step_tie_param(self.current_step)] > 0.5;
                    // Fixed gates never dipped, so old patches whose notes
                    // overlap still run them together
                    retrigger = gate_mode == GateMode::Step && self.gate_high && !self.tied;
                    self.gate_timer = self.gate_samples(gate_mode, gate_length_percent, tie);
                    self.tied = tie;
                } else {
                    self.gate_timer = 0;
                    self.tied = false;
                }

                // Fire EOC pulse if we hit the end of cycle
                if hit_end && direction != SequenceDirection::Random {
                    self.eoc_timer = Self::EOC_PULSE_SAMPLES;
                }
            }

            // Ensure current step is within bounds (in case num_steps changed)
            if self.current_step >= num_steps {
                self.current_step = 0;
            }

            // Get current step's data
            let step_pitch = params[Self::step_pitch_param(self.current_step)] as u8;
            let step_gate_enabled = params[Self::step_gate_param(self.current_step)] > 0.5;
            let step_velocity = params[Self::step_velocity_param(self.current_step)] / 127.0;

            // Generate outputs (access directly by index to avoid multiple mutable borrows)
            outputs[Self::PORT_PITCH].samples[i] = note_to_voct(step_pitch);

            // Gate output: high if timer > 0 and step gate is enabled. A new
            // note over a gate that's still high dips for this one sample,
            // so it starts with a rising edge
            let gate_active = self.gate_timer > 0 && step_gate_enabled && !retrigger;
            outputs[Self::PORT_GATE].samples[i] = if gate_active { 1.0 } else { 0.0 };
            self.gate_high = gate_active;

            outputs[Self::PORT_VELOCITY].samples[i] = step_velocity;

            // Step output: current step as 0-1 value
            outputs[Self::PORT_STEP].samples[i] = self.current_step as f32 / (num_steps - 1).max(1) as f32;

            // EOC output
            outputs[Self::PORT_EOC].samples[i] = if self.eoc_timer > 0 { 1.0 } else { 0.0 };

            // Decrement timers
            if self.gate_timer > 0 {
                self.gate_timer -= 1;
            }
            if self.eoc_timer > 0 {
                self.eoc_timer -= 1;
            }
            if let Some(samples) = self.since_clock.as_mut() {
                *samples = samples.saturating_add(1);
            }
        }
    }

    fn reset(&mut self) {
        self.current_step = 0;
        self.ping_pong_direction = 1;
        self.reset_pending = true;
        self.prev_clock = false;
        self.prev_reset = false;
        self.gate_timer = 0;
        self.tied = false;
        self.gate_high = false;
        self.since_clock = None;
        self.step_samples = None;
        self.earlier_steps = [None; 2];
        self.eoc_timer = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every parameter at its default (C4, gate on, velocity 100, no ties,
    /// Step gate mode), with the given Steps, Direction and Gate Length.
    fn params_with(steps: f32, direction: f32, gate_length: f32) -> Vec<f32> {
        let mut params: Vec<f32> = StepSequencer::new().parameters().iter().map(|p| p.default).collect();
        params[StepSequencer::PARAM_STEPS] = steps;
        params[StepSequencer::PARAM_DIRECTION] = direction;
        params[StepSequencer::PARAM_GATE_LENGTH] = gate_length;
        params
    }

    #[test]
    fn test_sequencer_info() {
        let seq = StepSequencer::new();
        assert_eq!(seq.info().id, "seq.step");
        assert_eq!(seq.info().name, "Step Sequencer");
        assert_eq!(seq.info().category, ModuleCategory::Utility);
    }

    #[test]
    fn test_sequencer_ports() {
        let seq = StepSequencer::new();
        let ports = seq.ports();

        // 3 inputs + 5 outputs = 8 ports
        assert_eq!(ports.len(), 8);

        // Inputs
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "clock");
        assert!(ports[1].is_input());
        assert_eq!(ports[1].id, "reset");
        assert!(ports[2].is_input());
        assert_eq!(ports[2].id, "run");

        // Outputs
        assert!(ports[3].is_output());
        assert_eq!(ports[3].id, "pitch");
        assert!(ports[4].is_output());
        assert_eq!(ports[4].id, "gate");
        assert!(ports[5].is_output());
        assert_eq!(ports[5].id, "velocity");
        assert!(ports[6].is_output());
        assert_eq!(ports[6].id, "step_out");
        assert!(ports[7].is_output());
        assert_eq!(ports[7].id, "eoc");
    }

    #[test]
    fn test_sequencer_parameters() {
        let seq = StepSequencer::new();
        let params = seq.parameters();

        // 3 global + 16 steps * 3 params each, then Gate Mode and 16 ties
        assert_eq!(params.len(), 3 + MAX_STEPS * 3 + 1 + MAX_STEPS);
        assert_eq!(params[StepSequencer::PARAM_GATE_MODE].id, "gate_mode");
        assert_eq!(params[StepSequencer::step_tie_param(0)].id, "step_1_tie");
        assert_eq!(params[StepSequencer::step_tie_param(15)].id, "step_16_tie");

        // Global params
        assert_eq!(params[0].id, "steps");
        assert_eq!(params[1].id, "direction");
        assert_eq!(params[2].id, "gate_length");

        // First step params
        assert_eq!(params[3].id, "step_1_pitch");
        assert_eq!(params[4].id, "step_1_gate");
        assert_eq!(params[5].id, "step_1_velocity");
    }

    #[test]
    fn test_direction_conversion() {
        assert_eq!(SequenceDirection::from_param(0.0), SequenceDirection::Forward);
        assert_eq!(SequenceDirection::from_param(1.0), SequenceDirection::Backward);
        assert_eq!(SequenceDirection::from_param(2.0), SequenceDirection::PingPong);
        assert_eq!(SequenceDirection::from_param(3.0), SequenceDirection::Random);
        assert_eq!(SequenceDirection::from_param(99.0), SequenceDirection::Forward);
    }

    #[test]
    fn test_note_to_voct() {
        // C4 (60) = 0V
        assert!((note_to_voct(60) - 0.0).abs() < 0.001);
        // C5 (72) = +1V
        assert!((note_to_voct(72) - 1.0).abs() < 0.001);
        // C3 (48) = -1V
        assert!((note_to_voct(48) - -1.0).abs() < 0.001);
    }

    #[test]
    fn test_sequencer_advances_on_clock() {
        let mut seq = StepSequencer::new();
        seq.prepare(44100.0, 256);

        // Create clock pulse
        let mut clock = SignalBuffer::control(256);
        // Rising edge at sample 50
        for i in 50..100 {
            clock.samples[i] = 1.0;
        }

        let mut outputs = vec![
            SignalBuffer::control(256), // pitch
            SignalBuffer::control(256), // gate
            SignalBuffer::control(256), // velocity
            SignalBuffer::control(256), // step
            SignalBuffer::control(256), // eoc
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        // Default params: 8 steps, forward, 50% gate
        let params = params_with(8.0, 0.0, 50.0);

        // The first clock plays the first step rather than moving past it
        seq.process(&[&clock], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 0);
        assert_eq!(outputs[1].samples[60], 1.0, "step 1's gate fires");

        // The next clock advances
        seq.process(&[&clock], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 1);
    }

    /// One sample of clock and reset into a one-sample block.
    fn tick(seq: &mut StepSequencer, params: &[f32], clock: bool, reset: bool) -> f32 {
        let mut clock_buf = SignalBuffer::control(1);
        clock_buf.samples[0] = if clock { 1.0 } else { 0.0 };
        let mut reset_buf = SignalBuffer::control(1);
        reset_buf.samples[0] = if reset { 1.0 } else { 0.0 };
        let mut outputs: Vec<SignalBuffer> = (0..5).map(|_| SignalBuffer::control(1)).collect();
        seq.process(&[&clock_buf, &reset_buf], &mut outputs, params, &ProcessContext::new(44100.0, 1));
        outputs[1].samples[0]
    }

    #[test]
    fn test_first_clock_after_reset_plays_start_step() {
        for (direction, start) in [(0.0, 0), (1.0, 3), (2.0, 0), (3.0, 0)] {
            let mut seq = StepSequencer::new();
            seq.prepare(44100.0, 1);
            let params = params_with(4.0, direction, 50.0);

            // Run a few steps in, then reset
            for _ in 0..3 {
                tick(&mut seq, &params, true, false);
                tick(&mut seq, &params, false, false);
            }
            tick(&mut seq, &params, false, true);
            tick(&mut seq, &params, false, false);

            // The next clock sounds the start step
            let gate = tick(&mut seq, &params, true, false);
            assert_eq!(seq.current_step(), start, "direction {direction}");
            assert_eq!(gate, 1.0, "direction {direction}");
        }
    }

    #[test]
    fn test_sequencer_reset() {
        let mut seq = StepSequencer::new();
        seq.prepare(44100.0, 256);

        // Manually advance
        seq.current_step = 5;

        // Create reset pulse
        let mut reset = SignalBuffer::control(256);
        for i in 50..100 {
            reset.samples[i] = 1.0;
        }

        let clock = SignalBuffer::control(256);
        let mut outputs = vec![
            SignalBuffer::control(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
            SignalBuffer::control(256),
        ];
        let ctx = ProcessContext::new(44100.0, 256);

        let params = params_with(8.0, 0.0, 50.0);

        seq.process(&[&clock, &reset], &mut outputs, &params, &ctx);

        // Should be back at step 0
        assert_eq!(seq.current_step(), 0);
    }

    #[test]
    fn test_sequencer_backward_direction() {
        let mut seq = StepSequencer::new();
        seq.prepare(44100.0, 1);

        let params = params_with(4.0, 1.0, 50.0); // 4 steps, backward

        // Advance through sequence
        let mut clock_high = SignalBuffer::control(1);
        clock_high.samples[0] = 1.0;
        let clock_low = SignalBuffer::control(1);
        let mut outputs = vec![
            SignalBuffer::control(1),
            SignalBuffer::control(1),
            SignalBuffer::control(1),
            SignalBuffer::control(1),
            SignalBuffer::control(1),
        ];
        let ctx = ProcessContext::new(44100.0, 1);

        // Initial state: step 0
        assert_eq!(seq.current_step(), 0);

        // First clock pulse: backward starts from the last step
        seq.process(&[&clock_high], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 3);

        // Clock low (no advance)
        seq.process(&[&clock_low], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 3);

        // Second clock pulse: backward from 3 goes to 2
        seq.process(&[&clock_high], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 2);

        // Third clock pulse: backward from 2 goes to 1
        seq.process(&[&clock_low], &mut outputs, &params, &ctx);
        seq.process(&[&clock_high], &mut outputs, &params, &ctx);
        assert_eq!(seq.current_step(), 1);
    }

    /// Runs the sequencer at 1 kHz (a sample a millisecond) for `ms`, with a
    /// 1 ms clock pulse at each time in `clocks` and a reset at each time in
    /// `resets`, and returns the Gate output.
    fn gate_over(params: &[f32], ms: usize, clocks: &[usize], resets: &[usize]) -> Vec<f32> {
        let mut seq = StepSequencer::new();
        seq.prepare(1000.0, ms);
        let mut clock = SignalBuffer::control(ms);
        let mut reset = SignalBuffer::control(ms);
        for &t in clocks {
            clock.samples[t] = 1.0;
        }
        for &t in resets {
            reset.samples[t] = 1.0;
        }
        let mut outputs: Vec<SignalBuffer> = (0..5).map(|_| SignalBuffer::control(ms)).collect();
        seq.process(&[&clock, &reset], &mut outputs, params, &ProcessContext::new(1000.0, ms));
        outputs[1].samples.clone()
    }

    /// Times the gate rises.
    fn rises(gate: &[f32]) -> Vec<usize> {
        (0..gate.len()).filter(|&t| gate[t] > 0.5 && (t == 0 || gate[t - 1] < 0.5)).collect()
    }

    /// How long the gate stays high from `t`.
    fn high_for(gate: &[f32], t: usize) -> usize {
        gate[t..].iter().take_while(|&&g| g > 0.5).count()
    }

    #[test]
    fn test_gate_length_is_a_share_of_the_step() {
        // 60 BPM in quarter notes is a clock a second; 50% is 500 ms
        let params = params_with(8.0, 0.0, 50.0);
        let gate = gate_over(&params, 4000, &[0, 1000, 2000, 3000], &[]);
        assert_eq!(rises(&gate), [0, 1000, 2000, 3000]);
        // Before a second clock there's no step to measure: 50% of 100 ms
        assert_eq!(high_for(&gate, 0), 50);
        assert_eq!(high_for(&gate, 1000), 500);
        assert_eq!(high_for(&gate, 2000), 500);
    }

    #[test]
    fn test_gates_follow_a_swung_clock() {
        // Straight, a step would be 500 ms; swung at 66%, the steps go
        // 660, 340, 660, 340
        let params = params_with(8.0, 0.0, 50.0);
        let clocks = [0, 660, 1000, 1660, 2000, 2660, 3000, 3660];
        let gate = gate_over(&params, 4000, &clocks, &[]);
        assert_eq!(rises(&gate), clocks);
        // Once the rhythm shows, each note is half of its own step: long on
        // the grid, short on the swung step
        assert_eq!(high_for(&gate, 2000), 330);
        assert_eq!(high_for(&gate, 2660), 170);
        assert_eq!(high_for(&gate, 3000), 330);
    }

    #[test]
    fn test_fixed_gate_mode_keeps_100_ms() {
        let mut params = params_with(8.0, 0.0, 50.0);
        params[StepSequencer::PARAM_GATE_MODE] = GateMode::Fixed as usize as f32;
        let gate = gate_over(&params, 3000, &[0, 1000, 2000], &[]);
        assert_eq!(high_for(&gate, 1000), 50);
        assert_eq!(high_for(&gate, 2000), 50);

        // Notes that overlap the next clock run together, as they always did
        params[StepSequencer::PARAM_GATE_LENGTH] = 99.0;
        let gate = gate_over(&params, 300, &[0, 60, 120], &[]);
        assert_eq!(rises(&gate), [0]);
        assert_eq!(high_for(&gate, 0), 120 + 99);
    }

    #[test]
    fn test_gate_follows_the_tempo() {
        let params = params_with(8.0, 0.0, 50.0);
        let gate = gate_over(&params, 2000, &[0, 1000, 1200, 1400], &[]);
        // Step 2's 500 ms gate is still up at 1200, so step 3 dips it first
        assert_eq!(gate[1200], 0.0);
        assert_eq!(high_for(&gate, 1400), 100, "the step got shorter, so did the gate");
    }

    #[test]
    fn test_full_gate_holds_until_the_next_clock_and_retriggers() {
        let mut params = params_with(4.0, 0.0, 100.0);
        params[StepSequencer::step_gate_param(2)] = 0.0;
        let gate = gate_over(&params, 5000, &[0, 1000, 2000, 3000, 4000], &[]);

        // Step 2 holds right up to step 3's clock, which is a rest
        assert_eq!(high_for(&gate, 1000), 1000);
        assert_eq!(gate[2000], 0.0);
        // Step 4 holds into step 1, which starts a new note: the gate dips
        // for one sample so an envelope sees a new rising edge
        assert_eq!(high_for(&gate, 3000), 1000);
        assert_eq!(gate[4000], 0.0);
        assert_eq!(gate[4001], 1.0);
        assert_eq!(rises(&gate), [0, 1000, 3000, 4001]);
    }

    #[test]
    fn test_tie_carries_the_gate_without_retriggering() {
        let mut params = params_with(4.0, 0.0, 50.0);
        params[StepSequencer::step_tie_param(1)] = 1.0;
        params[StepSequencer::step_pitch_param(2)] = 67.0;
        let gate = gate_over(&params, 4000, &[0, 1000, 2000, 3000], &[]);

        // Step 2 ties into step 3: one rising edge for both, held through
        // step 2 and then for step 3's own 50%
        assert_eq!(rises(&gate), [0, 1000, 3000]);
        assert_eq!(high_for(&gate, 1000), 1500);
    }

    #[test]
    fn test_envelope_is_not_retriggered_across_a_tie() {
        use crate::modules::envelope::AdsrEnvelope;

        // A rest, then step 2 tied into step 3
        let mut params = params_with(3.0, 0.0, 50.0);
        params[StepSequencer::step_gate_param(0)] = 0.0;
        params[StepSequencer::step_tie_param(1)] = 1.0;
        let gate = gate_over(&params, 3000, &[0, 1000, 2000], &[]);

        // A 1.5 s attack from step 2 crosses into step 3. Any dip in the
        // gate would start a release, and a retrigger would restart the
        // attack from where it was and reach the peak late
        let mut env = AdsrEnvelope::new();
        env.prepare(1000.0, 3000);
        let mut env_params: Vec<f32> = env.parameters().iter().map(|p| p.default).collect();
        for (i, p) in env.parameters().iter().enumerate() {
            match p.name {
                "Attack" => env_params[i] = 1.5,
                "Sustain" => env_params[i] = 1.0,
                _ => {}
            }
        }
        let mut gate_buf = SignalBuffer::control(3000);
        gate_buf.samples.copy_from_slice(&gate);
        let mut out: Vec<SignalBuffer> = (0..env.ports().iter().filter(|p| p.is_output()).count())
            .map(|_| SignalBuffer::control(3000))
            .collect();
        env.process(&[&gate_buf], &mut out, &env_params, &ProcessContext::new(1000.0, 3000));
        let level = &out[0].samples;
        assert!((1001..2500).all(|t| level[t] >= level[t - 1]), "rises without a break across the tie");
        assert!(level[2499] > 0.95, "reaches the peak on time: {}", level[2499]);
    }

    #[test]
    fn test_held_gate_ends_when_the_clock_stops() {
        let params = params_with(4.0, 0.0, 100.0);
        // The clock stops after its third pulse
        let gate = gate_over(&params, 5000, &[0, 1000, 2000], &[]);
        assert_eq!(high_for(&gate, 2001), 1999, "held for two steps, then let go");
        assert!(gate[4000..].iter().all(|&g| g == 0.0));
    }

    #[test]
    fn test_reset_does_not_measure_a_stopped_clock() {
        let params = params_with(4.0, 0.0, 50.0);
        // Stops for 3 s and restarts with a reset, at the same tempo
        let gate = gate_over(&params, 6000, &[0, 1000, 4000, 5000], &[4000]);
        assert_eq!(high_for(&gate, 4000), 500);
    }

    #[test]
    fn test_sequencer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<StepSequencer>();
    }

    #[test]
    fn test_sequencer_default() {
        let seq = StepSequencer::default();
        assert_eq!(seq.info().id, "seq.step");
    }

    #[test]
    fn test_note_to_name() {
        assert_eq!(note_to_name(60), "C4");
        assert_eq!(note_to_name(69), "A4");
        assert_eq!(note_to_name(72), "C5");
        assert_eq!(note_to_name(48), "C3");
    }

    #[test]
    fn test_sequencer_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<StepSequencer>();

        assert!(registry.contains("seq.step"));

        let module = registry.create("seq.step");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "seq.step");
        assert_eq!(module.info().name, "Step Sequencer");
    }
}
