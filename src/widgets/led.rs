//! LED indicator widget for visual feedback.
//!
//! Provides a small LED-style indicator light that can show on/off states
//! or continuous brightness levels. Useful for gate indicators, activity
//! lights, and status displays.

use eframe::egui::{self, Color32, Pos2, Response, Sense, Ui, Vec2};

/// Configuration for the LED indicator widget.
#[derive(Clone, Debug)]
pub struct LedConfig {
    /// Size (diameter) of the LED.
    pub size: f32,
    /// Color when the LED is fully on.
    pub on_color: Color32,
    /// Color when the LED is off.
    pub off_color: Color32,
    /// Optional label displayed next to the LED.
    pub label: Option<String>,
    /// Whether to show a glow effect when on.
    pub show_glow: bool,
    /// Glow radius multiplier (1.0 = same as LED size).
    pub glow_radius: f32,
}

impl Default for LedConfig {
    fn default() -> Self {
        Self {
            size: 12.0,
            on_color: Color32::from_rgb(100, 255, 100), // Bright green
            off_color: Color32::from_rgb(40, 60, 40),   // Dark green
            label: None,
            show_glow: true,
            glow_radius: 1.5,
        }
    }
}

impl LedConfig {
    /// Create a green LED (default gate/trigger indicator).
    pub fn green() -> Self {
        Self::default()
    }

    /// Create a red LED.
    pub fn red() -> Self {
        Self {
            on_color: Color32::from_rgb(255, 80, 80),
            off_color: Color32::from_rgb(80, 40, 40),
            ..Default::default()
        }
    }

    /// Create an orange LED (matches control signal color).
    pub fn orange() -> Self {
        Self {
            on_color: Color32::from_rgb(255, 180, 80),
            off_color: Color32::from_rgb(80, 60, 40),
            ..Default::default()
        }
    }

    /// Create a blue LED (matches audio signal color).
    pub fn blue() -> Self {
        Self {
            on_color: Color32::from_rgb(100, 180, 255),
            off_color: Color32::from_rgb(40, 60, 80),
            ..Default::default()
        }
    }

    /// Set the size of the LED.
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    /// Set the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Enable or disable the glow effect.
    pub fn with_glow(mut self, enabled: bool) -> Self {
        self.show_glow = enabled;
        self
    }

    /// Set custom on/off colors.
    pub fn with_colors(mut self, on_color: Color32, off_color: Color32) -> Self {
        self.on_color = on_color;
        self.off_color = off_color;
        self
    }
}

/// An LED indicator widget.
///
/// Displays a small circular light that can show binary on/off states
/// or continuous brightness levels (0.0 to 1.0).
///
/// # Example
/// ```ignore
/// // Binary on/off
/// led(ui, gate_is_high, &LedConfig::green().with_label("Gate"));
///
/// // Continuous brightness
/// led(ui, envelope_level, &LedConfig::orange());
/// ```
pub fn led(ui: &mut Ui, brightness: f32, config: &LedConfig) -> Response {
    let brightness = brightness.clamp(0.0, 1.0);

    // Calculate total widget size including label
    let label_height = if config.label.is_some() { 14.0 } else { 0.0 };
    let total_height = config.size + label_height;

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.size + 8.0, total_height),
        Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // LED center position
        let center = Pos2::new(
            rect.center().x,
            rect.min.y + config.size / 2.0,
        );
        let radius = config.size / 2.0 - 1.0;

        // Interpolate color based on brightness
        let current_color = interpolate_color(config.off_color, config.on_color, brightness);

        // Draw glow effect when brightness > 0
        if config.show_glow && brightness > 0.01 {
            let glow_alpha = (brightness * 80.0) as u8;

            // Outer glow (multiple layers for smooth falloff)
            let glow_radius = radius * config.glow_radius;
            for i in 0..3 {
                let r = glow_radius - (i as f32 * glow_radius / 4.0);
                let alpha = glow_alpha / (i as u8 + 2);
                let layer_color = Color32::from_rgba_unmultiplied(
                    config.on_color.r(),
                    config.on_color.g(),
                    config.on_color.b(),
                    alpha,
                );
                painter.circle_filled(center, r, layer_color);
            }
        }

        // Draw LED body (outer ring for 3D effect)
        let ring_color = Color32::from_rgba_unmultiplied(0, 0, 0, 60);
        painter.circle_filled(center, radius + 1.0, ring_color);

        // Main LED body
        painter.circle_filled(center, radius, current_color);

        // Highlight (top-left reflection)
        let highlight_offset = Vec2::new(-radius * 0.3, -radius * 0.3);
        let highlight_radius = radius * 0.3;
        let highlight_alpha = 40 + (brightness * 40.0) as u8;
        painter.circle_filled(
            center + highlight_offset,
            highlight_radius,
            Color32::from_rgba_unmultiplied(255, 255, 255, highlight_alpha),
        );

        // Inner shadow (bottom-right)
        let shadow_offset = Vec2::new(radius * 0.2, radius * 0.2);
        painter.circle_filled(
            center + shadow_offset,
            radius * 0.4,
            Color32::from_rgba_unmultiplied(0, 0, 0, 20),
        );

        // Draw label if present
        if let Some(label) = &config.label {
            let label_pos = Pos2::new(center.x, rect.min.y + config.size + 8.0);
            painter.text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                Color32::from_gray(180),
            );
        }
    }

    response
}

