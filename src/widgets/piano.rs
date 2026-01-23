//! Piano keyboard widget for visual feedback of active notes.
//!
//! Provides a one-octave piano keyboard display that shows which notes are
//! currently pressed. Used by the Keyboard and MIDI Note input modules.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

/// Key layout constants - which semitones correspond to white/black keys.
const WHITE_KEY_NOTES: [u8; 7] = [0, 2, 4, 5, 7, 9, 11]; // C D E F G A B
const BLACK_KEY_NOTES: [u8; 5] = [1, 3, 6, 8, 10];       // C# D# F# G# A#
/// Black key positions relative to white keys (fractional position from left).
const BLACK_KEY_POSITIONS: [f32; 5] = [0.75, 1.75, 3.75, 4.75, 5.75]; // After C, D, F, G, A

/// Configuration for the piano keyboard widget.
#[derive(Clone, Debug)]
pub struct PianoConfig {
    /// Total width of the keyboard.
    pub width: f32,
    /// Total height of the keyboard.
    pub height: f32,
    /// Color of white keys when inactive.
    pub white_key_color: Color32,
    /// Color of black keys when inactive.
    pub black_key_color: Color32,
    /// Color of white keys when active/pressed.
    pub white_key_active: Color32,
    /// Color of black keys when active/pressed.
    pub black_key_active: Color32,
    /// Glow color for active keys.
    pub glow_color: Color32,
    /// Whether to show the octave label below.
    pub show_octave: bool,
}

impl Default for PianoConfig {
    fn default() -> Self {
        Self {
            width: 140.0,
            height: 45.0,
            white_key_color: Color32::from_rgb(240, 240, 235),  // Off-white
            black_key_color: Color32::from_rgb(30, 30, 35),     // Near-black
            white_key_active: Color32::from_rgb(100, 180, 255), // Blue tint
            black_key_active: Color32::from_rgb(80, 140, 200),  // Darker blue
            glow_color: Color32::from_rgb(100, 180, 255),       // Blue glow
            show_octave: true,
        }
    }
}

impl PianoConfig {
    /// Create a keyboard-style piano config (blue tint for active keys).
    pub fn keyboard() -> Self {
        Self::default()
    }

    /// Create a MIDI-style piano config (purple tint for active keys).
    pub fn midi() -> Self {
        Self {
            white_key_active: Color32::from_rgb(180, 100, 200), // Purple tint
            black_key_active: Color32::from_rgb(140, 80, 160),  // Darker purple
            glow_color: Color32::from_rgb(180, 100, 200),       // Purple glow
            ..Default::default()
        }
    }

    /// Set the size of the keyboard.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set whether to show the octave label.
    pub fn with_octave_label(mut self, show: bool) -> Self {
        self.show_octave = show;
        self
    }
}

/// Data for the piano display.
#[derive(Clone, Debug, Default)]
pub struct PianoData {
    /// MIDI note numbers currently pressed (0-127).
    pub active_notes: Vec<u8>,
    /// The base MIDI note of the displayed octave (e.g., 60 for C4).
    pub base_note: u8,
    /// Octave shift for display label (e.g., 0 for C4, 1 for C5).
    pub octave_shift: i32,
}

impl PianoData {
    /// Create a new PianoData.
    pub fn new(active_notes: Vec<u8>, base_note: u8, octave_shift: i32) -> Self {
        Self {
            active_notes,
            base_note,
            octave_shift,
        }
    }

    /// Check if a note (0-11 semitone within octave) is active.
    /// Maps any MIDI note to its position within an octave.
    fn is_note_active(&self, semitone: u8) -> bool {
        self.active_notes.iter().any(|&note| note % 12 == semitone)
    }
}

