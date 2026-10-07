//! Modular Synth - A node-based modular audio synthesizer
//!
//! Entry point for the application.

use std::path::PathBuf;

use eframe::egui;
use modular_synth::app::capture::CaptureConfig;
use modular_synth::app::SynthApp;

fn main() -> eframe::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let test_tone = args.iter().any(|arg| arg == "--test-tone");
    // Filming the app: `modular_synth <patch> --capture <script> --out <dir>`
    let capture = match CaptureConfig::from_args(&args) {
        Ok(capture) => capture,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(2);
        }
    };
    // A patch file to open, e.g. `modular_synth patches/lush-pad.json`
    // (skipping the values that follow options)
    let patch_path = args
        .iter()
        .enumerate()
        .skip(1)
        .find(|(i, arg)| !arg.starts_with("--") && !CaptureConfig::is_option(&args[i - 1]))
        .map(|(_, arg)| PathBuf::from(arg));

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 600.0])
        .with_title("Modular Synth");
    if let Some(capture) = &capture {
        viewport = viewport.with_inner_size(capture.size_in_points()).with_resizable(false);
    }
    let options = eframe::NativeOptions {
        viewport,
        // A capture's window size is its own, not one to remember
        persist_window: capture.is_none(),
        ..Default::default()
    };

    eframe::run_native(
        "Modular Synth",
        options,
        Box::new(move |cc| {
            let mut app = SynthApp::new(test_tone);
            app.open_on_launch(patch_path.as_deref());
            match capture {
                Some(config) => {
                    if let Err(e) = app.start_capture(config) {
                        eprintln!("capture: {}", e);
                        std::process::exit(2);
                    }
                }
                None => app.restore_session(cc.storage),
            }
            Ok(Box::new(app))
        }),
    )
}
