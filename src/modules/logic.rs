//! Logic module.
//!
//! Processes gates: divides a clock by counting its edges, says where the
//! count is, combines two gates (AND, OR, XOR, NOT), and turns a control
//! voltage into a gate where it crosses a threshold. A divider that counts
//! edges can't drift from its input, so phrases built on it stay on the beat
//! at any tempo.

use crate::dsp::{
    context::ProcessContext,
    module_trait::{DspModule, ModuleCategory, ModuleInfo, Readout},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::SignalBuffer,
    ParameterDisplay, SignalType,
};

/// The largest division: eight bars of sixteenths.
pub const MAX_DIVIDE: usize = 128;

/// Divides, counts and combines gates, and compares a CV to a threshold.
///
/// # Ports
///
/// **Inputs:**
/// - **Clock** (Gate): Each rising edge counts one.
/// - **Reset** (Gate): Starts the count over: the next clock is count 0.
/// - **A**, **B** (Gate): The two gates the logic outputs combine.
/// - **CV** (Control): The voltage Above compares to Threshold.
///
/// **Outputs:**
/// - **Trig** (Gate): The clock's own pulse, on the counts that fire.
/// - **Gate** (Gate): Opens on a count that fires and stays open for
///   Length clocks.
/// - **Count** (Control): Where the count is, as a 0 to 1 ramp.
/// - **AND**, **OR**, **XOR**, **NOT A** (Gate): A and B combined.
/// - **Above** (Gate): High while CV is above Threshold.
///
/// # Parameters
///
/// - **Divide** (1-128): Fires once every this many clocks.
/// - **Offset** (0-127): Which count fires, from 0, the first clock after a
///   reset.
/// - **Length** (1-128): How many clocks Gate stays open. At Divide or more
///   it never closes, dipping for a sample to mark each new count.
/// - **Threshold** (-1 to 1): Where Above switches, with a little
///   hysteresis so a noisy CV doesn't chatter.
pub struct Logic {
    /// The current count, `None` before the first clock and after a reset.
    count: Option<usize>,
    /// Clocks left before Gate closes; 0 when it's closed.
    gate_remaining: usize,
    /// The last sample's Gate, to dip for a sample when a count fires while
    /// it's still open.
    gate_was_high: bool,
    /// Whether the clock pulse now high fired: Trig follows it while it lasts.
    trig_active: bool,
    /// The clock's period, from its last two rising edges.
    period: Option<u64>,
    /// Samples since the clock last rose, `None` until it has.
    since_clock: Option<u64>,
    prev_clock: bool,
    prev_reset: bool,
    /// Whether Above is high.
    above: bool,
    /// The last CV sample, for the display.
    last_cv: f32,
    ports: Vec<PortDefinition>,
    parameters: Vec<ParameterDefinition>,
}

