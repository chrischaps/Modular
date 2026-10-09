//! The Logic node's display: one picture for each of its three jobs.
//!
//! - **The count** is a ring of beads, one per count, read clockwise from the
//!   top like a clock face. The count that fires wears a ring. The counts the
//!   Gate stays open for lie on a green arc, which lights while the gate is
//!   open. A bead in the Count's orange travels round as the clock ticks.
//! - **The logic** is four lamps, lit while AND, OR, XOR and NOT A are high.
//! - **The comparator** is a column the CV rises in, with Threshold drawn
//!   across it as a line you can drag. The CV turns green when it's above.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Stroke, Vec2};
use egui_node_graph2::NodeId;

use crate::app::theme;
use crate::modules::logic::{Logic, MAX_DIVIDE};

use super::{SynthGraph, SynthGraphState, SynthValueType};

/// Engine output ports of the four logic outputs, with their lamp labels.
const LAMPS: [(usize, &str); 4] = [(3, "AND"), (4, "OR"), (5, "XOR"), (6, "NOT A")];

/// Draws the display and returns the Threshold to set while its line is
/// being dragged: `("Threshold", value)`.
pub fn logic_display(
    ui: &mut egui::Ui,
    node_id: NodeId,
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    zoom: f32,
) -> Option<(String, f32)> {
    let z = zoom;
    let node = graph.nodes.get(node_id)?;
    let value_of = |name: &str, default: f32| {
        node.inputs
            .iter()
            .find(|(input, _)| input == name)
            .and_then(|(_, id)| match graph.get_input(*id).value {
                SynthValueType::Number { value, .. } => Some(value),
                _ => None,
            })
            .unwrap_or(default)
    };
    let divide = (value_of("Divide", 4.0).round() as usize).clamp(1, MAX_DIVIDE);
    let offset = (value_of("Offset", 0.0).round().max(0.0) as usize) % divide;
    let length = (value_of("Length", 1.0).round() as usize).clamp(1, divide);
    let threshold = value_of("Threshold", 0.5);

    let engine_node_id = user_state.get_engine_node_id(node_id);
    let readout = engine_node_id.and_then(|id| user_state.readouts.get(&id)).copied().unwrap_or_default();
    let count = readout.values[Logic::READOUT_COUNT];
    let count = (count >= 0.0).then(|| (count.round() as usize) % divide);
    let gate_open = readout.values[Logic::READOUT_GATE] > 0.5;
    let cv = readout.values[Logic::READOUT_CV];
    let above = readout.values[Logic::READOUT_ABOVE] > 0.5;

    // As wide as the knob row below: four 44-pt columns
    let gap = ui.spacing().item_spacing.x;
    let width = 4.0 * 44.0 * z + 3.0 * gap - 8.0 * z;
    let height = 62.0 * z;

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
    let green = theme::signal::GATE;
    let orange = theme::signal::CONTROL;
    let small = egui::FontId::proportional(8.0 * z);

    // --- The count ring ---
    let radius = height / 2.0 - 5.0 * z;
    let center = Pos2::new(rect.left() + radius + 5.0 * z, rect.center().y);
    let angle_of = |n: f32| -FRAC_PI_2 + TAU * n / divide as f32;
    let at = |n: f32, r: f32| center + Vec2::angled(angle_of(n)) * r;
    let bead = (radius * TAU / divide as f32 * 0.28).clamp(0.9 * z, 2.6 * z);

    // The gate's arc: from the count that fires through the last one it's
    // open for, drawn just inside the beads
    let in_gate = |n: usize| (n + divide - offset) % divide < length;
    let arc_radius = radius - bead - 2.5 * z;
    let arc_ink = if gate_open { green } else { green.gamma_multiply(0.35) };
    let arc_end = length as f32 - 0.5;
    let segments = ((arc_end + 0.5) * 48.0 / divide as f32).ceil().max(2.0) as usize;
    let arc: Vec<Pos2> = (0..=segments)
        .map(|s| at(offset as f32 - 0.5 + (arc_end + 0.5) * s as f32 / segments as f32, arc_radius))
        .collect();
    painter.add(egui::Shape::line(arc, Stroke::new(if gate_open { 2.0 * z } else { 1.4 * z }, arc_ink)));

    for n in 0..divide {
        let pos = at(n as f32, radius);
        let fill = if Some(n) == count {
            orange
        } else if in_gate(n) {
            green.gamma_multiply(if gate_open { 0.7 } else { 0.4 })
        } else {
            theme::background::GRID_MAJOR.gamma_multiply(1.8)
        };
        if Some(n) == count {
            painter.circle_filled(pos, bead * 2.2, orange.gamma_multiply(0.25));
        }
        painter.circle_filled(pos, bead, fill);
        if n == offset {
            painter.circle_stroke(pos, bead + 1.8 * z, Stroke::new(1.0 * z, green));
        }
    }
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        format!("÷{divide}"),
        egui::FontId::new(11.0 * z, egui::FontFamily::Name(theme::TITLE_FAMILY.into())),
        theme::text::SECONDARY,
    );
    let ring = Rect::from_center_size(center, Vec2::splat(radius * 2.0 + 4.0 * z));
    let place = match count {
        Some(n) => format!("Count {} of {divide}", n + 1),
        None => "Waiting for the first clock".to_string(),
    };
    let open_for = if length >= divide { "always".to_string() } else { format!("for {length} clock{}", if length == 1 { "" } else { "s" }) };
    ui.interact(ring, ui.id().with(("logic_ring", node_id)), Sense::hover())
        .on_hover_text(format!("{place}\nFires on count {}, the ringed bead\nGate stays open {open_for}, along the green arc", offset + 1));

    // --- The column, at the right: CV against Threshold ---
    let column = Rect::from_x_y_ranges(rect.right() - 12.0 * z..=rect.right() - 2.0 * z, rect.top() + 2.0 * z..=rect.bottom() - 11.0 * z);
    let y_at = |value: f32| column.bottom() - column.height() * ((value + 1.0) / 2.0).clamp(0.0, 1.0);
    painter.rect(column, 3.0 * z, theme::background::MAIN, Stroke::new(1.0 * z, theme::background::GRID_MAJOR));
    let level = Rect::from_x_y_ranges(column.shrink(2.0 * z).x_range(), y_at(cv)..=column.bottom() - 2.0 * z);
    if level.height() > 0.0 {
        painter.rect_filled(level, 2.0 * z, if above { green } else { orange.gamma_multiply(0.6) });
    }
    painter.text(Pos2::new(column.center().x, rect.bottom()), egui::Align2::CENTER_BOTTOM, "CV", small.clone(), theme::text::DISABLED);

    // The Threshold, a line across the column, draggable
    let line_y = y_at(threshold);
    let grab = Rect::from_x_y_ranges(column.left() - 4.0 * z..=column.right() + 2.0 * z, line_y - 4.0 * z..=line_y + 4.0 * z);
    let response = ui
        .interact(grab, ui.id().with(("logic_threshold", node_id)), Sense::drag())
        .on_hover_cursor(CursorIcon::ResizeVertical);
    let hot = response.hovered() || response.dragged();
    let line_ink = if hot { green } else { green.gamma_multiply(0.75) };
    painter.hline(column.left() - 3.0 * z..=column.right() + 1.0 * z, line_y, Stroke::new(if hot { 1.8 * z } else { 1.2 * z }, line_ink));
    let dragged = response
        .dragged()
        .then(|| response.interact_pointer_pos())
        .flatten()
        .map(|pointer| ((column.bottom() - pointer.y) / column.height() * 2.0 - 1.0).clamp(-1.0, 1.0))
        .map(|value| ("Threshold".to_string(), (value * 100.0).round() / 100.0));
    response.on_hover_text(format!(
        "CV {cv:.2}, Threshold {threshold:.2}: Above is {}\nDrag the line to move the Threshold",
        if above { "high" } else { "low" }
    ));

    // --- The lamps, between: AND, OR, XOR, NOT A ---
    let lamps = Rect::from_x_y_ranges(ring.right() + 10.0 * z..=column.left() - 12.0 * z, rect.top() + 4.0 * z..=rect.bottom() - 4.0 * z);
    let lamp_gap = 4.0 * z;
    let lamp_size = Vec2::new((lamps.width() - lamp_gap) / 2.0, (lamps.height() - lamp_gap) / 2.0);
    for (i, (port, label)) in LAMPS.into_iter().enumerate() {
        let min = lamps.left_top() + Vec2::new((i % 2) as f32 * (lamp_size.x + lamp_gap), (i / 2) as f32 * (lamp_size.y + lamp_gap));
        let lamp = Rect::from_min_size(min, lamp_size);
        let lit = engine_node_id.and_then(|id| user_state.get_output_value(id, port)).is_some_and(|v| v > 0.5);
        if lit {
            painter.rect(lamp, 4.0 * z, green.gamma_multiply(0.8), Stroke::new(1.0 * z, green));
            painter.text(lamp.center(), egui::Align2::CENTER_CENTER, label, small.clone(), Color32::WHITE);
        } else {
            painter.rect(lamp, 4.0 * z, theme::background::MAIN, Stroke::new(1.0 * z, theme::background::GRID_MAJOR));
            painter.text(lamp.center(), egui::Align2::CENTER_CENTER, label, small.clone(), theme::text::DISABLED);
        }
    }

    dragged
}
