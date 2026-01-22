//! Rotary knob widget for audio parameters.
//!
//! Provides a 3D-styled knob with value display, drag interaction,
//! and fine control via Shift+drag.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use std::ops::RangeInclusive;

use crate::app::theme;

/// Parameter display format for value formatting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParamFormat {
    /// Raw numeric value with optional decimal places.
    Raw { decimals: usize },
    /// Raw numeric value with a custom unit suffix.
    RawWithUnit { decimals: usize, unit: &'static str },
    /// Percentage (0-100%).
    Percent,
    /// Frequency in Hz/kHz.
    Frequency,
    /// Time in ms/s.
    Time,
    /// Decibels (dB).
    Decibels,
    /// Semitones.
    Semitones,
}

impl ParamFormat {
    /// Format a value according to this format.
    pub fn format(&self, value: f32) -> String {
        match self {
            ParamFormat::Raw { decimals } => {
                format!("{:.prec$}", value, prec = decimals)
            }
            ParamFormat::RawWithUnit { decimals, unit } => {
                format!("{:.prec$} {}", value, unit, prec = decimals)
            }
            ParamFormat::Percent => {
                format!("{:.0}%", value * 100.0)
            }
            ParamFormat::Frequency => {
                if value >= 1000.0 {
                    format!("{:.2} kHz", value / 1000.0)
                } else if value >= 100.0 {
                    format!("{:.0} Hz", value)
                } else if value >= 10.0 {
                    format!("{:.1} Hz", value)
                } else {
                    format!("{:.2} Hz", value)
                }
            }
            ParamFormat::Time => {
                if value >= 1.0 {
                    format!("{:.2} s", value)
                } else if value >= 0.01 {
                    format!("{:.0} ms", value * 1000.0)
                } else {
                    format!("{:.1} ms", value * 1000.0)
                }
            }
            ParamFormat::Decibels => {
                if value <= -60.0 {
                    "-∞ dB".to_string()
                } else {
                    format!("{:.1} dB", value)
                }
            }
            ParamFormat::Semitones => {
                if value >= 0.0 {
                    format!("+{:.0} st", value)
                } else {
                    format!("{:.0} st", value)
                }
            }
        }
    }
}

impl Default for ParamFormat {
    fn default() -> Self {
        ParamFormat::Raw { decimals: 2 }
    }
}

/// Configuration for the Knob widget.
#[derive(Clone)]
pub struct KnobConfig {
    /// Size of the knob (diameter).
    pub size: f32,
    /// Value range.
    pub range: RangeInclusive<f32>,
    /// Default value (for double-click reset).
    pub default: f32,
    /// Display format for the value.
    pub format: ParamFormat,
    /// Whether to use logarithmic scaling.
    pub logarithmic: bool,
    /// Label shown below the knob.
    pub label: Option<String>,
    /// Show value display.
    pub show_value: bool,
    /// Drag sensitivity (pixels per full range).
    pub drag_sensitivity: f32,
    /// Fine control multiplier when Shift is held.
    pub fine_multiplier: f32,
}

impl Default for KnobConfig {
    fn default() -> Self {
        Self {
            size: 50.0,
            range: 0.0..=1.0,
            default: 0.5,
            format: ParamFormat::default(),
            logarithmic: false,
            label: None,
            show_value: true,
            drag_sensitivity: 200.0,
            fine_multiplier: 0.1,
        }
    }
}

impl KnobConfig {
    /// Create a frequency knob configuration.
    pub fn frequency(min: f32, max: f32, default: f32) -> Self {
        Self {
            range: min..=max,
            default,
            format: ParamFormat::Frequency,
            logarithmic: true,
            ..Default::default()
        }
    }

    /// Create a time knob configuration.
    pub fn time(min: f32, max: f32, default: f32) -> Self {
        Self {
            range: min..=max,
            default,
            format: ParamFormat::Time,
            logarithmic: false,
            ..Default::default()
        }
    }

    /// Create a percentage knob configuration.
    pub fn percent(default: f32) -> Self {
        Self {
            range: 0.0..=1.0,
            default,
            format: ParamFormat::Percent,
            ..Default::default()
        }
    }

    /// Create a decibel knob configuration.
    pub fn decibels(min: f32, max: f32, default: f32) -> Self {
        Self {
            range: min..=max,
            default,
            format: ParamFormat::Decibels,
            ..Default::default()
        }
    }

