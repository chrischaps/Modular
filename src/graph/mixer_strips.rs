//! The Mixer's display: a stereo panorama, then a meter and mute button per
//! channel strip, and the master meter.
//!
//! The panorama has a lane per channel, and a light on it wherever that
//! channel sounds: one for a mono cable, one per voice for a polyphonic one,
//! fanned out by Spread. A channel sent to an effect blooms sideways along
//! its lane, wider the more it sends, as a sound spreads into a room. It's drawn from the same pan law and spread rule the
//! audio uses, so what you see is where you hear it.
//!
//! The meters sit in the knob columns below them, so each channel reads as a
//! strip from top to bottom: its light, its meter, its mute, its level, its pan.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use egui_node_graph2::NodeId;

use crate::app::theme;
use crate::modules::mixer::{voice_pan, STRIPS};
use crate::widgets::{column_meter, PeakBallistics};

use super::{SynthGraph, SynthGraphState, SynthValueType};

/// Each channel's colour, for its lane, its lights and its number. Four
/// light, cool tints that read apart on the dark node and never compete with
/// the signal colours of the cables.
const CHANNEL_HUES: [Color32; STRIPS] = [
    Color32::from_rgb(110, 185, 255),
    Color32::from_rgb(110, 220, 185),
    Color32::from_rgb(185, 160, 255),
    Color32::from_rgb(245, 150, 190),
];

/// Engine input port of channel 1's Pan CV; the other channels' follow it.
const PAN_CV_PORT: usize = 8;

/// One channel's state, as the display needs it.
struct StripView {
    patched: bool,
    muted: bool,
    /// Pan with any CV added, from -1 to 1.
    pan: f32,
    /// The Send knob, from 0 to 1.
    send: f32,
    /// Each voice's last peak, or `None` before any reading.
    voices: Vec<Option<f32>>,
}

