//! The Clock Divider's display: the count as a ring of beads.
//!
//! One bead per count, read clockwise from the top like a clock face. The
//! count that fires wears a ring. The counts the Gate stays open for lie
//! along a green arc, which lights while the gate is open. A bead in the
//! Count's orange travels round as the clock ticks.

use std::f32::consts::{FRAC_PI_2, TAU};

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use egui_node_graph2::NodeId;

use crate::app::theme;
use crate::modules::divider::{ClockDivider, MAX_DIVIDE};

use super::{SynthGraph, SynthGraphState, SynthValueType};

/// Draws the ring.
pub fn divider_display(ui: &mut egui::Ui, node_id: NodeId, graph: &SynthGraph, user_state: &SynthGraphState, zoom: f32) {
    let z = zoom;
    let Some(node) = graph.nodes.get(node_id) else { return };
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
    let (divide, offset, length) =
        ClockDivider::counts(&[value_of("Divide", 4.0), value_of("Offset", 0.0), value_of("Length", 1.0)]);
    debug_assert!(divide <= MAX_DIVIDE);

    let readout = user_state
        .get_engine_node_id(node_id)
        .and_then(|id| user_state.readouts.get(&id))
        .copied()
        .unwrap_or_default();
    let count = readout.values[ClockDivider::READOUT_COUNT];
    let count = (count >= 0.0).then(|| (count.round() as usize) % divide);
    let gate_open = readout.values[ClockDivider::READOUT_GATE] > 0.5;

    // As wide as the knob row below: three 44-pt columns
    let gap = ui.spacing().item_spacing.x;
    let width = 3.0 * 44.0 * z + 2.0 * gap - 8.0 * z;
    let height = 74.0 * z;

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

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect.expand(2.0 * z));
    let green = theme::signal::GATE;
    let orange = theme::signal::CONTROL;

    let radius = height / 2.0 - 5.0 * z;
    let center = rect.center();
    let at = |n: f32, r: f32| center + Vec2::angled(-FRAC_PI_2 + TAU * n / divide as f32) * r;
    let bead = (radius * TAU / divide as f32 * 0.28).clamp(0.9 * z, 4.0 * z);

    // The gate's arc: from the count that fires through the last one it's
    // open for, drawn just inside the beads
    let in_gate = |n: usize| (n + divide - offset) % divide < length;
    let arc_radius = radius - bead - 2.5 * z;
    let arc_ink = if gate_open { green } else { green.gamma_multiply(0.35) };
    let span = length as f32;
    let segments = (span * 48.0 / divide as f32).ceil().max(2.0) as usize;
    let arc: Vec<Pos2> = (0..=segments)
        .map(|s| at(offset as f32 - 0.5 + span * s as f32 / segments as f32, arc_radius))
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

    // ÷N in the middle, and the count under it
    painter.text(
        center - Vec2::new(0.0, 4.0 * z),
        egui::Align2::CENTER_CENTER,
        format!("÷{divide}"),
        egui::FontId::new(12.0 * z, egui::FontFamily::Name(theme::TITLE_FAMILY.into())),
        theme::text::SECONDARY,
    );
    painter.text(
        center + Vec2::new(0.0, 9.0 * z),
        egui::Align2::CENTER_CENTER,
        count.map_or("–".to_string(), |n| (n + 1).to_string()),
        egui::FontId::proportional(9.0 * z),
        if count.is_some() { orange } else { theme::text::DISABLED },
    );

    let place = match count {
        Some(n) => format!("Count {} of {divide}", n + 1),
        None => "Waiting for the first clock".to_string(),
    };
    let open_for = if length >= divide { "always".to_string() } else { format!("for {length} clock{}", if length == 1 { "" } else { "s" }) };
    let ring = Rect::from_center_size(center, Vec2::splat(radius * 2.0 + 4.0 * z));
    if response.hover_pos().is_some_and(|pos| ring.contains(pos)) {
        response.on_hover_text(format!("{place}\nFires on count {}, the ringed bead\nGate stays open {open_for}, along the green arc", offset + 1));
    }
}
