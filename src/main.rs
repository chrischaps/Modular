//! Modular Synth - A node-based modular audio synthesizer
//!
//! Entry point for the application.

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Modular Synth"),
        ..Default::default()
    };

    eframe::run_native(
        "Modular Synth",
        options,
        Box::new(|_cc| Ok(Box::new(ModularApp::default()))),
    )
}

struct ModularApp {
    // Placeholder for future state
}

impl Default for ModularApp {
    fn default() -> Self {
        Self {}
    }
}

impl eframe::App for ModularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Modular Synth");
            ui.label("Node-based audio synthesis");
        });
    }
}
