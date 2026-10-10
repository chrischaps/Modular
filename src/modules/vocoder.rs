//! Vocoder: one sound's spectral envelope worn by another.
//!
//! The **modulator** (usually a voice) is split into a bank of band-passes,
//! and a follower on each band tracks how loud that part of the spectrum is.
//! The **carrier** (a synth chord) runs through a matching bank, and each of
//! its bands is turned up and down by its twin's follower, so the chord
//! takes on the voice's vowels.
//!
//! - **Formant** slides the carrier's bank against the voice's, by up to an
//!   octave either way: a smaller or larger throat.
//! - **Sibilance** lets the voice's own highs (above the bank) through, so
//!   consonants stay readable.
//! - **Unvoiced** swaps the carrier for noise while the voice is hissing,
//!   so an "s" or a "t" comes through even over a dull carrier, or none.
//! - The bands alternate left and right, as far apart as **Width** says.
//!
//! Each band is two cascaded second-order sections (TPT state-variable
//! band-passes), so neighbours cross at -3 dB and the bank sums flat. The
//! whole bank is fixed-size: nothing is allocated, even in `prepare`.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, SQRT_2};

use crate::dsp::{
    context::ProcessContext,
    denormal::flush,
    module_trait::{DspModule, ModuleCategory, ModuleInfo},
    parameter::ParameterDefinition,
    port::PortDefinition,
    primitives::noise::NoiseFloor,
    signal::SignalBuffer,
    smoothed_value::SmoothedValue,
    ParameterDisplay, Readout, SignalType, MAX_READOUT,
};

/// The most bands the bank runs.
pub const MAX_BANDS: usize = 24;

/// The choices of the Bands parameter.
pub const BAND_COUNTS: [usize; 3] = [8, 16, 24];

/// Centre of the lowest band, in Hz.
pub const LOWEST_BAND: f32 = 100.0;

/// The bank spans this many octaves, lowest centre to highest: 100 Hz to
/// 6.4 kHz, from a low voice's fundamental to the top of its "s".
pub const BANK_OCTAVES: f32 = 6.0;

/// Where Sibilance's high-pass opens, in Hz.
const SIBILANCE_HZ: f32 = 6000.0;

/// The unvoiced detector compares the voice's level above this with its
/// whole level.
const HISS_HZ: f32 = 4000.0;

/// How much of the voice's level must lie above [`HISS_HZ`] before the
/// detector starts calling it unvoiced, and by how much it's sure.
const HISS_FROM: f32 = 0.3;
const HISS_TO: f32 = 0.6;

/// Peak level of the noise Unvoiced swaps in (about -11 dBFS RMS).
const NOISE_LEVEL: f32 = 0.5;

/// Wet level: the bank's output with every follower at full scale would be
/// about the carrier's own level divided by √bands, so this times √bands
/// brings a voice at a speaking level up to sit with the carrier.
const MAKEUP: f32 = 4.0;

/// Sibilance at full is the voice's highs at this gain, level with the
/// vocoded voice from an even carrier.
const SIBILANCE_GAIN: f32 = 2.0;

/// How long the bank fades out, and back in, to change its band count.
const SWITCH_MS: f32 = 6.0;

/// The bank's synthesis tuning follows Formant in steps this many samples
/// apart.
const RETUNE_EVERY: usize = 16;

/// Two cascaded sections each this much sharper than the pair should be
/// give the pair the bandwidth wanted (√(√2 - 1)).
const CASCADE_Q: f32 = 0.643_594_3;

/// Centre of band `band` of `bands`, before Formant, in Hz.
pub fn band_centre(band: usize, bands: usize) -> f32 {
    LOWEST_BAND * 2f32.powf(BANK_OCTAVES * band as f32 / (bands - 1).max(1) as f32)
}

/// The Q of each section of a band, for `bands` bands: the pair's -3 dB
/// edges land halfway to the neighbouring centres.
fn section_q(bands: usize) -> f32 {
    let ratio = 2f32.powf(BANK_OCTAVES / (bands - 1).max(1) as f32);
    CASCADE_Q * ratio.sqrt() / (ratio - 1.0)
}

/// The band count a Bands parameter value selects.
fn band_count(param: f32) -> usize {
    BAND_COUNTS[(param.round().max(0.0) as usize).min(BAND_COUNTS.len() - 1)]
}

/// Coefficients of a TPT state-variable filter (Zavalishin; Simper's form).
#[derive(Clone, Copy, Debug, Default)]
struct SvfCoefficients {
    a1: f32,
    a2: f32,
    a3: f32,
    /// 1/Q, which also scales the band-pass to a peak of unity.
    k: f32,
}

impl SvfCoefficients {
    fn new(cutoff_hz: f32, q: f32, sample_rate: f32) -> Self {
        let g = (PI * cutoff_hz.clamp(10.0, sample_rate * 0.45) / sample_rate).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        Self { a1, a2, a3: g * a2, k }
    }
}

