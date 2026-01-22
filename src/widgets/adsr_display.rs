//! ADSR envelope visualization widget.
//!
//! Provides a visual display of ADSR envelope shape based on current parameter values.
//! The display shows attack, decay, sustain, and release segments with the actual
//! exponential curves matching the audio engine's envelope generator.

use eframe::egui::{self, Color32, Pos2, Response, Sense, Stroke, Ui, Vec2};

use crate::app::theme;

/// Configuration for the ADSR display widget.
#[derive(Clone)]
pub struct AdsrConfig {
    /// Size of the display (width x height).
    pub size: Vec2,
    /// Envelope line color.
    pub color: Color32,
    /// Line thickness.
    pub line_thickness: f32,
    /// Whether to show a glow effect.
    pub glow: bool,
    /// Whether to fill below the envelope curve.
    pub filled: bool,
    /// Fill color (uses main color with reduced alpha if None).
    pub fill_color: Option<Color32>,
    /// Whether to show segment labels (A, D, S, R).
    pub show_labels: bool,
    /// Whether to show the sustain level line.
    pub show_sustain_line: bool,
    /// Whether to show grid lines.
    pub show_grid: bool,
}

impl Default for AdsrConfig {
    fn default() -> Self {
        Self {
            size: Vec2::new(140.0, 50.0),
            color: theme::signal::CONTROL,
            line_thickness: 1.5,
            glow: true,
            filled: true,
            fill_color: None,
            show_labels: true,
            show_sustain_line: true,
            show_grid: true,
        }
    }
}

impl AdsrConfig {
    /// Create a new ADSR config with the specified size.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
            ..Default::default()
        }
    }

    /// Set the display size.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = Vec2::new(width, height);
        self
    }

    /// Set the envelope color.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Enable or disable glow effect.
    pub fn with_glow(mut self, glow: bool) -> Self {
        self.glow = glow;
        self
    }

    /// Enable or disable fill.
    pub fn with_fill(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Show or hide segment labels.
    pub fn with_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }
}

/// ADSR envelope parameters for visualization.
#[derive(Clone, Copy, Debug)]
pub struct AdsrParams {
    /// Attack time in seconds (0.001 to 10.0).
    pub attack: f32,
    /// Decay time in seconds (0.001 to 10.0).
    pub decay: f32,
    /// Sustain level (0.0 to 1.0).
    pub sustain: f32,
    /// Release time in seconds (0.001 to 10.0).
    pub release: f32,
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self {
            attack: 0.01,
            decay: 0.1,
            sustain: 0.7,
            release: 0.3,
        }
    }
}

impl AdsrParams {
    /// Create new ADSR parameters.
    pub fn new(attack: f32, decay: f32, sustain: f32, release: f32) -> Self {
        Self {
            attack: attack.clamp(0.001, 10.0),
            decay: decay.clamp(0.001, 10.0),
            sustain: sustain.clamp(0.0, 1.0),
            release: release.clamp(0.001, 10.0),
        }
    }
}

/// Generate ADSR envelope curve points for display.
///
/// Returns a vector of (x, y) normalized points where:
/// - x is in range [0, 1] representing time
/// - y is in range [0, 1] representing amplitude
///
/// The curve uses exponential shapes matching the actual audio engine implementation.
/// Time segments are scaled to ensure visual clarity - each segment gets a minimum
/// visual width so the envelope shape is always readable.
pub fn generate_adsr_curve(params: &AdsrParams, num_points: usize) -> Vec<(f32, f32)> {
    let mut points = Vec::with_capacity(num_points);

    // Use logarithmic scaling for times to better visualize short vs long segments
    // This prevents very short attack/decay from being invisible
    let attack_log = (1.0 + params.attack).ln();
    let decay_log = (1.0 + params.decay).ln();
    let sustain_log = (1.0 + 0.15_f32).ln(); // Fixed visual sustain hold
    let release_log = (1.0 + params.release).ln();
    let total_log = attack_log + decay_log + sustain_log + release_log;

    // Calculate segment boundaries with logarithmic scaling
    let attack_end = attack_log / total_log;
    let decay_end = (attack_log + decay_log) / total_log;
    let sustain_end = (attack_log + decay_log + sustain_log) / total_log;

    for i in 0..num_points {
        let x = i as f32 / (num_points - 1) as f32;
        let y;

        if x <= attack_end {
            // Attack phase: exponential rise from 0 to 1
            let t = if attack_end > 0.0 {
                x / attack_end
            } else {
                1.0
            };
            // Exponential curve that reaches ~99.3% at t=1
            y = 1.0 - (-5.0 * t).exp();
        } else if x <= decay_end {
            // Decay phase: exponential fall from 1 to sustain
            let t = if (decay_end - attack_end) > 0.0 {
                (x - attack_end) / (decay_end - attack_end)
            } else {
                1.0
            };
            // Exponential decay from 1 to sustain level
            let decay_amount = 1.0 - params.sustain;
            y = params.sustain + decay_amount * (-5.0 * t).exp();
        } else if x <= sustain_end {
            // Sustain phase: hold at sustain level
            y = params.sustain;
        } else {
            // Release phase: exponential fall from sustain to 0
            let t = if (1.0 - sustain_end) > 0.0 {
                (x - sustain_end) / (1.0 - sustain_end)
            } else {
                1.0
            };
            // Exponential decay from sustain to 0
            y = params.sustain * (-5.0 * t).exp();
        }

        points.push((x, y.clamp(0.0, 1.0)));
    }

    points
}

