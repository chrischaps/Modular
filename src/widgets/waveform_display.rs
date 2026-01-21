//! Waveform display widget for visualizing audio signals.
//!
//! Provides a time-domain waveform display with anti-aliased rendering,
//! subtle grid background, and multiple display modes for different use cases.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::app::theme;

/// Display mode for the waveform widget.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WaveformMode {
    /// Single cycle display - shows exactly one period of a waveform.
    /// Good for oscillator visualization.
    #[default]
    SingleCycle,
    /// Rolling buffer - continuous scrolling waveform like an oscilloscope.
    /// Good for real-time audio monitoring.
    Rolling,
    /// Static display - shows the entire buffer without scrolling.
    /// Good for captured/frozen waveforms.
    Static,
}

/// Grid style for the waveform background.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum GridStyle {
    /// No grid background.
    None,
    /// Subtle grid lines.
    #[default]
    Subtle,
    /// More visible grid with major/minor divisions.
    Detailed,
}

/// Configuration for the WaveformDisplay widget.
#[derive(Clone)]
pub struct WaveformConfig {
    /// Size of the display (width x height).
    pub size: Vec2,
    /// Waveform line color.
    pub color: Color32,
    /// Line thickness.
    pub line_thickness: f32,
    /// Whether to show a glow effect around the waveform.
    pub glow: bool,
    /// Glow color (uses main color with reduced alpha if None).
    pub glow_color: Option<Color32>,
    /// Grid style for the background.
    pub grid_style: GridStyle,
    /// Display mode.
    pub mode: WaveformMode,
    /// Whether to fill below the waveform.
    pub filled: bool,
    /// Fill color (uses main color with reduced alpha if None).
    pub fill_color: Option<Color32>,
    /// Vertical scale (1.0 = full range, 0.5 = half amplitude, etc.).
    pub scale: f32,
    /// Vertical offset (-1.0 to 1.0 range).
    pub offset: f32,
    /// Number of samples to display (for decimation).
    /// If None, auto-calculates based on width.
    pub display_samples: Option<usize>,
}

impl Default for WaveformConfig {
    fn default() -> Self {
        Self {
            size: Vec2::new(120.0, 60.0),
            color: theme::signal::AUDIO,
            line_thickness: 1.5,
            glow: true,
            glow_color: None,
            grid_style: GridStyle::Subtle,
            mode: WaveformMode::SingleCycle,
            filled: false,
            fill_color: None,
            scale: 1.0,
            offset: 0.0,
            display_samples: None,
        }
    }
}

impl WaveformConfig {
    /// Create a new waveform config with the specified size.
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
            ..Default::default()
        }
    }

    /// Create config for oscillator visualization (single cycle, blue).
    pub fn oscillator() -> Self {
        Self {
            color: theme::signal::AUDIO,
            mode: WaveformMode::SingleCycle,
            glow: true,
            ..Default::default()
        }
    }

    /// Create config for LFO visualization (single cycle, orange).
    pub fn lfo() -> Self {
        Self {
            color: theme::signal::CONTROL,
            mode: WaveformMode::SingleCycle,
            glow: true,
            ..Default::default()
        }
    }

    /// Create config for envelope visualization (static, orange, filled).
    pub fn envelope() -> Self {
        Self {
            color: theme::signal::CONTROL,
            mode: WaveformMode::Static,
            glow: false,
            filled: true,
            ..Default::default()
        }
    }

    /// Create config for real-time audio monitoring (rolling).
    pub fn monitor() -> Self {
        Self {
            color: theme::signal::AUDIO,
            mode: WaveformMode::Rolling,
            glow: false,
            grid_style: GridStyle::Detailed,
            ..Default::default()
        }
    }

    /// Set the waveform color.
    pub fn with_color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    /// Set the display size.
    pub fn with_size(mut self, width: f32, height: f32) -> Self {
        self.size = Vec2::new(width, height);
        self
    }

    /// Set the line thickness.
    pub fn with_thickness(mut self, thickness: f32) -> Self {
        self.line_thickness = thickness;
        self
    }

    /// Enable or disable glow effect.
    pub fn with_glow(mut self, glow: bool) -> Self {
        self.glow = glow;
        self
    }

    /// Enable filled waveform.
    pub fn with_fill(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }

    /// Set the grid style.
    pub fn with_grid(mut self, style: GridStyle) -> Self {
        self.grid_style = style;
        self
    }

    /// Set the display mode.
    pub fn with_mode(mut self, mode: WaveformMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set vertical scale.
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

/// A waveform display widget for visualizing audio and control signals.
///
/// Features:
/// - Anti-aliased line rendering
/// - Optional subtle grid background
/// - Glow effect for signal clarity
/// - Multiple display modes (single cycle, rolling, static)
/// - Fill option for envelope-style visualization
pub fn waveform_display(ui: &mut Ui, samples: &[f32], config: &WaveformConfig) -> Response {
    let (rect, response) = ui.allocate_exact_size(config.size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        // Draw background
        draw_waveform_background(painter, rect, config.grid_style);

        // Skip if no samples
        if samples.is_empty() {
            return response;
        }

        // Calculate display points
        let display_width = rect.width();
        let num_points = config
            .display_samples
            .unwrap_or_else(|| (display_width as usize).min(samples.len()));

        // Decimate samples to fit display
        let points = calculate_display_points(samples, num_points, rect, config);

        // Draw filled area if enabled
        if config.filled {
            draw_filled_waveform(painter, &points, rect, config);
        }

        // Draw glow effect (wider, semi-transparent line behind main line)
        if config.glow && points.len() >= 2 {
            let glow_color = config.glow_color.unwrap_or_else(|| {
                let c = config.color;
                Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 60)
            });
            draw_polyline(painter, &points, glow_color, config.line_thickness * 3.0);
        }

        // Draw main waveform line
        if points.len() >= 2 {
            draw_polyline(painter, &points, config.color, config.line_thickness);
        }

        // Draw zero line (subtle reference)
        let zero_y = rect.center().y + config.offset * rect.height() * 0.5;
        painter.line_segment(
            [
                Pos2::new(rect.left(), zero_y),
                Pos2::new(rect.right(), zero_y),
            ],
            Stroke::new(0.5, Color32::from_rgba_unmultiplied(255, 255, 255, 30)),
        );
    }

    response
}

