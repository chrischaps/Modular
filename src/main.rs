//! Modular Synth - A node-based modular audio synthesizer
//!
//! Entry point for the application.

use std::path::PathBuf;

use eframe::egui;
use modular_synth::app::SynthApp;

fn main() -> eframe::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let test_tone = args.iter().any(|arg| arg == "--test-tone");
    // A patch file to open, e.g. `modular_synth patches/lush-pad.json`
    let patch_path = args.iter().skip(1).find(|arg| !arg.starts_with("--")).map(PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Modular Synth"),
        ..Default::default()
    };

    eframe::run_native(
        "Modular Synth",
        options,
        Box::new(move |_cc| {
            let mut app = SynthApp::new(test_tone);
            app.open_on_launch(patch_path.as_deref());
            Ok(Box::new(app))
        }),
    )
}
