//! Output level meter.
//!
//! A compact stereo peak meter for the Audio Output module that shows what the
//! output stage is doing, not just how loud it is:
//!
//! - A gradient bar for the level you hear (post-limiter).
//! - A faint orange "ghost" beyond it while the limiter is working, showing
//!   how far the patch drove into the ceiling.
//! - A ceiling tick, peak-hold marks, and a gain-reduction readout.

use eframe::egui::{self, epaint::Mesh, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::app::theme;
use crate::dsp::analysis::amp_to_db;
use crate::dsp::OutputLevels;

/// Bottom of the meter scale.
const FLOOR_DB: f32 = -48.0;
/// Top of the meter scale: room above full scale to show overshoot.
const TOP_DB: f32 = 6.0;
/// How fast the bars fall once the signal drops.
const FALL_DB_PER_SEC: f32 = 18.0;
/// How fast the gain-reduction readout recovers.
const REDUCTION_FALL_DB_PER_SEC: f32 = 24.0;
/// How long a peak-hold mark stays before falling.
const HOLD_SECS: f32 = 1.5;

/// Meter state with display ballistics.
///
/// The audio thread reports raw block peaks. The UI feeds every report it
/// receives, then ticks once per frame; bars jump up instantly and fall at a
/// steady rate, which is how hardware peak meters read.
#[derive(Clone, Debug)]
pub struct LevelMeter {
    /// Readings received since the last tick, merged (loudest wins).
    pending: Option<OutputLevels>,
    /// Displayed post-limiter level per channel, in dBFS.
    post_db: [f32; 2],
    /// Displayed pre-limiter level per channel, in dBFS.
    pre_db: [f32; 2],
    /// Peak-hold level per channel, in dBFS.
    hold_db: [f32; 2],
    /// Seconds since each hold mark was last pushed up.
    hold_age: [f32; 2],
    /// Displayed gain reduction in dB (positive = reducing).
    reduction_db: f32,
}

impl Default for LevelMeter {
    fn default() -> Self {
        Self {
            pending: None,
            post_db: [FLOOR_DB; 2],
            pre_db: [FLOOR_DB; 2],
            hold_db: [FLOOR_DB; 2],
            hold_age: [0.0; 2],
            reduction_db: 0.0,
        }
    }
}

impl LevelMeter {
    /// Adds a reading from the audio thread.
    pub fn feed(&mut self, levels: OutputLevels) {
        let merged = match self.pending {
            None => levels,
            Some(p) => OutputLevels {
                pre: [p.pre[0].max(levels.pre[0]), p.pre[1].max(levels.pre[1])],
                post: [p.post[0].max(levels.post[0]), p.post[1].max(levels.post[1])],
                limiter_gain: p.limiter_gain.min(levels.limiter_gain),
            },
        };
        self.pending = Some(merged);
    }

    /// Advances the ballistics by `dt` seconds, taking in pending readings.
    pub fn tick(&mut self, dt: f32) {
        let fall = FALL_DB_PER_SEC * dt;
        let incoming = self.pending.take().unwrap_or_default();

        for ch in 0..2 {
            let post = amp_to_db(incoming.post[ch]).max(FLOOR_DB);
            let pre = amp_to_db(incoming.pre[ch]).max(FLOOR_DB);
            self.post_db[ch] = post.max(self.post_db[ch] - fall);
            self.pre_db[ch] = pre.max(self.pre_db[ch] - fall);

            if post >= self.hold_db[ch] {
                self.hold_db[ch] = post;
                self.hold_age[ch] = 0.0;
            } else {
                self.hold_age[ch] += dt;
                if self.hold_age[ch] > HOLD_SECS {
                    self.hold_db[ch] = (self.hold_db[ch] - fall).max(self.post_db[ch]);
                }
            }
        }

        let reduction = -amp_to_db(incoming.limiter_gain).min(0.0);
        self.reduction_db = reduction.max(self.reduction_db - REDUCTION_FALL_DB_PER_SEC * dt);
    }

    /// True once everything has settled at the floor (no repaints needed).
    pub fn is_idle(&self) -> bool {
        self.pending.is_none()
            && self.hold_db.iter().all(|&db| db <= FLOOR_DB)
            && self.pre_db.iter().all(|&db| db <= FLOOR_DB)
            && self.reduction_db <= 0.0
    }

    /// Displayed post-limiter level per channel, in dBFS.
    pub fn post_db(&self) -> [f32; 2] {
        self.post_db
    }

    /// Displayed pre-limiter level per channel, in dBFS.
    pub fn pre_db(&self) -> [f32; 2] {
        self.pre_db
    }

    /// Displayed gain reduction, in dB.
    pub fn reduction_db(&self) -> f32 {
        self.reduction_db
    }
}

/// Configuration for the level meter widget.
#[derive(Clone, Debug)]
pub struct LevelMeterConfig {
    /// Width of the bars in pixels.
    pub width: f32,
    /// Height of each channel's bar in pixels.
    pub bar_height: f32,
    /// Gap between the two bars.
    pub gap: f32,
    /// Limiter ceiling in dBFS, drawn as a tick.
    pub ceiling_db: f32,
    /// Whether the limiter is on (shows the readout; otherwise marks overs red).
    pub limiter_enabled: bool,
}

impl Default for LevelMeterConfig {
    fn default() -> Self {
        Self {
            width: 132.0,
            bar_height: 5.0,
            gap: 3.0,
            ceiling_db: -0.3,
            limiter_enabled: true,
        }
    }
}

impl LevelMeterConfig {
    /// Scales every dimension, for the zoomable node graph.
    pub fn scaled(mut self, zoom: f32) -> Self {
        self.width *= zoom;
        self.bar_height *= zoom;
        self.gap *= zoom;
        self
    }
}

/// Fraction of the bar width for a level in dBFS.
fn db_to_fraction(db: f32) -> f32 {
    ((db - FLOOR_DB) / (TOP_DB - FLOOR_DB)).clamp(0.0, 1.0)
}

/// Colour stops along the scale: calm blue for the body of the signal,
/// brightening into amber as it nears full scale.
fn gradient_stops() -> [(f32, Color32); 4] {
    [
        (FLOOR_DB, Color32::from_rgb(38, 96, 160)),
        (-12.0, theme::signal::AUDIO),
        (-3.0, Color32::from_rgb(170, 225, 255)),
        (TOP_DB, theme::accent::WARNING),
    ]
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Fills `rect` from its left edge up to `level_db` with the meter gradient.
fn paint_gradient_bar(painter: &egui::Painter, rect: Rect, level_db: f32) {
    let end_x = rect.left() + rect.width() * db_to_fraction(level_db);
    if end_x <= rect.left() {
        return;
    }

    let mut mesh = Mesh::default();
    let stops = gradient_stops();
    for pair in stops.windows(2) {
        let (db0, c0) = pair[0];
        let (db1, c1) = pair[1];
        let x0 = rect.left() + rect.width() * db_to_fraction(db0);
        let x1_full = rect.left() + rect.width() * db_to_fraction(db1);
        if x0 >= end_x {
            break;
        }
        let x1 = x1_full.min(end_x);
        let c1 = lerp_color(c0, c1, (x1 - x0) / (x1_full - x0).max(f32::EPSILON));

        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(Pos2::new(x0, rect.top()), c0);
        mesh.colored_vertex(Pos2::new(x1, rect.top()), c1);
        mesh.colored_vertex(Pos2::new(x1, rect.bottom()), c1);
        mesh.colored_vertex(Pos2::new(x0, rect.bottom()), c0);
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Draws the stereo output meter.
pub fn level_meter(ui: &mut Ui, meter: &LevelMeter, config: &LevelMeterConfig) -> Response {
    let readout_width = config.bar_height * 9.0;
    let bars_height = config.bar_height * 2.0 + config.gap;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(config.width + readout_width, bars_height),
        Sense::hover(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let x_at = |db: f32, r: Rect| r.left() + r.width() * db_to_fraction(db);
        let rounding = config.bar_height * 0.4;

        for ch in 0..2 {
            let top = rect.top() + ch as f32 * (config.bar_height + config.gap);
            let bar = Rect::from_min_size(Pos2::new(rect.left(), top), Vec2::new(config.width, config.bar_height));

            // Track
            painter.rect_filled(bar, rounding, theme::background::WIDGET);

            // What the limiter caught: from the output level up to the input level
            let post_db = meter.post_db[ch];
            let pre_db = meter.pre_db[ch];
            if config.limiter_enabled && pre_db > post_db + 0.1 {
                let ghost = Rect::from_x_y_ranges(x_at(post_db, bar)..=x_at(pre_db, bar), bar.y_range());
                painter.rect_filled(ghost, rounding, theme::signal::CONTROL.gamma_multiply(0.35));
            }

            // What you hear
            paint_gradient_bar(painter, bar, post_db);

            // Without the limiter, anything over full scale clips the converter
            if !config.limiter_enabled && post_db > 0.0 {
                let over = Rect::from_x_y_ranges(x_at(0.0, bar)..=x_at(post_db, bar), bar.y_range());
                painter.rect_filled(over, rounding, theme::accent::ERROR);
            }

            // Peak hold
            let hold_db = meter.hold_db[ch];
            if hold_db > FLOOR_DB {
                let x = x_at(hold_db, bar);
                painter.line_segment(
                    [Pos2::new(x, bar.top()), Pos2::new(x, bar.bottom())],
                    egui::Stroke::new(1.5, theme::text::PRIMARY.gamma_multiply(0.8)),
                );
            }
        }

        // Full-scale and ceiling ticks span both bars
        let all_bars = Rect::from_min_size(rect.min, Vec2::new(config.width, bars_height));
        let tick_db = if config.limiter_enabled { config.ceiling_db } else { 0.0 };
        let x = x_at(tick_db, all_bars);
        painter.line_segment(
            [Pos2::new(x, all_bars.top() - 1.0), Pos2::new(x, all_bars.bottom() + 1.0)],
            egui::Stroke::new(1.0, theme::text::SECONDARY.gamma_multiply(0.7)),
        );

        // Gain-reduction readout
        let reduction = meter.reduction_db;
        let (text, color) = if !config.limiter_enabled {
            ("off".to_string(), theme::text::DISABLED)
        } else if reduction >= 0.1 {
            (format!("-{:.1} dB", reduction), theme::signal::CONTROL)
        } else {
            ("0.0 dB".to_string(), theme::text::DISABLED)
        };
        painter.text(
            Pos2::new(rect.right(), rect.center().y),
            egui::Align2::RIGHT_CENTER,
            text,
            egui::FontId::monospace(config.bar_height * 2.0),
            color,
        );
    }

    let fmt = |db: f32| if db <= FLOOR_DB { "-inf".to_string() } else { format!("{:+.1}", db) };
    response.on_hover_text(format!(
        "Output: L {} / R {} dBFS\nInto limiter: L {} / R {} dBFS\nLimiter reduction: {:.1} dB",
        fmt(meter.post_db[0]),
        fmt(meter.post_db[1]),
        fmt(meter.pre_db[0]),
        fmt(meter.pre_db[1]),
        meter.reduction_db,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels(pre: f32, post: f32, gain: f32) -> OutputLevels {
        OutputLevels { pre: [pre; 2], post: [post; 2], limiter_gain: gain }
    }

    #[test]
    fn test_starts_idle_at_floor() {
        let meter = LevelMeter::default();
        assert!(meter.is_idle());
        assert_eq!(meter.post_db(), [FLOOR_DB; 2]);
    }

    #[test]
    fn test_rises_instantly_and_falls_steadily() {
        let mut meter = LevelMeter::default();
        meter.feed(levels(0.5, 0.5, 1.0));
        meter.tick(0.016);
        let db = amp_to_db(0.5);
        assert!((meter.post_db()[0] - db).abs() < 1e-3);

        // One second of silence: falls by the fall rate
        for _ in 0..10 {
            meter.tick(0.1);
        }
        assert!((meter.post_db()[0] - (db - FALL_DB_PER_SEC)).abs() < 0.01);
    }

    #[test]
    fn test_feed_merges_loudest_reading() {
        let mut meter = LevelMeter::default();
        meter.feed(levels(1.5, 0.9, 0.7));
        meter.feed(levels(0.2, 0.1, 1.0));
        meter.tick(0.016);
        assert!((meter.post_db()[0] - amp_to_db(0.9)).abs() < 1e-3);
        assert!((meter.pre_db()[0] - amp_to_db(1.5)).abs() < 1e-3);
        assert!((meter.reduction_db() - -amp_to_db(0.7)).abs() < 1e-3);
    }

    #[test]
    fn test_settles_back_to_idle() {
        let mut meter = LevelMeter::default();
        meter.feed(levels(2.0, 0.96, 0.48));
        meter.tick(0.016);
        assert!(!meter.is_idle());
        for _ in 0..100 {
            meter.tick(0.1);
        }
        assert!(meter.is_idle());
    }

    #[test]
    fn test_db_to_fraction_spans_scale() {
        assert_eq!(db_to_fraction(FLOOR_DB), 0.0);
        assert_eq!(db_to_fraction(TOP_DB), 1.0);
        assert_eq!(db_to_fraction(-200.0), 0.0);
        assert!(db_to_fraction(0.0) > 0.85 && db_to_fraction(0.0) < 0.9);
    }
}