/// Get the segment boundaries for label positioning.
/// Returns (attack_end, decay_end, sustain_end) as normalized x positions.
pub fn get_adsr_segment_boundaries(params: &AdsrParams) -> (f32, f32, f32) {
    let attack_log = (1.0 + params.attack).ln();
    let decay_log = (1.0 + params.decay).ln();
    let sustain_log = (1.0 + 0.15_f32).ln();
    let release_log = (1.0 + params.release).ln();
    let total_log = attack_log + decay_log + sustain_log + release_log;

    let attack_end = attack_log / total_log;
    let decay_end = (attack_log + decay_log) / total_log;
    let sustain_end = (attack_log + decay_log + sustain_log) / total_log;

    (attack_end, decay_end, sustain_end)
}

/// Display an ADSR envelope visualization.
///
/// Shows the envelope shape based on current Attack, Decay, Sustain, and Release values.
/// Features:
/// - Exponential curves matching actual envelope behavior
/// - Optional segment labels (A, D, S, R)
/// - Sustain level indicator line
/// - Fill and glow effects for visual clarity
pub fn adsr_display(ui: &mut Ui, params: &AdsrParams, config: &AdsrConfig) -> Response {
    let (rect, response) = ui.allocate_exact_size(config.size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Draw background
        painter.rect_filled(rect, 2.0, Color32::from_rgb(20, 22, 30));

        // Draw subtle grid
        if config.show_grid {
            let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 15);

            // Vertical divisions (4)
            for i in 1..4 {
                let x = rect.left() + rect.width() * (i as f32 / 4.0);
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.5, grid_color),
                );
            }

            // Horizontal divisions (4)
            for i in 1..4 {
                let y = rect.top() + rect.height() * (i as f32 / 4.0);
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.5, grid_color),
                );
            }
        }

        // Generate envelope curve
        let num_points = (config.size.x as usize).max(64);
        let curve = generate_adsr_curve(params, num_points);

        // Convert to screen coordinates
        // Y is inverted (0 at bottom, 1 at top)
        let padding_top = 4.0;
        let padding_bottom = if config.show_labels { 14.0 } else { 4.0 };
        let draw_height = rect.height() - padding_top - padding_bottom;

        let points: Vec<Pos2> = curve
            .iter()
            .map(|(x, y)| {
                Pos2::new(
                    rect.left() + x * rect.width(),
                    rect.top() + padding_top + (1.0 - y) * draw_height,
                )
            })
            .collect();

        // Draw sustain level line
        if config.show_sustain_line && params.sustain > 0.01 {
            let sustain_y = rect.top() + padding_top + (1.0 - params.sustain) * draw_height;
            let sustain_color = Color32::from_rgba_unmultiplied(
                config.color.r(),
                config.color.g(),
                config.color.b(),
                40,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.left(), sustain_y),
                    Pos2::new(rect.right(), sustain_y),
                ],
                Stroke::new(1.0, sustain_color),
            );
        }

        // Draw filled area if enabled
        if config.filled && !points.is_empty() {
            let fill_color = config.fill_color.unwrap_or_else(|| {
                let c = config.color;
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 30)
            });

            let baseline_y = rect.top() + padding_top + draw_height;

            // Create filled polygon
            let mut mesh_points = points.clone();
            mesh_points.push(Pos2::new(rect.right(), baseline_y));
            mesh_points.push(Pos2::new(rect.left(), baseline_y));

            // Draw as triangle fan from center
            if mesh_points.len() >= 3 {
                let center = Pos2::new(rect.center().x, baseline_y);
                for i in 0..mesh_points.len() - 1 {
                    painter.add(egui::Shape::convex_polygon(
                        vec![center, mesh_points[i], mesh_points[i + 1]],
                        fill_color,
                        Stroke::NONE,
                    ));
                }
            }
        }

        // Draw glow effect
        if config.glow && points.len() >= 2 {
            let glow_color = Color32::from_rgba_unmultiplied(
                config.color.r(),
                config.color.g(),
                config.color.b(),
                50,
            );
            draw_polyline(painter, &points, glow_color, config.line_thickness * 3.0);
        }

        // Draw main envelope line
        if points.len() >= 2 {
            draw_polyline(painter, &points, config.color, config.line_thickness);
        }

        // Draw segment labels
        if config.show_labels {
            let label_y = rect.bottom() - 2.0;
            let label_color = Color32::from_rgba_unmultiplied(255, 255, 255, 120);
            let font = egui::FontId::proportional(9.0);

            // Get segment boundaries using the same logarithmic scaling as the curve
            let (attack_end, decay_end, sustain_end) = get_adsr_segment_boundaries(params);

            // Position labels at the center of each segment
            let attack_x = rect.left() + (attack_end * 0.5) * rect.width();
            let decay_x = rect.left() + ((attack_end + decay_end) * 0.5) * rect.width();
            let sustain_x = rect.left() + ((decay_end + sustain_end) * 0.5) * rect.width();
            let release_x = rect.left() + ((sustain_end + 1.0) * 0.5) * rect.width();

            // Draw labels centered under each segment
            painter.text(
                Pos2::new(attack_x, label_y),
                egui::Align2::CENTER_BOTTOM,
                "A",
                font.clone(),
                label_color,
            );
            painter.text(
                Pos2::new(decay_x, label_y),
                egui::Align2::CENTER_BOTTOM,
                "D",
                font.clone(),
                label_color,
            );
            painter.text(
                Pos2::new(sustain_x, label_y),
                egui::Align2::CENTER_BOTTOM,
                "S",
                font.clone(),
                label_color,
            );
            painter.text(
                Pos2::new(release_x, label_y),
                egui::Align2::CENTER_BOTTOM,
                "R",
                font,
                label_color,
            );
        }

        // Draw border
        painter.rect_stroke(rect, 2.0, Stroke::new(1.0, Color32::from_rgb(50, 55, 70)));
    }

    response
}

