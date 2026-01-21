//! Clock module.
//!
//! Generates periodic gate triggers for driving envelopes and creating rhythmic patterns.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// Clock division values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockDivision {
    /// Whole note (4 beats)
    Whole = 0,
    /// Half note (2 beats)
    Half = 1,
    /// Quarter note (1 beat)
    Quarter = 2,
    /// Eighth note (0.5 beats)
    Eighth = 3,
    /// Sixteenth note (0.25 beats)
    Sixteenth = 4,
}

impl ClockDivision {
    /// Convert from parameter value (0-4) to division.
    pub fn from_param(value: f32) -> Self {
        match value as usize {
            0 => ClockDivision::Whole,
            1 => ClockDivision::Half,
            2 => ClockDivision::Quarter,
            3 => ClockDivision::Eighth,
            4 => ClockDivision::Sixteenth,
            _ => ClockDivision::Quarter,
        }
    }

    /// Get the beat multiplier for this division.
    /// Quarter note = 1.0 beat, whole = 4.0, sixteenth = 0.25
    pub fn beat_multiplier(&self) -> f32 {
        match self {
            ClockDivision::Whole => 4.0,
            ClockDivision::Half => 2.0,
            ClockDivision::Quarter => 1.0,
            ClockDivision::Eighth => 0.5,
            ClockDivision::Sixteenth => 0.25,
        }
    }
}

/// A clock module that generates periodic gate triggers.
///
/// Essential for testing envelope modules and creating rhythmic patterns
/// without external input.
///
/// # Ports
///
/// - **Sync** (Gate, Input): External clock sync input (resets phase on rising edge).
/// - **Gate** (Gate, Output): Periodic gate output (0.0 or 1.0).
///
/// # Parameters
///
/// - **Tempo** (20-300 BPM): Speed of the clock.
/// - **Gate Length** (1-99%): Duration of the gate high as percentage of beat.
/// - **Division** (0-4): Note division (whole, half, quarter, eighth, sixteenth).
/// - **Run** (toggle): Whether the clock is running.
pub struct Clock {
    /// Current phase within the beat cycle (0.0 to 1.0).
    phase: f32,
    /// Previous sync state for edge detection.
    prev_sync: bool,
    /// Sample rate from last prepare() call.
    sample_rate: f32,
    /// Port definitions.
    ports: Vec<PortDefinition>,
    /// Parameter definitions.
    parameters: Vec<ParameterDefinition>,
}

