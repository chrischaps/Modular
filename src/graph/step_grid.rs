//! The Step Sequencer's grid: a button per step in rows of eight, with its
//! note underneath.
//!
//! - Click switches a step's gate, Shift+click its tie.
//! - Drag a step up or down to move its note a semitone at a time, or with
//!   Shift an octave at a time. The note it will land on shows above it.
//! - Right-click opens a two-octave piano under the step, for writing notes
//!   the way a hardware sequencer's step-record does: each key played writes
//!   the step and moves along to the next one, so a melody goes in key after
//!   key. The piano stays put, and the step it will write next is outlined
//!   in the pitch cable's orange.
//!
//! A tied step reaches across the gap into the next one, the way a held note
//! looks on a piano roll. At the end of a row, or of the pattern, it reaches
//! out of its right side and into the next step's left.

use eframe::egui::{self, Color32, CursorIcon, Id, Key, LayerId, Modifiers, Order, PopupCloseBehavior, RichText, Sense};
use egui_node_graph2::{NodeId, NodeResponse};

use crate::app::theme;
use crate::modules::sequencer::note_to_name;
use crate::widgets::{piano_keys, PianoConfig, PianoData};

use super::{SynthNodeData, SynthResponse};

const GATE_ON: Color32 = Color32::from_rgb(100, 200, 100);
const GATE_OFF: Color32 = Color32::from_rgb(60, 60, 70);
/// Points of upward drag per semitone, before zoom: an octave is 72.
const DRAG_PER_SEMITONE: f32 = 6.0;
/// Points of drag per octave with Shift held, before zoom.
const DRAG_PER_OCTAVE: f32 = 18.0;
/// The highest C the piano's lower octave can start on, so its two octaves
/// stay inside MIDI's 0–127.
const TOP_BASE: u8 = 96;
/// One octave of the piano, in points.
const OCTAVE_WIDTH: f32 = 140.0;
const KEY_HEIGHT: f32 = 48.0;
const HINT: &str = "Click: gate · Shift+click: tie\nDrag up or down: note (Shift: by octave)\nRight-click: piano";

/// A Step Sequencer's pattern, as its grid shows it.
pub(super) struct StepPattern {
    pub steps: usize,
    /// The step sounding now.
    pub current: usize,
    pub pitches: [u8; 16],
    pub gates: [bool; 16],
    pub ties: [bool; 16],
}

/// The piano's place in the pattern while it's open: the step its next key
/// writes, the C its lower octave starts on, and the step it opened under.
/// It stays under that one as the writing moves along, so the keys don't
/// slide away from the hand playing them.
#[derive(Clone, Copy, Debug, PartialEq)]
struct StepEntry {
    step: usize,
    base: u8,
    anchor: usize,
}

/// A drag under way: the step, and its note when the drag began.
#[derive(Clone, Copy, Debug)]
struct StepDrag {
    step: usize,
    from: u8,
}

/// The note a drag lands on: `dy` points down the screen from where it
/// began, so dragging up raises the note.
fn dragged_pitch(from: u8, dy: f32, zoom: f32, octaves: bool) -> u8 {
    let semitones = if octaves {
        (-dy / (DRAG_PER_OCTAVE * zoom)).round() as i32 * 12
    } else {
        (-dy / (DRAG_PER_SEMITONE * zoom)).round() as i32
    };
    (from as i32 + semitones).clamp(0, 127) as u8
}

/// Where the piano opens for a step's note: the note in its upper octave,
/// so there's an octave below it and most of one above.
fn popover_base(pitch: u8) -> u8 {
    shift_base(pitch / 12 * 12, -12)
}

/// The piano moved by `semitones` (a multiple of 12), kept on the keyboard.
fn shift_base(base: u8, semitones: i32) -> u8 {
    (base as i32 + semitones).clamp(0, TOP_BASE as i32) as u8
}

/// The step `by` steps along from `step`, round the loop.
fn step_along(step: usize, by: isize, steps: usize) -> usize {
    (step as isize + by).rem_euclid(steps.max(1) as isize) as usize
}

