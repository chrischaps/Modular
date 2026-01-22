//! Spectrum display widget for visualizing frequency response curves.
//!
//! Provides a frequency-domain display for filter response visualization,
//! EQ curves, and spectrum analyzers with logarithmic frequency scaling.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::app::theme;

/// Display style for the spectrum widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SpectrumStyle {
    /// Line only - shows frequency response as a curve.
    #[default]
    Line,
    /// Filled area from curve to bottom.
    Filled,
    /// Gradient fill with intensity mapping.
    Gradient,
    /// Bar graph style (for spectrum analyzers).
    Bars,
}

/// Configuration for the SpectrumDisplay widget.
#[derive(Clone)]
pub struct SpectrumConfig {
    /// Size of the display (width x height).
    pub size: Vec2,
    /// Main color for the curve/bars.
    pub color: Color32,
    /// Secondary color for gradients (optional).
    pub secondary_color: Option<Color32>,
    /// Line thickness (for Line style).
    pub line_thickness: f32,
    /// Whether to use logarithmic frequency scaling.
    pub log_frequency: bool,
    /// Minimum frequency to display (Hz).
    pub min_freq: f32,
    /// Maximum frequency to display (Hz).
    pub max_freq: f32,
    /// Minimum amplitude to display (dB).
    pub min_db: f32,
    /// Maximum amplitude to display (dB).
    pub max_db: f32,
    /// Display style.
    pub style: SpectrumStyle,
    /// Show frequency grid lines.
    pub show_grid: bool,
    /// Number of frequency grid divisions.
    pub grid_divisions: usize,
    /// Show glow effect.
    pub glow: bool,
}

impl Default for SpectrumConfig {
    fn default() -> Self {
        Self {
            size: Vec2::new(120.0, 60.0),
            color: theme::module::FILTER,
            secondary_color: None,
            line_thickness: 1.5,
            log_frequency: true,
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -24.0,
            max_db: 6.0,
            style: SpectrumStyle::Filled,
            show_grid: true,
            grid_divisions: 4,
            glow: true,
        }
    }
}

impl SpectrumConfig {
    /// Create config for a low-pass filter response display.
    pub fn lowpass() -> Self {
        Self {
            color: theme::module::FILTER,
            style: SpectrumStyle::Filled,
            min_db: -48.0,
            max_db: 6.0,
            ..Default::default()
        }
    }

    /// Create config for a bandpass filter response display.
    pub fn bandpass() -> Self {
        Self {
            color: theme::module::FILTER,
            style: SpectrumStyle::Filled,
            min_db: -48.0,
            max_db: 6.0,
            ..Default::default()
        }
    }

    /// Create config for a spectrum analyzer display.
    pub fn analyzer() -> Self {
        Self {
            color: theme::signal::AUDIO,
            style: SpectrumStyle::Bars,
            min_db: -60.0,
            max_db: 0.0,
            grid_divisions: 6,
            glow: false,
            ..Default::default()
        }
    }

    /// Create config for an EQ curve display.
    pub fn eq_curve() -> Self {
        Self {
            color: theme::accent::PRIMARY,
            style: SpectrumStyle::Line,
            min_db: -12.0,
            max_db: 12.0,
            glow: true,
            ..Default::default()
        }
    }

    /// Set the display size.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = Vec2::new(width, height);
        self
    }

    /// Set the main color.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Set the frequency range.
    pub fn with_freq_range(mut self, min: f32, max: f32) -> Self {
        self.min_freq = min;
        self.max_freq = max;
        self
    }

    /// Set the amplitude range in dB.
    pub fn with_db_range(mut self, min: f32, max: f32) -> Self {
        self.min_db = min;
        self.max_db = max;
        self
    }

    /// Set the display style.
    pub fn with_style(mut self, style: SpectrumStyle) -> Self {
        self.style = style;
        self
    }

    /// Enable or disable glow effect.
    pub fn with_glow(mut self, glow: bool) -> Self {
        self.glow = glow;
        self
    }
}

/// A frequency response point.
#[derive(Clone, Copy, Debug)]
pub struct FrequencyPoint {
    /// Frequency in Hz.
    pub frequency: f32,
    /// Amplitude in dB.
    pub amplitude_db: f32,
}