impl Clock {
    /// Creates a new Clock.
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            prev_sync: false,
            sample_rate: 44100.0,
            ports: vec![
                // Input port
                PortDefinition::input_with_default("sync", "Sync", SignalType::Gate, 0.0),
                // Output port
                PortDefinition::output("gate", "Gate", SignalType::Gate),
            ],
            parameters: vec![
                // Tempo in BPM
                ParameterDefinition::new(
                    "tempo",
                    "Tempo",
                    20.0,
                    300.0,
                    120.0,
                    ParameterDisplay::linear("BPM"),
                ),
                // Gate length as percentage
                ParameterDefinition::new(
                    "gate_length",
                    "Gate Length",
                    1.0,
                    99.0,
                    50.0,
                    ParameterDisplay::linear("%"),
                ),
                // Division (discrete: whole, half, quarter, eighth, sixteenth)
                ParameterDefinition::choice(
                    "division",
                    "Division",
                    &["1", "1/2", "1/4", "1/8", "1/16"],
                    2, // Default to quarter note
                ),
                // Run toggle
                ParameterDefinition::toggle("run", "Run", true),
            ],
        }
    }

    /// Port index constants.
    const PORT_SYNC: usize = 0;
    const PORT_GATE: usize = 0;

    /// Parameter index constants.
    const PARAM_TEMPO: usize = 0;
    const PARAM_GATE_LENGTH: usize = 1;
    const PARAM_DIVISION: usize = 2;
    const PARAM_RUN: usize = 3;

    /// Sync threshold for detecting high/low states.
    const SYNC_THRESHOLD: f32 = 0.5;
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Clock {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "util.clock",
            name: "Clock",
            category: ModuleCategory::Utility,
            description: "Periodic gate trigger generator",
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
        let tempo = params[Self::PARAM_TEMPO];
        let gate_length_percent = params[Self::PARAM_GATE_LENGTH] / 100.0;
        let division = ClockDivision::from_param(params[Self::PARAM_DIVISION]);
        let is_running = params[Self::PARAM_RUN] > 0.5;

        // Get sync input
        let sync_in = inputs.get(Self::PORT_SYNC);

        // Get output buffer
        let output = &mut outputs[Self::PORT_GATE];

        // Calculate timing
        // Beats per second = BPM / 60
        // Samples per beat = sample_rate / beats_per_second = sample_rate * 60 / BPM
        // Apply division multiplier
        let beats_per_second = tempo / 60.0;
        let samples_per_cycle = self.sample_rate / beats_per_second * division.beat_multiplier();
        let phase_increment = 1.0 / samples_per_cycle;

        // Process each sample
        for i in 0..context.block_size {
            // Check for sync reset
            let sync_value = sync_in
                .map(|buf| buf.samples.get(i).copied().unwrap_or(0.0))
                .unwrap_or(0.0);
            let sync_high = sync_value > Self::SYNC_THRESHOLD;
            let sync_rising = sync_high && !self.prev_sync;
            self.prev_sync = sync_high;

            // Reset phase on sync rising edge
            if sync_rising {
                self.phase = 0.0;
            }

            // Generate gate output
            let gate_out = if is_running && self.phase < gate_length_percent {
                1.0
            } else {
                0.0
            };
            output.samples[i] = gate_out;

            // Advance phase if running
            if is_running {
                self.phase += phase_increment;

                // Wrap phase
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
            }
        }
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_sync = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_info() {
        let clock = Clock::new();
        assert_eq!(clock.info().id, "util.clock");
        assert_eq!(clock.info().name, "Clock");
        assert_eq!(clock.info().category, ModuleCategory::Utility);
    }

    #[test]
    fn test_clock_ports() {
        let clock = Clock::new();
        let ports = clock.ports();

        assert_eq!(ports.len(), 2);

        // Sync input
        assert!(ports[0].is_input());
        assert_eq!(ports[0].id, "sync");
        assert_eq!(ports[0].signal_type, SignalType::Gate);

        // Gate output
        assert!(ports[1].is_output());
        assert_eq!(ports[1].id, "gate");
        assert_eq!(ports[1].signal_type, SignalType::Gate);
    }

    #[test]
    fn test_clock_parameters() {
        let clock = Clock::new();
        let params = clock.parameters();

        assert_eq!(params.len(), 4);

        // Tempo
        assert_eq!(params[0].id, "tempo");
        assert_eq!(params[0].min, 20.0);
        assert_eq!(params[0].max, 300.0);
        assert_eq!(params[0].default, 120.0);

        // Gate Length
        assert_eq!(params[1].id, "gate_length");
        assert_eq!(params[1].min, 1.0);
        assert_eq!(params[1].max, 99.0);
        assert_eq!(params[1].default, 50.0);

        // Division
        assert_eq!(params[2].id, "division");
        assert_eq!(params[2].default, 2.0); // Quarter note

        // Run
        assert_eq!(params[3].id, "run");
        assert_eq!(params[3].default, 1.0); // Running by default
    }

    #[test]
    fn test_clock_division_conversion() {
        assert_eq!(ClockDivision::from_param(0.0), ClockDivision::Whole);
        assert_eq!(ClockDivision::from_param(1.0), ClockDivision::Half);
        assert_eq!(ClockDivision::from_param(2.0), ClockDivision::Quarter);
        assert_eq!(ClockDivision::from_param(3.0), ClockDivision::Eighth);
        assert_eq!(ClockDivision::from_param(4.0), ClockDivision::Sixteenth);
        assert_eq!(ClockDivision::from_param(99.0), ClockDivision::Quarter); // Out of range
    }

    #[test]
    fn test_clock_division_multipliers() {
        assert_eq!(ClockDivision::Whole.beat_multiplier(), 4.0);
        assert_eq!(ClockDivision::Half.beat_multiplier(), 2.0);
        assert_eq!(ClockDivision::Quarter.beat_multiplier(), 1.0);
        assert_eq!(ClockDivision::Eighth.beat_multiplier(), 0.5);
        assert_eq!(ClockDivision::Sixteenth.beat_multiplier(), 0.25);
    }

    #[test]
    fn test_clock_stopped_outputs_zero() {
        let mut clock = Clock::new();
        clock.prepare(44100.0, 256);

        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);

        // Run = false (0.0)
        clock.process(&[], &mut outputs, &[120.0, 50.0, 2.0, 0.0], &ctx);

        // All outputs should be zero when stopped
        assert!(outputs[0].samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_clock_generates_gates() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // 120 BPM, 50% gate, quarter note, running
        // At 120 BPM: 2 beats per second, so 22050 samples per beat
        clock.process(&[], &mut outputs, &[120.0, 50.0, 2.0, 1.0], &ctx);

        // Should have both high and low values
        let has_high = outputs[0].samples.iter().any(|&s| s == 1.0);
        let has_low = outputs[0].samples.iter().any(|&s| s == 0.0);
        assert!(has_high, "Clock should output high gates");
        assert!(has_low, "Clock should output low between gates");

        // Output should only be 0.0 or 1.0
        for &sample in &outputs[0].samples {
            assert!(
                sample == 0.0 || sample == 1.0,
                "Gate output should be 0 or 1, got {}",
                sample
            );
        }
    }

    #[test]
    fn test_clock_timing_accuracy() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // 60 BPM = 1 beat per second = 44100 samples per beat
        // Quarter note division, 50% gate length
        // So gate should be high for ~22050 samples, then low for ~22050
        clock.process(&[], &mut outputs, &[60.0, 50.0, 2.0, 1.0], &ctx);

        // Count high samples in first beat
        let high_count = outputs[0].samples[..44100]
            .iter()
            .filter(|&&s| s == 1.0)
            .count();

        // Should be approximately 50% (allowing some tolerance for edge cases)
        let expected = 22050;
        let tolerance = 100; // Allow small timing variance
        assert!(
            (high_count as i32 - expected as i32).abs() < tolerance,
            "Expected ~{} high samples, got {}",
            expected,
            high_count
        );
    }

    #[test]
    fn test_clock_gate_length() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 44100);

        // Test with 25% gate length
        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // 60 BPM, 25% gate, quarter note
        clock.process(&[], &mut outputs, &[60.0, 25.0, 2.0, 1.0], &ctx);

        let high_count = outputs[0].samples[..44100]
            .iter()
            .filter(|&&s| s == 1.0)
            .count();

        // Should be approximately 25%
        let expected = 11025;
        let tolerance = 100;
        assert!(
            (high_count as i32 - expected as i32).abs() < tolerance,
            "Expected ~{} high samples for 25% gate, got {}",
            expected,
            high_count
        );
    }

    #[test]
    fn test_clock_division_timing() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 88200); // 2 seconds

        let mut outputs = vec![SignalBuffer::control(88200)];
        let ctx = ProcessContext::new(sample_rate, 88200);

        // 60 BPM, eighth notes (0.5 beats)
        // At 60 BPM: 1 beat/sec, eighth = 0.5 beats = 0.5 sec = 22050 samples per cycle
        // In 2 seconds, should get 4 complete cycles
        clock.process(&[], &mut outputs, &[60.0, 50.0, 3.0, 1.0], &ctx);

        // Count rising edges (transitions from 0 to 1)
        let mut rising_edges = 0;
        let mut prev = 0.0;
        for &sample in &outputs[0].samples {
            if sample == 1.0 && prev == 0.0 {
                rising_edges += 1;
            }
            prev = sample;
        }

        // Should have 4 rising edges (4 eighth notes in 2 seconds at 60 BPM)
        // First rising edge is at start, so we should see 4 total
        assert!(
            rising_edges >= 3 && rising_edges <= 5,
            "Expected ~4 rising edges for eighth notes, got {}",
            rising_edges
        );
    }

    #[test]
    fn test_clock_sync_reset() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 1000);

        // Run clock to advance phase
        let mut outputs = vec![SignalBuffer::control(1000)];
        let ctx = ProcessContext::new(sample_rate, 1000);
        clock.process(&[], &mut outputs, &[120.0, 50.0, 2.0, 1.0], &ctx);

        // Now send a sync pulse
        let mut sync = SignalBuffer::control(100);
        sync.samples[50] = 1.0; // Rising edge at sample 50

        let mut outputs2 = vec![SignalBuffer::control(100)];
        let ctx2 = ProcessContext::new(sample_rate, 100);
        clock.process(&[&sync], &mut outputs2, &[120.0, 50.0, 2.0, 1.0], &ctx2);

        // After sync, the gate should be high (phase reset to 0, which is < gate_length)
        assert_eq!(
            outputs2[0].samples[51], 1.0,
            "Gate should be high immediately after sync"
        );
    }

    #[test]
    fn test_clock_reset() {
        let mut clock = Clock::new();
        clock.prepare(44100.0, 256);

        // Advance the clock
        let mut outputs = vec![SignalBuffer::control(256)];
        let ctx = ProcessContext::new(44100.0, 256);
        clock.process(&[], &mut outputs, &[120.0, 50.0, 2.0, 1.0], &ctx);

        // Reset
        clock.reset();

        // Phase should be back to 0, so first output should be high (0 < 0.5 gate length)
        let mut outputs2 = vec![SignalBuffer::control(1)];
        let ctx2 = ProcessContext::new(44100.0, 1);
        clock.process(&[], &mut outputs2, &[120.0, 50.0, 2.0, 1.0], &ctx2);

        assert_eq!(
            outputs2[0].samples[0], 1.0,
            "First sample after reset should be high"
        );
    }

    #[test]
    fn test_clock_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Clock>();
    }

    #[test]
    fn test_clock_default() {
        let clock = Clock::default();
        assert_eq!(clock.info().id, "util.clock");
    }

    #[test]
    fn test_clock_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Clock>();

        assert!(registry.contains("util.clock"));

        let module = registry.create("util.clock");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "util.clock");
        assert_eq!(module.info().name, "Clock");
        assert_eq!(module.ports().len(), 2);
        assert_eq!(module.parameters().len(), 4);
    }

    #[test]
    fn test_clock_fast_tempo() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 44100);

        let mut outputs = vec![SignalBuffer::control(44100)];
        let ctx = ProcessContext::new(sample_rate, 44100);

        // 300 BPM = 5 beats per second, sixteenth notes = 20 triggers per second
        clock.process(&[], &mut outputs, &[300.0, 50.0, 4.0, 1.0], &ctx);

        // Count rising edges
        let mut rising_edges = 0;
        let mut prev = 0.0;
        for &sample in &outputs[0].samples {
            if sample == 1.0 && prev == 0.0 {
                rising_edges += 1;
            }
            prev = sample;
        }

        // Should have approximately 20 triggers in 1 second
        assert!(
            rising_edges >= 18 && rising_edges <= 22,
            "Expected ~20 triggers at 300 BPM sixteenths, got {}",
            rising_edges
        );
    }

    #[test]
    fn test_clock_slow_tempo() {
        let mut clock = Clock::new();
        let sample_rate = 44100.0;
        clock.prepare(sample_rate, 88200); // 2 seconds

        let mut outputs = vec![SignalBuffer::control(88200)];
        let ctx = ProcessContext::new(sample_rate, 88200);

        // 30 BPM = 0.5 beats per second, whole notes = 1 trigger per 8 seconds
        // In 2 seconds, should see only partial first cycle
        clock.process(&[], &mut outputs, &[30.0, 50.0, 0.0, 1.0], &ctx);

        // Count rising edges - should be just 1 (the initial start)
        let mut rising_edges = 0;
        let mut prev = 0.0;
        for &sample in &outputs[0].samples {
            if sample == 1.0 && prev == 0.0 {
                rising_edges += 1;
            }
            prev = sample;
        }

        assert!(
            rising_edges <= 2,
            "Expected 1-2 triggers at 30 BPM whole notes in 2 seconds, got {}",
            rising_edges
        );
    }
}