/// Draws the display and returns a parameter to change, when a mute button
/// was clicked: `(name, value)`.
pub fn mixer_strips(
    ui: &mut egui::Ui,
    node_id: NodeId,
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    zoom: f32,
) -> Option<(String, f32)> {
    let z = zoom;
    let engine_node_id = user_state.get_engine_node_id(node_id);
    let node = graph.nodes.get(node_id)?;
    let input_of = |name: &str| node.inputs.iter().find(|(n, _)| n == name).map(|(_, id)| *id);
    let value_of = |name: &str| {
        input_of(name).map_or(0.0, |id| match &graph.get_input(id).value {
            SynthValueType::Number { value, .. } => *value,
            SynthValueType::Toggle { value, .. } => f32::from(u8::from(*value)),
            _ => 0.0,
        })
    };
    let feeding = |name: &str| {
        let input = input_of(name)?;
        graph.iter_connections().find(|(i, _)| *i == input).map(|(_, output)| output)
    };
    let cv = |name: &str, port: usize| match (feeding(name), engine_node_id) {
        (Some(_), Some(eid)) => user_state.get_input_value(eid, port).unwrap_or(0.0),
        _ => 0.0,
    };

    let spread = value_of("Spread");
    let strips: Vec<StripView> = (0..STRIPS)
        .map(|s| {
            let n = s + 1;
            let source = feeding(&format!("Ch {n}"));
            let voices = source
                .and_then(|output| {
                    let out = graph.outputs.get(output)?;
                    let peaks = user_state.output_peaks(out.node, graph.get_output_index(output)?)?;
                    Some((0..peaks.count()).map(|v| Some(peaks.peak(v))).collect())
                })
                .unwrap_or_else(|| vec![None]);
            StripView {
                patched: source.is_some(),
                muted: value_of(&format!("Mute {n}")) >= 0.5,
                pan: (value_of(&format!("Pan {n}")) + cv(&format!("Pan {n}"), PAN_CV_PORT + s)).clamp(-1.0, 1.0),
                send: value_of(&format!("Send {n}")),
                voices,
            }
        })
        .collect();
    let silent = PeakBallistics::default();
    let meters = engine_node_id.and_then(|eid| user_state.module_meters.get(&eid));
    let meter = |index: usize| meters.map_or(&silent, |m| m.get(index));

    // The knob columns below: each knob sits at the left of a column this
    // wide, so a strip's centre line runs through its knobs' centres
    let column = 44.0 * z;
    let knob_center = 18.0 * z;
    let gap = ui.spacing().item_spacing.x;
    // The four strips, then the master in a fifth column over its knob
    let width = (STRIPS + 1) as f32 * column + STRIPS as f32 * gap;

    let field_height = 32.0 * z;
    let number_height = 12.0 * z;
    let meter_height = 38.0 * z;
    let mute_height = 13.0 * z;
    let height = field_height + 6.0 * z + number_height + meter_height + 5.0 * z + mute_height;

    // Separator, as the other displays have
    ui.add_space(4.0 * z);
    let accent = crate::dsp::ModuleCategory::Utility.color();
    let full = ui.available_rect_before_wrap();
    ui.painter().hline(
        (full.left() + 4.0 * z)..=(full.right() - 4.0 * z),
        ui.cursor().top(),
        Stroke::new(1.0 * z, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 64)),
    );
    ui.add_space(6.0 * z);

    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return None;
    }
    let painter = ui.painter_at(rect.expand(2.0 * z));
    let small = egui::FontId::proportional(8.5 * z);

    // --- The panorama ---
    let field = Rect::from_min_size(rect.min, Vec2::new(width, field_height));
    painter.rect(field, 4.0 * z, theme::background::MAIN, Stroke::new(1.0 * z, theme::background::GRID_MAJOR));
    let inset = 13.0 * z;
    let (x_left, x_right) = (field.left() + inset, field.right() - inset);
    let x_at = |pan: f32| x_left + (x_right - x_left) * (pan + 1.0) * 0.5;
    painter.text(Pos2::new(field.left() + 6.0 * z, field.center().y), egui::Align2::CENTER_CENTER, "L", small.clone(), theme::text::DISABLED);
    painter.text(Pos2::new(field.right() - 6.0 * z, field.center().y), egui::Align2::CENTER_CENTER, "R", small.clone(), theme::text::DISABLED);
    let lane_top = field.top() + 7.0 * z;
    let lane_pitch = (field_height - 14.0 * z) / (STRIPS - 1) as f32;
    painter.line_segment(
        [Pos2::new(x_at(0.0), field.top() + 3.0 * z), Pos2::new(x_at(0.0), field.bottom() - 3.0 * z)],
        Stroke::new(1.0 * z, theme::background::GRID_MAJOR),
    );

    for (s, strip) in strips.iter().enumerate() {
        let y = lane_top + s as f32 * lane_pitch;
        let hue = CHANNEL_HUES[s];
        let lane_alpha = if strip.patched { 0.4 } else { 0.12 };
        painter.line_segment([Pos2::new(x_left, y), Pos2::new(x_right, y)], Stroke::new(1.0 * z, hue.gamma_multiply(lane_alpha)));
        if !strip.patched {
            continue;
        }

        // Brightness follows the strip's meter, after its fader and Level
        // CV, so the lights breathe with the music rather than flicker
        let glow = if strip.muted { 0.0 } else { meter(s).fraction().powf(1.5) };
        let count = strip.voices.len();
        for (v, &peak) in strip.voices.iter().enumerate() {
            let x = x_at(voice_pan(strip.pan, spread, v, count));
            let center = Pos2::new(x, y);
            let sounding = !user_state.is_playing || peak.is_none_or(|p| p.abs() > 1e-4);
            if !strip.muted && strip.send > 0.01 {
                // The send: a soft smear along the lane, brightest at the
                // light, that swells as the channel plays
                let amount = strip.send.sqrt();
                let alpha = amount * (0.07 + 0.1 * glow);
                for layer in 1..=3 {
                    let reach = (2.0 + 16.0 * amount * layer as f32 / 3.0) * z;
                    let bloom = Rect::from_center_size(center, Vec2::new(2.0 * reach, 3.0 * z));
                    painter.rect_filled(bloom, 1.5 * z, hue.gamma_multiply(alpha));
                }
            }
            if strip.muted {
                painter.circle_stroke(center, 2.2 * z, Stroke::new(1.0 * z, theme::text::DISABLED));
            } else if !sounding {
                // A voice with nothing to play: just its place
                painter.circle_stroke(center, 1.8 * z, Stroke::new(1.0 * z, hue.gamma_multiply(0.35)));
            } else {
                if glow > 0.02 {
                    painter.circle_filled(center, (4.0 + 4.0 * glow) * z, hue.gamma_multiply(0.18 * glow));
                }
                painter.circle_filled(center, (2.0 + 1.4 * glow) * z, hue.gamma_multiply(0.45 + 0.55 * glow));
            }
        }
    }
    let field_response = ui.interact(field, ui.id().with(("mixer_field", node_id)), Sense::hover());
    field_response.on_hover_text(
        "Where each channel sits, left to right, a lane per channel. A polyphonic channel shows a light per voice, fanned out by Spread. A channel with its Send up blooms along its lane",
    );

    // --- Strip meters and mutes, in the knob columns ---
    let numbers_top = field.bottom() + 6.0 * z;
    let meters_top = numbers_top + number_height;
    let mute_top = meters_top + meter_height + 5.0 * z;
    let meter_width = 6.0 * z;
    let mut clicked = None;

    for (s, strip) in strips.iter().enumerate() {
        let n = s + 1;
        let cx = rect.left() + s as f32 * (column + gap) + knob_center;
        let hue = CHANNEL_HUES[s];
        let number_color = if strip.patched { hue } else { hue.gamma_multiply(0.4) };
        painter.text(Pos2::new(cx, numbers_top + number_height / 2.0), egui::Align2::CENTER_CENTER, n.to_string(), small.clone(), number_color);

        let bar = Rect::from_center_size(Pos2::new(cx, meters_top + meter_height / 2.0), Vec2::new(meter_width, meter_height));
        column_meter(&painter, bar, meter(s), strip.muted);
        let level_db = meter(s).level_db();
        ui.interact(bar.expand2(Vec2::new(8.0 * z, 0.0)), ui.id().with(("mixer_meter", node_id, s)), Sense::hover())
            .on_hover_text(format!("Channel {n}: {} after its fader", format_db(level_db)));

        let button = Rect::from_center_size(Pos2::new(cx, mute_top + mute_height / 2.0), Vec2::new(20.0 * z, mute_height));
        let response = ui.interact(button, ui.id().with(("mixer_mute", node_id, s)), Sense::click());
        let hot = response.hovered();
        let (fill, stroke, text) = if strip.muted {
            let red = theme::accent::ERROR;
            (red.gamma_multiply(if hot { 1.0 } else { 0.85 }), red, Color32::from_rgb(40, 18, 20))
        } else if hot {
            (theme::background::WIDGET_HOVERED, theme::background::WIDGET_ACTIVE, theme::text::PRIMARY)
        } else {
            (theme::background::WIDGET, theme::background::WIDGET_ACTIVE, theme::text::SECONDARY)
        };
        painter.rect(button, 3.0 * z, fill, Stroke::new(1.0 * z, stroke));
        painter.text(button.center(), egui::Align2::CENTER_CENTER, "M", egui::FontId::proportional(8.0 * z), text);
        let tip = if strip.muted { format!("Channel {n} is muted: click to hear it") } else { format!("Mute channel {n}") };
        if response.clicked() {
            clicked = Some((format!("Mute {n}"), if strip.muted { 0.0 } else { 1.0 }));
        }
        response.on_hover_text(tip);
    }

    // --- Master: Out L and Out R ---
    let cx = rect.left() + STRIPS as f32 * (column + gap) + knob_center;
    for (side, label) in ["L", "R"].into_iter().enumerate() {
        let x = cx + (side as f32 - 0.5) * 8.0 * z;
        painter.text(Pos2::new(x, numbers_top + number_height / 2.0), egui::Align2::CENTER_CENTER, label, small.clone(), theme::text::SECONDARY);
        let bar = Rect::from_center_size(Pos2::new(x, meters_top + meter_height / 2.0), Vec2::new(5.0 * z, meter_height));
        column_meter(&painter, bar, meter(STRIPS + side), false);
    }
    painter.text(
        Pos2::new(cx, mute_top + mute_height / 2.0),
        egui::Align2::CENTER_CENTER,
        "Out",
        egui::FontId::proportional(8.0 * z),
        theme::text::DISABLED,
    );
    let master_area = Rect::from_x_y_ranges(cx - 12.0 * z..=cx + 12.0 * z, meters_top..=meters_top + meter_height);
    ui.interact(master_area, ui.id().with(("mixer_master", node_id)), Sense::hover()).on_hover_text(format!(
        "Master: L {} / R {}",
        format_db(meter(STRIPS).level_db()),
        format_db(meter(STRIPS + 1).level_db()),
    ));

    ui.add_space(2.0 * z);
    clicked
}

/// A meter level for a tooltip.
fn format_db(db: f32) -> String {
    if db <= -48.0 {
        "-inf dBFS".to_string()
    } else {
        format!("{db:+.1} dBFS")
    }
}
