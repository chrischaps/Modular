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
/// - **Gate Length** (1-99%): Gate duration as percentage of step length.
/// - **Step 1-16 Pitch** (0-127): MIDI note number for each step.
/// - **Step 1-16 Gate** (0/1): Gate on/off for each step.
/// - **Step 1-16 Velocity** (0-127): Velocity for each step.
pub struct StepSequencer {
    /// Current step index (0-based).
    current_step: usize,
    /// Direction for ping-pong mode (+1 or -1).
    ping_pong_direction: i32,
    /// Previous clock state for edge detection.
    prev_clock: bool,
    /// Previous reset state for edge detection.
    prev_reset: bool,
    /// Gate timer (samples remaining in gate).
    gate_timer: usize,
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
            PortDefinition::input_with_default("clock", "Clock", SignalType::Gate, 0.0),
            PortDefinition::input_with_default("reset", "Reset", SignalType::Gate, 0.0),
            PortDefinition::input_with_default("run", "Run", SignalType::Gate, 1.0),
            // Output ports
            PortDefinition::output("pitch", "Pitch", SignalType::Control),
            PortDefinition::output("gate", "Gate", SignalType::Gate),
            PortDefinition::output("velocity", "Velocity", SignalType::Control),
            PortDefinition::output("step_out", "Step", SignalType::Control),
            PortDefinition::output("eoc", "EOC", SignalType::Gate),
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
            ),
            ParameterDefinition::choice(
                "direction",
                "Direction",
                &["Fwd", "Bwd", "P-P", "Rnd"],
                0,
            ),
            ParameterDefinition::new(
                "gate_length",
                "Gate Length",
                1.0,
                99.0,
                50.0,
                ParameterDisplay::linear("%"),
            ),
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
            ));

            // Gate: on/off toggle (default on)
            parameters.push(ParameterDefinition::toggle(
                STEP_GATE_IDS[i],
                STEP_GATE_NAMES[i],
                true,
            ));

            // Velocity: 0-127 (default 100)
            parameters.push(ParameterDefinition::new(
                STEP_VELOCITY_IDS[i],
                STEP_VELOCITY_NAMES[i],
                0.0,
                127.0,
                100.0,
                ParameterDisplay::linear(""),
            ));
        }

        Self {
            current_step: 0,
            ping_pong_direction: 1,
            prev_clock: false,
            prev_reset: false,
            gate_timer: 0,
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
                self.current_step = 0;
                self.ping_pong_direction = 1;
                self.gate_timer = 0;
            }

            // Handle clock advance
            if clock_rising && is_running {
                let hit_end = self.advance_step(num_steps, direction);

                // Start gate timer based on gate length
                // We don't know the actual step duration, so use a fixed gate time
                // This will be retriggered on each clock, so gate_length controls duty cycle
                let gate_samples = (self.sample_rate * 0.1 * gate_length_percent) as usize;

                // Check if current step has gate enabled
                let step_gate = params[Self::step_gate_param(self.current_step)] > 0.5;
                if step_gate {
                    self.gate_timer = gate_samples.max(1);
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

            // Gate output: high if timer > 0 and step gate is enabled
            let gate_active = self.gate_timer > 0 && step_gate_enabled;
            outputs[Self::PORT_GATE].samples[i] = if gate_active { 1.0 } else { 0.0 };

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
        }
    }

    fn reset(&mut self) {
        self.current_step = 0;
        self.ping_pong_direction = 1;
        self.prev_clock = false;
        self.prev_reset = false;
        self.gate_timer = 0;
        self.eoc_timer = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // 3 global + 16 steps * 3 params each = 51 parameters
        assert_eq!(params.len(), 3 + MAX_STEPS * 3);

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
        let mut params = vec![8.0, 0.0, 50.0];
        // Add step params (all defaults: C4, gate on, vel 100)
        for _ in 0..MAX_STEPS {
            params.push(60.0); // pitch
            params.push(1.0);  // gate on
            params.push(100.0); // velocity
        }

        // Process with clock pulse
        seq.process(&[&clock], &mut outputs, &params, &ctx);

        // After clock pulse, should have advanced to step 1
        assert_eq!(seq.current_step(), 1);
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

        let mut params = vec![8.0, 0.0, 50.0];
        for _ in 0..MAX_STEPS {
            params.push(60.0);
            params.push(1.0);
            params.push(100.0);
        }

        seq.process(&[&clock, &reset], &mut outputs, &params, &ctx);

        // Should be back at step 0
        assert_eq!(seq.current_step(), 0);
    }

    #[test]
    fn test_sequencer_backward_direction() {
        let mut seq = StepSequencer::new();
        seq.prepare(44100.0, 1);

        let mut params = vec![4.0, 1.0, 50.0]; // 4 steps, backward
        for _ in 0..MAX_STEPS {
            params.push(60.0);
            params.push(1.0);
            params.push(100.0);
        }

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

        // First clock pulse: backward from 0 wraps to 3
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