impl FrequencyPoint {
    /// Create a new frequency point.
    pub fn new(frequency: f32, amplitude_db: f32) -> Self {
        Self { frequency, amplitude_db }
    }
}

/// A spectrum display widget for visualizing frequency response curves.
///
/// Features:
/// - Logarithmic or linear frequency scaling
/// - Multiple display styles (line, filled, bars)
/// - dB amplitude scaling
/// - Grid overlay for reference
/// - Glow effect for visual appeal
pub fn spectrum_display(
    ui: &mut Ui,
    response_curve: &[FrequencyPoint],
    config: &SpectrumConfig,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(config.size, Sense::hover());

    // Calculate zoom scale factor based on height (default 60.0)
    let zoom_scale = config.size.y / 60.0;

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Draw background
        draw_spectrum_background(painter, rect, config, zoom_scale);

        // Skip if no points
        if response_curve.is_empty() {
            return response;
        }

        // Convert frequency points to screen coordinates
        let points = calculate_spectrum_points(response_curve, rect, config);

        match config.style {
            SpectrumStyle::Line => {
                // Draw glow
                if config.glow && points.len() >= 2 {
                    let glow_color = Color32::from_rgba_unmultiplied(
                        config.color.r(),
                        config.color.g(),
                        config.color.b(),
                        50,
                    );
                    draw_spectrum_line(painter, &points, glow_color, config.line_thickness * 3.0);
                }
                // Draw main line
                if points.len() >= 2 {
                    draw_spectrum_line(painter, &points, config.color, config.line_thickness);
                }
            }
            SpectrumStyle::Filled => {
                draw_filled_spectrum(painter, &points, rect, config);
                // Draw outline
                if points.len() >= 2 {
                    draw_spectrum_line(painter, &points, config.color, config.line_thickness);
                }
            }
            SpectrumStyle::Gradient => {
                draw_gradient_spectrum(painter, &points, rect, config);
            }
            SpectrumStyle::Bars => {
                draw_bar_spectrum(painter, response_curve, rect, config);
            }
        }
    }

    response
}

/// Draw the background grid for the spectrum display.
fn draw_spectrum_background(painter: &egui::Painter, rect: Rect, config: &SpectrumConfig, zoom_scale: f32) {
    // Dark background
    painter.rect_filled(
        rect,
        2.0 * zoom_scale,
        Color32::from_rgb(20, 22, 30),
    );

    if config.show_grid {
        let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 15);
        let label_color = Color32::from_rgba_unmultiplied(255, 255, 255, 40);

        // Frequency grid lines (logarithmic spacing)
        let freq_markers = if config.log_frequency {
            vec![20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0]
        } else {
            let step = (config.max_freq - config.min_freq) / config.grid_divisions as f32;
            (0..=config.grid_divisions)
                .map(|i| config.min_freq + i as f32 * step)
                .collect()
        };

        for freq in &freq_markers {
            if *freq >= config.min_freq && *freq <= config.max_freq {
                let x = freq_to_x(*freq, rect, config);
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.5 * zoom_scale, grid_color),
                );
            }
        }

        // Amplitude grid lines (dB)
        let db_step = (config.max_db - config.min_db) / config.grid_divisions as f32;
        for i in 0..=config.grid_divisions {
            let db = config.min_db + i as f32 * db_step;
            let y = db_to_y(db, rect, config);
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(0.5 * zoom_scale, grid_color),
            );

            // dB labels (small, on left edge)
            if i > 0 && i < config.grid_divisions {
                let label = format!("{:.0}", db);
                painter.text(
                    Pos2::new(rect.left() + 2.0 * zoom_scale, y - 6.0 * zoom_scale),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(8.0 * zoom_scale),
                    label_color,
                );
            }
        }
    }

    // Border
    painter.rect_stroke(
        rect,
        2.0 * zoom_scale,
        Stroke::new(1.0 * zoom_scale, Color32::from_rgb(50, 55, 70)),
    );
}

/// Convert frequency to X coordinate.
fn freq_to_x(freq: f32, rect: Rect, config: &SpectrumConfig) -> f32 {
    let normalized = if config.log_frequency {
        let log_min = config.min_freq.ln();
        let log_max = config.max_freq.ln();
        (freq.ln() - log_min) / (log_max - log_min)
    } else {
        (freq - config.min_freq) / (config.max_freq - config.min_freq)
    };
    rect.left() + normalized * rect.width()
}