    /// Set the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set the size.
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

/// A rotary knob widget for audio parameter control.
///
/// Features:
/// - 3D appearance with highlight and shadow
/// - Value indicator arc
/// - Drag up/down to change value
/// - Shift+drag for fine control
/// - Double-click to reset to default
pub fn knob(ui: &mut Ui, value: &mut f32, config: &KnobConfig) -> Response {
    let desired_size = Vec2::splat(config.size);
    // Scale label/value heights proportionally with knob size (base 36.0 -> 16.0 ratio)
    let text_height = config.size * (16.0 / 36.0);
    let total_height = config.size
        + if config.show_value { text_height } else { 0.0 }
        + if config.label.is_some() { text_height } else { 0.0 };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.size, total_height),
        Sense::click_and_drag(),
    );

    // Handle double-click to reset
    if response.double_clicked() {
        *value = config.default;
    }

    // Handle drag
    if response.dragged() {
        let delta = response.drag_delta();
        let sensitivity = if ui.input(|i| i.modifiers.shift) {
            config.drag_sensitivity / config.fine_multiplier
        } else {
            config.drag_sensitivity
        };

        // Vertical drag: up increases, down decreases
        let delta_normalized = -delta.y / sensitivity;

        if config.logarithmic {
            // Logarithmic scaling
            let min = *config.range.start();
            let max = *config.range.end();
            let log_min = min.ln();
            let log_max = max.ln();
            let current_log = value.ln();
            let new_log = current_log + delta_normalized * (log_max - log_min);
            *value = new_log.exp().clamp(min, max);
        } else {
            // Linear scaling
            let range = config.range.end() - config.range.start();
            *value = (*value + delta_normalized * range)
                .clamp(*config.range.start(), *config.range.end());
        }
    }

    // Draw the knob
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let knob_rect = Rect::from_min_size(rect.min, desired_size);
        let center = knob_rect.center();

        // Scale factor relative to default size (50.0) for proportional scaling
        let scale = config.size / 50.0;
        let radius = config.size / 2.0 - 2.0 * scale;

        // Normalize value for display (0.0 to 1.0)
        let normalized = if config.logarithmic {
            let min = *config.range.start();
            let max = *config.range.end();
            let log_min = min.ln();
            let log_max = max.ln();
            (value.ln() - log_min) / (log_max - log_min)
        } else {
            (*value - config.range.start()) / (config.range.end() - config.range.start())
        };

        // Angle calculation: start from bottom-left (-225°) to bottom-right (+45°)
        // Arc spans 270 degrees
        let start_angle = -225.0_f32.to_radians();
        let end_angle = 45.0_f32.to_radians();
        let angle = start_angle + normalized * (end_angle - start_angle);

        // Draw outer ring (shadow)
        painter.circle(
            center + Vec2::new(1.0 * scale, 2.0 * scale),
            radius + 1.0 * scale,
            Color32::from_rgba_unmultiplied(0, 0, 0, 60),
            Stroke::NONE,
        );

        // Draw knob body (3D gradient effect using multiple circles)
        let base_color = if response.hovered() || response.dragged() {
            theme::background::WIDGET_HOVERED
        } else {
            theme::background::WIDGET
        };

        // Main body
        painter.circle_filled(center, radius, base_color);

        // Highlight (top-left)
        let highlight_offset = Vec2::new(-radius * 0.3, -radius * 0.3);
        let highlight_radius = radius * 0.5;
        painter.circle_filled(
            center + highlight_offset,
            highlight_radius,
            Color32::from_rgba_unmultiplied(255, 255, 255, 20),
        );

        // Inner shadow (bottom-right)
        let shadow_offset = Vec2::new(radius * 0.2, radius * 0.2);
        painter.circle_filled(
            center + shadow_offset,
            radius * 0.6,
            Color32::from_rgba_unmultiplied(0, 0, 0, 15),
        );

        // Draw value arc
        draw_value_arc(
            painter,
            center,
            radius - 4.0 * scale,
            start_angle,
            angle,
            theme::accent::PRIMARY,
            scale,
        );

        // Draw position indicator (notch)
        let notch_inner = radius - 12.0 * scale;
        let notch_outer = radius - 4.0 * scale;
        let notch_start = Pos2::new(
            center.x + notch_inner * angle.cos(),
            center.y + notch_inner * angle.sin(),
        );
        let notch_end = Pos2::new(
            center.x + notch_outer * angle.cos(),
            center.y + notch_outer * angle.sin(),
        );
        painter.line_segment(
            [notch_start, notch_end],
            Stroke::new(2.5 * scale, theme::text::PRIMARY),
        );

        // Draw center dot
        painter.circle_filled(center, 3.0 * scale, theme::text::SECONDARY);

        // Draw outer ring border
        painter.circle_stroke(
            center,
            radius,
            Stroke::new(
                1.0 * scale,
                if response.has_focus() || response.dragged() {
                    theme::accent::PRIMARY
                } else {
                    theme::node::BODY_STROKE
                },
            ),
        );

        // Draw value text
        let mut text_y = knob_rect.bottom() + 2.0 * scale;
        if config.show_value {
            let value_text = config.format.format(*value);
            painter.text(
                Pos2::new(center.x, text_y + 6.0 * scale),
                egui::Align2::CENTER_CENTER,
                value_text,
                egui::FontId::proportional(11.0 * scale),
                theme::text::PRIMARY,
            );
            text_y += 14.0 * scale;
        }

        // Draw label
        if let Some(label) = &config.label {
            painter.text(
                Pos2::new(center.x, text_y + 6.0 * scale),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0 * scale),
                theme::text::SECONDARY,
            );
        }
    }

    response
}