pub(super) fn step_grid(
    ui: &mut egui::Ui,
    zoom: f32,
    pattern: &StepPattern,
    node_id: NodeId,
    responses: &mut Vec<NodeResponse<SynthResponse, SynthNodeData>>,
) {
    let step_size = 24.0 * zoom;
    let step_spacing = 3.0 * zoom;
    let mut set = |param_name: String, value: f32| {
        responses.push(NodeResponse::User(SynthResponse::ParameterChanged { node_id, param_name, value }));
    };

    let entry_id = Id::new(("step_entry", node_id));
    let drag_id = Id::new(("step_drag", node_id));
    let mut entry = ui
        .memory(|m| m.is_popup_open(entry_id))
        .then(|| ui.data(|d| d.get_temp::<StepEntry>(entry_id)))
        .flatten()
        .map(|e| StepEntry { step: e.step.min(pattern.steps - 1), anchor: e.anchor.min(pattern.steps - 1), ..e });

    ui.vertical(|ui| {
        ui.set_min_width(220.0 * zoom);

        // Lay out every step first, so a tie can be drawn under both ends
        let mut cells = Vec::with_capacity(pattern.steps);
        for row_start in (0..pattern.steps).step_by(8) {
            if row_start > 0 {
                ui.add_space(2.0 * zoom);
            }
            ui.horizontal(|ui| {
                for _ in row_start..(row_start + 8).min(pattern.steps) {
                    let (rect, response) = ui
                        .allocate_exact_size(egui::vec2(step_size, step_size + 12.0 * zoom), Sense::click_and_drag());
                    cells.push((egui::Rect::from_min_size(rect.min, egui::vec2(step_size, step_size)), response.on_hover_text(HINT)));
                    ui.add_space(step_spacing);
                }
            });
        }

        // Ties, under the steps
        let painter = ui.painter();
        let band = step_size * 0.22;
        let reach = step_spacing * 2.0;
        let bar = |left: f32, right: f32, y: f32| egui::Rect::from_min_max(egui::pos2(left, y - band), egui::pos2(right, y + band));
        for step in (0..pattern.steps).filter(|&s| pattern.ties[s] && pattern.gates[s]) {
            let from = cells[step].0;
            let to = cells[(step + 1) % pattern.steps].0;
            if step + 1 < pattern.steps && (to.center().y - from.center().y).abs() < 1.0 {
                painter.rect_filled(bar(from.center().x, to.center().x, from.center().y), 0.0, GATE_ON);
            } else {
                painter.rect_filled(bar(from.center().x, from.right() + reach, from.center().y), band, GATE_ON);
                painter.rect_filled(bar(to.left() - reach, to.center().x, to.center().y), band, GATE_ON);
            }
        }

        for (step, (step_rect, response)) in cells.iter().enumerate() {
            let is_current = step == pattern.current;
            let base_color = if pattern.gates[step] { GATE_ON } else { GATE_OFF };
            let color = if is_current {
                // Brighten current step
                Color32::from_rgb(
                    (base_color.r() as u16 + 100).min(255) as u8,
                    (base_color.g() as u16 + 100).min(255) as u8,
                    (base_color.b() as u16 + 50).min(255) as u8,
                )
            } else {
                base_color
            };
            painter.rect_filled(*step_rect, 3.0, color);
            if is_current {
                painter.rect_stroke(*step_rect, 3.0, egui::Stroke::new(2.0, Color32::WHITE));
            }

            // Note name below
            let pitch = pattern.pitches[step];
            painter.text(
                egui::pos2(step_rect.center().x, step_rect.bottom() + 2.0 * zoom),
                egui::Align2::CENTER_TOP,
                note_to_name(pitch),
                egui::FontId::proportional(8.0 * zoom),
                Color32::from_gray(180),
            );

            if response.clicked() {
                if ui.input(|i| i.modifiers.shift) {
                    set(format!("Step {} Tie", step + 1), if pattern.ties[step] { 0.0 } else { 1.0 });
                } else {
                    set(format!("Step {} Gate", step + 1), if pattern.gates[step] { 0.0 } else { 1.0 });
                }
            }
            if response.secondary_clicked() {
                entry = Some(StepEntry { step, base: popover_base(pitch), anchor: step });
                ui.memory_mut(|m| m.open_popup(entry_id));
            }

            // Dragging up or down moves the note
            if response.drag_started() {
                ui.data_mut(|d| d.insert_temp(drag_id, StepDrag { step, from: pitch }));
            }
            let drag = ui.data(|d| d.get_temp::<StepDrag>(drag_id)).filter(|d| d.step == step);
            if let Some(drag) = drag.filter(|_| response.dragged()) {
                let (origin, now, octaves) =
                    ui.input(|i| (i.pointer.press_origin(), i.pointer.interact_pos(), i.modifiers.shift));
                if let (Some(origin), Some(now)) = (origin, now) {
                    let to = dragged_pitch(drag.from, now.y - origin.y, zoom, octaves);
                    if to != pitch {
                        set(format!("Step {} Pitch", step + 1), to as f32);
                    }
                    ui.ctx().set_cursor_icon(CursorIcon::ResizeVertical);
                    drag_badge(ui, node_id, *step_rect, &note_to_name(to), zoom);
                }
            }
            if response.drag_stopped() {
                ui.data_mut(|d| d.remove::<StepDrag>(drag_id));
            }
        }

        let Some(mut open) = entry else {
            return;
        };
        // The step the next key writes
        painter.rect_stroke(cells[open.step].0.expand(2.0 * zoom), 4.0, egui::Stroke::new(2.0, theme::signal::CONTROL));

        let (back, on, escape) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if escape {
            ui.memory_mut(|m| m.close_popup());
        }
        open.step = step_along(open.step, on as isize - back as isize, pattern.steps);

        let anchor = &cells[open.anchor].1;
        egui::popup_below_widget(ui, entry_id, anchor, PopupCloseBehavior::CloseOnClickOutside, |ui| {
            step_piano(ui, &mut open, pattern, &mut set);
        });
        if ui.memory(|m| m.is_popup_open(entry_id)) {
            ui.data_mut(|d| d.insert_temp(entry_id, open));
        } else {
            ui.data_mut(|d| d.remove::<StepEntry>(entry_id));
        }
    });
}

