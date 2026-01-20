//! Modular Synth - A node-based modular audio synthesizer
//!
//! Entry point for the application.

use eframe::egui;
use modular_synth::engine::{AudioEngine, AudioError};

fn main() -> eframe::Result<()> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let test_tone = args.iter().any(|arg| arg == "--test-tone");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 720.0])
            .with_title("Modular Synth"),
        ..Default::default()
    };

    eframe::run_native(
        "Modular Synth",
        options,
        Box::new(move |_cc| Ok(Box::new(ModularApp::new(test_tone)))),
    )
}

struct ModularApp {
    audio_engine: Result<AudioEngine, AudioError>,
    test_tone_enabled: bool,
    audio_error_message: Option<String>,
}

impl ModularApp {
    fn new(test_tone: bool) -> Self {
        let audio_engine = AudioEngine::new();

        // If test tone requested and engine created successfully, start it
        let audio_error_message = match &audio_engine {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        let mut app = Self {
            audio_engine,
            test_tone_enabled: false,
            audio_error_message,
        };

        // Start engine and enable test tone if requested via CLI
        if test_tone {
            app.start_audio();
            app.toggle_test_tone();
        }

        app
    }

    fn start_audio(&mut self) {
        if let Ok(ref mut engine) = self.audio_engine {
            if let Err(e) = engine.start() {
                self.audio_error_message = Some(e.to_string());
            }
        }
    }

    fn stop_audio(&mut self) {
        if let Ok(ref mut engine) = self.audio_engine {
            if let Err(e) = engine.stop() {
                self.audio_error_message = Some(e.to_string());
            }
        }
    }

    fn toggle_test_tone(&mut self) {
        self.test_tone_enabled = !self.test_tone_enabled;
        if let Ok(ref engine) = self.audio_engine {
            engine.set_test_tone(self.test_tone_enabled);
        }
    }
}

impl eframe::App for ModularApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Track button clicks outside the borrow
        let mut start_clicked = false;
        let mut stop_clicked = false;
        let mut tone_clicked = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Modular Synth");
            ui.label("Node-based audio synthesis");

            ui.add_space(20.0);

            // Audio engine status
            ui.group(|ui| {
                ui.label("Audio Engine");

                if let Some(ref error) = self.audio_error_message {
                    ui.colored_label(egui::Color32::RED, format!("Error: {}", error));
                }

                match &self.audio_engine {
                    Ok(engine) => {
                        ui.label(format!(
                            "Sample Rate: {} Hz | Channels: {}",
                            engine.sample_rate(),
                            engine.channels()
                        ));

                        ui.horizontal(|ui| {
                            let is_running = engine.is_running();

                            if is_running {
                                if ui.button("Stop Audio").clicked() {
                                    stop_clicked = true;
                                }
                            } else if ui.button("Start Audio").clicked() {
                                start_clicked = true;
                            }

                            ui.separator();

                            let tone_label = if self.test_tone_enabled {
                                "Test Tone: ON"
                            } else {
                                "Test Tone: OFF"
                            };

                            if ui.button(tone_label).clicked() {
                                tone_clicked = true;
                            }
                        });
                    }
                    Err(e) => {
                        ui.colored_label(egui::Color32::RED, format!("Audio unavailable: {}", e));
                    }
                }
            });

            // Device list
            if let Ok(ref engine) = self.audio_engine {
                ui.add_space(10.0);
                ui.collapsing("Output Devices", |ui| {
                    for device in engine.enumerate_devices() {
                        let label = if device.is_default {
                            format!("{} (default)", device.name)
                        } else {
                            device.name
                        };
                        ui.label(label);
                    }
                });
            }
        });

        // Handle button clicks after the UI borrow is released
        if start_clicked {
            self.start_audio();
        }
        if stop_clicked {
            self.stop_audio();
        }
        if tone_clicked {
            self.toggle_test_tone();
        }
    }
}