impl Logic {
    /// Creates a Logic module dividing by 4.
    pub fn new() -> Self {
        Self {
            count: None,
            gate_remaining: 0,
            gate_was_high: false,
            trig_active: false,
            period: None,
            since_clock: None,
            prev_clock: false,
            prev_reset: false,
            above: false,
            last_cv: 0.0,
            ports: vec![
                PortDefinition::input_with_default("clock", "Clock", SignalType::Gate, 0.0).describe("Each rising edge counts one"),
                PortDefinition::input_with_default("reset", "Reset", SignalType::Gate, 0.0).describe("A rising edge starts the count over: the next clock is count 0"),
                PortDefinition::input_with_default("a", "A", SignalType::Gate, 0.0).describe("First gate for AND, OR, XOR and NOT A"),
                PortDefinition::input_with_default("b", "B", SignalType::Gate, 0.0).describe("Second gate for AND, OR and XOR"),
                PortDefinition::input_with_default("cv", "CV", SignalType::Control, 0.0).describe("A voltage to compare with Threshold: Above is high while it's over"),
                PortDefinition::output("trig", "Trig", SignalType::Gate).describe("The clock's own pulse, once every Divide clocks"),
                PortDefinition::output("gate", "Gate", SignalType::Gate).describe("Opens once every Divide clocks and stays open for Length clocks"),
                PortDefinition::output("count", "Count", SignalType::Control).describe("Where the count is, as a 0 to 1 ramp"),
                PortDefinition::output("and", "AND", SignalType::Gate).describe("High while A and B are both high"),
                PortDefinition::output("or", "OR", SignalType::Gate).describe("High while A or B is high"),
                PortDefinition::output("xor", "XOR", SignalType::Gate).describe("High while exactly one of A and B is high"),
                PortDefinition::output("not_a", "NOT A", SignalType::Gate).describe("A turned upside down: high while A is low"),
                PortDefinition::output("above", "Above", SignalType::Gate).describe("High while CV is above Threshold: any control voltage made a gate"),
            ],
            parameters: vec![
                ParameterDefinition::new("divide", "Divide", 1.0, MAX_DIVIDE as f32, 4.0, ParameterDisplay::stepped(""))
                    .describe("Fires once every this many clocks"),
                ParameterDefinition::new("offset", "Offset", 0.0, (MAX_DIVIDE - 1) as f32, 0.0, ParameterDisplay::stepped(""))
                    .describe("Which count fires; 0 is the first clock after a reset"),
                ParameterDefinition::new("length", "Length", 1.0, MAX_DIVIDE as f32, 1.0, ParameterDisplay::stepped(""))
                    .describe("How many clocks Gate stays open"),
                ParameterDefinition::new("threshold", "Threshold", -1.0, 1.0, 0.5, ParameterDisplay::linear(""))
                    .describe("Where Above switches on and off"),
            ],
        }
    }

    const PORT_CLOCK: usize = 0;
    const PORT_RESET: usize = 1;
    const PORT_A: usize = 2;
    const PORT_B: usize = 3;
    const PORT_CV: usize = 4;

    const OUT_TRIG: usize = 0;
    const OUT_GATE: usize = 1;
    const OUT_COUNT: usize = 2;
    const OUT_AND: usize = 3;
    const OUT_OR: usize = 4;
    const OUT_XOR: usize = 5;
    const OUT_NOT_A: usize = 6;
    const OUT_ABOVE: usize = 7;

    const PARAM_DIVIDE: usize = 0;
    const PARAM_OFFSET: usize = 1;
    const PARAM_LENGTH: usize = 2;
    const PARAM_THRESHOLD: usize = 3;

    /// Readout slots: the count (-1 before the first clock), Gate, the CV
    /// and Above.
    pub const READOUT_COUNT: usize = 0;
    pub const READOUT_GATE: usize = 1;
    pub const READOUT_CV: usize = 2;
    pub const READOUT_ABOVE: usize = 3;

    const GATE_THRESHOLD: f32 = 0.5;

    /// How far past Threshold CV must go to switch Above, either way.
    pub const HYSTERESIS: f32 = 0.01;

    /// An open Gate lets go when no clock has come for this many periods,
    /// so a stopped clock can't hold it open for good.
    const STALL_PERIODS: u64 = 2;

    /// Divide, Offset and Length from the knobs, as counts.
    fn counts(params: &[f32]) -> (usize, usize, usize) {
        let divide = (params[Self::PARAM_DIVIDE].round() as usize).clamp(1, MAX_DIVIDE);
        let offset = (params[Self::PARAM_OFFSET].round().max(0.0) as usize) % divide;
        let length = (params[Self::PARAM_LENGTH].round() as usize).clamp(1, divide);
        (divide, offset, length)
    }
}

