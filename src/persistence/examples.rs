//! Example patches that ship with the app.
//!
//! The patches live in `patches/` at the repository root, where the docs
//! link to them, and are compiled into the binary so the File → Examples
//! menu works wherever the app is run from. Each recipe in the docs
//! (`docs/src/recipes/`) has a patch here that builds exactly what it
//! describes.

use super::{patch_from_json, Patch, PatchError};

/// A patch that ships with the app.
#[derive(Debug)]
pub struct Example {
    /// Name shown in the Examples menu.
    pub name: &'static str,
    /// File name in `patches/`, also offered when the example is saved.
    pub file_name: &'static str,
    /// One line on what the patch teaches, for the menu tooltip.
    pub description: &'static str,
    json: &'static str,
}

impl Example {
    /// Parses the example into a patch.
    pub fn patch(&self) -> Result<Patch, PatchError> {
        patch_from_json(self.json)
    }
}

/// Every example, in menu order: from a first note to self-playing patches.
pub const EXAMPLES: &[Example] = &[
    Example {
        name: "First Sound",
        file_name: "first-sound.json",
        description: "Oscillator, envelope and VCA: the smallest playable voice. Press Play, then play the Z to M keys",
        json: include_str!("../../patches/first-sound.json"),
    },
    Example {
        name: "Basic Subtractive",
        file_name: "basic-subtractive.json",
        description: "A saw through a filter, with separate envelopes for brightness and volume",
        json: include_str!("../../patches/basic-subtractive.json"),
    },
    Example {
        name: "FM Synthesis",
        file_name: "fm-synthesis.json",
        description: "One sine bending another's pitch into a bell that starts bright and fades pure",
        json: include_str!("../../patches/fm-synthesis.json"),
    },
    Example {
        name: "Lush Pad",
        file_name: "lush-pad.json",
        description: "A polyphonic unison saw pad with slow attack, chorus and reverb. Hold chords",
        json: include_str!("../../patches/lush-pad.json"),
    },
    Example {
        name: "Generative Ambient",
        file_name: "generative-ambient.json",
        description: "Plays itself: a slow pentatonic sequence, sample-and-hold brightness, long echoes",
        json: include_str!("../../patches/generative-ambient.json"),
    },
    Example {
        name: "Rhythmic Sequence",
        file_name: "rhythmic-sequence.json",
        description: "Plays itself: a 16-step acid bassline with a resonant filter, drive and delay",
        json: include_str!("../../patches/rhythmic-sequence.json"),
    },
    Example {
        name: "Afterglow",
        file_name: "afterglow.json",
        description: "Plays itself: one sequencer transposes another's arpeggio through a chord progression, over a warm pad and dotted-eighth tape echoes",
        json: include_str!("../../patches/afterglow.json"),
    },
];

/// The example opened when the app starts with nothing else to open.
pub fn first_sound() -> &'static Example {
    &EXAMPLES[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::analysis::{amp_to_db, peak, rms};
    use crate::engine::OfflineRenderer;
    use crate::persistence::{compile_patch, PATCH_VERSION};

    #[test]
    fn test_every_patch_file_is_an_example() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("patches");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.ends_with(".json"))
            .collect();
        files.sort();
        let mut listed: Vec<String> = EXAMPLES.iter().map(|e| e.file_name.to_string()).collect();
        listed.sort();
        assert_eq!(files, listed, "patches/ and EXAMPLES must list the same files");
    }

    #[test]
    fn test_examples_load_cleanly() {
        for example in EXAMPLES {
            let patch = example.patch().unwrap_or_else(|e| panic!("{}: {}", example.name, e));
            assert_eq!(patch.name, example.name);
            // Saved by the current build, so loading never has to migrate them
            assert_eq!(patch.version, PATCH_VERSION, "{}", example.name);
            let compiled = compile_patch(&patch).unwrap();
            assert!(compiled.warnings.is_empty(), "{}: {:?}", example.name, compiled.warnings);
        }
    }

    #[test]
    fn test_examples_make_sound() {
        for example in EXAMPLES {
            let patch = example.patch().unwrap();
            let (mut renderer, compiled) = OfflineRenderer::from_patch(&patch, 48_000.0, 256).unwrap();
            let audio = renderer.render_audition(&patch, &compiled, 5.0);

            for (side, channel) in [("left", &audio.left), ("right", &audio.right)] {
                assert!(
                    channel.iter().all(|s| s.is_finite()),
                    "{}: {} channel has NaN or infinite samples",
                    example.name,
                    side
                );
                let (peak_db, rms_db) = (amp_to_db(peak(channel)), amp_to_db(rms(channel)));
                assert!(rms_db > -35.0, "{}: {} channel too quiet, {:.1} dBFS RMS", example.name, side, rms_db);
                assert!(peak_db < 0.0, "{}: {} channel clips, {:.1} dBFS peak", example.name, side, peak_db);
            }
        }
    }
}
