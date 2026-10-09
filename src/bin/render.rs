//! Render a patch to a WAV file without opening the app.
//!
//! ```text
//! cargo run --release --bin render -- <patch.json> <out.wav> [--seconds N] [--sample-rate HZ] [--audition] [--input in.wav]
//! ```
//!
//! Prints the peak and RMS level of each channel so renders can be compared
//! before and after a DSP change. Patches that need live input (Keyboard,
//! MIDI Note, Poly MIDI) render silence unless something in the patch
//! triggers them, such as a Clock or Sequencer, or `--audition` is given to
//! play a short phrase into them. Audio Input modules are silent too, unless
//! `--input` gives them a WAV file to hear in place of the input device.

use std::path::PathBuf;
use std::process::ExitCode;

use modular_synth::dsp::analysis::{amp_to_db, peak, rms};
use modular_synth::engine::{read_wav, OfflineRenderer};
use modular_synth::persistence::sample_files::SampleBase;
use modular_synth::persistence::load_from_file;

const USAGE: &str = "usage: render <patch.json> <out.wav> [--seconds N] [--sample-rate HZ] [--block-size N] [--audition] [--input in.wav]";

struct Args {
    patch: PathBuf,
    out: PathBuf,
    seconds: f32,
    sample_rate: u32,
    block_size: usize,
    audition: bool,
    /// A WAV file for Audio Input modules to hear.
    input: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut seconds = 5.0;
    let mut sample_rate = 48_000;
    let mut block_size = 256;
    let mut audition = false;
    let mut input = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{} needs a value", name));
        match arg.as_str() {
            "--seconds" => seconds = value("--seconds")?.parse().map_err(|e| format!("--seconds: {}", e))?,
            "--sample-rate" => {
                sample_rate = value("--sample-rate")?.parse().map_err(|e| format!("--sample-rate: {}", e))?
            }
            "--block-size" => {
                block_size = value("--block-size")?.parse().map_err(|e| format!("--block-size: {}", e))?
            }
            "--audition" => audition = true,
            "--input" => input = Some(PathBuf::from(value("--input")?)),
            "-h" | "--help" => return Err(USAGE.to_string()),
            flag if flag.starts_with("--") => return Err(format!("unknown option {}\n{}", flag, USAGE)),
            _ => positional.push(PathBuf::from(arg)),
        }
    }

    match <[PathBuf; 2]>::try_from(positional) {
        Ok([patch, out]) if seconds > 0.0 && block_size > 0 => Ok(Args {
            patch,
            out,
            seconds,
            sample_rate,
            block_size,
            audition,
            input,
        }),
        _ => Err(USAGE.to_string()),
    }
}

fn run(args: Args) -> Result<(), String> {
    let patch = load_from_file(&args.patch).map_err(|e| format!("{}: {}", args.patch.display(), e))?;
    let (mut renderer, compiled) =
        OfflineRenderer::from_patch(&patch, args.sample_rate as f32, args.block_size)
            .map_err(|e| e.to_string())?;
    for warning in &compiled.warnings {
        eprintln!("warning: {}", warning);
    }
    // Samples saved relative to the patch are beside it
    let folder = args.patch.parent().unwrap_or(std::path::Path::new("."));
    for warning in renderer.load_samples(&compiled, SampleBase::Folder(folder)) {
        eprintln!("warning: {}", warning);
    }
    if let Some(path) = &args.input {
        let (input, rate) = read_wav(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        if rate != args.sample_rate {
            return Err(format!(
                "{} is {} Hz but the render is {} Hz: render with --sample-rate {}",
                path.display(),
                rate,
                args.sample_rate,
                rate
            ));
        }
        renderer.set_audio_input(input);
    }

    let audio = if args.audition {
        renderer.render_audition(&patch, &compiled, args.seconds)
    } else {
        renderer.render_seconds(args.seconds)
    };

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: args.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&args.out, spec)
        .map_err(|e| format!("{}: {}", args.out.display(), e))?;
    for (&l, &r) in audio.left.iter().zip(&audio.right) {
        writer.write_sample(l).map_err(|e| e.to_string())?;
        writer.write_sample(r).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())?;

    println!(
        "rendered '{}' ({:.2} s @ {} Hz) -> {}",
        patch.name,
        args.seconds,
        args.sample_rate,
        args.out.display()
    );
    for (name, channel) in [("L", &audio.left), ("R", &audio.right)] {
        println!(
            "  {}: peak {:6.1} dBFS   rms {:6.1} dBFS",
            name,
            amp_to_db(peak(channel)),
            amp_to_db(rms(channel))
        );
    }
    if audio.left.iter().chain(&audio.right).any(|s| !s.is_finite()) {
        return Err("render contains NaN or infinite samples".to_string());
    }
    Ok(())
}

fn main() -> ExitCode {
    match parse_args().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{}", message);
            ExitCode::FAILURE
        }
    }
}
