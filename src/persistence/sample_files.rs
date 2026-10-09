//! Sample files: reading WAVs into [`SampleData`], and where a patch's
//! samples are.
//!
//! In the editor, a Sampler names its file by a *key*: an absolute path, or
//! `example:samples/name.wav` for a sample that ships inside the app with
//! the example patches. A patch file stores the path relative to itself
//! when the sample sits beside or below it, and absolute otherwise, so a
//! patch folder can be moved or zipped and shared whole. [`resolve`] turns
//! a saved path back into a key, and [`to_patch_path`] goes the other way.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::dsp::{SampleData, MAX_SAMPLE_SECONDS};

use super::examples::EXAMPLE_SAMPLES;

/// What keys for the samples that ship with the examples start with.
pub const EXAMPLE_PREFIX: &str = "example:";

/// A decoded WAV file.
#[derive(Debug)]
pub struct Decoded {
    pub sample: SampleData,
    /// Whether it was longer than [`MAX_SAMPLE_SECONDS`] and was cut.
    pub truncated: bool,
}

/// Decodes a WAV file: integer or float, mono, stereo, or more (which
/// keeps its first two channels). Anything past [`MAX_SAMPLE_SECONDS`] is
/// left unread.
pub fn decode_wav(reader: impl Read) -> Result<Decoded, String> {
    let mut reader = hound::WavReader::new(reader).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let rate = spec.sample_rate.max(1);
    let total = reader.duration() as usize;
    let keep = total.min((MAX_SAMPLE_SECONDS * rate as f32) as usize);
    let wanted = keep * channels;

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().take(wanted).collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
            reader.samples::<i32>().take(wanted).map(|s| s.map(|s| s as f32 * scale)).collect::<Result<_, _>>()
        }
    }
    .map_err(|e| e.to_string())?;

    let frames = samples.len() / channels;
    let sample = if channels == 1 {
        SampleData::mono(samples, rate as f32)
    } else {
        let mut left = Vec::with_capacity(frames);
        let mut right = Vec::with_capacity(frames);
        for frame in samples.chunks_exact(channels) {
            left.push(frame[0]);
            right.push(frame[1]);
        }
        SampleData::stereo(left, right, rate as f32)
    };
    Ok(Decoded { sample, truncated: keep < total })
}

/// Reads a WAV file from disk.
pub fn read_wav_file(path: &Path) -> Result<Decoded, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    decode_wav(std::io::BufReader::new(file))
}

/// The bytes of a sample that ships with the examples, by key.
pub fn example_bytes(key: &str) -> Option<&'static [u8]> {
    let name = key.strip_prefix(EXAMPLE_PREFIX)?;
    EXAMPLE_SAMPLES.iter().find(|(path, _)| *path == name).map(|(_, bytes)| *bytes)
}

/// Loads the recording a key names: a shipped example's, or a file's.
pub fn load(key: &str) -> Result<Decoded, String> {
    if key.starts_with(EXAMPLE_PREFIX) {
        let bytes = example_bytes(key).ok_or_else(|| "no such example sample".to_string())?;
        return decode_wav(std::io::Cursor::new(bytes));
    }
    let path = Path::new(key);
    if !path.exists() {
        return Err("file not found".to_string());
    }
    read_wav_file(path)
}

/// Where a patch's relative sample paths start from.
#[derive(Clone, Copy, Debug)]
pub enum SampleBase<'a> {
    /// The folder the patch file is in.
    Folder(&'a Path),
    /// One of the examples, whose samples ship inside the app.
    Example,
    /// Nowhere: a patch from the clipboard or an autosave, whose sample
    /// paths are keys already.
    Keys,
}

/// The key for a sample path saved in a patch.
pub fn resolve(file: &str, base: SampleBase) -> String {
    if file.starts_with(EXAMPLE_PREFIX) || Path::new(file).is_absolute() {
        return file.to_string();
    }
    match base {
        SampleBase::Folder(folder) => {
            let relative: PathBuf = file.split('/').collect();
            folder.join(relative).to_string_lossy().into_owned()
        }
        SampleBase::Example => format!("{EXAMPLE_PREFIX}{file}"),
        SampleBase::Keys => file.to_string(),
    }
}

/// The path a patch saved in `folder` stores for the sample `key`:
/// relative, with forward slashes, for a file beside or below the patch,
/// and absolute otherwise. A shipped example's sample keeps its path among
/// the examples.
pub fn to_patch_path(key: &str, folder: &Path) -> String {
    if let Some(name) = key.strip_prefix(EXAMPLE_PREFIX) {
        return name.to_string();
    }
    match Path::new(key).strip_prefix(folder) {
        Ok(relative) => relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/"),
        Err(_) => key.to_string(),
    }
}

/// Whether a sample would need copying to sit beside a patch in `folder`:
/// a file elsewhere, or a shipped example's.
pub fn is_outside(key: &str, folder: &Path) -> bool {
    key.starts_with(EXAMPLE_PREFIX) || Path::new(key).strip_prefix(folder).is_err()
}