/// Draw a polyline with anti-aliasing.
fn draw_polyline(painter: &egui::Painter, points: &[Pos2], color: Color32, thickness: f32) {
    if points.len() < 2 {
        return;
    }

    for i in 0..points.len() - 1 {
        painter.line_segment([points[i], points[i + 1]], Stroke::new(thickness, color));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adsr_config_default() {
        let config = AdsrConfig::default();
        assert_eq!(config.size.x, 140.0);
        assert_eq!(config.size.y, 50.0);
        assert!(config.glow);
        assert!(config.filled);
        assert!(config.show_labels);
    }

    #[test]
    fn test_adsr_config_builder() {
        let config = AdsrConfig::default()
            .with_size(200.0, 100.0)
            .with_glow(false)
            .with_fill(false)
            .with_labels(false);

        assert_eq!(config.size, Vec2::new(200.0, 100.0));
        assert!(!config.glow);
        assert!(!config.filled);
        assert!(!config.show_labels);
    }

    #[test]
    fn test_adsr_params_default() {
        let params = AdsrParams::default();
        assert!((params.attack - 0.01).abs() < f32::EPSILON);
        assert!((params.decay - 0.1).abs() < f32::EPSILON);
        assert!((params.sustain - 0.7).abs() < f32::EPSILON);
        assert!((params.release - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_adsr_params_clamping() {
        let params = AdsrParams::new(-1.0, 100.0, 2.0, -5.0);
        assert_eq!(params.attack, 0.001);
        assert_eq!(params.decay, 10.0);
        assert_eq!(params.sustain, 1.0);
        assert_eq!(params.release, 0.001);
    }

    #[test]
    fn test_generate_adsr_curve_length() {
        let params = AdsrParams::default();
        let curve = generate_adsr_curve(&params, 100);
        assert_eq!(curve.len(), 100);
    }

    #[test]
    fn test_generate_adsr_curve_bounds() {
        let params = AdsrParams::default();
        let curve = generate_adsr_curve(&params, 100);

        for (x, y) in &curve {
            assert!(*x >= 0.0 && *x <= 1.0, "x={} out of bounds", x);
            assert!(*y >= 0.0 && *y <= 1.0, "y={} out of bounds", y);
        }
    }

    #[test]
    fn test_generate_adsr_curve_start_end() {
        let params = AdsrParams::default();
        let curve = generate_adsr_curve(&params, 100);

        // Should start at (0, ~0) - beginning of attack
        assert!(curve[0].0 < 0.01);
        assert!(curve[0].1 < 0.1);

        // Should end near (1, ~0) - end of release
        assert!(curve[99].0 > 0.99);
        assert!(curve[99].1 < 0.1);
    }

    #[test]
    fn test_generate_adsr_curve_peak() {
        let params = AdsrParams::default();
        let curve = generate_adsr_curve(&params, 100);

        // Should reach near 1.0 at peak (end of attack)
        let max_y = curve.iter().map(|(_, y)| *y).fold(0.0f32, f32::max);
        assert!(max_y > 0.9, "Peak should be near 1.0, got {}", max_y);
    }

    #[test]
    fn test_generate_adsr_curve_sustain_level() {
        let params = AdsrParams::new(0.01, 0.1, 0.5, 0.3);
        let curve = generate_adsr_curve(&params, 200);

        // Use helper to get segment boundaries
        let (_, decay_end, sustain_end) = get_adsr_segment_boundaries(&params);

        // Find points in the sustain region
        let sustain_points: Vec<f32> = curve
            .iter()
            .filter(|(x, _)| *x > decay_end + 0.02 && *x < sustain_end - 0.02)
            .map(|(_, y)| *y)
            .collect();

        // Sustain region should be near sustain level
        if !sustain_points.is_empty() {
            let avg_sustain: f32 = sustain_points.iter().sum::<f32>() / sustain_points.len() as f32;
            assert!(
                (avg_sustain - 0.5).abs() < 0.1,
                "Sustain region should be near 0.5, got {}",
                avg_sustain
            );
        }
    }

    #[test]
    fn test_generate_adsr_curve_zero_sustain() {
        let params = AdsrParams::new(0.01, 0.1, 0.0, 0.3);
        let curve = generate_adsr_curve(&params, 100);

        // Use helper to get segment boundaries
        let (_, decay_end, sustain_end) = get_adsr_segment_boundaries(&params);

        // With zero sustain, should decay to near zero
        let sustain_points: Vec<f32> = curve
            .iter()
            .filter(|(x, _)| *x > decay_end + 0.02 && *x < sustain_end - 0.02)
            .map(|(_, y)| *y)
            .collect();

        if !sustain_points.is_empty() {
            for y in sustain_points {
                assert!(y < 0.1, "Zero sustain should produce near-zero values");
            }
        }
    }

    #[test]
    fn test_generate_adsr_curve_full_sustain() {
        let params = AdsrParams::new(0.01, 0.1, 1.0, 0.3);
        let curve = generate_adsr_curve(&params, 100);

        // Use helper to get segment boundaries with logarithmic scaling
        let (_, decay_end, sustain_end) = get_adsr_segment_boundaries(&params);

        // Filter for sustain region only (between decay_end and sustain_end)
        let sustain_points: Vec<f32> = curve
            .iter()
            .filter(|(x, _)| *x > decay_end + 0.02 && *x < sustain_end - 0.02)
            .map(|(_, y)| *y)
            .collect();

        // With full sustain, should stay at 1.0 during sustain phase
        if !sustain_points.is_empty() {
            for y in &sustain_points {
                assert!(*y > 0.9, "Full sustain should stay near 1.0, got {}", y);
            }
        }
    }
}
