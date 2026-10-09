//! How a group's nodes look: the group's own node, with its name, a
//! miniature of what's inside it and the knobs pinned to it, and the Inputs
//! and Outputs nodes inside it.
//!
//! A group wears rose, the one header colour no built-in module has, so a
//! module of your own making reads as yours at a glance. Its face shows the
//! modules inside as small cards in their own colours, wired as they are,
//! and opens the group when clicked.

use egui::epaint::CubicBezierShape;
use egui::text::{CCursor, CCursorRange};
use egui::{vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};
use egui_node_graph2::{NodeId, NodeResponse};

use crate::app::theme;
use super::groups::NodeKind;
use super::node_data::KnobPlace;
use super::{SynthGraph, SynthGraphState, SynthNodeData, SynthResponse};

type Responses = Vec<NodeResponse<SynthResponse, SynthNodeData>>;

/// Width of the miniature on a group's face, unzoomed.
const PREVIEW_WIDTH: f32 = 176.0;
/// Its height follows the shape of what's inside, within these.
const PREVIEW_MIN_HEIGHT: f32 = 46.0;
const PREVIEW_MAX_HEIGHT: f32 = 100.0;
/// Pinned knobs per row on a group's face.
const KNOBS_PER_ROW: usize = 4;

/// The miniature's backing, a little deeper than the node body.
const PREVIEW_FILL: Color32 = Color32::from_rgb(28, 30, 40);
/// A miniature module's body.
const CARD_FILL: Color32 = Color32::from_rgb(58, 61, 74);

/// The header colour of a group's node, or of its Inputs and Outputs,
/// which wear it more quietly.
pub fn header_color(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Group(_) => theme::module::GROUP,
        _ => theme::module::GROUP.lerp_to_gamma(Color32::from_gray(96), 0.45),
    }
}

/// The header of a group's node or its Inputs or Outputs: its icon and its
/// right-click menu. A group opens on a double-click.
pub fn top_bar(data: &SynthNodeData, ui: &mut egui::Ui, node_id: NodeId, user_state: &mut SynthGraphState, zoom: f32) -> Responses {
    let mut responses = Vec::new();
    if let Some(window) = ui.ctx().read_response(egui::Id::new((node_id, "window"))) {
        if data.kind.group().is_some() {
            if window.double_clicked() {
                responses.push(NodeResponse::User(SynthResponse::EnterGroup(node_id)));
            }
            if window.secondary_clicked() {
                responses.push(NodeResponse::User(SynthResponse::NodeSelected(node_id)));
            }
            if window.context_menu(|ui| group_menu(ui, node_id, &mut responses)).is_some() {
                user_state.widget_context_menu_open = true;
            }
        }
    }

    let size = 14.0 * zoom;
    let (rect, response) = ui.allocate_exact_size(vec2(size + 4.0 * zoom, size), Sense::hover());
    if !data.description.is_empty() {
        response.on_hover_text(data.description);
    }
    let center = Pos2::new(rect.left() + size * 0.5, rect.center().y);
    let ink = data.titlebar_ink();
    match data.kind {
        NodeKind::Group(_) => draw_stack(ui.painter(), center, size, ink),
        NodeKind::Inputs(_) => draw_doorway(ui.painter(), center, size, ink, true),
        NodeKind::Outputs(_) => draw_doorway(ui.painter(), center, size, ink, false),
        NodeKind::Module => {}
    }
    responses
}

/// A group's right-click menu.
fn group_menu(ui: &mut egui::Ui, node_id: NodeId, responses: &mut Responses) {
    ui.set_min_width(190.0);
    let mut item = |ui: &mut egui::Ui, label: &str, shortcut: &str, response: SynthResponse| {
        if ui.add(egui::Button::new(label).shortcut_text(shortcut)).clicked() {
            responses.push(NodeResponse::User(response));
            ui.close_menu();
        }
    };
    item(ui, "Open", "Tab", SynthResponse::EnterGroup(node_id));
    item(ui, "Rename", "F2", SynthResponse::StartRename(node_id));
    ui.separator();
    item(ui, "Duplicate", "Ctrl+D", SynthResponse::DuplicateNode(node_id));
    item(ui, "Copy", "Ctrl+C", SynthResponse::CopyNode(node_id));
    item(ui, "Ungroup", "Ctrl+Alt+G", SynthResponse::UngroupNode(node_id));
    ui.separator();
    item(ui, "Delete", "Del", SynthResponse::DeleteNode(node_id));
}