/// One state-variable section's state.
#[derive(Clone, Copy, Debug, Default)]
struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// Advances the filter by one sample: (band-pass at unity peak, high-pass).
    #[inline]
    fn tick(&mut self, c: &SvfCoefficients, x: f32) -> (f32, f32) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = flush(2.0 * v1 - self.ic1);
        self.ic2 = flush(2.0 * v2 - self.ic2);
        (c.k * v1, x - c.k * v1 - v2)
    }

    #[inline]
    fn band(&mut self, c: &SvfCoefficients, x: f32) -> f32 {
        self.tick(c, x).0
    }

    #[inline]
    fn high(&mut self, c: &SvfCoefficients, x: f32) -> f32 {
        self.tick(c, x).1
    }
}

/// Two identical band-pass sections in a row.
#[inline]
fn band_pair(sections: &mut [Svf; 2], c: &SvfCoefficients, x: f32) -> f32 {
    let [first, second] = sections;
    second.band(c, first.band(c, x))
}

/// An envelope follower: a rectifier into a one-pole that rises at one rate
/// and falls at another, as on an analog vocoder's band.
#[derive(Clone, Copy, Debug, Default)]
struct Follower {
    level: f32,
}

impl Follower {
    #[inline]
    fn follow(&mut self, x: f32, attack: f32, release: f32) -> f32 {
        let x = x.abs();
        let rate = if x > self.level { attack } else { release };
        self.level = flush(self.level + rate * (x - self.level));
        self.level
    }
}

/// The per-sample coefficient of a one-pole that covers 63% of a step in
/// `ms` milliseconds.
fn one_pole(ms: f32, sample_rate: f32) -> f32 {
    1.0 - (-1000.0 / (ms.max(0.01) * sample_rate)).exp()
}

/// One band: the voice's side and the carrier's side.
#[derive(Clone, Copy, Debug, Default)]
struct Band {
    analysis: SvfCoefficients,
    synthesis: SvfCoefficients,
    voice: [Svf; 2],
    carrier: [Svf; 2],
    follower: Follower,
}

/// The vocoder.
///
/// # Ports
///
/// - **Carrier** (Audio, Input): the sound that's played, a synth chord.
///   A polyphonic cable is summed.
/// - **Modulator** (Audio, Input): the sound whose spectrum it wears, a voice.
/// - **Formant** (Control, Input): adds to the Formant knob, ±1 an octave.
/// - **Out L** / **Out R** (Audio, Output): the bands, alternating sides.
///
/// # Parameters
///
/// - **Formant** (−1..+1 octave): the carrier's bands against the voice's.
/// - **Bands** (8 / 16 / 24).
/// - **Attack** and **Release** of the band followers.
/// - **Sibilance**, **Unvoiced**, **Width** and **Mix**.
pub struct Vocoder {
    sample_rate: f32,
    bands: [Band; MAX_BANDS],
    /// The band count the bank is running.
    active: usize,
    /// Fades the bank out and in again around a change of band count.
    switch_gain: f32,
    /// The Formant the synthesis bank is tuned to, in octaves.
    tuned_formant: f32,
    /// Counts samples to the next retune.
    retune_in: usize,
    sibilance_filter: [Svf; 2],
    sibilance_coefficients: [SvfCoefficients; 2],
    hiss_filter: Svf,
    hiss_coefficients: SvfCoefficients,
    hiss_level: Follower,
    voice_level: Follower,
    /// How unvoiced the voice is right now, 0 to 1.
    unvoiced: f32,
    noise: NoiseFloor,
    formant: SmoothedValue,
    sibilance: SmoothedValue,
    unvoiced_amount: SmoothedValue,
    width: SmoothedValue,
    mix: SmoothedValue,
    ports: Vec<PortDefinition>,
    parameters: Vec<ParameterDefinition>,
}

impl Vocoder {
    pub const PORT_CARRIER: usize = 0;
    pub const PORT_MODULATOR: usize = 1;
    pub const PORT_FORMANT: usize = 2;
    pub const PORT_OUT_L: usize = 0;
    pub const PORT_OUT_R: usize = 1;

    pub const PARAM_FORMANT: usize = 0;
    pub const PARAM_BANDS: usize = 1;
    pub const PARAM_ATTACK: usize = 2;
    pub const PARAM_RELEASE: usize = 3;
    pub const PARAM_SIBILANCE: usize = 4;
    pub const PARAM_UNVOICED: usize = 5;
    pub const PARAM_WIDTH: usize = 6;
    pub const PARAM_MIX: usize = 7;

    /// Readout: the voice's level in each band, in amplitude, from 0 up to
    /// [`MAX_BANDS`].
    pub const READOUT_BANDS: usize = 0;
    /// Readout: how many bands are running.
    pub const READOUT_COUNT: usize = MAX_BANDS;
    /// Readout: the Formant shift heard, knob and CV, in octaves.
    pub const READOUT_FORMANT: usize = MAX_BANDS + 1;
    /// Readout: how unvoiced the voice is (0 to 1), before the Unvoiced knob.
    pub const READOUT_UNVOICED: usize = MAX_BANDS + 2;
    /// Readout: the voice's level above the hiss detector's high-pass.
    pub const READOUT_HIGHS: usize = MAX_BANDS + 3;

