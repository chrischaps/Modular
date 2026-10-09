//! Stereo Mixer module.
//!
//! Four channels, each with a level, a pan and a mute, summed into a stereo
//! pair under one master level. A polyphonic cable into a channel can be
//! fanned across the stereo field with Spread, so a chord opens up instead
//! of sitting dead centre.
//!
//! The mono Out is the plain sum of the channels at their levels, as the
//! mixer gave before it was stereo, so patches built on it sound the same.

use std::f32::consts::FRAC_PI_4;

use crate::dsp::{
    connected_input,
    context::ProcessContext,
    module_trait::{DspModule, MeterLevels, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    signal::{SignalBuffer, MAX_CHANNELS},
    smoothed_value::SmoothedValue,
    ParameterDisplay, SignalType,
};

/// The number of channel strips.
pub const STRIPS: usize = 4;

/// Samples between updates of the pan gains. In between, each gain glides
/// in a straight line, so a pan swept by an LFO moves smoothly without a
/// sine and cosine per voice per sample.
const PAN_STEP: usize = 32;

/// Where a channel's voice sits, from -1 (left) to 1 (right): the channel's
/// pan, moved by the voice's place in the spread.
pub fn voice_pan(pan: f32, spread: f32, voice: usize, voices: usize) -> f32 {
    (pan + spread * spread_offset(voice, voices)).clamp(-1.0, 1.0)
}

/// A voice's place in a full spread, from -1 to 1.
///
/// The places are evenly spaced, and the voices take them alternately left
/// and right from the outside in: the first voice hard left, the second hard
/// right, the third just inside the first. Poly MIDI hands notes out in
/// turn, and neighbouring voices sit on opposite sides, so even two notes
/// held on an eight-voice cable land either side of the pan.
pub fn spread_offset(voice: usize, voices: usize) -> f32 {
    if voices < 2 {
        return 0.0;
    }
    let place = if voice % 2 == 0 { voice / 2 } else { voices - 1 - voice / 2 };
    -1.0 + 2.0 * place as f32 / (voices - 1) as f32
}

/// Equal-power gains (left, right) for a pan from -1 (left) to 1 (right).
///
/// The power of the two sides adds up to the same at every position, so a
/// sound swept across keeps its loudness. At centre each side is at -3 dB.
#[inline]
pub fn pan_gains(pan: f32) -> [f32; 2] {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    let (sin, cos) = angle.sin_cos();
    [cos, sin]
}

/// One channel strip's running state.
#[derive(Clone)]
struct Strip {
    /// The Level knob, smoothed per sample. Level CV adds on unsmoothed.
    level: SmoothedValue,
    /// The Pan knob, smoothed once per [`PAN_STEP`].
    pan: SmoothedValue,
    /// 1 while heard, 0 while muted, smoothed so muting doesn't click.
    unmuted: SmoothedValue,
    /// Each voice's (left, right) pan gains, as they stand now.
    gains: [[f32; 2]; MAX_CHANNELS],
    /// The voices the strip played in the last block, 0 when unpatched.
    voices: usize,
    /// The loudest sample since the meters were last read, after the fader.
    peak: f32,
}

impl Strip {
    fn new(sample_rate: f32) -> Self {
        Self {
            level: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            pan: SmoothedValue::with_default_smoothing(0.0, sample_rate / PAN_STEP as f32),
            unmuted: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            gains: [pan_gains(0.0); MAX_CHANNELS],
            voices: 0,
            peak: 0.0,
        }
    }

    /// Jumps every smoothed value to where it's heading.
    fn settle(&mut self) {
        self.level.reset(self.level.target());
        self.pan.reset(self.pan.target());
        self.unmuted.reset(self.unmuted.target());
        self.voices = 0;
    }
}

/// A four-channel stereo mixer.
///
/// # Ports
///
/// **Inputs:**
/// - **Ch 1** to **Ch 4** (Audio): The channels. A polyphonic cable keeps
///   its voices apart, so Spread can place them.
/// - **Level 1** to **Level 4** (Control): CV added to each channel's level.
/// - **Pan 1** to **Pan 4** (Control): CV added to each channel's pan.
///
/// **Outputs:**
/// - **Out** (Audio): Mono sum of the channels at their levels, ignoring pan.
/// - **Out L**, **Out R** (Audio): The stereo mix.
///
/// # Parameters
///
/// - **Level 1** to **Level 4** (0 to 1): Each channel's volume. Default 1.
/// - **Pan 1** to **Pan 4** (-1 to 1): Each channel's place, equal power.
/// - **Mute 1** to **Mute 4**: Silences a channel.
/// - **Master** (-60 to +6 dB): Volume of every output. Default 0 dB.
/// - **Spread** (0 to 1): How far a polyphonic channel's voices fan out
///   around its pan. Default 0, where they all sit at the pan.
pub struct Mixer {
    ports: Vec<PortDefinition>,
    parameters: Vec<ParameterDefinition>,
    strips: [Strip; STRIPS],
    /// The Master knob, smoothed per sample.
    master: SmoothedValue,
    /// The Spread knob, smoothed once per [`PAN_STEP`].
    spread: SmoothedValue,
    /// The loudest samples of Out L and Out R since the meters were last read.
    master_peaks: [f32; 2],
}

impl Mixer {
    /// Creates a new Mixer.
    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let pan = |id, name, description| {
            ParameterDefinition::new(id, name, -1.0, 1.0, 0.0, ParameterDisplay::linear("pan")).describe(description)
        };
        let mute = |id, name, description| ParameterDefinition::toggle(id, name, false).describe(description);
        Self {
            ports: vec![
                // Channels 1 and 2 and the mono Out keep the names they had
                // before the mixer was stereo, so saved cables find them
                PortDefinition::input_with_default("ch1", "Ch 1", SignalType::Audio, 0.0).describe("Channel 1. A polyphonic cable keeps its voices apart, for Spread"),
                PortDefinition::input_with_default("ch2", "Ch 2", SignalType::Audio, 0.0).describe("Channel 2. A polyphonic cable keeps its voices apart, for Spread"),
                PortDefinition::input_with_default("ch3", "Ch 3", SignalType::Audio, 0.0).describe("Channel 3. A polyphonic cable keeps its voices apart, for Spread"),
                PortDefinition::input_with_default("ch4", "Ch 4", SignalType::Audio, 0.0).describe("Channel 4. A polyphonic cable keeps its voices apart, for Spread"),
                PortDefinition::input_with_default("level1_cv", "Level 1", SignalType::Control, 0.0).describe("CV added to channel 1's Level knob"),
                PortDefinition::input_with_default("level2_cv", "Level 2", SignalType::Control, 0.0).describe("CV added to channel 2's Level knob"),
                PortDefinition::input_with_default("level3_cv", "Level 3", SignalType::Control, 0.0).describe("CV added to channel 3's Level knob"),
                PortDefinition::input_with_default("level4_cv", "Level 4", SignalType::Control, 0.0).describe("CV added to channel 4's Level knob"),
                PortDefinition::input_with_default("pan1_cv", "Pan 1", SignalType::Control, 0.0).describe("CV added to channel 1's Pan knob. An LFO here pans it to and fro"),
                PortDefinition::input_with_default("pan2_cv", "Pan 2", SignalType::Control, 0.0).describe("CV added to channel 2's Pan knob. An LFO here pans it to and fro"),
                PortDefinition::input_with_default("pan3_cv", "Pan 3", SignalType::Control, 0.0).describe("CV added to channel 3's Pan knob. An LFO here pans it to and fro"),
                PortDefinition::input_with_default("pan4_cv", "Pan 4", SignalType::Control, 0.0).describe("CV added to channel 4's Pan knob. An LFO here pans it to and fro"),
                PortDefinition::output("out", "Out", SignalType::Audio).describe("Mono sum of every channel at its level, ignoring pan and spread"),
                PortDefinition::output("out_l", "Out L", SignalType::Audio).describe("Left side of the stereo mix"),
                PortDefinition::output("out_r", "Out R", SignalType::Audio).describe("Right side of the stereo mix"),
            ],
            parameters: vec![
                ParameterDefinition::new("level1", "Level 1", 0.0, 1.0, 1.0, ParameterDisplay::linear("")).describe("Volume of channel 1"),
                ParameterDefinition::new("level2", "Level 2", 0.0, 1.0, 1.0, ParameterDisplay::linear("")).describe("Volume of channel 2"),
                ParameterDefinition::new("level3", "Level 3", 0.0, 1.0, 1.0, ParameterDisplay::linear("")).describe("Volume of channel 3"),
                ParameterDefinition::new("level4", "Level 4", 0.0, 1.0, 1.0, ParameterDisplay::linear("")).describe("Volume of channel 4"),
                pan("pan1", "Pan 1", "Where channel 1 sits, from hard left to hard right. Each side is at -3 dB in the centre"),
                pan("pan2", "Pan 2", "Where channel 2 sits, from hard left to hard right. Each side is at -3 dB in the centre"),
                pan("pan3", "Pan 3", "Where channel 3 sits, from hard left to hard right. Each side is at -3 dB in the centre"),
                pan("pan4", "Pan 4", "Where channel 4 sits, from hard left to hard right. Each side is at -3 dB in the centre"),
                mute("mute1", "Mute 1", "Silences channel 1"),
                mute("mute2", "Mute 2", "Silences channel 2"),
                mute("mute3", "Mute 3", "Silences channel 3"),
                mute("mute4", "Mute 4", "Silences channel 4"),
                ParameterDefinition::new("master", "Master", MASTER_FLOOR_DB, 6.0, 0.0, ParameterDisplay::linear("dB"))
                    .describe("Volume of every output. Up to +6 dB, to win back the 3 dB a centred channel gives up on each side"),
                ParameterDefinition::new("spread", "Spread", 0.0, 1.0, 0.0, ParameterDisplay::linear(""))
                    .describe("Fans a polyphonic channel's voices out across the stereo field, around its pan. At 0 they all sit at the pan"),
            ],
            strips: std::array::from_fn(|_| Strip::new(sample_rate)),
            master: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            spread: SmoothedValue::with_default_smoothing(0.0, sample_rate / PAN_STEP as f32),
            master_peaks: [0.0; 2],
        }
    }

    /// Input port indices: the channels, then Level CVs, then Pan CVs.
    const PORT_CH: usize = 0;
    const PORT_LEVEL_CV: usize = 4;
    const PORT_PAN_CV: usize = 8;

    /// Parameter indices: four of each per-channel kind, then the master section.
    const PARAM_LEVEL: usize = 0;
    const PARAM_PAN: usize = 4;
    const PARAM_MUTE: usize = 8;
    const PARAM_MASTER: usize = 12;
    const PARAM_SPREAD: usize = 13;
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Mixer {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "util.mixer",
            name: "Mixer",
            category: ModuleCategory::Utility,
            description: "4-channel stereo mixer with pan, mute and poly spread",
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
        for strip in &mut self.strips {
            strip.level.set_sample_rate(sample_rate);
            strip.pan.set_sample_rate(sample_rate / PAN_STEP as f32);
            strip.unmuted.set_sample_rate(sample_rate);
        }
        self.master.set_sample_rate(sample_rate);
        self.spread.set_sample_rate(sample_rate / PAN_STEP as f32);
    }

    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    ) {
        // Out, Out L, Out R
        let [out, out_l, out_r] = outputs else {
            return;
        };
        let n = context.block_size.min(out.samples.len()).min(out_l.samples.len()).min(out_r.samples.len());
        let (out, out_l, out_r) = (&mut out.samples[..n], &mut out_l.samples[..n], &mut out_r.samples[..n]);
        out.fill(0.0);
        out_l.fill(0.0);
        out_r.fill(0.0);

        self.master.set_target(master_gain(params[Self::PARAM_MASTER]));
        self.spread.set_target(params[Self::PARAM_SPREAD]);

        for (s, strip) in self.strips.iter_mut().enumerate() {
            strip.level.set_target(params[Self::PARAM_LEVEL + s]);
            strip.pan.set_target(params[Self::PARAM_PAN + s]);
            strip.unmuted.set_target(if params[Self::PARAM_MUTE + s] >= 0.5 { 0.0 } else { 1.0 });

            let Some(input) = connected_input(inputs, Self::PORT_CH + s).filter(|buf| buf.samples.len() >= n) else {
                strip.settle();
                continue;
            };
            let cv = |port: usize| connected_input(inputs, port).map(|buf| &buf.samples[..]).filter(|cv| cv.len() >= n);
            let level_cv = cv(Self::PORT_LEVEL_CV + s);
            let pan_cv = cv(Self::PORT_PAN_CV + s);

            let voices = input.channels().min(MAX_CHANNELS);
            let mut channels: [&[f32]; MAX_CHANNELS] = [&[]; MAX_CHANNELS];
            for (v, channel) in channels[..voices].iter_mut().enumerate() {
                *channel = &input.voice(v).samples[..n];
            }
            let channels = &channels[..voices];

            // Every strip sees the same Spread glide
            let mut spread = self.spread.clone();

            let mut start = 0;
            while start < n {
                let end = (start + PAN_STEP).min(n);
                let len = (end - start) as f32;

                // Where each voice is heading by the end of this stretch.
                // A voice that has just joined starts there, as it was silent
                let pan = strip.pan.next() + pan_cv.map_or(0.0, |cv| cv[end - 1]);
                let spread = spread.next();
                let mut targets = [[0.0; 2]; MAX_CHANNELS];
                let mut steps = [[0.0; 2]; MAX_CHANNELS];
                for v in 0..voices {
                    let target = pan_gains(voice_pan(pan, spread, v, voices));
                    if v >= strip.voices {
                        strip.gains[v] = target;
                    }
                    targets[v] = target;
                    steps[v] = [(target[0] - strip.gains[v][0]) / len, (target[1] - strip.gains[v][1]) / len];
                }
                strip.voices = voices;

                for i in start..end {
                    let level = (strip.level.next() + level_cv.map_or(0.0, |cv| cv[i])).clamp(0.0, 1.0);
                    let gain = level * strip.unmuted.next();
                    let (mut mono, mut left, mut right) = (0.0, 0.0, 0.0);
                    for (v, channel) in channels.iter().enumerate() {
                        let x = channel[i];
                        let g = &mut strip.gains[v];
                        g[0] += steps[v][0];
                        g[1] += steps[v][1];
                        mono += x;
                        left += x * g[0];
                        right += x * g[1];
                    }
                    let mono = mono * gain;
                    out[i] += mono;
                    out_l[i] += left * gain;
                    out_r[i] += right * gain;
                    strip.peak = strip.peak.max(mono.abs());
                }

                // Land exactly, so rounding never builds up
                strip.gains[..voices].copy_from_slice(&targets[..voices]);
                start = end;
            }
        }

        // Spread moved on once per stretch, as each strip's copy did
        for _ in 0..n.div_ceil(PAN_STEP) {
            self.spread.next();
        }

        for i in 0..n {
            let master = self.master.next();
            out[i] = soft_clip(out[i] * master);
            out_l[i] = soft_clip(out_l[i] * master);
            out_r[i] = soft_clip(out_r[i] * master);
            self.master_peaks[0] = self.master_peaks[0].max(out_l[i].abs());
            self.master_peaks[1] = self.master_peaks[1].max(out_r[i].abs());
        }
    }

    fn reset(&mut self) {
        for strip in &mut self.strips {
            strip.settle();
            strip.peak = 0.0;
        }
        self.master.reset(self.master.target());
        self.spread.reset(self.spread.target());
        self.master_peaks = [0.0; 2];
    }

    /// Each channel's peak after its fader (Ch 1 to Ch 4), then Out L and Out R.
    fn take_meter_levels(&mut self) -> Option<MeterLevels> {
        let mut levels = MeterLevels::default();
        for (peak, strip) in levels.peaks.iter_mut().zip(&mut self.strips) {
            *peak = std::mem::take(&mut strip.peak);
        }
        levels.peaks[STRIPS] = std::mem::take(&mut self.master_peaks[0]);
        levels.peaks[STRIPS + 1] = std::mem::take(&mut self.master_peaks[1]);
        Some(levels)
    }

    /// Runs as one module, but hears each voice of a polyphonic cable on its
    /// own, so Spread can place them. Its outputs carry one channel.
    fn polyphonic(&self) -> bool {
        true
    }
}