/// Draw a value arc on the knob.
fn draw_value_arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: Color32,
    scale: f32,
) {
    let segments = 32;
    let arc_span = end_angle - start_angle;

    // Draw the arc as line segments
    if arc_span.abs() > 0.01 {
        let mut points = Vec::with_capacity(segments + 1);
        for i in 0..=segments {
            let t = i as f32 / segments as f32;
            let current_angle = start_angle + t * arc_span;
            points.push(Pos2::new(
                center.x + radius * current_angle.cos(),
                center.y + radius * current_angle.sin(),
            ));
        }

        for i in 0..points.len().saturating_sub(1) {
            painter.line_segment([points[i], points[i + 1]], Stroke::new(3.0 * scale, color));
        }
    }
}

/// A compact mini-knob variant for tight layouts.
///
/// Same functionality as the full knob but smaller and without
/// value display or label.
pub fn mini_knob(ui: &mut Ui, value: &mut f32, config: &KnobConfig) -> Response {
    let mini_config = KnobConfig {
        size: 28.0,
        show_value: false,
        label: None,
        ..config.clone()
    };
    knob(ui, value, &mini_config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_format_percent() {
        assert_eq!(ParamFormat::Percent.format(0.5), "50%");
        assert_eq!(ParamFormat::Percent.format(1.0), "100%");
        assert_eq!(ParamFormat::Percent.format(0.0), "0%");
    }

    #[test]
    fn test_param_format_frequency() {
        assert_eq!(ParamFormat::Frequency.format(440.0), "440 Hz");
        assert_eq!(ParamFormat::Frequency.format(1000.0), "1.00 kHz");
        assert_eq!(ParamFormat::Frequency.format(20000.0), "20.00 kHz");
        assert_eq!(ParamFormat::Frequency.format(50.0), "50.0 Hz");
        assert_eq!(ParamFormat::Frequency.format(5.0), "5.00 Hz");
    }

    #[test]
    fn test_param_format_time() {
        assert_eq!(ParamFormat::Time.format(1.0), "1.00 s");
        assert_eq!(ParamFormat::Time.format(0.5), "500 ms");
        assert_eq!(ParamFormat::Time.format(0.001), "1.0 ms");
    }

    #[test]
    fn test_param_format_decibels() {
        assert_eq!(ParamFormat::Decibels.format(0.0), "0.0 dB");
        assert_eq!(ParamFormat::Decibels.format(-6.0), "-6.0 dB");
        assert_eq!(ParamFormat::Decibels.format(-70.0), "-∞ dB");
    }

    #[test]
    fn test_param_format_semitones() {
        assert_eq!(ParamFormat::Semitones.format(0.0), "+0 st");
        assert_eq!(ParamFormat::Semitones.format(12.0), "+12 st");
        assert_eq!(ParamFormat::Semitones.format(-7.0), "-7 st");
    }

    #[test]
    fn test_knob_config_default() {
        let config = KnobConfig::default();
        assert_eq!(config.size, 50.0);
        assert_eq!(*config.range.start(), 0.0);
        assert_eq!(*config.range.end(), 1.0);
        assert!(!config.logarithmic);
    }

    #[test]
    fn test_knob_config_frequency() {
        let config = KnobConfig::frequency(20.0, 20000.0, 440.0);
        assert_eq!(*config.range.start(), 20.0);
        assert_eq!(*config.range.end(), 20000.0);
        assert_eq!(config.default, 440.0);
        assert!(config.logarithmic);
        assert_eq!(config.format, ParamFormat::Frequency);
    }

    #[test]
    fn test_knob_config_with_label() {
        let config = KnobConfig::default().with_label("Volume");
        assert_eq!(config.label, Some("Volume".to_string()));
    }

    #[test]
    fn test_knob_config_with_size() {
        let config = KnobConfig::default().with_size(60.0);
        assert_eq!(config.size, 60.0);
    }
}