    pub fn new() -> Self {
        let sample_rate = 44100.0;
        let mut vocoder = Self {
            sample_rate,
            bands: [Band::default(); MAX_BANDS],
            active: 16,
            switch_gain: 1.0,
            tuned_formant: 0.0,
            retune_in: 0,
            sibilance_filter: [Svf::default(); 2],
            sibilance_coefficients: [SvfCoefficients::default(); 2],
            hiss_filter: Svf::default(),
            hiss_coefficients: SvfCoefficients::default(),
            hiss_level: Follower::default(),
            voice_level: Follower::default(),
            unvoiced: 0.0,
            noise: NoiseFloor::new(NOISE_LEVEL),
            formant: SmoothedValue::with_default_smoothing(0.0, sample_rate),
            sibilance: SmoothedValue::with_default_smoothing(0.25, sample_rate),
            unvoiced_amount: SmoothedValue::with_default_smoothing(0.25, sample_rate),
            width: SmoothedValue::with_default_smoothing(0.5, sample_rate),
            mix: SmoothedValue::with_default_smoothing(1.0, sample_rate),
            ports: vec![
                PortDefinition::input_with_default("carrier", "Carrier", SignalType::Audio, 0.0)
                    .describe("The sound that's played, a synth chord; a polyphonic cable is summed"),
                PortDefinition::input_with_default("modulator", "Modulator", SignalType::Audio, 0.0)
                    .describe("The sound whose spectrum the carrier wears, usually a voice"),
                PortDefinition::input_with_default("formant_cv", "Formant", SignalType::Control, 0.0)
                    .describe("Adds to the Formant knob: +1 moves the carrier's bands an octave up"),
                PortDefinition::output("out_l", "Out L", SignalType::Audio).describe("The odd bands, toward the left as Width opens"),
                PortDefinition::output("out_r", "Out R", SignalType::Audio).describe("The even bands, toward the right as Width opens"),
            ],
            parameters: vec![
                ParameterDefinition::new("formant", "Formant", -1.0, 1.0, 0.0, ParameterDisplay::linear("oct"))
                    .describe("Moves the carrier's bands against the voice's: up for a smaller throat, down for a larger"),
                ParameterDefinition::choice("bands", "Bands", &["8", "16", "24"], 1)
                    .describe("How many bands the spectrum is split into: fewer is rougher and more robotic, more is clearer"),
                ParameterDefinition::new("attack", "Attack", 0.5, 200.0, 4.0, ParameterDisplay::logarithmic("ms"))
                    .describe("How fast a band opens when the voice gets louder there"),
                ParameterDefinition::new("release", "Release", 5.0, 2000.0, 40.0, ParameterDisplay::logarithmic("ms"))
                    .describe("How fast a band closes again: short for speech, long for a choir smear"),
                ParameterDefinition::normalized("sibilance", "Sibilance", 0.25)
                    .describe("Lets the voice's own highs through, so consonants stay readable"),
                ParameterDefinition::normalized("unvoiced", "Unvoiced", 0.25)
                    .describe("Swaps the carrier for noise while the voice hisses, so 's' and 't' come through"),
                ParameterDefinition::normalized("width", "Width", 0.5)
                    .describe("Spreads the bands, alternating left and right"),
                ParameterDefinition::normalized("mix", "Mix", 1.0)
                    .describe("Blend from the carrier alone (0) to the vocoded sound (1)"),
            ],
        };
        vocoder.tune_all();
        vocoder
    }

    /// Tunes both banks, the sibilance and the hiss filters for the band
    /// count and sample rate, and clears every filter.
    fn tune_all(&mut self) {
        let q = section_q(self.active);
        for (index, band) in self.bands.iter_mut().enumerate().take(self.active) {
            band.analysis = SvfCoefficients::new(band_centre(index, self.active), q, self.sample_rate);
        }
        self.retune_synthesis(self.tuned_formant);
        for band in &mut self.bands {
            band.voice = [Svf::default(); 2];
            band.carrier = [Svf::default(); 2];
            band.follower = Follower::default();
        }
        // A fourth-order Butterworth high-pass
        self.sibilance_coefficients = [
            SvfCoefficients::new(SIBILANCE_HZ, 0.541_196_1, self.sample_rate),
            SvfCoefficients::new(SIBILANCE_HZ, 1.306_563, self.sample_rate),
        ];
        self.hiss_coefficients = SvfCoefficients::new(HISS_HZ, FRAC_1_SQRT_2, self.sample_rate);
    }