/// Convert dB amplitude to Y coordinate.
fn db_to_y(db: f32, rect: Rect, config: &SpectrumConfig) -> f32 {
    let normalized = (db - config.min_db) / (config.max_db - config.min_db);
    rect.bottom() - normalized * rect.height()
}

/// Calculate screen points from frequency response data.
fn calculate_spectrum_points(
    response_curve: &[FrequencyPoint],
    rect: Rect,
    config: &SpectrumConfig,
) -> Vec<Pos2> {
    response_curve
        .iter()
        .filter(|p| p.frequency >= config.min_freq && p.frequency <= config.max_freq)
        .map(|p| {
            let x = freq_to_x(p.frequency, rect, config);
            let y = db_to_y(p.amplitude_db.clamp(config.min_db, config.max_db), rect, config);
            Pos2::new(x, y)
        })
        .collect()
}

/// Draw spectrum as a line.
fn draw_spectrum_line(painter: &egui::Painter, points: &[Pos2], color: Color32, thickness: f32) {
    if points.len() < 2 {
        return;
    }

    for i in 0..points.len() - 1 {
        painter.line_segment([points[i], points[i + 1]], Stroke::new(thickness, color));
    }
}

/// Draw filled spectrum area.
fn draw_filled_spectrum(painter: &egui::Painter, points: &[Pos2], rect: Rect, config: &SpectrumConfig) {
    if points.is_empty() {
        return;
    }

    let fill_color = Color32::from_rgba_unmultiplied(
        config.color.r(),
        config.color.g(),
        config.color.b(),
        60,
    );

    // Create filled polygon from curve to bottom
    let mut polygon_points = Vec::with_capacity(points.len() + 2);

    // Start at bottom-left
    if let Some(first) = points.first() {
        polygon_points.push(Pos2::new(first.x, rect.bottom()));
    }

    // Add all curve points
    polygon_points.extend_from_slice(points);

    // End at bottom-right
    if let Some(last) = points.last() {
        polygon_points.push(Pos2::new(last.x, rect.bottom()));
    }

    // Draw as filled polygon using triangles
    if polygon_points.len() >= 3 {
        let base_y = rect.bottom();
        for i in 1..points.len() {
            let prev = points[i - 1];
            let curr = points[i];

            painter.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(prev.x, base_y),
                    prev,
                    curr,
                    Pos2::new(curr.x, base_y),
                ],
                fill_color,
                Stroke::NONE,
            ));
        }
    }
}

/// Draw gradient-filled spectrum.
fn draw_gradient_spectrum(painter: &egui::Painter, points: &[Pos2], rect: Rect, config: &SpectrumConfig) {
    if points.is_empty() {
        return;
    }

    let secondary = config.secondary_color.unwrap_or_else(|| {
        let c = config.color;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 20)
    });

    // Draw vertical strips with gradient from point to bottom
    for i in 0..points.len().saturating_sub(1) {
        let p1 = points[i];
        let p2 = points[i + 1];

        // Use mesh for gradient
        let top_color = Color32::from_rgba_unmultiplied(
            config.color.r(),
            config.color.g(),
            config.color.b(),
            100,
        );

        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(p1.x, rect.bottom()),
                p1,
                p2,
                Pos2::new(p2.x, rect.bottom()),
            ],
            top_color,
            Stroke::NONE,
        ));

        // Add secondary fill at bottom
        let bottom_height = rect.height() * 0.3;
        painter.add(egui::Shape::convex_polygon(
            vec![
                Pos2::new(p1.x, rect.bottom()),
                Pos2::new(p1.x, rect.bottom() - bottom_height),
                Pos2::new(p2.x, rect.bottom() - bottom_height),
                Pos2::new(p2.x, rect.bottom()),
            ],
            secondary,
            Stroke::NONE,
        ));
    }

    // Draw the line on top
    draw_spectrum_line(painter, points, config.color, config.line_thickness);
}