/// A piano keyboard widget showing which notes are currently pressed.
///
/// # Example
/// ```ignore
/// let data = PianoData {
///     active_notes: vec![60, 64, 67], // C4, E4, G4
///     base_note: 60,
///     octave_shift: 0,
/// };
/// let config = PianoConfig::keyboard().with_size(140.0, 45.0);
/// piano(ui, &data, &config);
/// ```
pub fn piano(ui: &mut Ui, data: &PianoData, config: &PianoConfig) -> Response {
    // Calculate total height including label
    let label_height = if config.show_octave { 12.0 } else { 0.0 };
    let total_height = config.height + label_height;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.width, total_height),
        Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let keyboard_rect = Rect::from_min_size(
            rect.min,
            Vec2::new(config.width, config.height),
        );

        // Calculate key dimensions
        let white_key_width = config.width / 7.0;
        let black_key_width = white_key_width * 0.6;
        let black_key_height = config.height * 0.6;

        // Draw white keys first (background)
        for (i, &semitone) in WHITE_KEY_NOTES.iter().enumerate() {
            let key_rect = Rect::from_min_size(
                Pos2::new(
                    keyboard_rect.left() + i as f32 * white_key_width,
                    keyboard_rect.top(),
                ),
                Vec2::new(white_key_width - 1.0, config.height),
            );

            let is_active = data.is_note_active(semitone);

            // Draw glow effect for active keys
            if is_active {
                // Multi-layer glow
                for layer in 0..3 {
                    let glow_alpha = 40 - layer * 12;
                    let expand = (3 - layer) as f32 * 2.0;
                    let glow_rect = key_rect.expand(expand);
                    let glow_color = Color32::from_rgba_unmultiplied(
                        config.glow_color.r(),
                        config.glow_color.g(),
                        config.glow_color.b(),
                        glow_alpha as u8,
                    );
                    painter.rect_filled(glow_rect, 2.0, glow_color);
                }
            }

            // Key background
            let key_color = if is_active {
                config.white_key_active
            } else {
                config.white_key_color
            };
            painter.rect_filled(key_rect, 2.0, key_color);

            // Key border (subtle)
            painter.rect_stroke(
                key_rect,
                2.0,
                egui::Stroke::new(0.5, Color32::from_gray(120)),
            );

            // Add subtle 3D effect (top highlight)
            if !is_active {
                let highlight_rect = Rect::from_min_size(
                    key_rect.min,
                    Vec2::new(key_rect.width(), 2.0),
                );
                painter.rect_filled(highlight_rect, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 80));
            }
        }

        // Draw black keys on top
        for (i, &semitone) in BLACK_KEY_NOTES.iter().enumerate() {
            let x_pos = keyboard_rect.left() + BLACK_KEY_POSITIONS[i] * white_key_width - black_key_width / 2.0;
            let key_rect = Rect::from_min_size(
                Pos2::new(x_pos, keyboard_rect.top()),
                Vec2::new(black_key_width, black_key_height),
            );

            let is_active = data.is_note_active(semitone);

            // Draw glow effect for active keys
            if is_active {
                for layer in 0..3 {
                    let glow_alpha = 50 - layer * 15;
                    let expand = (3 - layer) as f32 * 1.5;
                    let glow_rect = key_rect.expand(expand);
                    let glow_color = Color32::from_rgba_unmultiplied(
                        config.glow_color.r(),
                        config.glow_color.g(),
                        config.glow_color.b(),
                        glow_alpha as u8,
                    );
                    painter.rect_filled(glow_rect, 1.5, glow_color);
                }
            }

            // Key background
            let key_color = if is_active {
                config.black_key_active
            } else {
                config.black_key_color
            };
            painter.rect_filled(key_rect, 1.5, key_color);

            // Subtle highlight on black keys
            if !is_active {
                let highlight_rect = Rect::from_min_size(
                    key_rect.min + Vec2::new(1.0, 1.0),
                    Vec2::new(key_rect.width() - 2.0, 3.0),
                );
                painter.rect_filled(highlight_rect, 1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 20));
            }
        }

        // Draw octave label below
        if config.show_octave {
            let octave_num = 4 + data.octave_shift; // Base octave is C4
            let label = format!("C{}", octave_num);
            let label_pos = Pos2::new(
                rect.center().x,
                keyboard_rect.bottom() + 6.0,
            );
            painter.text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(9.0),
                Color32::from_gray(160),
            );
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piano_config_default() {
        let config = PianoConfig::default();
        assert_eq!(config.width, 140.0);
        assert_eq!(config.height, 45.0);
        assert!(config.show_octave);
    }

    #[test]
    fn test_piano_config_keyboard() {
        let config = PianoConfig::keyboard();
        // Should have blue tint
        assert!(config.white_key_active.b() > config.white_key_active.r());
    }

    #[test]
    fn test_piano_config_midi() {
        let config = PianoConfig::midi();
        // Should have purple tint (R and B both high)
        assert!(config.white_key_active.r() > 150);
        assert!(config.white_key_active.b() > 150);
    }

    #[test]
    fn test_piano_config_with_size() {
        let config = PianoConfig::default().with_size(200.0, 60.0);
        assert_eq!(config.width, 200.0);
        assert_eq!(config.height, 60.0);
    }

    #[test]
    fn test_piano_data_is_note_active() {
        let data = PianoData {
            active_notes: vec![60, 64, 67], // C4, E4, G4
            base_note: 60,
            octave_shift: 0,
        };

        // C (semitone 0) should be active
        assert!(data.is_note_active(0));
        // E (semitone 4) should be active
        assert!(data.is_note_active(4));
        // G (semitone 7) should be active
        assert!(data.is_note_active(7));
        // D (semitone 2) should not be active
        assert!(!data.is_note_active(2));
    }

    #[test]
    fn test_piano_data_octave_wrapping() {
        // Notes from different octaves should light up the same keys
        let data = PianoData {
            active_notes: vec![48, 60, 72], // C3, C4, C5 - all C notes
            base_note: 60,
            octave_shift: 0,
        };

        // All should show as C (semitone 0)
        assert!(data.is_note_active(0));
        // But not other notes
        assert!(!data.is_note_active(1));
    }

    #[test]
    fn test_white_key_notes() {
        // Verify white key notes are correct (C D E F G A B)
        assert_eq!(WHITE_KEY_NOTES, [0, 2, 4, 5, 7, 9, 11]);
    }

    #[test]
    fn test_black_key_notes() {
        // Verify black key notes are correct (C# D# F# G# A#)
        assert_eq!(BLACK_KEY_NOTES, [1, 3, 6, 8, 10]);
    }
}