    /// Tunes the carrier's bank `formant` octaves from the voice's.
    fn retune_synthesis(&mut self, formant: f32) {
        let q = section_q(self.active);
        let shift = 2f32.powf(formant);
        for (index, band) in self.bands.iter_mut().enumerate().take(self.active) {
            band.synthesis = SvfCoefficients::new(band_centre(index, self.active) * shift, q, self.sample_rate);
        }
        self.tuned_formant = formant;
    }
}

const FRAC_1_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

impl Default for Vocoder {
    fn default() -> Self {
        Self::new()
    }
}

impl DspModule for Vocoder {
    fn info(&self) -> &ModuleInfo {
        static INFO: ModuleInfo = ModuleInfo {
            id: "fx.vocoder",
            name: "Vocoder",
            category: ModuleCategory::Effect,
            description: "Puts a voice's spectrum on a synth: a bank of band-passes and followers",
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
        for smoothed in [&mut self.formant, &mut self.sibilance, &mut self.unvoiced_amount, &mut self.width, &mut self.mix] {
            smoothed.set_sample_rate(sample_rate);
        }
        self.tune_all();
    }

    fn process(&mut self, inputs: &[&SignalBuffer], outputs: &mut [SignalBuffer], params: &[f32], context: &ProcessContext) {
        let sample_rate = self.sample_rate;
        let attack = one_pole(params[Self::PARAM_ATTACK], sample_rate);
        let release = one_pole(params[Self::PARAM_RELEASE], sample_rate);
        // The hiss detector listens as fast as speech moves
        let (hiss_attack, hiss_release) = (one_pole(2.0, sample_rate), one_pole(25.0, sample_rate));
        let switch_step = 1.0 / (SWITCH_MS * 0.001 * sample_rate);
        let wanted = band_count(params[Self::PARAM_BANDS]);

        self.formant.set_target(params[Self::PARAM_FORMANT]);
        self.sibilance.set_target(params[Self::PARAM_SIBILANCE]);
        self.unvoiced_amount.set_target(params[Self::PARAM_UNVOICED]);
        self.width.set_target(params[Self::PARAM_WIDTH]);
        self.mix.set_target(params[Self::PARAM_MIX]);

        let carrier = &inputs[Self::PORT_CARRIER].samples;
        let modulator = &inputs[Self::PORT_MODULATOR].samples;
        let formant_cv = &inputs[Self::PORT_FORMANT].samples;
        let (left, right) = outputs.split_at_mut(1);
        let (out_l, out_r) = (&mut left[0].samples, &mut right[0].samples);

        for i in 0..context.block_size {
            let voice = modulator[i];
            let dry = carrier[i];

            // Fade out to change the band count, then back in
            if wanted != self.active {
                self.switch_gain -= switch_step;
                if self.switch_gain <= 0.0 {
                    self.switch_gain = 0.0;
                    self.active = wanted;
                    self.tune_all();
                }
            } else if self.switch_gain < 1.0 {
                self.switch_gain = (self.switch_gain + switch_step).min(1.0);
            }

            let formant = (self.formant.next() + formant_cv[i]).clamp(-1.0, 1.0);
            if self.retune_in == 0 {
                if (formant - self.tuned_formant).abs() > 1e-4 {
                    self.retune_synthesis(formant);
                }
                self.retune_in = RETUNE_EVERY;
            }
            self.retune_in -= 1;

            // How much of the voice is hiss: noise stands in for the carrier
            let hiss = self.hiss_level.follow(self.hiss_filter.high(&self.hiss_coefficients, voice), hiss_attack, hiss_release);
            let whole = self.voice_level.follow(voice, hiss_attack, hiss_release);
            let share = hiss / (whole + 1e-6);
            self.unvoiced = ((share - HISS_FROM) / (HISS_TO - HISS_FROM)).clamp(0.0, 1.0);
            let swap = self.unvoiced * self.unvoiced_amount.next();
            let excitation = dry + swap * (self.noise.sample() - dry);

            // Odd bands lean left and even bands right, at constant power
            let angle = FRAC_PI_4 * (1.0 - self.width.next());
            let (near, far) = (SQRT_2 * angle.cos(), SQRT_2 * angle.sin());

            let mut sides = [0.0f32; 2];
            for (index, band) in self.bands.iter_mut().enumerate().take(self.active) {
                let heard = band_pair(&mut band.voice, &band.analysis, voice);
                let level = band.follower.follow(heard, attack, release);
                let played = band_pair(&mut band.carrier, &band.synthesis, excitation);
                let y = played * level;
                let side = index & 1;
                sides[side] += near * y;
                sides[1 - side] += far * y;
            }

            let [low_stage, high_stage] = &mut self.sibilance_filter;
            let highs = high_stage.high(&self.sibilance_coefficients[1], low_stage.high(&self.sibilance_coefficients[0], voice));
            let sibilance = SIBILANCE_GAIN * self.sibilance.next() * highs;

            // Rectified, a band reads 2/π of its amplitude
            let wet_gain = MAKEUP * FRAC_PI_2 * (self.active as f32).sqrt() * self.switch_gain;
            let mix = self.mix.next();
            out_l[i] = dry * (1.0 - mix) + mix * (sides[0] * wet_gain + sibilance);
            out_r[i] = dry * (1.0 - mix) + mix * (sides[1] * wet_gain + sibilance);
        }
    }

    fn reset(&mut self) {
        self.tune_all();
        self.hiss_filter = Svf::default();
        self.sibilance_filter = [Svf::default(); 2];
        self.hiss_level = Follower::default();
        self.voice_level = Follower::default();
        self.unvoiced = 0.0;
        for smoothed in [&mut self.formant, &mut self.sibilance, &mut self.unvoiced_amount, &mut self.width, &mut self.mix] {
            smoothed.reset(smoothed.target());
        }
    }

    fn key_inputs(&self) -> &'static [usize] {
        &[Self::PORT_MODULATOR]
    }