/// Two cards, one behind the other: a group of modules.
fn draw_stack(painter: &egui::Painter, center: Pos2, size: f32, ink: Color32) {
    let side = size * 0.62;
    let back = Rect::from_center_size(center + vec2(size * 0.14, -size * 0.14), vec2(side, side));
    let front = Rect::from_center_size(center + vec2(-size * 0.12, size * 0.12), vec2(side, side));
    let rounding = size * 0.12;
    painter.rect_stroke(back, rounding, Stroke::new((size * 0.1).max(1.0), ink.gamma_multiply(0.75)));
    painter.rect_filled(front.expand(size * 0.07), rounding, theme::module::GROUP);
    painter.rect_filled(front, rounding, ink);
}

/// An arrow through an edge: signals coming in through the group's input
/// jacks, or going out through its outputs.
fn draw_doorway(painter: &egui::Painter, center: Pos2, size: f32, ink: Color32, inward: bool) {
    let s = size * 0.5;
    let stroke = Stroke::new((size * 0.12).max(1.0), ink);
    let edge_x = if inward { center.x - s * 0.55 } else { center.x + s * 0.55 };
    painter.line_segment([Pos2::new(edge_x, center.y - s * 0.8), Pos2::new(edge_x, center.y + s * 0.8)], stroke);
    let (from, to) = (center.x - s * 0.9, center.x + s * 0.9);
    painter.line_segment([Pos2::new(from, center.y), Pos2::new(to, center.y)], stroke);
    painter.line_segment([Pos2::new(to, center.y), Pos2::new(to - s * 0.45, center.y - s * 0.45)], stroke);
    painter.line_segment([Pos2::new(to, center.y), Pos2::new(to - s * 0.45, center.y + s * 0.45)], stroke);
}

