//! Modular Synth - A node-based modular audio synthesizer
//!
//! Entry point for the application.

use eframe::egui;
use modular_synth::app::SynthApp;

fn main() -> eframe::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let test_tone = args.iter().any(|arg| arg == "--test-tone");

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
        Box::new(move |_cc| Ok(Box::new(SynthApp::new(test_tone)))),
    )
}