    fn readout(&self, _params: &[f32]) -> Option<Readout> {
        let mut readout = Readout::default();
        for (value, band) in readout.values[Self::READOUT_BANDS..].iter_mut().zip(&self.bands).take(self.active) {
            *value = band.follower.level * FRAC_PI_2;
        }
        readout.values[Self::READOUT_COUNT] = self.active as f32;
        readout.values[Self::READOUT_FORMANT] = self.tuned_formant;
        readout.values[Self::READOUT_UNVOICED] = self.unvoiced;
        readout.values[Self::READOUT_HIGHS] = self.hiss_level.level * FRAC_PI_2;
        Some(readout)
    }
}

const _: () = assert!(Vocoder::READOUT_HIGHS < MAX_READOUT);

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;
    const BLOCK: usize = 256;

    /// Knobs: Formant, Bands, Attack, Release, Sibilance, Unvoiced, Width, Mix.
    fn knobs(formant: f32, bands: f32, sibilance: f32, unvoiced: f32, width: f32, mix: f32) -> [f32; 8] {
        [formant, bands, 4.0, 40.0, sibilance, unvoiced, width, mix]
    }

    /// Plays `seconds` through a new vocoder at 48 kHz. See [`play_on`].
    fn play(
        seconds: f32,
        carrier: impl FnMut(usize) -> f32,
        voice: impl FnMut(usize) -> f32,
        formant_cv: f32,
        params: impl Fn(usize) -> [f32; 8],
    ) -> (Vec<f32>, Vec<f32>) {
        play_on(&mut Vocoder::new(), seconds, carrier, voice, formant_cv, params)
    }

    /// Plays `seconds` through `vocoder` at 48 kHz, the carrier and the voice
    /// given sample by sample, the knobs block by block. Returns (L, R).
    fn play_on(
        vocoder: &mut Vocoder,
        seconds: f32,
        mut carrier: impl FnMut(usize) -> f32,
        mut voice: impl FnMut(usize) -> f32,
        formant_cv: f32,
        params: impl Fn(usize) -> [f32; 8],
    ) -> (Vec<f32>, Vec<f32>) {
        vocoder.prepare(SR, BLOCK);
        let ctx = ProcessContext::new(SR, BLOCK);
        let mut c = SignalBuffer::audio(BLOCK);
        let mut m = SignalBuffer::audio(BLOCK);
        let mut cv = SignalBuffer::control(BLOCK);
        cv.fill(formant_cv);
        let mut outputs = vec![SignalBuffer::audio(BLOCK), SignalBuffer::audio(BLOCK)];
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for b in 0..(seconds * SR) as usize / BLOCK {
            for j in 0..BLOCK {
                c.samples[j] = carrier(b * BLOCK + j);
                m.samples[j] = voice(b * BLOCK + j);
            }
            vocoder.process(&[&c, &m, &cv], &mut outputs, &params(b), &ctx);
            left.extend_from_slice(&outputs[0].samples);
            right.extend_from_slice(&outputs[1].samples);
        }
        (left, right)
    }

    /// A bright carrier: a sawtooth at `hz`.
    fn saw(hz: f32) -> impl FnMut(usize) -> f32 {
        move |n| 0.5 * (2.0 * (n as f32 * hz / SR).fract() - 1.0)
    }

    /// An even carrier: every harmonic of `hz` up to 8 kHz at one level, so
    /// the vocoder's peaks are the voice's alone.
    fn buzz(hz: f32) -> impl FnMut(usize) -> f32 {
        let harmonics = (8000.0 / hz) as usize;
        move |n| (1..=harmonics).map(|k| 0.05 * (2.0 * PI * (k as f32 * hz * n as f32 / SR).fract()).sin()).sum()
    }

    /// White noise, uniform in ±1.
    fn white(seed: u32) -> impl FnMut() -> f32 {
        let mut x = seed;
        move || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as i32 as f32 / 2_147_483_648.0
        }
    }

    /// A vowel-like voice: pink noise through band-passes at `formants`.
    fn vowel(formants: &'static [f32]) -> impl FnMut(usize) -> f32 {
        let mut noise = white(7);
        let (mut b0, mut b1, mut b2) = (0.0f32, 0.0f32, 0.0f32);
        let coefficients: Vec<SvfCoefficients> = formants.iter().map(|&hz| SvfCoefficients::new(hz, 6.0, SR)).collect();
        let mut filters = vec![Svf::default(); formants.len()];
        move |_| {
            // Paul Kellett's economy pink noise
            let w = noise();
            b0 = 0.99765 * b0 + w * 0.099_046;
            b1 = 0.96300 * b1 + w * 0.296_516_4;
            b2 = 0.57000 * b2 + w * 1.052_691_3;
            let pink = 0.25 * (b0 + b1 + b2 + w * 0.1848);
            filters.iter_mut().zip(&coefficients).map(|(f, c)| f.band(c, pink)).sum()
        }
    }

    /// The level of `x` at `hz` (Goertzel), as a sine's amplitude.
    fn level_at(x: &[f32], hz: f32) -> f32 {
        let w = 2.0 * PI * hz / SR;
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        let coeff = 2.0 * (w as f64).cos();
        for &v in x {
            let s = v as f64 + coeff * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        let power = s1 * s1 + s2 * s2 - coeff * s1 * s2;
        (2.0 * power.max(0.0).sqrt() / x.len() as f64) as f32
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn mono(left: &[f32], right: &[f32]) -> Vec<f32> {
        left.iter().zip(right).map(|(l, r)| 0.5 * (l + r)).collect()
    }

    #[test]
    fn info_ports_and_parameters() {
        let vocoder = Vocoder::new();
        assert_eq!(vocoder.info().id, "fx.vocoder");
        assert_eq!(vocoder.info().category, ModuleCategory::Effect);
        let ports = vocoder.ports();
        assert_eq!(ports.len(), 5);
        assert_eq!(ports[Vocoder::PORT_CARRIER].name, "Carrier");
        assert_eq!(ports[Vocoder::PORT_MODULATOR].name, "Modulator");
        assert_eq!(ports[Vocoder::PORT_FORMANT].name, "Formant");
        let names: Vec<&str> = vocoder.parameters().iter().map(|p| p.name).collect();
        assert_eq!(names, ["Formant", "Bands", "Attack", "Release", "Sibilance", "Unvoiced", "Width", "Mix"]);
    }

    #[test]
    fn bands_cover_the_bank_and_meet_at_their_edges() {
        for bands in BAND_COUNTS {
            assert!((band_centre(0, bands) - 100.0).abs() < 1e-3);
            assert!((band_centre(bands - 1, bands) - 6400.0).abs() < 0.5);
            // A tone halfway between two centres is -3 dB through each pair
            let between = (band_centre(3, bands) * band_centre(4, bands)).sqrt();
            let c = SvfCoefficients::new(band_centre(3, bands), section_q(bands), SR);
            let mut pair = [Svf::default(); 2];
            let out: Vec<f32> = (0..48000).map(|n| band_pair(&mut pair, &c, (2.0 * PI * between * n as f32 / SR).sin())).collect();
            let gain = rms(&out[24000..]) * SQRT_2;
            assert!((gain - FRAC_1_SQRT_2).abs() < 0.06, "{bands} bands: {gain} between centres");
        }
    }

    /// The output's level at each harmonic of 125 Hz up to 6 kHz, playing a
    /// buzz through a voice with formants at 500 Hz and 2 kHz.
    fn harmonics_of_a_vowel(bands: f32, formant: f32) -> Vec<(f32, f32)> {
        let (l, r) = play(2.0, buzz(125.0), vowel(&[500.0, 2000.0]), 0.0, |_| knobs(formant, bands, 0.0, 0.0, 0.0, 1.0));
        let out = mono(&l[48000..], &r[48000..]);
        (1..=48).map(|k| k as f32 * 125.0).map(|hz| (hz, level_at(&out, hz))).collect()
    }

    /// Checks the output peaks at `peaks`: the loudest harmonic is near one
    /// of them, and near each, the output is `contrast` times as loud as in
    /// the valley halfway between them.
    fn assert_peaks_at(levels: &[(f32, f32)], peaks: [f32; 2], contrast: f32, what: &str) {
        let near = |hz: f32, octaves: f32| {
            levels.iter().filter(|(h, _)| (h / hz).log2().abs() <= octaves).map(|(_, l)| *l).fold(0.0, f32::max)
        };
        let valley = near((peaks[0] * peaks[1]).sqrt(), 0.15);
        for peak in peaks {
            assert!(near(peak, 0.3) > contrast * valley, "{what}: {} near {peak} Hz against {valley} between", near(peak, 0.3));
        }
        let loudest = levels.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
        assert!(peaks.iter().any(|p| (loudest / p).log2().abs() <= 0.3), "{what}: the loudest harmonic is {loudest} Hz");
    }

    #[test]
    fn a_vowel_puts_the_carriers_peaks_on_its_formants() {
        // A voice with formants at 500 Hz and 2 kHz, on a buzz whose
        // harmonics fall on both formants and the valley between
        for bands in [0.0, 1.0, 2.0] {
            assert_peaks_at(&harmonics_of_a_vowel(bands, 0.0), [500.0, 2000.0], 3.0, &format!("bands {bands}"));
        }
    }

    #[test]
    fn formant_carries_the_peaks_up_an_octave() {
        assert_peaks_at(&harmonics_of_a_vowel(1.0, 1.0), [1000.0, 4000.0], 3.0, "up an octave");
        assert_peaks_at(&harmonics_of_a_vowel(1.0, -1.0), [250.0, 1000.0], 3.0, "down an octave");
    }

    #[test]
    fn formant_cv_adds_to_the_knob() {
        let knob_only = play(0.5, saw(125.0), vowel(&[700.0]), 0.0, |_| knobs(0.6, 1.0, 0.0, 0.0, 0.5, 1.0));
        let with_cv = play(0.5, saw(125.0), vowel(&[700.0]), 0.4, |_| knobs(0.2, 1.0, 0.0, 0.0, 0.5, 1.0));
        // Both knobs glide from 0, so compare once they've settled
        let tail = 12000..;
        let diff = knob_only.0[tail.clone()].iter().zip(&with_cv.0[tail.clone()]).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(diff < 0.05 * rms(&knob_only.0[tail]), "differ by {diff}");
    }

    #[test]
    fn a_silent_voice_is_silence() {
        let (l, r) = play(0.5, saw(125.0), |_| 0.0, 0.0, |_| knobs(0.0, 1.0, 1.0, 1.0, 0.5, 1.0));
        assert_eq!(rms(&l), 0.0);
        assert_eq!(rms(&r), 0.0);
    }

    #[test]
    fn mix_at_zero_is_the_carrier() {
        let mut carrier = saw(220.0);
        let expected: Vec<f32> = (0..23808).map(&mut carrier).collect();
        let (l, r) = play(0.5, saw(220.0), vowel(&[700.0]), 0.0, |_| knobs(0.0, 1.0, 0.5, 0.5, 1.0, 0.0));
        // The Mix knob starts at its default, full, and glides down
        assert_eq!(&l[12000..], &expected[12000..]);
        assert_eq!(&r[12000..], &expected[12000..]);
    }

    #[test]
    fn hiss_comes_through_as_noise_only_with_unvoiced() {
        // No carrier at all, and a voice that's only an "s": noise high up
        let hiss = |seed| {
            let mut noise = white(seed);
            let c = SvfCoefficients::new(5000.0, 0.7, SR);
            let mut f = Svf::default();
            move |_| 0.3 * f.high(&c, noise())
        };
        let (l, _) = play(1.0, |_| 0.0, hiss(3), 0.0, |_| knobs(0.0, 1.0, 0.0, 1.0, 0.0, 1.0));
        assert!(rms(&l[24000..]) > 0.02, "unvoiced: {}", rms(&l[24000..]));
        let (l, _) = play(1.0, |_| 0.0, hiss(3), 0.0, |_| knobs(0.0, 1.0, 0.0, 0.0, 0.0, 1.0));
        assert!(rms(&l[24000..]) < 1e-6, "Unvoiced off: {}", rms(&l[24000..]));

        // A vowel isn't hiss: no noise for it, however far Unvoiced is up
        let mut vocoder = Vocoder::new();
        let (l, _) = play_on(&mut vocoder, 1.0, |_| 0.0, vowel(&[500.0, 1500.0]), 0.0, |_| knobs(0.0, 1.0, 0.0, 1.0, 0.0, 1.0));
        assert!(rms(&l[24000..]) < 0.002, "a vowel let noise in: {}", rms(&l[24000..]));
        assert!(vocoder.readout(&[]).unwrap().values[Vocoder::READOUT_UNVOICED] < 0.05);
    }

    #[test]
    fn sibilance_passes_the_voices_highs() {
        let tone = |n: usize| 0.3 * (2.0 * PI * 9000.0 * n as f32 / SR).sin();
        let (l, r) = play(0.5, |_| 0.0, tone, 0.0, |_| knobs(0.0, 1.0, 1.0, 0.0, 0.0, 1.0));
        let level = level_at(&l[12000..], 9000.0);
        assert!((level - SIBILANCE_GAIN * 0.3).abs() < 0.1, "9 kHz through at {level}");
        assert_eq!(l, r, "the highs sit in the middle");
        let (l, _) = play(0.5, |_| 0.0, tone, 0.0, |_| knobs(0.0, 1.0, 0.0, 0.0, 0.0, 1.0));
        assert!(rms(&l[12000..]) < 1e-4, "Sibilance off: {}", rms(&l[12000..]));
    }

    #[test]
    fn width_parts_odd_and_even_bands() {
        // Carrier and voice both on the lowest of 8 bands, which leans left
        let low = |n: usize| 0.4 * (2.0 * PI * 100.0 * n as f32 / SR).sin();
        let (l, r) = play(1.0, low, low, 0.0, |_| knobs(0.0, 1.0, 0.0, 0.0, 0.0, 1.0));
        assert!(l[12000..].iter().zip(&r[12000..]).all(|(a, b)| (a - b).abs() < 1e-6), "Width 0 is mono");
        let (l, r) = play(1.0, low, low, 0.0, |_| knobs(0.0, 0.0, 0.0, 0.0, 1.0, 1.0));
        let (l, r) = (rms(&l[24000..]), rms(&r[24000..]));
        assert!(l > 5.0 * r, "left {l}, right {r}");
    }

    #[test]
    fn levels_hold_across_band_counts() {
        let mut levels = Vec::new();
        for bands in [0.0, 1.0, 2.0] {
            let (l, r) = play(2.0, saw(110.0), vowel(&[600.0, 1200.0, 2600.0]), 0.0, |_| knobs(0.0, bands, 0.0, 0.0, 0.0, 1.0));
            levels.push(rms(&mono(&l[48000..], &r[48000..])));
        }
        let voice: Vec<f32> = (0..96000).map(vowel(&[600.0, 1200.0, 2600.0])).collect();
        println!("voice rms {}, carrier rms {}, out {levels:?}", rms(&voice), 0.5 / 3f32.sqrt());
        for level in &levels {
            assert!(*level > 0.05 && *level < 0.5, "{levels:?}");
            assert!((20.0 * (level / levels[1]).log10()).abs() < 3.0, "{levels:?}");
        }
    }

    /// The steepest step in `x` against what a tone at `hz` could make at
    /// the level around it: 1 for a tone at a steady level. A click is a
    /// jump far steeper than the sound can move on its own.
    fn steepest(x: &[f32], hz: f32) -> f32 {
        let slope = 2.0 * PI * hz / SR;
        x.chunks(240)
            .map(|w| {
                let peak = w.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                let step = w.windows(2).fold(0.0f32, |m, p| m.max((p[1] - p[0]).abs()));
                step / (slope * peak + 1e-4)
            })
            .fold(0.0, f32::max)
    }

    #[test]
    fn changing_bands_or_formant_does_not_click() {
        // Steady for half a second, then a change every quarter: the bank
        // fades out and in around each change of band count, and Formant
        // glides the carrier's bands, which sweep past the tone
        let changes = |block: usize| {
            let step = (block * BLOCK) / 12000;
            let bands = [1.0, 1.0, 2.0, 0.0, 1.0, 2.0][step.min(5)];
            let formant = [0.0, 0.0, 0.5, -0.5, 1.0, 0.0][step.min(5)];
            knobs(formant, bands, 0.0, 0.0, 0.5, 1.0)
        };
        let mut vocoder = Vocoder::new();
        let tone = |n: usize| 0.4 * (2.0 * PI * 220.0 * n as f32 / SR).sin();
        let (l, r) = play_on(&mut vocoder, 1.5, tone, vowel(&[700.0, 1100.0]), 0.0, changes);
        let steady = steepest(&l[12000..24000], 220.0).max(steepest(&r[12000..24000], 220.0));
        assert!(steady < 1.2, "steady: {steady}");
        for side in [&l, &r] {
            let changing = steepest(&side[24000..], 220.0);
            assert!(changing < 3.0, "clicked: {changing} times a tone's steepest");
        }
        assert!(l.iter().chain(&r).all(|s| s.is_finite()));
        assert_eq!(vocoder.readout(&[]).unwrap().values[Vocoder::READOUT_COUNT], 24.0);
    }

    #[test]
    fn readout_shows_the_voices_spectrum() {
        let mut vocoder = Vocoder::new();
        play_on(&mut vocoder, 1.0, |_| 0.0, vowel(&[500.0, 2000.0]), 0.0, |_| knobs(0.0, 1.0, 0.0, 0.0, 0.0, 1.0));
        let values = vocoder.readout(&[]).unwrap().values;
        assert_eq!(values[Vocoder::READOUT_COUNT], 16.0);
        // The loudest band is one nearest a formant
        let loudest = (0..16).max_by(|&a, &b| values[a].total_cmp(&values[b])).unwrap();
        let centre = band_centre(loudest, 16);
        let near = |hz: f32| (centre / hz).log2().abs() < 0.25;
        assert!(near(500.0) || near(2000.0), "loudest band at {centre} Hz");
        assert!(values[16..MAX_BANDS].iter().all(|&v| v == 0.0), "bands past 16 are silent");
    }

    #[test]
    fn registry_makes_one() {
        let mut registry = crate::dsp::ModuleRegistry::new();
        registry.register::<Vocoder>();
        let module = registry.create("fx.vocoder").unwrap();
        assert_eq!(module.parameters().len(), 8);
    }
}