/// The body of a group's node: the name being typed, if it is, then the
/// miniature, then the pinned knobs.
pub fn body(
    data: &SynthNodeData,
    ui: &mut egui::Ui,
    node_id: NodeId,
    graph: &SynthGraph,
    user_state: &mut SynthGraphState,
    zoom: f32,
) -> Responses {
    let mut responses = Vec::new();
    let Some(id) = data.kind.group() else {
        // Inputs or Outputs, which say so when there's nothing to show
        let node = &graph[node_id];
        let none = match data.kind {
            NodeKind::Inputs(_) if node.outputs.is_empty() => "No input jacks",
            NodeKind::Outputs(_) if node.inputs.is_empty() => "No output jacks",
            _ => return responses,
        };
        ui.label(RichText::new(none).small().color(theme::text::DISABLED));
        return responses;
    };
    ui.add_space(4.0 * zoom);
    rename_field(ui, node_id, data, user_state, zoom, &mut responses);

    let face = user_state.group_faces.get(&id).cloned().unwrap_or_default();
    let preview = &face.preview;
    let width = PREVIEW_WIDTH * zoom;
    let aspect = if preview.bounds.is_positive() { preview.bounds.width() / preview.bounds.height() } else { 3.0 };
    let height = (width / aspect).clamp(PREVIEW_MIN_HEIGHT * zoom, PREVIEW_MAX_HEIGHT * zoom);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let hovered = response.hovered();
    let painter = ui.painter_at(rect.expand(1.0));
    let rounding = 6.0 * zoom;
    let rose = theme::module::GROUP;
    painter.rect(rect, rounding, PREVIEW_FILL, Stroke::new(1.0, rose.gamma_multiply(if hovered { 0.8 } else { 0.3 })));

    let inner = rect.shrink2(vec2(8.0, 7.0) * zoom);
    if preview.boxes.is_empty() {
        painter.text(rect.center(), Align2::CENTER_CENTER, "Empty", FontId::proportional(11.0 * zoom), theme::text::DISABLED);
    } else {
        let bounds = preview.bounds;
        let scale = (inner.width() / bounds.width()).min(inner.height() / bounds.height());
        let map = |p: Pos2| inner.center() + (p - bounds.center()) * scale;
        let line = (1.3 * zoom).max(0.8);
        for ([a, b], color) in &preview.wires {
            let (a, b) = (map(*a), map(*b));
            let reach = ((b.x - a.x).abs() * 0.5).max(10.0 * zoom);
            painter.add(CubicBezierShape::from_points_stroke(
                [a, a + vec2(reach, 0.0), b - vec2(reach, 0.0), b],
                false,
                Color32::TRANSPARENT,
                Stroke::new(line, color.gamma_multiply(0.8)),
            ));
        }
        for (card, color) in &preview.boxes {
            let card = Rect::from_min_max(map(card.min), map(card.max)).shrink(1.0 * zoom);
            let corner = (2.5 * zoom).min(card.height() * 0.3);
            painter.rect_filled(card, corner, CARD_FILL);
            let band = Rect::from_min_max(card.min, Pos2::new(card.max.x, card.min.y + (card.height() * 0.3).max(2.0)));
            painter.rect_filled(band, corner, *color);
        }
    }
    if hovered {
        let label = "Open  ▸";
        let font = FontId::proportional(12.0 * zoom);
        let galley = painter.layout_no_wrap(label.to_string(), font, theme::text::PRIMARY);
        let pill = Rect::from_center_size(rect.center(), galley.size() + vec2(18.0, 8.0) * zoom);
        painter.rect_filled(pill, pill.height() / 2.0, Color32::from_black_alpha(190));
        painter.rect_stroke(pill, pill.height() / 2.0, Stroke::new(1.0, rose.gamma_multiply(0.9)));
        painter.galley(pill.center() - galley.size() / 2.0, galley, theme::text::PRIMARY);
    }
    if response.on_hover_text("Open the group (double-click it, or Tab)").clicked() {
        responses.push(NodeResponse::User(SynthResponse::EnterGroup(node_id)));
    }

    // The knobs pinned to it, in their modules' colours
    if !face.knobs.is_empty() {
        ui.add_space(6.0 * zoom);
        let outer = data.parent.is_some();
        for row in face.knobs.chunks(KNOBS_PER_ROW) {
            ui.horizontal(|ui| {
                for knob in row {
                    let Some(module) = graph.nodes.get(knob.node) else { continue };
                    let Some(param) = module.user_data.knob_params.iter().find(|k| k.param_name == knob.param) else {
                        continue;
                    };
                    let place = KnobPlace::Face { depth: knob.depth, outer };
                    module.user_data.knob_cell(ui, knob.node, param, graph, user_state, zoom, &mut responses, place);
                }
            });
        }
    }
    responses
}

/// The field a group's name is typed into, while it's being named.
fn rename_field(
    ui: &mut egui::Ui,
    node_id: NodeId,
    data: &SynthNodeData,
    user_state: &mut SynthGraphState,
    zoom: f32,
    responses: &mut Responses,
) {
    let Some(id) = data.kind.group() else { return };
    let Some(renaming) = user_state.renaming.as_mut().filter(|r| r.group == id) else { return };
    let edit_id = egui::Id::new((node_id, "group name"));
    let mut output = egui::TextEdit::singleline(&mut renaming.text)
        .id(edit_id)
        .font(FontId::proportional(14.0 * zoom))
        .hint_text("Name this group")
        .desired_width(PREVIEW_WIDTH * zoom)
        .show(ui);
    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if renaming.fresh {
        renaming.fresh = false;
        output.response.request_focus();
        let all = CCursorRange::two(CCursor::new(0), CCursor::new(renaming.text.chars().count()));
        output.state.cursor.set_char_range(Some(all));
        output.state.store(ui.ctx(), edit_id);
    } else if output.response.lost_focus() || !output.response.has_focus() {
        let name = renaming.text.trim().to_string();
        user_state.renaming = None;
        if !escape && !name.is_empty() {
            responses.push(NodeResponse::User(SynthResponse::RenameGroup { node_id, name }));
        }
    }
    ui.label(RichText::new("Enter to name it, Esc to keep the name").small().color(theme::text::DISABLED));
    ui.add_space(4.0 * zoom);
}