/// The file name a key ends in, for showing on a node.
pub fn file_name(key: &str) -> &str {
    let name = key.strip_prefix(EXAMPLE_PREFIX).unwrap_or(key);
    name.rsplit(['/', '\\']).next().unwrap_or(name)
}

/// The folder a patch saved as `patch` keeps its copied samples in:
/// `<patch name> samples`, beside it.
pub fn samples_folder(patch: &Path) -> PathBuf {
    let stem = patch.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "patch".to_string());
    patch.with_file_name(format!("{stem} samples"))
}

/// Writes 32-bit float WAV bytes for a recording, for tests and tools.
pub fn encode_wav(sample: &SampleData) -> Vec<u8> {
    let channels = if sample.is_stereo() { 2 } else { 1 };
    let spec = hound::WavSpec {
        channels,
        sample_rate: sample.sample_rate() as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut bytes, spec).expect("WAV header");
        for (i, &left) in sample.left().iter().enumerate() {
            writer.write_sample(left).expect("in memory");
            if channels == 2 {
                writer.write_sample(sample.right()[i]).expect("in memory");
            }
        }
        writer.finalize().expect("in memory");
    }
    bytes.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_what_it_encodes() {
        let stereo = SampleData::stereo(vec![0.5, -0.25, 0.0], vec![0.1, 0.2, 0.3], 44100.0);
        let decoded = decode_wav(std::io::Cursor::new(encode_wav(&stereo))).unwrap();
        assert_eq!(decoded.sample, stereo);
        assert!(!decoded.truncated);
        let mono = SampleData::mono(vec![0.5; 10], 22050.0);
        assert_eq!(decode_wav(std::io::Cursor::new(encode_wav(&mono))).unwrap().sample, mono);
    }

    #[test]
    fn decodes_integer_wavs() {
        let spec = hound::WavSpec { channels: 1, sample_rate: 48000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut bytes = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
            for s in [16384i16, -32768, 0] {
                writer.write_sample(s).unwrap();
            }
            writer.finalize().unwrap();
        }
        let decoded = decode_wav(std::io::Cursor::new(bytes.into_inner())).unwrap();
        assert_eq!(decoded.sample.left(), &[0.5, -1.0, 0.0]);
    }

    #[test]
    fn a_long_file_is_cut_to_the_cap() {
        // At 10 Hz, the cap is 3000 frames
        let long = SampleData::mono(vec![0.1; 3500], 10.0);
        let decoded = decode_wav(std::io::Cursor::new(encode_wav(&long))).unwrap();
        assert!(decoded.truncated);
        assert_eq!(decoded.sample.frames(), (MAX_SAMPLE_SECONDS * 10.0) as usize);
    }

    #[test]
    fn paths_are_relative_beside_or_below_the_patch_and_absolute_elsewhere() {
        let folder = std::env::temp_dir().join("patches");
        let below = folder.join("kit").join("kick.wav");
        assert_eq!(to_patch_path(&below.to_string_lossy(), &folder), "kit/kick.wav");
        assert_eq!(resolve("kit/kick.wav", SampleBase::Folder(&folder)), below.to_string_lossy());
        let elsewhere = std::env::temp_dir().join("elsewhere.wav");
        let key = elsewhere.to_string_lossy();
        assert_eq!(to_patch_path(&key, &folder), key);
        assert_eq!(resolve(&key, SampleBase::Folder(&folder)), key);
        assert!(is_outside(&key, &folder));
        assert!(!is_outside(&below.to_string_lossy(), &folder));
    }

    #[test]
    fn example_samples_resolve_to_keys_and_save_as_their_paths() {
        assert_eq!(resolve("samples/keys.wav", SampleBase::Example), "example:samples/keys.wav");
        assert_eq!(to_patch_path("example:samples/keys.wav", Path::new("anywhere")), "samples/keys.wav");
        assert!(is_outside("example:samples/keys.wav", Path::new("anywhere")));
        assert_eq!(file_name("example:samples/keys.wav"), "keys.wav");
        assert_eq!(file_name(r"C:\Music\choir ah.wav"), "choir ah.wav");
    }

    #[test]
    fn every_example_sample_loads() {
        for (path, _) in EXAMPLE_SAMPLES {
            let key = resolve(path, SampleBase::Example);
            let decoded = load(&key).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert!(!decoded.sample.is_empty());
        }
    }

    #[test]
    fn a_missing_file_is_an_error_not_a_panic() {
        assert!(load(&std::env::temp_dir().join("no-such-sample.wav").to_string_lossy()).is_err());
        assert!(load("example:samples/no-such.wav").is_err());
    }

    #[test]
    fn copied_samples_go_beside_the_patch() {
        let folder = samples_folder(Path::new("songs").join("Night Drive.json").as_path());
        assert_eq!(folder, Path::new("songs").join("Night Drive samples"));
    }
}