/// Draw the grid background for the waveform display.
fn draw_waveform_background(painter: &egui::Painter, rect: Rect, style: GridStyle) {
    // Dark background
    painter.rect_filled(
        rect,
        2.0,
        Color32::from_rgb(20, 22, 30),
    );

    match style {
        GridStyle::None => {}
        GridStyle::Subtle => {
            let grid_color = Color32::from_rgba_unmultiplied(255, 255, 255, 15);
            let num_divisions = 4;

            // Vertical divisions
            for i in 1..num_divisions {
                let x = rect.left() + rect.width() * (i as f32 / num_divisions as f32);
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.5, grid_color),
                );
            }

            // Horizontal divisions (centered)
            for i in 1..num_divisions {
                let y = rect.top() + rect.height() * (i as f32 / num_divisions as f32);
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.5, grid_color),
                );
            }
        }
        GridStyle::Detailed => {
            let major_color = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
            let minor_color = Color32::from_rgba_unmultiplied(255, 255, 255, 10);
            let num_major = 4;
            let num_minor = 8;

            // Minor grid lines
            for i in 1..num_minor {
                let x = rect.left() + rect.width() * (i as f32 / num_minor as f32);
                let y = rect.top() + rect.height() * (i as f32 / num_minor as f32);
                if i % (num_minor / num_major) != 0 {
                    painter.line_segment(
                        [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                        Stroke::new(0.5, minor_color),
                    );
                    painter.line_segment(
                        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                        Stroke::new(0.5, minor_color),
                    );
                }
            }

            // Major grid lines
            for i in 1..num_major {
                let x = rect.left() + rect.width() * (i as f32 / num_major as f32);
                let y = rect.top() + rect.height() * (i as f32 / num_major as f32);
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.5, major_color),
                );
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.5, major_color),
                );
            }
        }
    }

    // Border
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, Color32::from_rgb(50, 55, 70)),
    );
}

