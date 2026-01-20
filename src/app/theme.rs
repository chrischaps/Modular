//! Theme definitions for the Modular Synth UI
//!
//! Provides color constants, styling utilities, and theme configuration
//! for a dark, audio-software aesthetic.

use eframe::egui::{self, Color32, Stroke, Rounding, Vec2};

/// Background colors
pub mod background {
    use super::Color32;

    /// Main window background - deep dark blue
    pub const MAIN: Color32 = Color32::from_rgb(26, 26, 46);

    /// Grid line color - subtle
    pub const GRID: Color32 = Color32::from_rgb(40, 40, 60);

    /// Panel background - slightly lighter than main
    pub const PANEL: Color32 = Color32::from_rgb(35, 35, 55);

    /// Widget background (buttons, inputs)
    pub const WIDGET: Color32 = Color32::from_rgb(45, 45, 70);

    /// Widget background when hovered
    pub const WIDGET_HOVERED: Color32 = Color32::from_rgb(55, 55, 85);

    /// Widget background when active/pressed
    pub const WIDGET_ACTIVE: Color32 = Color32::from_rgb(65, 65, 100);
}

/// Signal type colors - used for cables and port indicators
pub mod signal {
    use super::Color32;

    /// Audio signal - blue
    pub const AUDIO: Color32 = Color32::from_rgb(66, 165, 245);

    /// Control/CV signal - orange
    pub const CONTROL: Color32 = Color32::from_rgb(255, 183, 77);

    /// Gate signal - green
    pub const GATE: Color32 = Color32::from_rgb(129, 199, 132);

    /// MIDI signal - purple
    pub const MIDI: Color32 = Color32::from_rgb(186, 104, 200);
}

/// Module header colors by category
pub mod module {
    use super::Color32;

    /// Source modules (oscillators) - blue
    pub const SOURCE: Color32 = Color32::from_rgb(66, 165, 245);

    /// Filter modules - green
    pub const FILTER: Color32 = Color32::from_rgb(129, 199, 132);

    /// Modulation modules (envelopes, LFOs) - orange
    pub const MODULATION: Color32 = Color32::from_rgb(255, 183, 77);

    /// Output modules - purple
    pub const OUTPUT: Color32 = Color32::from_rgb(186, 104, 200);

    /// Utility modules - gray
    pub const UTILITY: Color32 = Color32::from_rgb(158, 158, 158);

    /// Effect modules - cyan
    pub const EFFECT: Color32 = Color32::from_rgb(77, 208, 225);
}

/// Text colors
pub mod text {
    use super::Color32;

    /// Primary text - bright white
    pub const PRIMARY: Color32 = Color32::from_rgb(240, 240, 245);

    /// Secondary text - dimmed
    pub const SECONDARY: Color32 = Color32::from_rgb(160, 160, 175);

    /// Disabled text
    pub const DISABLED: Color32 = Color32::from_rgb(100, 100, 115);

    /// Accent/highlight text
    pub const ACCENT: Color32 = Color32::from_rgb(130, 180, 255);
}

/// UI accent colors
pub mod accent {
    use super::Color32;

    /// Primary accent - blue
    pub const PRIMARY: Color32 = Color32::from_rgb(66, 165, 245);

    /// Success/active - green
    pub const SUCCESS: Color32 = Color32::from_rgb(129, 199, 132);

    /// Warning - orange
    pub const WARNING: Color32 = Color32::from_rgb(255, 183, 77);

    /// Error - red
    pub const ERROR: Color32 = Color32::from_rgb(239, 83, 80);
}

/// Grid spacing for the background pattern
pub const GRID_SPACING: f32 = 20.0;

/// Standard rounding for UI elements
pub const ROUNDING: Rounding = Rounding {
    nw: 6.0,
    ne: 6.0,
    sw: 6.0,
    se: 6.0,
};

/// Smaller rounding for compact elements
pub const ROUNDING_SMALL: Rounding = Rounding {
    nw: 4.0,
    ne: 4.0,
    sw: 4.0,
    se: 4.0,
};

/// Apply the dark synth theme to an egui context
pub fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Visuals
    let visuals = &mut style.visuals;
    visuals.dark_mode = true;

    // Window styling
    visuals.window_fill = background::PANEL;
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(60, 60, 80));
    visuals.window_rounding = ROUNDING;

    // Panel styling
    visuals.panel_fill = background::MAIN;

    // Widget styling
    visuals.widgets.noninteractive.bg_fill = background::WIDGET;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, text::SECONDARY);
    visuals.widgets.noninteractive.rounding = ROUNDING_SMALL;

    visuals.widgets.inactive.bg_fill = background::WIDGET;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, text::PRIMARY);
    visuals.widgets.inactive.rounding = ROUNDING_SMALL;

    visuals.widgets.hovered.bg_fill = background::WIDGET_HOVERED;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, text::PRIMARY);
    visuals.widgets.hovered.rounding = ROUNDING_SMALL;

    visuals.widgets.active.bg_fill = background::WIDGET_ACTIVE;
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, accent::PRIMARY);
    visuals.widgets.active.rounding = ROUNDING_SMALL;

    visuals.widgets.open.bg_fill = background::WIDGET_ACTIVE;
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, text::PRIMARY);
    visuals.widgets.open.rounding = ROUNDING_SMALL;

    // Selection styling
    visuals.selection.bg_fill = accent::PRIMARY.gamma_multiply(0.3);
    visuals.selection.stroke = Stroke::new(1.0, accent::PRIMARY);

    // Hyperlink color
    visuals.hyperlink_color = text::ACCENT;

    // Extreme background (for things like text edit backgrounds)
    visuals.extreme_bg_color = Color32::from_rgb(20, 20, 35);

    // Faint background for code/monospace
    visuals.code_bg_color = Color32::from_rgb(35, 35, 50);

    // Spacing
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.window_margin = egui::Margin::same(12.0);

    ctx.set_style(style);
}

/// Draw a grid background pattern on a painter
pub fn draw_grid_background(painter: &egui::Painter, rect: egui::Rect) {
    // Fill with main background color
    painter.rect_filled(rect, 0.0, background::MAIN);

    // Draw vertical grid lines
    let mut x = rect.left() - (rect.left() % GRID_SPACING);
    while x <= rect.right() {
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(1.0, background::GRID),
        );
        x += GRID_SPACING;
    }

    // Draw horizontal grid lines
    let mut y = rect.top() - (rect.top() % GRID_SPACING);
    while y <= rect.bottom() {
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            Stroke::new(1.0, background::GRID),
        );
        y += GRID_SPACING;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_colors_are_distinct() {
        // Ensure signal colors are visually distinct
        assert_ne!(signal::AUDIO, signal::CONTROL);
        assert_ne!(signal::AUDIO, signal::GATE);
        assert_ne!(signal::AUDIO, signal::MIDI);
        assert_ne!(signal::CONTROL, signal::GATE);
        assert_ne!(signal::CONTROL, signal::MIDI);
        assert_ne!(signal::GATE, signal::MIDI);
    }

    #[test]
    fn module_colors_match_categories() {
        // Module colors should correspond to their signal types
        // Source (oscillators) are blue like audio
        assert_eq!(module::SOURCE, signal::AUDIO);
        // Modulation is orange like control
        assert_eq!(module::MODULATION, signal::CONTROL);
        // Output is purple like MIDI
        assert_eq!(module::OUTPUT, signal::MIDI);
    }

    #[test]
    fn grid_spacing_is_reasonable() {
        assert!(GRID_SPACING >= 10.0);
        assert!(GRID_SPACING <= 50.0);
    }
}