/// Draw spectrum as bars (for spectrum analyzer).
fn draw_bar_spectrum(
    painter: &egui::Painter,
    response_curve: &[FrequencyPoint],
    rect: Rect,
    config: &SpectrumConfig,
) {
    if response_curve.is_empty() {
        return;
    }

    let num_bars = response_curve.len();
    let bar_spacing = 1.0;
    let available_width = rect.width() - (num_bars as f32 - 1.0) * bar_spacing;
    let bar_width = (available_width / num_bars as f32).max(2.0);

    for (i, point) in response_curve.iter().enumerate() {
        let x = rect.left() + i as f32 * (bar_width + bar_spacing);
        let height_ratio = (point.amplitude_db - config.min_db) / (config.max_db - config.min_db);
        let bar_height = height_ratio.clamp(0.0, 1.0) * rect.height();

        let bar_rect = Rect::from_min_size(
            Pos2::new(x, rect.bottom() - bar_height),
            Vec2::new(bar_width, bar_height),
        );

        // Bar fill with gradient effect
        let intensity = height_ratio.clamp(0.0, 1.0);
        let bar_color = if intensity > 0.8 {
            theme::accent::ERROR // Red for peaks
        } else if intensity > 0.6 {
            theme::accent::WARNING // Yellow/orange for high levels
        } else {
            config.color
        };

        painter.rect_filled(bar_rect, 1.0, bar_color);

        // Subtle highlight at top
        if bar_height > 2.0 {
            painter.rect_filled(
                Rect::from_min_size(
                    bar_rect.min,
                    Vec2::new(bar_width, 2.0),
                ),
                0.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 80),
            );
        }
    }
}

/// Generate a simple filter response curve for visualization.
///
/// This is useful for showing filter response without computing the actual
/// frequency response from filter coefficients.
pub fn generate_filter_response(
    filter_type: FilterResponseType,
    cutoff_hz: f32,
    resonance: f32,
    num_points: usize,
) -> Vec<FrequencyPoint> {
    let mut points = Vec::with_capacity(num_points);

    let min_freq = 20.0_f32;
    let max_freq = 20000.0_f32;
    let log_min = min_freq.ln();
    let log_max = max_freq.ln();

    for i in 0..num_points {
        let t = i as f32 / (num_points - 1) as f32;
        let freq = (log_min + t * (log_max - log_min)).exp();

        let amplitude_db = match filter_type {
            FilterResponseType::LowPass => {
                let ratio = freq / cutoff_hz;
                let response = 1.0 / (1.0 + ratio.powi(4));
                // Add resonance peak
                let res_peak = if (ratio - 1.0).abs() < 0.3 {
                    resonance * 12.0 * (1.0 - (ratio - 1.0).abs() / 0.3)
                } else {
                    0.0
                };
                20.0 * response.log10() + res_peak
            }
            FilterResponseType::HighPass => {
                let ratio = cutoff_hz / freq;
                let response = 1.0 / (1.0 + ratio.powi(4));
                let res_peak = if (freq / cutoff_hz - 1.0).abs() < 0.3 {
                    resonance * 12.0 * (1.0 - (freq / cutoff_hz - 1.0).abs() / 0.3)
                } else {
                    0.0
                };
                20.0 * response.log10() + res_peak
            }
            FilterResponseType::BandPass { bandwidth } => {
                let center = cutoff_hz;
                let dist = (freq.ln() - center.ln()).abs() / bandwidth;
                let response = (-dist * dist * 2.0).exp();
                let res_peak = if dist < 0.5 {
                    resonance * 6.0 * (1.0 - dist * 2.0)
                } else {
                    0.0
                };
                20.0 * response.log10() + res_peak
            }
            FilterResponseType::Notch { width } => {
                let center = cutoff_hz;
                let dist = (freq.ln() - center.ln()).abs() / width;
                let notch = 1.0 - (-dist * dist * 8.0).exp() * (1.0 - 0.1 * (1.0 - resonance));
                20.0 * notch.max(0.001).log10()
            }
            FilterResponseType::Shelf { gain_db } => {
                let ratio = freq / cutoff_hz;
                let transition = 1.0 / (1.0 + (-4.0 * (ratio.log2())).exp());
                gain_db * transition
            }
        };

        points.push(FrequencyPoint::new(freq, amplitude_db.clamp(-60.0, 24.0)));
    }

    points
}