/// Calculate display points by decimating the sample buffer.
fn calculate_display_points(
    samples: &[f32],
    num_points: usize,
    rect: Rect,
    config: &WaveformConfig,
) -> Vec<Pos2> {
    let mut points = Vec::with_capacity(num_points);

    if samples.is_empty() || num_points == 0 {
        return points;
    }

    let step = samples.len() as f32 / num_points as f32;
    let center_y = rect.center().y + config.offset * rect.height() * 0.5;
    let amplitude = rect.height() * 0.5 * config.scale;

    for i in 0..num_points {
        let sample_idx = (i as f32 * step) as usize;
        let sample_idx = sample_idx.min(samples.len() - 1);

        // Use peak detection for better visualization at high sample rates
        let (min_val, max_val) = if step > 1.0 {
            let start = (i as f32 * step) as usize;
            let end = ((i + 1) as f32 * step).min(samples.len() as f32) as usize;
            let slice = &samples[start.min(samples.len() - 1)..end.min(samples.len())];
            if slice.is_empty() {
                (0.0, 0.0)
            } else {
                let min = slice.iter().cloned().fold(f32::INFINITY, f32::min);
                let max = slice.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                (min, max)
            }
        } else {
            let val = samples[sample_idx];
            (val, val)
        };

        // Use the value with larger absolute magnitude for better peak representation
        let sample = if min_val.abs() > max_val.abs() {
            min_val
        } else {
            max_val
        };

        let x = rect.left() + (i as f32 / (num_points - 1).max(1) as f32) * rect.width();
        let y = center_y - sample.clamp(-1.0, 1.0) * amplitude;

        points.push(Pos2::new(x, y));
    }

    points
}

/// Draw a polyline with anti-aliasing.
fn draw_polyline(painter: &egui::Painter, points: &[Pos2], color: Color32, thickness: f32) {
    if points.len() < 2 {
        return;
    }

    // Draw line segments
    for i in 0..points.len() - 1 {
        painter.line_segment([points[i], points[i + 1]], Stroke::new(thickness, color));
    }
}

/// Draw filled area under the waveform.
fn draw_filled_waveform(painter: &egui::Painter, points: &[Pos2], rect: Rect, config: &WaveformConfig) {
    if points.is_empty() {
        return;
    }

    let fill_color = config.fill_color.unwrap_or_else(|| {
        let c = config.color;
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 40)
    });

    let center_y = rect.center().y + config.offset * rect.height() * 0.5;

    // Create filled polygon
    let mut mesh_points = Vec::with_capacity(points.len() * 2);

    // Top edge (waveform)
    mesh_points.extend_from_slice(points);

    // Bottom edge (baseline), reversed
    for point in points.iter().rev() {
        mesh_points.push(Pos2::new(point.x, center_y));
    }

    // Draw as triangle fan
    if mesh_points.len() >= 3 {
        let center = Pos2::new(rect.center().x, center_y);
        for i in 0..mesh_points.len() - 1 {
            let next = (i + 1) % mesh_points.len();
            painter.add(egui::Shape::convex_polygon(
                vec![center, mesh_points[i], mesh_points[next]],
                fill_color,
                Stroke::NONE,
            ));
        }
    }
}

/// Ring buffer for rolling waveform display.
///
/// Use this to accumulate samples over time for real-time visualization.
#[derive(Clone)]
pub struct WaveformBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
    capacity: usize,
}

impl WaveformBuffer {
    /// Create a new ring buffer with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity],
            write_pos: 0,
            capacity,
        }
    }

    /// Push a single sample into the buffer.
    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.capacity;
    }

    /// Push multiple samples into the buffer.
    pub fn push_slice(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.push(sample);
        }
    }

    /// Get samples in order (oldest to newest).
    pub fn samples(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.capacity);
        // From write_pos to end
        result.extend_from_slice(&self.buffer[self.write_pos..]);
        // From start to write_pos
        result.extend_from_slice(&self.buffer[..self.write_pos]);
        result
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    /// Get the buffer capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Generate a single cycle of a basic waveform for display purposes.
///
/// This is useful for showing the current oscillator waveform type
/// without needing actual audio data.
pub fn generate_waveform_cycle(waveform: WaveformType, num_samples: usize) -> Vec<f32> {
    let mut samples = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let phase = i as f32 / num_samples as f32;
        let sample = match waveform {
            WaveformType::Sine => (phase * std::f32::consts::TAU).sin(),
            WaveformType::Triangle => {
                if phase < 0.25 {
                    phase * 4.0
                } else if phase < 0.75 {
                    2.0 - phase * 4.0
                } else {
                    phase * 4.0 - 4.0
                }
            }
            WaveformType::Saw => 2.0 * phase - 1.0,
            WaveformType::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            WaveformType::Pulse { width } => if phase < width { 1.0 } else { -1.0 },
        };
        samples.push(sample);
    }

    samples
}

/// Basic waveform types for visualization.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WaveformType {
    /// Sine wave
    Sine,
    /// Triangle wave
    Triangle,
    /// Sawtooth wave
    Saw,
    /// Square wave (50% duty cycle)
    Square,
    /// Pulse wave with variable duty cycle
    Pulse { width: f32 },
}