/// Helper function to interpolate between two colors.
fn interpolate_color(from: Color32, to: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        (from.r() as f32 + (to.r() as f32 - from.r() as f32) * t) as u8,
        (from.g() as f32 + (to.g() as f32 - from.g() as f32) * t) as u8,
        (from.b() as f32 + (to.b() as f32 - from.b() as f32) * t) as u8,
        (from.a() as f32 + (to.a() as f32 - from.a() as f32) * t) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_led_config_default() {
        let config = LedConfig::default();
        assert_eq!(config.size, 12.0);
        assert!(config.show_glow);
        assert!(config.label.is_none());
    }

    #[test]
    fn test_led_config_colors() {
        let green = LedConfig::green();
        let red = LedConfig::red();
        let orange = LedConfig::orange();
        let blue = LedConfig::blue();

        // Each should have distinct on_color
        assert_ne!(green.on_color, red.on_color);
        assert_ne!(red.on_color, orange.on_color);
        assert_ne!(orange.on_color, blue.on_color);
    }

    #[test]
    fn test_led_config_with_size() {
        let config = LedConfig::default().with_size(20.0);
        assert_eq!(config.size, 20.0);
    }

    #[test]
    fn test_led_config_with_label() {
        let config = LedConfig::default().with_label("Gate");
        assert_eq!(config.label, Some("Gate".to_string()));
    }

    #[test]
    fn test_led_config_with_glow() {
        let config = LedConfig::default().with_glow(false);
        assert!(!config.show_glow);
    }

    #[test]
    fn test_led_config_with_colors() {
        let on = Color32::YELLOW;
        let off = Color32::DARK_GRAY;
        let config = LedConfig::default().with_colors(on, off);
        assert_eq!(config.on_color, on);
        assert_eq!(config.off_color, off);
    }

    #[test]
    fn test_interpolate_color_extremes() {
        let black = Color32::BLACK;
        let white = Color32::WHITE;

        let at_zero = interpolate_color(black, white, 0.0);
        let at_one = interpolate_color(black, white, 1.0);

        assert_eq!(at_zero, black);
        assert_eq!(at_one, white);
    }

    #[test]
    fn test_interpolate_color_middle() {
        let black = Color32::from_rgb(0, 0, 0);
        let white = Color32::from_rgb(200, 200, 200);

        let middle = interpolate_color(black, white, 0.5);

        // Should be approximately in the middle
        assert!(middle.r() >= 95 && middle.r() <= 105);
        assert!(middle.g() >= 95 && middle.g() <= 105);
        assert!(middle.b() >= 95 && middle.b() <= 105);
    }

    #[test]
    fn test_interpolate_color_clamped() {
        let black = Color32::BLACK;
        let white = Color32::WHITE;

        // Values outside 0-1 should be clamped
        let below = interpolate_color(black, white, -0.5);
        let above = interpolate_color(black, white, 1.5);

        assert_eq!(below, black);
        assert_eq!(above, white);
    }
}