impl Default for Logic {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Logic {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "util.logic",
            name: "Logic",
            category: ModuleCategory::Utility,
            description: "Divide and count a clock, combine gates, turn a CV into a gate",
        };
        &INFO
    }

    fn ports(&self) -> &[PortDefinition] {
        &self.ports
    }

    fn parameters(&self) -> &[ParameterDefinition] {
        &self.parameters
    }

    fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {
        self.period = None;
        self.since_clock = None;
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        let (divide, offset, length) = Self::counts(params);
        let threshold = params[Self::PARAM_THRESHOLD];
        let at = |port: usize, i: usize| inputs.get(port).map_or(0.0, |buf| buf.samples.get(i).copied().unwrap_or(0.0));

        for i in 0..context.block_size {
            let clock = at(Self::PORT_CLOCK, i) > Self::GATE_THRESHOLD;
            let reset = at(Self::PORT_RESET, i) > Self::GATE_THRESHOLD;
            let clock_rising = clock && !self.prev_clock;
            let reset_rising = reset && !self.prev_reset;
            self.prev_clock = clock;
            self.prev_reset = reset;

            // A reset ends the phrase: the gate closes, and the next clock,
            // even one on this same sample, is count 0
            if reset_rising {
                self.count = None;
                self.gate_remaining = 0;
            }

            let mut fired = false;
            if clock_rising {
                if let Some(samples) = self.since_clock {
                    self.period = Some(samples.max(1));
                }
                self.since_clock = Some(0);

                let count = self.count.map_or(0, |count| (count + 1) % divide);
                self.count = Some(count);
                self.gate_remaining = self.gate_remaining.saturating_sub(1);
                if count == offset {
                    fired = true;
                    self.gate_remaining = length;
                }
            }
            if !clock {
                self.trig_active = false;
            }
            self.trig_active |= fired;

            // A stopped clock lets the gate go
            if let (Some(period), Some(since)) = (self.period, self.since_clock) {
                if since > period * Self::STALL_PERIODS {
                    self.gate_remaining = 0;
                }
            }

            // A count that fires over an open gate dips it for a sample, so
            // whatever it drives sees a new rising edge
            let gate = self.gate_remaining > 0 && !(fired && self.gate_was_high);
            self.gate_was_high = gate;

            let count = self.count.unwrap_or(0);
            outputs[Self::OUT_TRIG].samples[i] = if self.trig_active { 1.0 } else { 0.0 };
            outputs[Self::OUT_GATE].samples[i] = if gate { 1.0 } else { 0.0 };
            outputs[Self::OUT_COUNT].samples[i] = count as f32 / (divide - 1).max(1) as f32;

            let a = at(Self::PORT_A, i) > Self::GATE_THRESHOLD;
            let b = at(Self::PORT_B, i) > Self::GATE_THRESHOLD;
            let level = |high: bool| if high { 1.0 } else { 0.0 };
            outputs[Self::OUT_AND].samples[i] = level(a && b);
            outputs[Self::OUT_OR].samples[i] = level(a || b);
            outputs[Self::OUT_XOR].samples[i] = level(a != b);
            outputs[Self::OUT_NOT_A].samples[i] = level(!a);

            let cv = at(Self::PORT_CV, i);
            if self.above {
                self.above = cv >= threshold - Self::HYSTERESIS;
            } else {
                self.above = cv > threshold + Self::HYSTERESIS;
            }
            self.last_cv = cv;
            outputs[Self::OUT_ABOVE].samples[i] = level(self.above);

            if let Some(since) = self.since_clock.as_mut() {
                *since = since.saturating_add(1);
            }
        }
    }

    fn reset(&mut self) {
        self.count = None;
        self.gate_remaining = 0;
        self.gate_was_high = false;
        self.trig_active = false;
        self.period = None;
        self.since_clock = None;
        self.prev_clock = false;
        self.prev_reset = false;
        self.above = false;
        self.last_cv = 0.0;
    }

    fn readout(&self, _params: &[f32]) -> Option<Readout> {
        let mut readout = Readout::default();
        readout.values[Self::READOUT_COUNT] = self.count.map_or(-1.0, |count| count as f32);
        readout.values[Self::READOUT_GATE] = if self.gate_was_high { 1.0 } else { 0.0 };
        readout.values[Self::READOUT_CV] = self.last_cv;
        readout.values[Self::READOUT_ABOVE] = if self.above { 1.0 } else { 0.0 };
        Some(readout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::Clock;

    const SAMPLE_RATE: f32 = 48000.0;
    const BLOCK: usize = 256;

    /// Divide, Offset, Length, Threshold.
    fn params(divide: usize, offset: usize, length: usize) -> [f32; 4] {
        [divide as f32, offset as f32, length as f32, 0.5]
    }

    fn outputs() -> Vec<SignalBuffer> {
        (0..8).map(|_| SignalBuffer::control(BLOCK)).collect()
    }

    /// Runs the five inputs (one value per sample; missing ones read 0)
    /// through a fresh Logic, returning all eight outputs.
    fn run(inputs: [&[f32]; 5], params: &[f32]) -> Vec<Vec<f32>> {
        let mut logic = Logic::new();
        logic.prepare(SAMPLE_RATE, BLOCK);
        let len = inputs.iter().map(|input| input.len()).max().unwrap_or(0);
        let ctx = ProcessContext::new(SAMPLE_RATE, BLOCK);
        let mut outs = outputs();
        let mut result = vec![Vec::with_capacity(len); 8];
        for start in (0..len).step_by(BLOCK) {
            let buffers: Vec<SignalBuffer> = inputs
                .iter()
                .map(|input| {
                    let mut buf = SignalBuffer::control(BLOCK);
                    for (i, sample) in buf.samples.iter_mut().enumerate() {
                        *sample = input.get(start + i).copied().unwrap_or(0.0);
                    }
                    buf
                })
                .collect();
            let refs: Vec<&SignalBuffer> = buffers.iter().collect();
            logic.process(&refs, &mut outs, params, &ctx);
            for (out, buf) in result.iter_mut().zip(&outs) {
                out.extend_from_slice(&buf.samples[..BLOCK.min(len - start)]);
            }
        }
        result
    }

    /// A clock: a `width`-sample pulse every `period` samples, `count` times.
    fn pulses(period: usize, width: usize, count: usize) -> Vec<f32> {
        (0..period * count).map(|i| if i % period < width { 1.0 } else { 0.0 }).collect()
    }

    fn rises(signal: &[f32]) -> Vec<usize> {
        (0..signal.len()).filter(|&i| signal[i] > 0.5 && (i == 0 || signal[i - 1] <= 0.5)).collect()
    }

    fn falls(signal: &[f32]) -> Vec<usize> {
        (1..signal.len()).filter(|&i| signal[i] <= 0.5 && signal[i - 1] > 0.5).collect()
    }

    #[test]
    fn test_logic_info() {
        let logic = Logic::new();
        assert_eq!(logic.info().id, "util.logic");
        assert_eq!(logic.info().category, ModuleCategory::Utility);
        assert_eq!(logic.ports().iter().filter(|p| p.is_input()).count(), 5);
        assert_eq!(logic.ports().iter().filter(|p| p.is_output()).count(), 8);
        assert_eq!(logic.parameters().len(), 4);
    }

    #[test]
    fn test_divide_fires_on_the_first_clock_then_every_nth() {
        let clock = pulses(100, 10, 12);
        let out = run([&clock, &[], &[], &[], &[]], &params(4, 0, 1));
        assert_eq!(rises(&out[Logic::OUT_TRIG]), vec![0, 400, 800]);
        assert_eq!(rises(&out[Logic::OUT_GATE]), vec![0, 400, 800]);
        // Trig is the clock's own pulse; Gate lasts one clock period
        assert_eq!(falls(&out[Logic::OUT_TRIG]), vec![10, 410, 810]);
        assert_eq!(falls(&out[Logic::OUT_GATE]), vec![100, 500, 900]);
    }

    #[test]
    fn test_offset_picks_the_count_that_fires() {
        let clock = pulses(100, 10, 12);
        let out = run([&clock, &[], &[], &[], &[]], &params(4, 2, 1));
        assert_eq!(rises(&out[Logic::OUT_TRIG]), vec![200, 600, 1000]);
        // Offsets past Divide wrap
        let wrapped = run([&clock, &[], &[], &[], &[]], &params(4, 6, 1));
        assert_eq!(rises(&wrapped[Logic::OUT_TRIG]), vec![200, 600, 1000]);
    }

    #[test]
    fn test_length_holds_the_gate_for_that_many_clocks() {
        let clock = pulses(100, 10, 12);
        let out = run([&clock, &[], &[], &[], &[]], &params(8, 0, 3));
        assert_eq!(rises(&out[Logic::OUT_GATE]), vec![0, 800]);
        assert_eq!(falls(&out[Logic::OUT_GATE]), vec![300, 1100]);
    }

    #[test]
    fn test_full_length_gate_dips_for_a_sample_at_each_division() {
        let clock = pulses(100, 10, 9);
        let gate = &run([&clock, &[], &[], &[], &[]], &params(3, 0, 64))[Logic::OUT_GATE];
        assert_eq!(rises(gate), vec![0, 301, 601]);
        assert_eq!(falls(gate), vec![300, 600]);
    }

    #[test]
    fn test_reset_starts_the_count_over() {
        let clock = pulses(100, 10, 12);
        // A reset between clocks 5 and 6: clock 6 is count 0 again
        let mut reset = vec![0.0; 1200];
        reset[550..560].fill(1.0);
        let out = run([&clock, &reset, &[], &[], &[]], &params(4, 0, 1));
        assert_eq!(rises(&out[Logic::OUT_TRIG]), vec![0, 400, 600, 1000]);

        // A reset on the same sample as a clock makes that clock count 0
        let mut reset = vec![0.0; 1200];
        reset[500..510].fill(1.0);
        let out = run([&clock, &reset, &[], &[], &[]], &params(4, 0, 1));
        assert_eq!(rises(&out[Logic::OUT_TRIG]), vec![0, 400, 500, 900]);
    }

    #[test]
    fn test_reset_closes_an_open_gate() {
        let clock = pulses(100, 10, 12);
        let mut reset = vec![0.0; 1200];
        reset[250..260].fill(1.0);
        let gate = &run([&clock, &reset, &[], &[], &[]], &params(8, 0, 6))[Logic::OUT_GATE];
        assert_eq!(falls(gate), vec![250, 900]);
        assert_eq!(rises(gate), vec![0, 300, 1100]);
    }

    #[test]
    fn test_count_ramps_from_0_to_1() {
        let clock = pulses(100, 10, 5);
        let count = &run([&clock, &[], &[], &[], &[]], &params(5, 0, 1))[Logic::OUT_COUNT];
        for (n, expected) in [0.0, 0.25, 0.5, 0.75, 1.0].into_iter().enumerate() {
            assert_eq!(count[n * 100 + 50], expected, "count {n}");
        }
    }

    #[test]
    fn test_a_stopped_clock_lets_the_gate_go() {
        // Two clocks, then nothing: Gate (Length 8) closes two periods on
        let mut clock = pulses(100, 10, 2);
        clock.resize(1000, 0.0);
        let gate = &run([&clock, &[], &[], &[], &[]], &params(8, 0, 8))[Logic::OUT_GATE];
        assert_eq!(falls(gate), vec![301]);
    }

    #[test]
    fn test_logic_truth_table() {
        let a = [0.0, 1.0, 0.0, 1.0];
        let b = [0.0, 0.0, 1.0, 1.0];
        let out = run([&[], &[], &a, &b, &[]], &params(4, 0, 1));
        assert_eq!(out[Logic::OUT_AND], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(out[Logic::OUT_OR], [0.0, 1.0, 1.0, 1.0]);
        assert_eq!(out[Logic::OUT_XOR], [0.0, 1.0, 1.0, 0.0]);
        assert_eq!(out[Logic::OUT_NOT_A], [1.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn test_above_switches_with_hysteresis() {
        // Wobbling right at the threshold holds; crossing it clearly switches
        let cv = [0.0, 0.505, 0.52, 0.495, 0.505, 0.48, 0.495, 0.51, 0.9];
        let above = &run([&[], &[], &[], &[], &cv], &params(4, 0, 1))[Logic::OUT_ABOVE];
        assert_eq!(*above, [0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_divider_stays_exact_against_a_clock_for_ten_minutes() {
        // A 96 BPM sixteenth clock divided by 64, the Backbeat phrase: every
        // 64th clock edge, and only those, must fire, on the same sample
        let mut clock = Clock::new();
        let mut logic = Logic::new();
        clock.prepare(SAMPLE_RATE, BLOCK);
        logic.prepare(SAMPLE_RATE, BLOCK);
        let clock_params = [96.0, 50.0, 4.0, 1.0, 0.0];
        let logic_params = params(64, 0, 17);
        let ctx = ProcessContext::new(SAMPLE_RATE, BLOCK);
        let unconnected = SignalBuffer::control(BLOCK);
        let mut clock_out: Vec<SignalBuffer> = (0..3).map(|_| SignalBuffer::control(BLOCK)).collect();
        let mut outs = outputs();

        let blocks = (600.0 * SAMPLE_RATE) as usize / BLOCK;
        let (mut prev_clock, mut prev_trig, mut prev_gate) = (false, false, false);
        let (mut edges, mut fires, mut gate_opens) = (0usize, 0usize, 0usize);
        for _ in 0..blocks {
            clock.process(&[], &mut clock_out, &clock_params, &ctx);
            let gate_in = &clock_out[0];
            logic.process(&[gate_in, &unconnected, &unconnected, &unconnected, &unconnected], &mut outs, &logic_params, &ctx);
            for i in 0..BLOCK {
                let clock_high = gate_in.samples[i] > 0.5;
                let trig = outs[Logic::OUT_TRIG].samples[i] > 0.5;
                let gate = outs[Logic::OUT_GATE].samples[i] > 0.5;
                if clock_high && !prev_clock {
                    let should_fire = edges % 64 == 0;
                    assert_eq!(trig && !prev_trig, should_fire, "clock edge {edges}");
                    assert_eq!(gate && !prev_gate, should_fire, "clock edge {edges}");
                    // The gate spans the fill bar and the downbeat after it
                    assert_eq!(gate, edges % 64 <= 16, "clock edge {edges}");
                    fires += should_fire as usize;
                    edges += 1;
                } else {
                    assert!(!(trig && !prev_trig), "Trig rose off a clock edge");
                    assert!(!(gate && !prev_gate), "Gate rose off a clock edge");
                }
                gate_opens += (gate && !prev_gate) as usize;
                (prev_clock, prev_trig, prev_gate) = (clock_high, trig, gate);
            }
        }
        // 96 BPM sixteenths: 6.4 a second
        assert_eq!(edges, 3840);
        assert_eq!(fires, 60);
        assert_eq!(gate_opens, 60);
    }

    #[test]
    fn test_readout_reports_the_count() {
        let mut logic = Logic::new();
        assert_eq!(logic.readout(&[]).unwrap().values[Logic::READOUT_COUNT], -1.0);
        let clock = pulses(100, 10, 3);
        let mut input = SignalBuffer::control(300);
        input.samples.copy_from_slice(&clock);
        let unconnected = SignalBuffer::control(300);
        let mut outs: Vec<SignalBuffer> = (0..8).map(|_| SignalBuffer::control(300)).collect();
        logic.process(&[&input, &unconnected, &unconnected, &unconnected, &unconnected], &mut outs, &params(4, 0, 1), &ProcessContext::new(SAMPLE_RATE, 300));
        assert_eq!(logic.readout(&[]).unwrap().values[Logic::READOUT_COUNT], 2.0);
    }
}
