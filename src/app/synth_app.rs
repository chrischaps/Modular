//! Main application struct for the Modular Synth
//!
//! Contains the SynthApp which implements eframe::App and manages
//! the synthesizer's UI state, audio engine, and graph state.

use eframe::egui::{self, RichText, Layout, Align};
use crate::engine::{AudioEngine, AudioError};
use super::theme;

/// Main application state for the Modular Synth
pub struct SynthApp {
    /// Audio engine handle
    audio_engine: Result<AudioEngine, AudioError>,

    /// Whether the test tone is currently enabled
    test_tone_enabled: bool,

    /// Last audio error message to display
    audio_error_message: Option<String>,

    /// Whether the transport is "playing" (for future use)
    is_playing: bool,

    /// Whether theme has been applied
    theme_applied: bool,
}

impl SynthApp {
    /// Create a new SynthApp instance
    ///
    /// If `enable_test_tone` is true, audio will start with a test tone immediately.
    pub fn new(enable_test_tone: bool) -> Self {
        let audio_engine = AudioEngine::new();

        let audio_error_message = match &audio_engine {
            Ok(_) => None,
            Err(e) => Some(e.to_string()),
        };

        let mut app = Self {
            audio_engine,
            test_tone_enabled: false,
            audio_error_message,
            is_playing: false,
            theme_applied: false,
        };

        // Start engine and enable test tone if requested
        if enable_test_tone {
            app.start_audio();
            app.toggle_test_tone();
        }

        app
    }

    /// Start the audio engine
    fn start_audio(&mut self) {
        if let Ok(ref mut engine) = self.audio_engine {
            if let Err(e) = engine.start() {
                self.audio_error_message = Some(e.to_string());
            }
        }
    }

    /// Stop the audio engine
    fn stop_audio(&mut self) {
        if let Ok(ref mut engine) = self.audio_engine {
            if let Err(e) = engine.stop() {
                self.audio_error_message = Some(e.to_string());
            }
        }
    }

    /// Toggle the test tone on/off
    fn toggle_test_tone(&mut self) {
        self.test_tone_enabled = !self.test_tone_enabled;
        if let Ok(ref engine) = self.audio_engine {
            engine.set_test_tone(self.test_tone_enabled);
        }
    }

    /// Draw the top toolbar with transport controls and status
    fn draw_toolbar(&mut self, ui: &mut egui::Ui) -> ToolbarActions {
        let mut actions = ToolbarActions::default();

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Application title
            ui.label(RichText::new("MODULAR SYNTH")
                .size(18.0)
                .color(theme::text::PRIMARY)
                .strong());

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Transport controls
            ui.label(RichText::new("Transport").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            // Play/Stop button
            let play_text = if self.is_playing { "⏹ Stop" } else { "▶ Play" };
            let play_color = if self.is_playing {
                theme::accent::WARNING
            } else {
                theme::accent::SUCCESS
            };

            if ui.button(RichText::new(play_text).color(play_color)).clicked() {
                self.is_playing = !self.is_playing;
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);

            // Audio engine controls
            ui.label(RichText::new("Audio").color(theme::text::SECONDARY));
            ui.add_space(8.0);

            match &self.audio_engine {
                Ok(engine) => {
                    let is_running = engine.is_running();

                    if is_running {
                        if ui.button("⏹ Stop Audio").clicked() {
                            actions.stop_audio = true;
                        }
                    } else if ui.button("▶ Start Audio").clicked() {
                        actions.start_audio = true;
                    }

                    ui.add_space(10.0);

                    // Test tone toggle
                    let tone_text = if self.test_tone_enabled {
                        "🔊 Tone ON"
                    } else {
                        "🔇 Tone OFF"
                    };
                    let tone_color = if self.test_tone_enabled {
                        theme::accent::SUCCESS
                    } else {
                        theme::text::SECONDARY
                    };

                    if ui.button(RichText::new(tone_text).color(tone_color)).clicked() {
                        actions.toggle_test_tone = true;
                    }

                    // Status indicator
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let status_color = if is_running {
                            theme::accent::SUCCESS
                        } else {
                            theme::text::DISABLED
                        };
                        let status_text = if is_running { "● Running" } else { "○ Stopped" };
                        ui.label(RichText::new(status_text).color(status_color).small());

                        ui.label(RichText::new(format!(
                            "{}Hz • {}ch",
                            engine.sample_rate(),
                            engine.channels()
                        )).color(theme::text::SECONDARY).small());
                    });
                }
                Err(e) => {
                    ui.label(RichText::new(format!("⚠ Audio unavailable: {}", e))
                        .color(theme::accent::ERROR));
                }
            }
        });

        actions
    }

    /// Draw the main content area with grid background
    fn draw_main_area(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let painter = ui.painter();

        // Draw the grid background
        theme::draw_grid_background(painter, rect);

        // Placeholder content - centered message
        let center = rect.center();
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            "Node Graph Editor\n(Coming in Phase 3)",
            egui::FontId::proportional(24.0),
            theme::text::DISABLED,
        );

        // Reserve the space
        ui.allocate_rect(rect, egui::Sense::hover());
    }

    /// Draw the bottom status bar
    fn draw_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // Error display
            if let Some(ref error) = self.audio_error_message {
                ui.label(RichText::new(format!("⚠ {}", error))
                    .color(theme::accent::ERROR)
                    .small());
            } else {
                ui.label(RichText::new("Ready")
                    .color(theme::text::SECONDARY)
                    .small());
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new("Modular Synth v0.1")
                    .color(theme::text::DISABLED)
                    .small());
            });
        });
    }
}

/// Actions collected from the toolbar for deferred execution
#[derive(Default)]
struct ToolbarActions {
    start_audio: bool,
    stop_audio: bool,
    toggle_test_tone: bool,
}

impl eframe::App for SynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply theme on first frame
        if !self.theme_applied {
            theme::apply_theme(ctx);
            self.theme_applied = true;
        }

        // Top toolbar panel
        let toolbar_actions = egui::TopBottomPanel::top("toolbar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 8.0)))
            .show(ctx, |ui| {
                self.draw_toolbar(ui)
            })
            .inner;

        // Bottom status bar
        egui::TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::none()
                .fill(theme::background::PANEL)
                .inner_margin(egui::Margin::symmetric(0.0, 4.0)))
            .show(ctx, |ui| {
                self.draw_status_bar(ui);
            });

        // Main content area
        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                self.draw_main_area(ui);
            });

        // Handle deferred actions (to avoid borrow checker issues)
        if toolbar_actions.start_audio {
            self.start_audio();
        }
        if toolbar_actions.stop_audio {
            self.stop_audio();
        }
        if toolbar_actions.toggle_test_tone {
            self.toggle_test_tone();
        }
    }
}
