//! Where the patch is in the bar, and what's keeping time.
//!
//! The Clock's display is a bar of four beat lamps. The lamp of the beat
//! playing flashes as the beat lands and fades through it, the downbeat's a
//! little brighter, and a thin line sweeps under them across the bar. Beside
//! them a badge says where the time comes from: INT for the Clock's own
//! Tempo, MIDI for a clock master, lit while its ticks are coming in.
//!
//! Knobs whose job another control has taken over are drawn still and
//! dimmed, reading what's actually in charge: an LFO's Rate shows its
//! division while it's synced, and the Clock's Tempo shows the master's
//! tempo while it follows MIDI.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use egui_node_graph2::NodeId;

use crate::app::theme;
use crate::dsp::Readout;
use crate::modules::clock::Clock;
use crate::modules::lfo::{Lfo, SYNC_DIVISIONS};

use super::{SynthGraph, SynthGraphState, SynthValueType};

/// Beats in the bar the lamps show.
const BEATS_PER_BAR: usize = 4;

/// The value of the Select parameter `name` on a node, if it has one.
fn selected(graph: &SynthGraph, node_id: NodeId, name: &str) -> Option<usize> {
    let node = graph.nodes.get(node_id)?;
    let (_, input) = node.inputs.iter().find(|(input, _)| input == name)?;
    match graph.get_input(*input).value {
        SynthValueType::Select { value, .. } => Some(value),
        _ => None,
    }
}

/// Whether a Clock node follows MIDI.
fn follows_midi(graph: &SynthGraph, node_id: NodeId) -> bool {
    selected(graph, node_id, "Source") == Some(1)
}

/// The latest readout from a node's module.
fn readout(user_state: &SynthGraphState, node_id: NodeId) -> Option<&Readout> {
    user_state.get_engine_node_id(node_id).and_then(|id| user_state.readouts.get(&id))
}

/// A knob another control has taken over.
pub struct KnobTakeover {
    /// Where the knob points, or `None` to leave it at its own value.
    pub value: Option<f32>,
    /// What it reads instead of its own value.
    pub text: String,
}

/// The takeover of a node's knob for `param`, if something else is in charge
/// of it right now.
pub fn knob_takeover(
    module_id: &str,
    param: &str,
    node_id: NodeId,
    graph: &SynthGraph,
    user_state: &SynthGraphState,
) -> Option<KnobTakeover> {
    let readout = readout(user_state, node_id);
    match (module_id, param) {
        ("mod.lfo", "Rate") => {
            let &(label, beats) = SYNC_DIVISIONS.get(selected(graph, node_id, "Tempo Sync")?)?;
            beats?;
            Some(KnobTakeover {
                value: readout.map(|r| r.values[Lfo::READOUT_RATE]).filter(|&hz| hz > 0.0),
                text: label.to_string(),
            })
        }
        ("util.clock", "Tempo") if follows_midi(graph, node_id) => {
            let receiving = readout.is_some_and(|r| r.values[Clock::READOUT_RECEIVING] > 0.5);
            let tempo = readout.map(|r| r.values[Clock::READOUT_TEMPO]);
            Some(KnobTakeover {
                value: tempo.filter(|_| receiving),
                text: match tempo {
                    Some(bpm) if receiving => format!("{bpm:.1} BPM"),
                    _ => "No clock".to_string(),
                },
            })
        }
        _ => None,
    }
}