/// Filter response types for visualization.
#[derive(Clone, Copy, Debug)]
pub enum FilterResponseType {
    /// Low-pass filter (passes frequencies below cutoff).
    LowPass,
    /// High-pass filter (passes frequencies above cutoff).
    HighPass,
    /// Band-pass filter (passes frequencies around center).
    BandPass { bandwidth: f32 },
    /// Notch filter (attenuates frequencies around center).
    Notch { width: f32 },
    /// Shelf filter (boost/cut above or below cutoff).
    Shelf { gain_db: f32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_config_default() {
        let config = SpectrumConfig::default();
        assert_eq!(config.size.x, 120.0);
        assert_eq!(config.size.y, 60.0);
        assert!(config.log_frequency);
        assert_eq!(config.style, SpectrumStyle::Filled);
    }

    #[test]
    fn test_spectrum_config_lowpass() {
        let config = SpectrumConfig::lowpass();
        assert_eq!(config.min_db, -48.0);
        assert_eq!(config.style, SpectrumStyle::Filled);
    }

    #[test]
    fn test_spectrum_config_analyzer() {
        let config = SpectrumConfig::analyzer();
        assert_eq!(config.style, SpectrumStyle::Bars);
        assert!(!config.glow);
    }

    #[test]
    fn test_spectrum_config_builder() {
        let config = SpectrumConfig::default()
            .with_size(200.0, 100.0)
            .with_freq_range(100.0, 10000.0)
            .with_db_range(-48.0, 12.0)
            .with_style(SpectrumStyle::Line)
            .with_glow(false);

        assert_eq!(config.size, Vec2::new(200.0, 100.0));
        assert_eq!(config.min_freq, 100.0);
        assert_eq!(config.max_freq, 10000.0);
        assert_eq!(config.min_db, -48.0);
        assert_eq!(config.max_db, 12.0);
        assert_eq!(config.style, SpectrumStyle::Line);
        assert!(!config.glow);
    }

    #[test]
    fn test_frequency_point() {
        let point = FrequencyPoint::new(1000.0, -6.0);
        assert_eq!(point.frequency, 1000.0);
        assert_eq!(point.amplitude_db, -6.0);
    }

    #[test]
    fn test_generate_lowpass_response() {
        let response = generate_filter_response(
            FilterResponseType::LowPass,
            1000.0, // 1kHz cutoff
            0.5,    // moderate resonance
            50,
        );

        assert_eq!(response.len(), 50);

        // Check that low frequencies are near 0dB
        let low_freq_point = response.iter()
            .find(|p| p.frequency < 200.0)
            .unwrap();
        assert!(low_freq_point.amplitude_db > -6.0, "Low frequencies should pass");

        // Check that high frequencies are attenuated
        let high_freq_point = response.iter()
            .find(|p| p.frequency > 5000.0)
            .unwrap();
        assert!(high_freq_point.amplitude_db < -12.0, "High frequencies should be attenuated");
    }

    #[test]
    fn test_generate_highpass_response() {
        let response = generate_filter_response(
            FilterResponseType::HighPass,
            1000.0,
            0.5,
            50,
        );

        // Check that low frequencies are attenuated
        let low_freq_point = response.iter()
            .find(|p| p.frequency < 200.0)
            .unwrap();
        assert!(low_freq_point.amplitude_db < -12.0, "Low frequencies should be attenuated");

        // Check that high frequencies pass
        let high_freq_point = response.iter()
            .find(|p| p.frequency > 5000.0)
            .unwrap();
        assert!(high_freq_point.amplitude_db > -6.0, "High frequencies should pass");
    }

    #[test]
    fn test_generate_bandpass_response() {
        let response = generate_filter_response(
            FilterResponseType::BandPass { bandwidth: 1.0 },
            1000.0,
            0.5,
            50,
        );

        // Check that center frequency has highest amplitude
        let center_point = response.iter()
            .min_by(|a, b| {
                (a.frequency - 1000.0).abs()
                    .partial_cmp(&(b.frequency - 1000.0).abs())
                    .unwrap()
            })
            .unwrap();

        let edge_point = response.iter()
            .find(|p| p.frequency < 100.0 || p.frequency > 10000.0)
            .unwrap();

        assert!(center_point.amplitude_db > edge_point.amplitude_db,
            "Center should have higher amplitude than edges");
    }
}