/// The bottom of the Master knob, where it turns the mix off.
const MASTER_FLOOR_DB: f32 = -60.0;

/// The Master knob's gain: its decibels, or silence at the bottom.
fn master_gain(db: f32) -> f32 {
    if db <= MASTER_FLOOR_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// Where soft clipping starts: anything within ±1 passes untouched.
const CLIP_KNEE: f32 = 1.0;
/// The level the clipped sum eases toward but never reaches. Above 1 so two
/// envelopes summed for a wider filter sweep still open it further than one.
const CLIP_CEILING: f32 = 1.5;

/// Soft clipping to keep hot sums from clipping harshly.
///
/// Unity up to the knee, then a tanh curve that meets it with the same value
/// and slope and eases toward the ceiling, so a signal crossing full scale
/// bends instead of jumping.
#[inline]
fn soft_clip(x: f32) -> f32 {
    let over = x.abs() - CLIP_KNEE;
    if over <= 0.0 {
        x
    } else {
        let room = CLIP_CEILING - CLIP_KNEE;
        x.signum() * (CLIP_KNEE + room * (over / room).tanh())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK: usize = 256;

    /// Default parameters: every level at 1 and the master at 0 dB,
    /// centred, unmuted, no spread.
    fn params() -> [f32; 14] {
        let mut p = [0.0; 14];
        p[..4].fill(1.0);
        p
    }

    fn outputs() -> Vec<SignalBuffer> {
        vec![SignalBuffer::audio(BLOCK), SignalBuffer::audio(BLOCK), SignalBuffer::audio(BLOCK)]
    }

    /// An unpatched input, as the engine passes one.
    fn unpatched() -> SignalBuffer {
        SignalBuffer::unconnected(BLOCK, SignalType::Audio)
    }

    /// Runs `blocks` blocks with `inputs` laid on the ports by index (the
    /// rest unpatched), returning the outputs of the last.
    fn run(mixer: &mut Mixer, patched: &[(usize, &SignalBuffer)], params: &[f32], blocks: usize) -> Vec<SignalBuffer> {
        let empty = unpatched();
        let mut inputs: Vec<&SignalBuffer> = vec![&empty; 12];
        for &(port, buf) in patched {
            inputs[port] = buf;
        }
        let mut out = outputs();
        let ctx = ProcessContext::new(44100.0, BLOCK);
        for _ in 0..blocks {
            mixer.process(&inputs, &mut out, params, &ctx);
        }
        out
    }

    fn constant(value: f32) -> SignalBuffer {
        let mut buf = SignalBuffer::audio(BLOCK);
        buf.fill(value);
        buf
    }

    /// A polyphonic buffer with each channel at its own constant.
    fn poly(values: &[f32]) -> SignalBuffer {
        let mut buf = SignalBuffer::polyphonic(BLOCK, SignalType::Audio);
        buf.set_channels(values.len());
        for (c, &v) in values.iter().enumerate() {
            buf.channel_mut(c).fill(v);
        }
        buf
    }

    fn last(buf: &SignalBuffer) -> f32 {
        buf.samples[BLOCK - 1]
    }

    #[test]
    fn test_mixer_info() {
        let mixer = Mixer::new();
        assert_eq!(mixer.info().id, "util.mixer");
        assert_eq!(mixer.info().name, "Mixer");
        assert_eq!(mixer.info().category, ModuleCategory::Utility);
        assert!(mixer.polyphonic(), "hears poly voices apart");
    }

    #[test]
    fn test_old_names_keep_their_places() {
        // Patches save cables and values by name, and MIDI mappings by
        // parameter index: channels 1 and 2, Out and the two levels stay first
        let mixer = Mixer::new();
        let inputs: Vec<_> = mixer.ports().iter().filter(|p| p.is_input()).collect();
        let outputs: Vec<_> = mixer.ports().iter().filter(|p| p.is_output()).collect();
        assert_eq!((inputs[0].id, inputs[0].name), ("ch1", "Ch 1"));
        assert_eq!((inputs[1].id, inputs[1].name), ("ch2", "Ch 2"));
        assert_eq!((outputs[0].id, outputs[0].name), ("out", "Out"));
        assert_eq!(inputs.len(), 12);
        assert_eq!(outputs.iter().map(|p| p.name).collect::<Vec<_>>(), ["Out", "Out L", "Out R"]);
        assert!(inputs[..4].iter().all(|p| p.signal_type == SignalType::Audio));
        assert!(inputs[4..].iter().all(|p| p.signal_type == SignalType::Control));

        let params = mixer.parameters();
        assert_eq!(params.len(), 14);
        assert_eq!((params[0].id, params[0].name, params[0].default), ("level1", "Level 1", 1.0));
        assert_eq!((params[1].id, params[1].name, params[1].default), ("level2", "Level 2", 1.0));
        assert_eq!(params[Mixer::PARAM_MASTER].default, 0.0, "0 dB: unity, as before");
        assert_eq!(params[Mixer::PARAM_SPREAD].default, 0.0, "no spread: poly sums to the pan, as before");
        assert!(params[Mixer::PARAM_PAN..Mixer::PARAM_MUTE].iter().all(|p| p.default == 0.0));
    }

    #[test]
    fn test_mono_out_is_the_old_sum() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let (a, b) = (constant(0.5), constant(0.3));
        let mut p = params();
        p[0] = 0.5;
        p[1] = 0.25;
        // Pan and spread don't touch the mono sum
        p[Mixer::PARAM_PAN] = -1.0;
        p[Mixer::PARAM_PAN + 1] = 0.6;
        let out = run(&mut mixer, &[(0, &a), (1, &b)], &p, 20);
        assert!((last(&out[0]) - (0.5 * 0.5 + 0.3 * 0.25)).abs() < 1e-4);
    }

    #[test]
    fn test_pan_law() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.5);

        // Centre: each side at -3 dB
        let out = run(&mut mixer, &[(0, &x)], &params(), 4);
        let centre_db = 20.0 * (last(&out[1]) / 0.5).log10();
        assert!((centre_db + 3.01).abs() < 0.02, "centre is {centre_db} dB per side");
        assert!((last(&out[1]) - last(&out[2])).abs() < 1e-6);

        // Hard left: silence on the right
        let mut p = params();
        p[Mixer::PARAM_PAN] = -1.0;
        let out = run(&mut mixer, &[(0, &x)], &p, 40);
        assert!((last(&out[1]) - 0.5).abs() < 1e-4);
        assert!(last(&out[2]).abs() < 1e-6, "right should be silent, got {}", last(&out[2]));

        // Power is the same wherever it sits
        for pan in [-0.7, -0.2, 0.3, 0.9] {
            let [l, r] = pan_gains(pan);
            assert!((l * l + r * r - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_pan_cv_moves_the_channel() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.5);
        let cv = constant(1.0);
        let out = run(&mut mixer, &[(0, &x), (Mixer::PORT_PAN_CV, &cv)], &params(), 4);
        assert!(last(&out[1]).abs() < 1e-6, "CV of 1 pans a centred channel hard right");
        assert!((last(&out[2]) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn test_level_cv_adds_to_the_knob() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.5);
        let cv = constant(0.25);
        let mut p = params();
        p[0] = 0.5;
        let out = run(&mut mixer, &[(0, &x), (Mixer::PORT_LEVEL_CV, &cv)], &p, 20);
        assert!((last(&out[0]) - 0.5 * 0.75).abs() < 1e-4);

        // Never past full level, nor below silence
        let cv = constant(4.0);
        let out = run(&mut mixer, &[(0, &x), (Mixer::PORT_LEVEL_CV, &cv)], &p, 2);
        assert!((last(&out[0]) - 0.5).abs() < 1e-4);
        let cv = constant(-4.0);
        let out = run(&mut mixer, &[(0, &x), (Mixer::PORT_LEVEL_CV, &cv)], &p, 2);
        assert_eq!(last(&out[0]), 0.0);
    }

    #[test]
    fn test_mute_silences_without_a_click() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.8);
        run(&mut mixer, &[(0, &x)], &params(), 4);

        let mut p = params();
        p[Mixer::PARAM_MUTE] = 1.0;
        let out = run(&mut mixer, &[(0, &x)], &p, 1);
        // Fades over the first block rather than dropping at once
        assert!(out[0].samples[0] > 0.7);
        let out = run(&mut mixer, &[(0, &x)], &p, 20);
        assert!(last(&out[0]).abs() < 1e-4);
        assert!(last(&out[1]).abs() < 1e-4);
    }

    #[test]
    fn test_a_patch_starts_where_it_was_saved() {
        // Loaded muted and panned hard right: not a sample leaks through on
        // the way there
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.8);
        let mut p = params();
        p[Mixer::PARAM_PAN] = 1.0;
        p[Mixer::PARAM_MUTE + 1] = 1.0;
        let out = run(&mut mixer, &[(0, &x), (1, &x)], &p, 1);
        assert!(out[1].samples.iter().all(|s| s.abs() < 1e-6), "left should be silent");
        assert!((out[2].samples[0] - 0.8).abs() < 1e-4, "channel 1 is right from the start");
        assert_eq!(mixer.take_meter_levels().unwrap().peaks[1], 0.0, "channel 2 never sounds");
    }

    #[test]
    fn test_master_scales_every_output() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(0.5);
        let mut p = params();
        p[Mixer::PARAM_MASTER] = -6.0206;
        let out = run(&mut mixer, &[(0, &x)], &p, 20);
        assert!((last(&out[0]) - 0.25).abs() < 1e-4);
        assert!((last(&out[1]) - 0.25 * FRAC_PI_4.cos()).abs() < 1e-4);

        // +3 dB wins back what a centred channel gives up on each side
        p[Mixer::PARAM_MASTER] = 3.0103;
        let out = run(&mut mixer, &[(0, &x)], &p, 40);
        assert!((last(&out[1]) - 0.5).abs() < 1e-3);

        // The bottom of the knob is silence
        p[Mixer::PARAM_MASTER] = MASTER_FLOOR_DB;
        let out = run(&mut mixer, &[(0, &x)], &p, 40);
        assert_eq!(last(&out[0]), 0.0);
    }

    #[test]
    fn test_four_channels_sum() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let bufs = [constant(0.1), constant(0.2), constant(0.3), constant(0.15)];
        let patched: Vec<_> = bufs.iter().enumerate().collect();
        let out = run(&mut mixer, &patched, &params(), 4);
        assert!((last(&out[0]) - 0.75).abs() < 1e-4);
    }

    #[test]
    fn test_poly_without_spread_is_the_old_mixdown() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let chord = poly(&[0.1, 0.2, 0.15, 0.05]);
        let summed = constant(0.5);
        let mut p = params();
        p[Mixer::PARAM_PAN] = 0.3;
        p[Mixer::PARAM_PAN + 1] = 0.3;
        p[1] = 0.0;
        let a = run(&mut mixer, &[(0, &chord)], &p, 40);
        let mut other = Mixer::new();
        other.prepare(44100.0, BLOCK);
        let b = run(&mut other, &[(0, &summed)], &p, 40);
        for out in 0..3 {
            assert!((last(&a[out]) - last(&b[out])).abs() < 1e-5, "output {out}");
        }
        assert_eq!(a[0].channels(), 1, "the mix is one channel");
    }

    #[test]
    fn test_spread_places_each_voice() {
        // Four voices, each its own level, so each one's place shows in L and R
        let levels = [0.1, 0.2, 0.3, 0.4];
        let mut p = params();
        p[Mixer::PARAM_SPREAD] = 1.0;
        let mut energies = Vec::new();
        for (v, &level) in levels.iter().enumerate() {
            let mut voice = [0.0; 4];
            voice[v] = level;
            let input = poly(&voice);
            let mut mixer = Mixer::new();
            mixer.prepare(44100.0, BLOCK);
            let out = run(&mut mixer, &[(0, &input)], &p, 40);
            let [l, r] = [last(&out[1]), last(&out[2])];
            // Each voice sits where its place says, at full power
            let [gl, gr] = pan_gains(spread_offset(v, 4));
            assert!((l - level * gl).abs() < 1e-4 && (r - level * gr).abs() < 1e-4, "voice {v}: {l} {r}");
            energies.push((l / level, r / level));
        }
        // Alternating sides from the outside in: hard left, hard right, then inside
        assert!(energies[0].1.abs() < 1e-4, "voice 1 hard left");
        assert!(energies[1].0.abs() < 1e-4, "voice 2 hard right");
        assert!(energies[2].0 > energies[2].1 && energies[2].1 > 0.1, "voice 3 left of centre");
        assert!(energies[3].1 > energies[3].0 && energies[3].0 > 0.1, "voice 4 right of centre");
        // Four distinct places
        for a in 0..4 {
            for b in a + 1..4 {
                assert!((energies[a].0 - energies[b].0).abs() > 0.05, "voices {a} and {b} share a place");
            }
        }

        // With every voice playing, the left/right balance is even
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let input = poly(&[0.2; 4]);
        let out = run(&mut mixer, &[(0, &input)], &p, 40);
        assert!((last(&out[1]) - last(&out[2])).abs() < 1e-4);
    }

    #[test]
    fn test_spread_offsets() {
        assert_eq!(spread_offset(0, 1), 0.0);
        assert_eq!([spread_offset(0, 2), spread_offset(1, 2)], [-1.0, 1.0]);
        assert_eq!([spread_offset(0, 3), spread_offset(1, 3), spread_offset(2, 3)], [-1.0, 1.0, 0.0]);
        // Eight voices take eight different places
        let mut places: Vec<f32> = (0..8).map(|v| spread_offset(v, 8)).collect();
        places.sort_by(f32::total_cmp);
        for pair in places.windows(2) {
            assert!((pair[1] - pair[0] - 2.0 / 7.0).abs() < 1e-5);
        }
        // Spread fans out around the pan, and stops at the edges
        assert_eq!(voice_pan(0.5, 1.0, 1, 2), 1.0);
        assert_eq!(voice_pan(0.5, 0.25, 0, 2), 0.25);
    }

    #[test]
    fn test_meters_read_each_strip_and_the_master() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let (a, b) = (constant(0.5), constant(-0.2));
        let mut p = params();
        p[1] = 0.5;
        run(&mut mixer, &[(0, &a), (2, &b)], &p, 20);
        let levels = mixer.take_meter_levels().unwrap().peaks;
        assert!((levels[0] - 0.5).abs() < 1e-3);
        assert_eq!(levels[1], 0.0, "channel 2 isn't patched");
        assert!((levels[2] - 0.2).abs() < 1e-3);
        assert!((levels[4] - 0.3 * FRAC_PI_4.cos()).abs() < 1e-3);
        // Reading starts a new measurement
        assert_eq!(mixer.take_meter_levels().unwrap().peaks, [0.0; 8]);
    }

    #[test]
    fn test_unpatched_is_silent() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let out = run(&mut mixer, &[], &params(), 2);
        for buf in &out {
            assert!(buf.samples.iter().all(|&s| s == 0.0));
        }
    }

    #[test]
    fn test_soft_clip_is_continuous_and_eases() {
        // Unity inside full scale
        assert_eq!(soft_clip(0.7), 0.7);
        assert_eq!(soft_clip(-1.0), -1.0);

        // No step at the knee: just past it, still just past 1
        assert!((soft_clip(1.001) - 1.001).abs() < 1e-4);
        assert!((soft_clip(-1.001) + 1.001).abs() < 1e-4);

        // Never falls back as the input grows, and stays under the ceiling
        let mut prev = soft_clip(1.0);
        for i in 1..=400 {
            let y = soft_clip(1.0 + i as f32 * 0.01);
            assert!(y >= prev, "falls at {}", 1.0 + i as f32 * 0.01);
            assert!(y <= CLIP_CEILING);
            prev = y;
        }

        // Two full-scale signals land a little under the ceiling
        assert!((soft_clip(2.0) - 1.482).abs() < 0.001);
    }

    #[test]
    fn test_hot_sums_soft_clip_on_every_output() {
        let mut mixer = Mixer::new();
        mixer.prepare(44100.0, BLOCK);
        let x = constant(1.0);
        let mut p = params();
        p[Mixer::PARAM_PAN] = -1.0;
        p[Mixer::PARAM_PAN + 1] = -1.0;
        let out = run(&mut mixer, &[(0, &x), (1, &x)], &p, 40);
        assert!((last(&out[0]) - soft_clip(2.0)).abs() < 1e-4);
        assert!((last(&out[1]) - soft_clip(2.0)).abs() < 1e-4);
    }

    #[test]
    fn test_mixer_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Mixer>();
    }

    #[test]
    fn test_mixer_registry_instantiation() {
        use crate::dsp::ModuleRegistry;

        let mut registry = ModuleRegistry::new();
        registry.register::<Mixer>();
        let module = registry.create("util.mixer").unwrap();
        assert_eq!(module.info().id, "util.mixer");
        assert_eq!(module.ports().len(), 15); // 12 inputs + 3 outputs
        assert_eq!(module.parameters().len(), 14);
    }
}