/// The note a drag will land on, floated above its step where the node's
/// edge can't clip it.
fn drag_badge(ui: &egui::Ui, node_id: NodeId, step: egui::Rect, name: &str, zoom: f32) {
    let painter = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::new(("step_drag_badge", node_id))));
    let font = egui::FontId::proportional((12.0 * zoom).max(11.0));
    let galley = painter.layout_no_wrap(name.to_string(), font, Color32::from_rgb(30, 22, 12));
    let size = galley.size() + egui::vec2(10.0, 4.0);
    let rect = egui::Rect::from_center_size(egui::pos2(step.center().x, step.top() - 4.0 * zoom - size.y / 2.0), size);
    painter.rect_filled(rect, 4.0, theme::signal::CONTROL);
    painter.galley(rect.center() - galley.size() / 2.0, galley, Color32::PLACEHOLDER);
}

/// The piano under a step: two octaves, the step's note lit. A key writes
/// its note to the step, switches a rest on, and moves along to the next step.
fn step_piano(ui: &mut egui::Ui, open: &mut StepEntry, pattern: &StepPattern, set: &mut impl FnMut(String, f32)) {
    let step = open.step;
    let pitch = pattern.pitches[step];

    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Step {}", step + 1)).strong());
        ui.label(RichText::new(note_to_name(pitch)).color(theme::signal::CONTROL));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("▸").on_hover_text("Next step (→)").clicked() {
                open.step = step_along(step, 1, pattern.steps);
            }
            if ui.small_button("◂").on_hover_text("Previous step (←)").clicked() {
                open.step = step_along(step, -1, pattern.steps);
            }
        });
    });

    ui.horizontal(|ui| {
        if ui.small_button("‹").on_hover_text("Octave down").clicked() {
            open.base = shift_base(open.base, -12);
        }
        let range = format!("{} – {}", note_to_name(open.base), note_to_name(open.base + 23));
        ui.label(RichText::new(range).weak());
        if ui.small_button("›").on_hover_text("Octave up").clicked() {
            open.base = shift_base(open.base, 12);
        }
    });

    let config = PianoConfig::scale(theme::signal::CONTROL).with_size(OCTAVE_WIDTH, KEY_HEIGHT);
    let played = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let mut played = None;
            for octave in [open.base, open.base + 12] {
                let lit = (octave..octave + 12).contains(&pitch);
                let data = PianoData { active_notes: if lit { vec![pitch] } else { Vec::new() }, base_note: octave, ..Default::default() };
                let (response, key) = piano_keys(ui, &data, &config);
                if let Some(key) = key {
                    let note = octave + key;
                    if response.on_hover_text(note_to_name(note)).clicked() {
                        played = Some(note);
                    }
                }
            }
            played
        })
        .inner;
    if let Some(note) = played {
        set(format!("Step {} Pitch", step + 1), note as f32);
        if !pattern.gates[step] {
            set(format!("Step {} Gate", step + 1), 1.0);
        }
        open.step = step_along(step, 1, pattern.steps);
    }

    ui.separator();
    let mut tie = pattern.ties[step];
    let hint = "Holds this note into the next step, which continues it without a new attack (Shift+click)";
    if ui.checkbox(&mut tie, "Tie into next step").on_hover_text(hint).changed() {
        set(format!("Step {} Tie", step + 1), if tie { 1.0 } else { 0.0 });
    }
    ui.label(RichText::new("Each key writes this step, then moves to the next").small().weak());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragging_up_raises_the_note_a_semitone_per_six_points() {
        assert_eq!(dragged_pitch(60, 0.0, 1.0, false), 60);
        assert_eq!(dragged_pitch(60, -6.0, 1.0, false), 61);
        assert_eq!(dragged_pitch(60, -72.0, 1.0, false), 72);
        assert_eq!(dragged_pitch(60, 12.0, 1.0, false), 58);
        // Under half a semitone's travel stays put
        assert_eq!(dragged_pitch(60, -2.9, 1.0, false), 60);
    }

    #[test]
    fn drag_distance_grows_with_zoom() {
        assert_eq!(dragged_pitch(60, -12.0, 2.0, false), 61);
    }

    #[test]
    fn shift_drags_by_octaves() {
        assert_eq!(dragged_pitch(60, -18.0, 1.0, true), 72);
        assert_eq!(dragged_pitch(60, 36.0, 1.0, true), 36);
        assert_eq!(dragged_pitch(64, -5.0, 1.0, true), 64);
    }

    #[test]
    fn drags_stop_at_the_ends_of_midi() {
        assert_eq!(dragged_pitch(125, -60.0, 1.0, false), 127);
        assert_eq!(dragged_pitch(2, 60.0, 1.0, false), 0);
        assert_eq!(dragged_pitch(120, -18.0, 1.0, true), 127);
    }

    #[test]
    fn the_piano_opens_with_the_note_in_its_upper_octave() {
        assert_eq!(popover_base(60), 48); // C4 shows C3–B4
        assert_eq!(popover_base(71), 48);
        assert_eq!(popover_base(5), 0);
        assert_eq!(popover_base(127), TOP_BASE);
    }

    #[test]
    fn the_piano_stays_on_the_keyboard() {
        assert_eq!(shift_base(48, 12), 60);
        assert_eq!(shift_base(0, -12), 0);
        assert_eq!(shift_base(TOP_BASE, 12), TOP_BASE);
        assert!(TOP_BASE as u32 + 23 <= 127);
    }

    #[test]
    fn stepping_wraps_round_the_pattern() {
        assert_eq!(step_along(3, 1, 8), 4);
        assert_eq!(step_along(7, 1, 8), 0);
        assert_eq!(step_along(0, -1, 5), 4);
        assert_eq!(step_along(2, 0, 5), 2);
    }
}