impl Default for WaveformType {
    fn default() -> Self {
        WaveformType::Sine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveform_config_default() {
        let config = WaveformConfig::default();
        assert_eq!(config.size.x, 120.0);
        assert_eq!(config.size.y, 60.0);
        assert_eq!(config.mode, WaveformMode::SingleCycle);
        assert!(config.glow);
    }

    #[test]
    fn test_waveform_config_oscillator() {
        let config = WaveformConfig::oscillator();
        assert_eq!(config.mode, WaveformMode::SingleCycle);
        assert_eq!(config.color, theme::signal::AUDIO);
        assert!(config.glow);
    }

    #[test]
    fn test_waveform_config_lfo() {
        let config = WaveformConfig::lfo();
        assert_eq!(config.color, theme::signal::CONTROL);
    }

    #[test]
    fn test_waveform_config_envelope() {
        let config = WaveformConfig::envelope();
        assert!(config.filled);
        assert!(!config.glow);
        assert_eq!(config.mode, WaveformMode::Static);
    }

    #[test]
    fn test_waveform_config_builder() {
        let config = WaveformConfig::default()
            .with_size(200.0, 100.0)
            .with_thickness(2.0)
            .with_glow(false)
            .with_fill(true)
            .with_grid(GridStyle::Detailed);

        assert_eq!(config.size, Vec2::new(200.0, 100.0));
        assert_eq!(config.line_thickness, 2.0);
        assert!(!config.glow);
        assert!(config.filled);
        assert_eq!(config.grid_style, GridStyle::Detailed);
    }

    #[test]
    fn test_waveform_buffer_basic() {
        let mut buffer = WaveformBuffer::new(4);
        buffer.push(1.0);
        buffer.push(2.0);
        buffer.push(3.0);
        buffer.push(4.0);

        let samples = buffer.samples();
        assert_eq!(samples, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_waveform_buffer_wrap() {
        let mut buffer = WaveformBuffer::new(4);
        buffer.push(1.0);
        buffer.push(2.0);
        buffer.push(3.0);
        buffer.push(4.0);
        buffer.push(5.0); // Overwrites position 0
        buffer.push(6.0); // Overwrites position 1

        let samples = buffer.samples();
        // Should be [3, 4, 5, 6] (oldest to newest)
        assert_eq!(samples, vec![3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_waveform_buffer_clear() {
        let mut buffer = WaveformBuffer::new(4);
        buffer.push(1.0);
        buffer.push(2.0);
        buffer.clear();

        let samples = buffer.samples();
        assert!(samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_generate_sine_wave() {
        let samples = generate_waveform_cycle(WaveformType::Sine, 100);
        assert_eq!(samples.len(), 100);

        // Check amplitude bounds
        for &s in &samples {
            assert!(s >= -1.0 && s <= 1.0);
        }

        // Check that it crosses zero
        let has_positive = samples.iter().any(|&s| s > 0.5);
        let has_negative = samples.iter().any(|&s| s < -0.5);
        assert!(has_positive && has_negative);
    }

    #[test]
    fn test_generate_square_wave() {
        let samples = generate_waveform_cycle(WaveformType::Square, 100);

        // Square wave should only have values near -1 or 1
        for &s in &samples {
            assert!(s == 1.0 || s == -1.0);
        }

        // Roughly half should be positive
        let positive_count = samples.iter().filter(|&&s| s > 0.0).count();
        assert!(positive_count >= 40 && positive_count <= 60);
    }

    #[test]
    fn test_generate_saw_wave() {
        let samples = generate_waveform_cycle(WaveformType::Saw, 100);

        // First sample should be near -1, last near 1
        assert!(samples[0] < -0.9);
        assert!(samples[99] > 0.9);
    }

    #[test]
    fn test_generate_triangle_wave() {
        let samples = generate_waveform_cycle(WaveformType::Triangle, 100);

        // Check peak is at 25% (quarter way through)
        let peak_idx = samples.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        assert!(peak_idx >= 20 && peak_idx <= 30);
    }

    #[test]
    fn test_generate_pulse_wave() {
        let samples = generate_waveform_cycle(WaveformType::Pulse { width: 0.25 }, 100);

        // 25% should be positive
        let positive_count = samples.iter().filter(|&&s| s > 0.0).count();
        assert!(positive_count >= 20 && positive_count <= 30);
    }
}