/// Draws the Clock's beat lamps and source badge.
pub fn clock_display(ui: &mut egui::Ui, node_id: NodeId, graph: &SynthGraph, user_state: &SynthGraphState, zoom: f32) {
    let z = zoom;
    let midi = follows_midi(graph, node_id);
    let readout = readout(user_state, node_id).copied().unwrap_or_default();
    let beat = readout.values[Clock::READOUT_BEAT].max(0.0);
    let running = readout.values[Clock::READOUT_RUNNING] > 0.5 && user_state.is_playing;
    let receiving = readout.values[Clock::READOUT_RECEIVING] > 0.5 && user_state.is_playing;

    // Separator, as the other displays have, as wide as the display: the
    // node is no wider than its rows, which may be narrower than the space
    // on offer
    let width = 132.0 * z;
    ui.add_space(4.0 * z);
    let accent = crate::dsp::ModuleCategory::Utility.color();
    let left = ui.cursor().left();
    ui.painter().hline(
        left..=(left + width),
        ui.cursor().top(),
        Stroke::new(1.0 * z, Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 64)),
    );
    ui.add_space(6.0 * z);

    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 22.0 * z), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter_at(rect.expand(2.0 * z));

    // --- The source badge ---
    let badge = Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 3.0 * z), Vec2::new(34.0 * z, 16.0 * z));
    let (label, ink, fill, stroke) = if midi {
        let purple = theme::signal::MIDI;
        if receiving {
            ("MIDI", Color32::WHITE, purple.gamma_multiply(0.85), purple)
        } else {
            ("MIDI", purple, Color32::TRANSPARENT, purple.gamma_multiply(0.6))
        }
    } else {
        ("INT", theme::text::SECONDARY, theme::background::WIDGET, theme::background::GRID_MAJOR)
    };
    painter.rect(badge, 8.0 * z, fill, Stroke::new(1.0 * z, stroke));
    painter.text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::new(9.0 * z, egui::FontFamily::Name(theme::TITLE_FAMILY.into())),
        ink,
    );

    // --- The bar: four lamps and the sweep under them ---
    let lamps = Rect::from_min_max(Pos2::new(badge.right() + 8.0 * z, rect.top() + 2.0 * z), Pos2::new(rect.right(), rect.bottom() - 6.0 * z));
    let gap = 4.0 * z;
    let lamp_width = (lamps.width() - gap * (BEATS_PER_BAR - 1) as f32) / BEATS_PER_BAR as f32;
    let current = (beat as usize).min(BEATS_PER_BAR - 1);
    let through = beat.fract();
    let green = theme::signal::GATE;
    for n in 0..BEATS_PER_BAR {
        let lamp = Rect::from_min_size(
            Pos2::new(lamps.left() + n as f32 * (lamp_width + gap), lamps.top()),
            Vec2::new(lamp_width, lamps.height()),
        );
        painter.rect_filled(lamp, 3.0 * z, theme::background::MAIN);
        if n == current && running {
            // Flash on the beat, fade through it; the downbeat flashes hardest
            let peak = if n == 0 { 1.0 } else { 0.75 };
            let glow = peak * (1.0 - 0.75 * through);
            painter.rect_filled(lamp, 3.0 * z, green.gamma_multiply(glow));
            if n == 0 {
                painter.rect_stroke(lamp.expand(1.0 * z), 4.0 * z, Stroke::new(1.0 * z, green.gamma_multiply(0.5 * glow)));
            }
        } else if n == current && readout != Readout::default() {
            // Stopped: where it'll carry on from, faintly
            painter.rect_stroke(lamp, 3.0 * z, Stroke::new(1.0 * z, green.gamma_multiply(0.35)));
        } else {
            let rim = if n == 0 { theme::background::GRID_MAJOR.gamma_multiply(1.6) } else { theme::background::GRID_MAJOR };
            painter.rect_stroke(lamp, 3.0 * z, Stroke::new(1.0 * z, rim));
        }
    }
    let sweep_y = rect.bottom() - 2.0 * z;
    painter.hline(lamps.x_range(), sweep_y, Stroke::new(1.0 * z, theme::background::GRID));
    if running || readout != Readout::default() {
        let x = lamps.left() + lamps.width() * (beat / BEATS_PER_BAR as f32).min(1.0);
        let ink = if running { green } else { green.gamma_multiply(0.35) };
        painter.hline(lamps.left()..=x, sweep_y, Stroke::new(1.5 * z, ink));
    }

    let state = match (midi, running, receiving) {
        (true, true, _) => "Following MIDI clock".to_string(),
        (true, false, true) => "The MIDI clock master is stopped".to_string(),
        (true, false, false) => "Waiting for MIDI clock on the MIDI input".to_string(),
        (false, true, _) => "Keeping its own time at Tempo".to_string(),
        (false, false, _) => "Stopped".to_string(),
    };
    response.on_hover_text(format!("{state}\nBeat {} of the bar", current + 1));
}
