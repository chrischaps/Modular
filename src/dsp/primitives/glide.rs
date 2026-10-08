//! Glide (portamento): pitch slides from one note to the next.
//!
//! The slide is a one-pole lag in semitones, so it is linear in pitch and
//! sounds even across the keyboard. It is constant time: an octave leap
//! arrives as quickly as a semitone step. The Glide time is the time to get
//! 99% of the way there; 63% takes about a fifth of it.

use crate::dsp::{ParameterDefinition, ParameterDisplay};

/// When a new note glides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlideMode {
    /// Every note glides from the one before.
    Always = 0,
    /// Only a note played while another key is still held glides, as on a
    /// 303 or a Minimoog. A note after a gap starts on its own pitch.
    Legato = 1,
}

impl GlideMode {
    /// The choices, in parameter order.
    pub const LABELS: &'static [&'static str] = &["Always", "Legato"];

    /// Converts from the parameter value.
    pub fn from_param(value: f32) -> Self {
        match value.round() as i32 {
            0 => GlideMode::Always,
            _ => GlideMode::Legato,
        }
    }

    /// Whether a note played now glides, given whether a key was already held.
    pub fn glides(self, key_held: bool) -> bool {
        self == GlideMode::Always || key_held
    }
}

/// The Glide and Glide Mode parameters, as the note modules declare them.
pub fn glide_parameters() -> [ParameterDefinition; 2] {
    [
        ParameterDefinition::new("glide", "Glide", 0.0, 2.0, 0.0, ParameterDisplay::logarithmic("s"))
            .describe("Time to slide to each new note; 0 is off"),
        ParameterDefinition::choice("glide_mode", "Glide Mode", GlideMode::LABELS, GlideMode::Legato as usize)
            .describe("Glide on every note, or only on notes played while a key is held"),
    ]
}

/// ln(100): the number of time constants to arrive within 1%.
const ARRIVAL: f32 = 2.0 * std::f32::consts::LN_10;

/// The pitch of one voice as it glides.
#[derive(Clone, Copy, Debug, Default)]
pub struct Glide {
    /// Where the pitch is, in semitones. `None` until the first note, which
    /// has nothing to glide from.
    at: Option<f32>,
}

impl Glide {
    /// No note played yet.
    pub const NEW: Glide = Glide { at: None };

    /// The per-sample coefficient for a glide of `seconds`. Zero is no glide.
    pub fn coefficient(seconds: f32, sample_rate: f32) -> f32 {
        if seconds > 0.0 {
            1.0 - (-ARRIVAL / (seconds * sample_rate)).exp()
        } else {
            1.0
        }
    }

    /// A note starts. It slides from the last pitch if `glide` is set,
    /// otherwise (or if it is the first note) it starts on its own pitch.
    pub fn start(&mut self, note: f32, glide: bool) {
        if !glide || self.at.is_none() {
            self.at = Some(note);
        }
    }

    /// Moves one sample toward `target` and returns the pitch. A coefficient
    /// of 1 lands exactly on the target. Before the first note, the pitch
    /// is the target.
    #[inline]
    pub fn next(&mut self, target: f32, coefficient: f32) -> f32 {
        let Some(at) = self.at.as_mut() else {
            return target;
        };
        if coefficient >= 1.0 {
            *at = target;
        } else {
            *at += (target - *at) * coefficient;
        }
        *at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48000.0;

    /// Samples until a glide from 0 to `step` passes `fraction` of it.
    fn samples_to(fraction: f32, step: f32, seconds: f32) -> usize {
        let coefficient = Glide::coefficient(seconds, SR);
        let mut glide = Glide::NEW;
        glide.start(0.0, false);
        (1..).find(|_| glide.next(step, coefficient) / step >= fraction).unwrap()
    }

    #[test]
    fn test_arrives_in_the_glide_time() {
        let seconds = 0.2;
        let tau = seconds / ARRIVAL;
        let at63 = samples_to(1.0 - (-1.0f32).exp(), 12.0, seconds) as f32 / SR;
        let at99 = samples_to(0.99, 12.0, seconds) as f32 / SR;
        assert!((at63 - tau).abs() < 0.0005, "63% at {at63} s, expected {tau} s");
        assert!((at99 - seconds).abs() < 0.0005, "99% at {at99} s, expected {seconds} s");
    }

    #[test]
    fn test_constant_time() {
        // An octave takes as long as a semitone
        assert_eq!(samples_to(0.99, 1.0, 0.1), samples_to(0.99, 12.0, 0.1));
        assert_eq!(samples_to(0.99, -24.0, 0.1), samples_to(0.99, 1.0, 0.1));
    }

    #[test]
    fn test_zero_glide_lands_exactly() {
        let mut glide = Glide::NEW;
        assert_eq!(glide.next(60.0, Glide::coefficient(0.0, SR)), 60.0);
        assert_eq!(glide.next(67.3, Glide::coefficient(0.0, SR)), 67.3);
    }

    #[test]
    fn test_first_note_has_nothing_to_glide_from() {
        let coefficient = Glide::coefficient(1.0, SR);
        let mut glide = Glide::NEW;
        assert_eq!(glide.next(60.0, coefficient), 60.0, "resting, before any note");
        glide.start(72.0, true);
        assert_eq!(glide.next(72.0, coefficient), 72.0);
        glide.start(60.0, true);
        assert!(glide.next(60.0, coefficient) > 71.9, "the next one glides");
        glide.start(48.0, false);
        assert_eq!(glide.next(48.0, coefficient), 48.0, "unless told not to");
    }

    #[test]
    fn test_mode() {
        assert_eq!(GlideMode::from_param(0.0), GlideMode::Always);
        assert_eq!(GlideMode::from_param(1.0), GlideMode::Legato);
        assert!(GlideMode::Always.glides(false));
        assert!(!GlideMode::Legato.glides(false));
        assert!(GlideMode::Legato.glides(true));
    }
}
