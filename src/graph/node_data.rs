//! Node data for the synthesizer graph.
//!
//! Defines the per-node data stored in the graph editor.
//!
//! # Custom Rendering
//!
//! This module implements custom node rendering to match the concept image aesthetic:
//! - Colored header bars based on module category
//! - Module icons in the header
//! - Horizontal knob row at the bottom for controllable parameters
//! - Category labels in the footer

use eframe::egui::{self, Color32, RichText};
use egui_node_graph2::{NodeDataTrait, NodeResponse, UserResponseTrait};

use crate::dsp::ModuleCategory;
use crate::engine::midi_engine::MidiEvent;
use crate::widgets::{knob, led, waveform_display, generate_waveform_cycle, KnobConfig, LedConfig, ParamFormat, WaveformConfig, WaveformType, adsr_display, AdsrConfig, AdsrParams, spectrum_display, SpectrumConfig, SpectrumStyle, generate_filter_response, FilterResponseType};
use super::{SynthResponse, SynthValueType};

/// MIDI event colors for the MIDI Monitor display.
mod midi_colors {
    use super::Color32;

    /// Note On/Off events (green)
    pub const NOTE: Color32 = Color32::from_rgb(100, 200, 100);
    /// Control Change events (orange)
    pub const CC: Color32 = Color32::from_rgb(255, 165, 0);
    /// Pitch Bend events (purple)
    pub const PITCH_BEND: Color32 = Color32::from_rgb(180, 100, 200);
    /// Other events (gray)
    pub const OTHER: Color32 = Color32::from_rgb(150, 150, 150);
}

/// Convert a MIDI note number to a note name (e.g., 60 -> "C4").
fn note_to_name(note: u8) -> String {
    const NOTES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = (note / 12) as i32 - 1; // MIDI note 60 = C4
    let name = NOTES[(note % 12) as usize];
    format!("{}{}", name, octave)
}

/// Format a MIDI event for display.
fn format_midi_event(event: &MidiEvent) -> (String, Color32) {
    match event {
        MidiEvent::NoteOn { channel, note, velocity } => (
            format!("NoteOn Ch{} {} vel={}", channel + 1, note_to_name(*note), velocity),
            midi_colors::NOTE,
        ),
        MidiEvent::NoteOff { channel, note, .. } => (
            format!("NoteOff Ch{} {}", channel + 1, note_to_name(*note)),
            midi_colors::NOTE,
        ),
        MidiEvent::ControlChange { channel, controller, value } => (
            format!("CC Ch{} #{} val={}", channel + 1, controller, value),
            midi_colors::CC,
        ),
        MidiEvent::PitchBend { channel, value } => (
            format!("PitchBend Ch{} {}", channel + 1, value),
            midi_colors::PITCH_BEND,
        ),
        MidiEvent::ChannelPressure { channel, pressure } => (
            format!("Pressure Ch{} {}", channel + 1, pressure),
            midi_colors::OTHER,
        ),
        MidiEvent::PolyPressure { channel, note, pressure } => (
            format!("PolyPres Ch{} {} {}", channel + 1, note_to_name(*note), pressure),
            midi_colors::OTHER,
        ),
        MidiEvent::ProgramChange { channel, program } => (
            format!("Program Ch{} #{}", channel + 1, program),
            midi_colors::OTHER,
        ),
    }
}

/// Describes a knob parameter that appears in the bottom section of a node.
///
/// Knob parameters provide manual control over module values. They can optionally
/// be "exposed" as input ports, allowing external signals to modulate or replace
/// the knob value.
#[derive(Clone, Debug)]
pub struct KnobParam {
    /// Name of the corresponding input parameter in the graph (must match exactly).
    pub param_name: String,
    /// Short label displayed below the knob (e.g., "Freq", "FM Dpth").
    pub label: String,
    /// Whether this parameter is also exposed as an input port.
    /// When connected, the input value replaces/modulates the knob value.
    pub exposed_as_input: bool,
}

impl KnobParam {
    /// Create a new knob parameter.
    pub fn new(param_name: impl Into<String>, label: impl Into<String>, exposed_as_input: bool) -> Self {
        Self {
            param_name: param_name.into(),
            label: label.into(),
            exposed_as_input,
        }
    }

    /// Create a knob-only parameter (no corresponding input port).
    pub fn knob_only(param_name: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(param_name, label, false)
    }

    /// Create an exposed parameter (has both knob and input port).
    pub fn exposed(param_name: impl Into<String>, label: impl Into<String>) -> Self {
        Self::new(param_name, label, true)
    }
}

/// Describes an LED indicator that appears in the node's bottom section.
///
/// LED indicators show the state of output ports (e.g., gate triggers, activity).
#[derive(Clone, Debug)]
pub struct LedIndicator {
    /// The output port index to monitor (0-based index among output ports).
    pub output_index: usize,
    /// Short label displayed below the LED.
    pub label: String,
    /// LED configuration (color, size, etc.).
    pub config: LedConfig,
}

impl LedIndicator {
    /// Create a new LED indicator for an output port.
    pub fn new(output_index: usize, label: impl Into<String>, config: LedConfig) -> Self {
        Self {
            output_index,
            label: label.into(),
            config,
        }
    }

    /// Create a green gate indicator (for trigger/gate outputs).
    pub fn gate(output_index: usize, label: impl Into<String>) -> Self {
        Self::new(output_index, label, LedConfig::green().with_size(10.0))
    }

    /// Create an orange activity indicator (for control signals).
    pub fn activity(output_index: usize, label: impl Into<String>) -> Self {
        Self::new(output_index, label, LedConfig::orange().with_size(10.0))
    }
}

/// Data stored per node in the graph.
///
/// This contains information about which module type this node represents
/// and any per-instance display settings.
#[derive(Clone, Debug)]
pub struct SynthNodeData {
    /// The module type identifier (e.g., "osc.sine", "output.audio").
    pub module_id: &'static str,
    /// Display name shown in the node header.
    pub display_name: String,
    /// The category of this module (for header coloring).
    pub category: ModuleCategory,
    /// Knob parameters to display in the bottom section of the node.
    pub knob_params: Vec<KnobParam>,
    /// LED indicators to display in the bottom section of the node.
    pub led_indicators: Vec<LedIndicator>,
    /// Output ports to monitor for feedback without LED indicators.
    /// Used for waveform displays, phase indicators, etc.
    pub monitored_outputs: Vec<usize>,
}

/// Configuration for MIDI mapping display on a knob.
///
/// This struct is used to pass MIDI mapping state to the knob rendering function.
#[derive(Default)]
struct KnobMidiConfig {
    /// Whether this knob has a MIDI CC mapping.
    has_midi_mapping: bool,
    /// The CC number if mapped.
    cc_number: Option<u8>,
    /// Whether this knob is the current MIDI Learn target.
    is_learn_target: bool,
    /// Parameter min value (for MIDI Learn).
    min_value: f32,
    /// Parameter max value (for MIDI Learn).
    max_value: f32,
}

impl SynthNodeData {
    /// Create new node data for a module.
    pub fn new(module_id: &'static str, display_name: impl Into<String>, category: ModuleCategory) -> Self {
        Self {
            module_id,
            display_name: display_name.into(),
            category,
            knob_params: Vec::new(),
            led_indicators: Vec::new(),
            monitored_outputs: Vec::new(),
        }
    }

    /// Builder method to add knob parameters.
    pub fn with_knob_params(mut self, knob_params: Vec<KnobParam>) -> Self {
        self.knob_params = knob_params;
        self
    }

    /// Builder method to add LED indicators.
    pub fn with_led_indicators(mut self, led_indicators: Vec<LedIndicator>) -> Self {
        self.led_indicators = led_indicators;
        self
    }

    /// Builder method to add outputs that should be monitored for UI feedback.
    pub fn with_monitored_outputs(mut self, outputs: Vec<usize>) -> Self {
        self.monitored_outputs = outputs;
        self
    }

    /// Get the header color for this node based on its category.
    pub fn header_color(&self) -> Color32 {
        self.category.color()
    }

    /// Draw the category icon at the given position.
    /// Uses vector shapes for cross-platform reliability.
    fn draw_category_icon(&self, painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
        let s = size * 0.5; // Half-size for calculations
        match self.category {
            ModuleCategory::Source => {
                // Sine wave icon
                let points: Vec<egui::Pos2> = (0..=12)
                    .map(|i| {
                        let t = i as f32 / 12.0;
                        let x = center.x - s + t * s * 2.0;
                        let y = center.y - (t * std::f32::consts::TAU).sin() * s * 0.6;
                        egui::pos2(x, y)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
            }
            ModuleCategory::Filter => {
                // Triangle/slope icon (low-pass filter shape)
                let points = vec![
                    egui::pos2(center.x - s, center.y - s * 0.5),
                    egui::pos2(center.x, center.y - s * 0.5),
                    egui::pos2(center.x + s * 0.3, center.y + s * 0.5),
                    egui::pos2(center.x + s, center.y + s * 0.5),
                ];
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
            }
            ModuleCategory::Modulation => {
                // Diamond icon
                let points = vec![
                    egui::pos2(center.x, center.y - s * 0.7),
                    egui::pos2(center.x + s * 0.5, center.y),
                    egui::pos2(center.x, center.y + s * 0.7),
                    egui::pos2(center.x - s * 0.5, center.y),
                    egui::pos2(center.x, center.y - s * 0.7),
                ];
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
            }
            ModuleCategory::Effect => {
                // Star/sparkle icon
                for i in 0..4 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                    let len = if i % 2 == 0 { s * 0.7 } else { s * 0.4 };
                    let dx = angle.cos() * len;
                    let dy = angle.sin() * len;
                    painter.line_segment(
                        [egui::pos2(center.x - dx, center.y - dy), egui::pos2(center.x + dx, center.y + dy)],
                        egui::Stroke::new(1.5, color),
                    );
                }
            }
            ModuleCategory::Utility => {
                // Hash/grid icon
                let d = s * 0.4;
                painter.line_segment([egui::pos2(center.x - d, center.y - s * 0.6), egui::pos2(center.x - d, center.y + s * 0.6)], egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(center.x + d, center.y - s * 0.6), egui::pos2(center.x + d, center.y + s * 0.6)], egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(center.x - s * 0.6, center.y - d), egui::pos2(center.x + s * 0.6, center.y - d)], egui::Stroke::new(1.5, color));
                painter.line_segment([egui::pos2(center.x - s * 0.6, center.y + d), egui::pos2(center.x + s * 0.6, center.y + d)], egui::Stroke::new(1.5, color));
            }
            ModuleCategory::Output => {
                // Speaker cone icon
                painter.rect_stroke(
                    egui::Rect::from_center_size(egui::pos2(center.x - s * 0.3, center.y), egui::vec2(s * 0.4, s * 0.6)),
                    0.0,
                    egui::Stroke::new(1.5, color),
                );
                let cone = vec![
                    egui::pos2(center.x - s * 0.1, center.y - s * 0.3),
                    egui::pos2(center.x + s * 0.6, center.y - s * 0.6),
                    egui::pos2(center.x + s * 0.6, center.y + s * 0.6),
                    egui::pos2(center.x - s * 0.1, center.y + s * 0.3),
                ];
                painter.add(egui::Shape::line(cone, egui::Stroke::new(1.5, color)));
            }
        }
    }

    /// Draw a secondary/smaller icon at the given position.
    fn draw_secondary_icon(&self, painter: &egui::Painter, center: egui::Pos2, size: f32, color: Color32) {
        let s = size * 0.4; // Smaller than category icon
        match self.category {
            ModuleCategory::Source => {
                // Small wave
                let points: Vec<egui::Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0;
                        let x = center.x - s + t * s * 2.0;
                        let y = center.y - (t * std::f32::consts::TAU).sin() * s * 0.5;
                        egui::pos2(x, y)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.2, color)));
            }
            ModuleCategory::Filter => {
                // Curved response line
                let points: Vec<egui::Pos2> = (0..=8)
                    .map(|i| {
                        let t = i as f32 / 8.0;
                        let x = center.x - s + t * s * 2.0;
                        let curve = 1.0 - (t * 2.0).min(1.0).powi(2);
                        let y = center.y + s * 0.5 - curve * s;
                        egui::pos2(x, y)
                    })
                    .collect();
                painter.add(egui::Shape::line(points, egui::Stroke::new(1.2, color)));
            }
            ModuleCategory::Modulation => {
                // Up-down arrows
                painter.line_segment([egui::pos2(center.x, center.y - s * 0.8), egui::pos2(center.x, center.y + s * 0.8)], egui::Stroke::new(1.2, color));
                // Up arrow head
                painter.line_segment([egui::pos2(center.x - s * 0.3, center.y - s * 0.4), egui::pos2(center.x, center.y - s * 0.8)], egui::Stroke::new(1.2, color));
                painter.line_segment([egui::pos2(center.x + s * 0.3, center.y - s * 0.4), egui::pos2(center.x, center.y - s * 0.8)], egui::Stroke::new(1.2, color));
                // Down arrow head
                painter.line_segment([egui::pos2(center.x - s * 0.3, center.y + s * 0.4), egui::pos2(center.x, center.y + s * 0.8)], egui::Stroke::new(1.2, color));
                painter.line_segment([egui::pos2(center.x + s * 0.3, center.y + s * 0.4), egui::pos2(center.x, center.y + s * 0.8)], egui::Stroke::new(1.2, color));
            }
            ModuleCategory::Effect | ModuleCategory::Output => {
                // Small filled circle
                painter.circle_stroke(center, s * 0.5, egui::Stroke::new(1.2, color));
                painter.circle_filled(center, s * 0.2, color);
            }
            ModuleCategory::Utility => {
                // Small gear (simplified)
                painter.circle_stroke(center, s * 0.4, egui::Stroke::new(1.2, color));
                for i in 0..6 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_3;
                    let inner = s * 0.4;
                    let outer = s * 0.7;
                    painter.line_segment(
                        [
                            egui::pos2(center.x + angle.cos() * inner, center.y + angle.sin() * inner),
                            egui::pos2(center.x + angle.cos() * outer, center.y + angle.sin() * outer),
                        ],
                        egui::Stroke::new(1.2, color),
                    );
                }
            }
        }
    }

    /// Render an interactive knob widget for a parameter value.
    ///
    /// When the knob is changed, emits a ParameterChanged response.
    /// If `is_connected` is true, the knob is dimmed and changes are ignored
    /// (the value is controlled externally).
    /// If `signal_value` is Some, the knob displays that value instead of the stored value.
    #[allow(clippy::too_many_arguments)]
    fn render_knob_for_value(
        ui: &mut egui::Ui,
        value: &SynthValueType,
        label: &str,
        size: f32,
        is_connected: bool,
        node_id: egui_node_graph2::NodeId,
        param_name: &str,
        responses: &mut Vec<NodeResponse<SynthResponse, Self>>,
        signal_value: Option<f32>,
        _midi_config: &KnobMidiConfig,
    ) {
        // Dim the knob if it's connected (externally controlled)
        let alpha = if is_connected { 0.5 } else { 1.0 };

        match value {
            SynthValueType::Scalar { value: val, .. } => {
                let config = KnobConfig {
                    size,
                    range: 0.0..=1.0,
                    default: 0.5,
                    format: ParamFormat::Percent,
                    logarithmic: false,
                    label: Some(label.to_string()),
                    show_value: true,
                    ..Default::default()
                };
                let original_val = *val;
                // Use signal value if connected and available, otherwise use stored value
                let mut display_val = signal_value
                    .map(|sv| sv.clamp(0.0, 1.0))
                    .unwrap_or(original_val);
                ui.scope(|ui| {
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color =
                        ui.style().visuals.widgets.inactive.fg_stroke.color.gamma_multiply(alpha);
                    if is_connected {
                        ui.disable();
                    }
                    knob(ui, &mut display_val, &config);
                });
                // Emit response if value changed and not connected
                if !is_connected && (display_val - original_val).abs() > f32::EPSILON {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: display_val,
                    }));
                }
            }
            SynthValueType::Frequency { value: val, min, max, .. } => {
                let config = KnobConfig::frequency(*min, *max, 440.0)
                    .with_label(label)
                    .with_size(size);
                let original_val = *val;
                // For frequency, the signal value represents the actual Hz value
                let mut display_val = signal_value
                    .map(|sv| sv.clamp(*min, *max))
                    .unwrap_or(original_val);
                ui.scope(|ui| {
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color =
                        ui.style().visuals.widgets.inactive.fg_stroke.color.gamma_multiply(alpha);
                    if is_connected {
                        ui.disable();
                    }
                    knob(ui, &mut display_val, &config);
                });
                if !is_connected && (display_val - original_val).abs() > 0.01 {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: display_val,
                    }));
                }
            }
            SynthValueType::LinearHz { value: val, min, max, .. } => {
                let config = KnobConfig {
                    size,
                    range: *min..=*max,
                    default: *min,
                    format: ParamFormat::Frequency,
                    logarithmic: false,
                    label: Some(label.to_string()),
                    show_value: true,
                    ..Default::default()
                };
                let original_val = *val;
                let mut display_val = signal_value
                    .map(|sv| sv.clamp(*min, *max))
                    .unwrap_or(original_val);
                ui.scope(|ui| {
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color =
                        ui.style().visuals.widgets.inactive.fg_stroke.color.gamma_multiply(alpha);
                    if is_connected {
                        ui.disable();
                    }
                    knob(ui, &mut display_val, &config);
                });
                if !is_connected && (display_val - original_val).abs() > 0.01 {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: display_val,
                    }));
                }
            }
            SynthValueType::Time { value: val, min, max, .. } => {
                let config = KnobConfig::time(*min, *max, (*min + *max) / 2.0)
                    .with_label(label)
                    .with_size(size);
                let original_val = *val;
                let mut display_val = signal_value
                    .map(|sv| sv.clamp(*min, *max))
                    .unwrap_or(original_val);
                ui.scope(|ui| {
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color =
                        ui.style().visuals.widgets.inactive.fg_stroke.color.gamma_multiply(alpha);
                    if is_connected {
                        ui.disable();
                    }
                    knob(ui, &mut display_val, &config);
                });
                if !is_connected && (display_val - original_val).abs() > 0.0001 {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: display_val,
                    }));
                }
            }
            SynthValueType::LinearRange { value: val, min, max, unit, .. } => {
                // Map common unit strings to static format specifiers
                let format = match unit.as_str() {
                    "BPM" => ParamFormat::RawWithUnit { decimals: 0, unit: "BPM" },
                    "%" => ParamFormat::RawWithUnit { decimals: 0, unit: "%" },
                    "dB" => ParamFormat::Decibels,
                    "st" => ParamFormat::Semitones,
                    _ => ParamFormat::Raw { decimals: 1 },
                };
                let config = KnobConfig {
                    size,
                    range: *min..=*max,
                    default: (*min + *max) / 2.0,
                    format,
                    logarithmic: false,
                    label: Some(label.to_string()),
                    show_value: true,
                    ..Default::default()
                };
                let original_val = *val;
                let mut display_val = signal_value
                    .map(|sv| sv.clamp(*min, *max))
                    .unwrap_or(original_val);
                ui.scope(|ui| {
                    ui.style_mut().visuals.widgets.inactive.fg_stroke.color =
                        ui.style().visuals.widgets.inactive.fg_stroke.color.gamma_multiply(alpha);
                    if is_connected {
                        ui.disable();
                    }
                    knob(ui, &mut display_val, &config);
                });
                if !is_connected && (display_val - original_val).abs() > 0.01 {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: display_val,
                    }));
                }
            }
            SynthValueType::Toggle { value: val, .. } => {
                let original_val = *val;
                let mut display_val = original_val;
                ui.vertical(|ui| {
                    if is_connected {
                        ui.disable();
                    }
                    ui.checkbox(&mut display_val, "");
                    ui.label(RichText::new(label).small());
                });
                if !is_connected && display_val != original_val {
                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                        node_id,
                        param_name: param_name.to_string(),
                        value: if display_val { 1.0 } else { 0.0 },
                    }));
                }
            }
            SynthValueType::Select { value: val, options, .. } => {
                // Display-only for now - selections are better handled inline
                ui.vertical(|ui| {
                    ui.label(RichText::new(options.get(*val).map(|s| s.as_str()).unwrap_or("?")).small());
                    ui.label(RichText::new(label).small().weak());
                });
            }
        }
    }
}

impl NodeDataTrait for SynthNodeData {
    type Response = SynthResponse;
    type UserState = super::SynthGraphState;
    type DataType = super::SynthDataType;
    type ValueType = super::SynthValueType;

    fn top_bar_ui(
        &self,
        ui: &mut egui::Ui,
        _node_id: egui_node_graph2::NodeId,
        _graph: &egui_node_graph2::Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<Self::Response, Self>>
    where
        Self::Response: UserResponseTrait,
    {
        // Draw icons directly with painter to avoid intercepting mouse events
        // This allows the entire title bar to be draggable
        let rect = ui.available_rect_before_wrap();
        let painter = ui.painter();
        // Use white for icons to ensure visibility against colored title bars
        let icon_color = Color32::WHITE;

        // Left icon (category icon) - vector drawn for cross-platform reliability
        let icon_size = 14.0;
        let left_center = egui::pos2(rect.left() + icon_size * 0.6, rect.center().y);
        self.draw_category_icon(painter, left_center, icon_size, icon_color);

        // Right icon (secondary icon) - smaller
        let secondary_size = 12.0;
        let right_center = egui::pos2(rect.right() - secondary_size * 0.6, rect.center().y);
        self.draw_secondary_icon(painter, right_center, secondary_size, icon_color);

        // Allocate the space without creating any interactive widgets
        ui.allocate_space(ui.available_size());

        Vec::new()
    }

    fn bottom_ui(
        &self,
        ui: &mut egui::Ui,
        node_id: egui_node_graph2::NodeId,
        graph: &egui_node_graph2::Graph<Self, Self::DataType, Self::ValueType>,
        user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<Self::Response, Self>>
    where
        Self::Response: UserResponseTrait,
    {
        let mut responses = Vec::new();

        // Get the engine node ID for looking up input values
        let engine_node_id = user_state.get_engine_node_id(node_id);

        // Special rendering for MIDI Monitor module
        if self.module_id == "util.midi_monitor" {
            // Get filter settings from the node's input parameters
            let (channel_filter, show_notes, show_cc, show_pitch_bend) = if let Some(node) = graph.nodes.get(node_id) {
                let mut channel = 0usize; // 0 = all channels
                let mut notes = true;
                let mut cc = true;
                let mut pb = true;

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);
                    match name.as_str() {
                        "Channel" => {
                            if let SynthValueType::Select { value, .. } = &input.value {
                                channel = *value;
                            }
                        }
                        "Notes" => {
                            if let SynthValueType::Toggle { value, .. } = &input.value {
                                notes = *value;
                            }
                        }
                        "CC" => {
                            if let SynthValueType::Toggle { value, .. } = &input.value {
                                cc = *value;
                            }
                        }
                        "Pitch Bend" => {
                            if let SynthValueType::Toggle { value, .. } = &input.value {
                                pb = *value;
                            }
                        }
                        _ => {}
                    }
                }
                (channel, notes, cc, pb)
            } else {
                (0, true, true, true)
            };

            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Render MIDI event log
            let midi_events = user_state.midi_events();

            if midi_events.is_empty() {
                ui.label(RichText::new("No MIDI events").small().weak().italics());
            } else {
                // Display events (most recent first for better visibility)
                ui.vertical(|ui| {
                    ui.set_min_width(180.0);
                    ui.set_max_height(120.0);

                    for event in midi_events.iter().rev().take(8) {
                        // Apply channel filter (0 = all, 1-16 = specific channel)
                        if channel_filter > 0 {
                            let event_channel = event.event.channel() as usize + 1;
                            if event_channel != channel_filter {
                                continue;
                            }
                        }

                        // Apply event type filters
                        let should_show = match &event.event {
                            MidiEvent::NoteOn { .. } | MidiEvent::NoteOff { .. } => show_notes,
                            MidiEvent::ControlChange { .. } => show_cc,
                            MidiEvent::PitchBend { .. } => show_pitch_bend,
                            _ => true, // Always show other events
                        };

                        if !should_show {
                            continue;
                        }

                        // Format and display the event
                        let (text, color) = format_midi_event(&event.event);
                        let timestamp = format!("{:.1}s", event.timestamp);

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&timestamp).small().weak().monospace());
                            ui.label(RichText::new(&text).small().color(color).monospace());
                        });
                    }
                });
            }
        }

        // Special rendering for Oscilloscope module
        if self.module_id == "util.oscilloscope" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get scope data from user state
            let scope_data = engine_node_id
                .and_then(|eid| user_state.get_scope_data(eid));

            // Get trigger level from the node's input parameters
            let trigger_level = if let Some(node) = graph.nodes.get(node_id) {
                let mut level = 0.0f32;
                for (name, input_id) in &node.inputs {
                    if name == "Trigger Level" {
                        let input = graph.get_input(*input_id);
                        if let SynthValueType::LinearRange { value, .. } = &input.value {
                            level = *value;
                        }
                    }
                }
                level
            } else {
                0.0
            };

            // Render the oscilloscope display
            let (channel1, channel2) = if let Some(data) = scope_data {
                (data.channel1.as_slice(), data.channel2.as_slice())
            } else {
                (&[] as &[f32], &[] as &[f32])
            };

            let config = crate::widgets::OscilloscopeConfig::new(200.0, 120.0)
                .with_trigger_level(trigger_level)
                .with_trigger_indicator(true);

            crate::widgets::oscilloscope_display(ui, channel1, channel2, &config);
        }

        // Special rendering for Step Sequencer module
        if self.module_id == "seq.step" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get step data from the node's input parameters
            let (num_steps, current_step_output, step_data) = if let Some(node) = graph.nodes.get(node_id) {
                let mut steps = 8usize;
                let mut pitches = [60u8; 16];
                let mut gates = [true; 16];

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);

                    if name == "Steps" {
                        if let SynthValueType::LinearRange { value, .. } = &input.value {
                            steps = (*value as usize).clamp(1, 16);
                        }
                    }

                    // Parse step parameters
                    for step in 1..=16 {
                        if *name == format!("Step {} Pitch", step) {
                            if let SynthValueType::LinearRange { value, .. } = &input.value {
                                pitches[step - 1] = *value as u8;
                            }
                        }
                        if *name == format!("Step {} Gate", step) {
                            if let SynthValueType::Toggle { value, .. } = &input.value {
                                gates[step - 1] = *value;
                            }
                        }
                    }
                }

                // Get current step from output (Step port is output index 3)
                let current = engine_node_id
                    .and_then(|eid| user_state.get_output_value(eid, 3))
                    .map(|v| ((v * (steps - 1).max(1) as f32).round() as usize).min(steps - 1))
                    .unwrap_or(0);

                (steps, current, (pitches, gates))
            } else {
                (8, 0, ([60u8; 16], [true; 16]))
            };

            let (pitches, gates) = step_data;

            // Render step grid (two rows of 8)
            ui.vertical(|ui| {
                ui.set_min_width(220.0);

                // Step size
                let step_size = 24.0;
                let step_spacing = 3.0;

                // Row 1: Steps 1-8
                ui.horizontal(|ui| {
                    for step in 0..8.min(num_steps) {
                        let is_current = step == current_step_output;
                        let has_gate = gates[step];
                        let pitch = pitches[step];

                        // Step button appearance
                        let base_color = if has_gate {
                            Color32::from_rgb(100, 200, 100) // Green for gate on
                        } else {
                            Color32::from_rgb(60, 60, 70) // Dark for gate off
                        };

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

                        // Draw step button
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(step_size, step_size + 12.0),
                            egui::Sense::click(),
                        );

                        let step_rect = egui::Rect::from_min_size(
                            rect.min,
                            egui::vec2(step_size, step_size),
                        );

                        // Background
                        ui.painter().rect_filled(step_rect, 3.0, color);

                        // Current step indicator (border)
                        if is_current {
                            ui.painter().rect_stroke(
                                step_rect,
                                3.0,
                                egui::Stroke::new(2.0, Color32::WHITE),
                            );
                        }

                        // Note name below
                        let note_name = crate::modules::sequencer::note_to_name(pitch);
                        let text_pos = egui::pos2(
                            rect.center().x,
                            step_rect.bottom() + 2.0,
                        );
                        ui.painter().text(
                            text_pos,
                            egui::Align2::CENTER_TOP,
                            &note_name,
                            egui::FontId::proportional(8.0),
                            Color32::from_gray(180),
                        );

                        // Handle click to toggle gate
                        if response.clicked() {
                            let param_name = format!("Step {} Gate", step + 1);
                            let new_value = if gates[step] { 0.0 } else { 1.0 };
                            responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                node_id,
                                param_name,
                                value: new_value,
                            }));
                        }

                        // Handle right-click to edit pitch
                        response.context_menu(|ui| {
                            ui.label(RichText::new(format!("Step {}", step + 1)).strong());
                            ui.separator();

                            // Pitch adjustment
                            let current_pitch = pitches[step] as i32;
                            if ui.button("Pitch +12 (Octave Up)").clicked() {
                                let new_pitch = (current_pitch + 12).min(127) as f32;
                                responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                    node_id,
                                    param_name: format!("Step {} Pitch", step + 1),
                                    value: new_pitch,
                                }));
                                ui.close_menu();
                            }
                            if ui.button("Pitch +1 (Semitone Up)").clicked() {
                                let new_pitch = (current_pitch + 1).min(127) as f32;
                                responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                    node_id,
                                    param_name: format!("Step {} Pitch", step + 1),
                                    value: new_pitch,
                                }));
                                ui.close_menu();
                            }
                            if ui.button("Pitch -1 (Semitone Down)").clicked() {
                                let new_pitch = (current_pitch - 1).max(0) as f32;
                                responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                    node_id,
                                    param_name: format!("Step {} Pitch", step + 1),
                                    value: new_pitch,
                                }));
                                ui.close_menu();
                            }
                            if ui.button("Pitch -12 (Octave Down)").clicked() {
                                let new_pitch = (current_pitch - 12).max(0) as f32;
                                responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                    node_id,
                                    param_name: format!("Step {} Pitch", step + 1),
                                    value: new_pitch,
                                }));
                                ui.close_menu();
                            }
                        });

                        ui.add_space(step_spacing);
                    }
                });

                // Row 2: Steps 9-16 (if num_steps > 8)
                if num_steps > 8 {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        for step in 8..16.min(num_steps) {
                            let is_current = step == current_step_output;
                            let has_gate = gates[step];
                            let pitch = pitches[step];

                            let base_color = if has_gate {
                                Color32::from_rgb(100, 200, 100)
                            } else {
                                Color32::from_rgb(60, 60, 70)
                            };

                            let color = if is_current {
                                Color32::from_rgb(
                                    (base_color.r() as u16 + 100).min(255) as u8,
                                    (base_color.g() as u16 + 100).min(255) as u8,
                                    (base_color.b() as u16 + 50).min(255) as u8,
                                )
                            } else {
                                base_color
                            };

                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(step_size, step_size + 12.0),
                                egui::Sense::click(),
                            );

                            let step_rect = egui::Rect::from_min_size(
                                rect.min,
                                egui::vec2(step_size, step_size),
                            );

                            ui.painter().rect_filled(step_rect, 3.0, color);

                            if is_current {
                                ui.painter().rect_stroke(
                                    step_rect,
                                    3.0,
                                    egui::Stroke::new(2.0, Color32::WHITE),
                                );
                            }

                            let note_name = crate::modules::sequencer::note_to_name(pitch);
                            let text_pos = egui::pos2(
                                rect.center().x,
                                step_rect.bottom() + 2.0,
                            );
                            ui.painter().text(
                                text_pos,
                                egui::Align2::CENTER_TOP,
                                &note_name,
                                egui::FontId::proportional(8.0),
                                Color32::from_gray(180),
                            );

                            if response.clicked() {
                                let param_name = format!("Step {} Gate", step + 1);
                                let new_value = if gates[step] { 0.0 } else { 1.0 };
                                responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                    node_id,
                                    param_name,
                                    value: new_value,
                                }));
                            }

                            response.context_menu(|ui| {
                                ui.label(RichText::new(format!("Step {}", step + 1)).strong());
                                ui.separator();

                                let current_pitch = pitches[step] as i32;
                                if ui.button("Pitch +12 (Octave Up)").clicked() {
                                    let new_pitch = (current_pitch + 12).min(127) as f32;
                                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                        node_id,
                                        param_name: format!("Step {} Pitch", step + 1),
                                        value: new_pitch,
                                    }));
                                    ui.close_menu();
                                }
                                if ui.button("Pitch +1 (Semitone Up)").clicked() {
                                    let new_pitch = (current_pitch + 1).min(127) as f32;
                                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                        node_id,
                                        param_name: format!("Step {} Pitch", step + 1),
                                        value: new_pitch,
                                    }));
                                    ui.close_menu();
                                }
                                if ui.button("Pitch -1 (Semitone Down)").clicked() {
                                    let new_pitch = (current_pitch - 1).max(0) as f32;
                                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                        node_id,
                                        param_name: format!("Step {} Pitch", step + 1),
                                        value: new_pitch,
                                    }));
                                    ui.close_menu();
                                }
                                if ui.button("Pitch -12 (Octave Down)").clicked() {
                                    let new_pitch = (current_pitch - 12).max(0) as f32;
                                    responses.push(NodeResponse::User(SynthResponse::ParameterChanged {
                                        node_id,
                                        param_name: format!("Step {} Pitch", step + 1),
                                        value: new_pitch,
                                    }));
                                    ui.close_menu();
                                }
                            });

                            ui.add_space(step_spacing);
                        }
                    });
                }
            });
        }

        // Special rendering for Oscillator module - waveform preview
        if self.module_id == "osc.sine" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get waveform type and pulse width from node parameters
            let (waveform_idx, pulse_width) = if let Some(node) = graph.nodes.get(node_id) {
                let mut wave_idx = 0usize;
                let mut pw = 0.5f32;

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);
                    match name.as_str() {
                        "Waveform" => {
                            if let SynthValueType::Select { value, .. } = &input.value {
                                wave_idx = *value;
                            }
                        }
                        "Pulse Width" => {
                            if let SynthValueType::LinearRange { value, .. } = &input.value {
                                pw = *value;
                            }
                        }
                        _ => {}
                    }
                }
                (wave_idx, pw)
            } else {
                (0, 0.5)
            };

            // Convert waveform index to WaveformType
            let waveform_type = match waveform_idx {
                0 => WaveformType::Sine,
                1 => WaveformType::Saw,
                2 => WaveformType::Pulse { width: pulse_width }, // Square with PWM
                3 => WaveformType::Triangle,
                _ => WaveformType::Sine,
            };

            // Generate single cycle of the waveform
            let num_samples = 128;
            let samples = generate_waveform_cycle(waveform_type, num_samples);

            // Display waveform with oscillator preset config
            let config = WaveformConfig::oscillator()
                .with_size(140.0, 50.0);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 140.0) / 2.0); // Center the display
                waveform_display(ui, &samples, &config);
            });
        }

        // Special rendering for ADSR Envelope module - envelope shape display
        if self.module_id == "mod.adsr" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get ADSR parameters from node inputs
            let adsr_params = if let Some(node) = graph.nodes.get(node_id) {
                let mut attack = 0.01f32;
                let mut decay = 0.1f32;
                let mut sustain = 0.7f32;
                let mut release = 0.3f32;

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);
                    match name.as_str() {
                        "Attack" => {
                            if let SynthValueType::Time { value, .. } = &input.value {
                                attack = *value;
                            }
                        }
                        "Decay" => {
                            if let SynthValueType::Time { value, .. } = &input.value {
                                decay = *value;
                            }
                        }
                        "Sustain" => {
                            if let SynthValueType::Scalar { value, .. } = &input.value {
                                sustain = *value;
                            }
                        }
                        "Release" => {
                            if let SynthValueType::Time { value, .. } = &input.value {
                                release = *value;
                            }
                        }
                        _ => {}
                    }
                }
                AdsrParams::new(attack, decay, sustain, release)
            } else {
                AdsrParams::default()
            };

            // Display ADSR envelope visualization
            let config = AdsrConfig::default()
                .with_size(140.0, 50.0);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 140.0) / 2.0); // Center the display
                adsr_display(ui, &adsr_params, &config);
            });
        }

        // Special rendering for SVF Filter module - frequency response display
        if self.module_id == "filter.svf" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get cutoff and resonance parameters from node inputs
            let (cutoff_hz, resonance) = if let Some(node) = graph.nodes.get(node_id) {
                let mut cutoff = 1000.0f32;
                let mut res = 0.5f32;

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);
                    match name.as_str() {
                        "Cutoff" => {
                            if let SynthValueType::Frequency { value, .. } = &input.value {
                                cutoff = *value;
                            }
                        }
                        "Resonance" => {
                            if let SynthValueType::Scalar { value, .. } = &input.value {
                                res = *value;
                            }
                        }
                        _ => {}
                    }
                }
                (cutoff, res)
            } else {
                (1000.0, 0.5)
            };

            // Generate filter response curve for lowpass (primary output)
            let response_points = generate_filter_response(
                FilterResponseType::LowPass,
                cutoff_hz,
                resonance,
                128, // More points for smoother curve
            );

            // Display filter response with custom config optimized for seeing resonance
            // Range: -24dB to +12dB shows both rolloff and resonance peak clearly
            let config = SpectrumConfig::default()
                .with_size(140.0, 50.0)
                .with_db_range(-24.0, 12.0)
                .with_style(SpectrumStyle::Line)
                .with_glow(true);

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 140.0) / 2.0); // Center the display
                spectrum_display(ui, &response_points, &config);
            });
        }

        // Special rendering for LFO module - waveform preview with phase marker
        if self.module_id == "mod.lfo" {
            // Add separator
            ui.add_space(4.0);
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Get waveform type and parameters from node inputs
            let (waveform_idx, is_bipolar, rate_hz) = if let Some(node) = graph.nodes.get(node_id) {
                let mut wave_idx = 0usize;
                let mut bipolar = true;
                let mut rate = 1.0f32;

                for (name, input_id) in &node.inputs {
                    let input = graph.get_input(*input_id);
                    match name.as_str() {
                        "Waveform" => {
                            if let SynthValueType::Select { value, .. } = &input.value {
                                wave_idx = *value;
                            }
                        }
                        "Bipolar" => {
                            if let SynthValueType::Toggle { value, .. } = &input.value {
                                bipolar = *value;
                            }
                        }
                        "Rate" => {
                            if let SynthValueType::Frequency { value, .. } = &input.value {
                                rate = *value;
                            }
                        }
                        _ => {}
                    }
                }
                (wave_idx, bipolar, rate)
            } else {
                (0, true, 1.0)
            };

            // Convert waveform index to WaveformType
            // LFO waveforms: 0=Sine, 1=Triangle, 2=Square, 3=Saw
            let waveform_type = match waveform_idx {
                0 => WaveformType::Sine,
                1 => WaveformType::Triangle,
                2 => WaveformType::Square,
                3 => WaveformType::Saw,
                _ => WaveformType::Sine,
            };

            // Generate single cycle of the waveform
            let num_samples = 128;
            let mut samples = generate_waveform_cycle(waveform_type, num_samples);

            // Convert to unipolar if needed (0 to 1 range)
            if !is_bipolar {
                for sample in &mut samples {
                    *sample = (*sample + 1.0) * 0.5;
                }
            }

            // Display waveform with LFO preset config (orange for control signals)
            let config = WaveformConfig::lfo()
                .with_size(140.0, 50.0);

            // Get real phase from audio engine feedback (output port 1)
            // Falls back to UI-time estimation if not yet available
            let phase = engine_node_id
                .and_then(|eid| user_state.get_output_value(eid, 1)) // Phase is output index 1
                .unwrap_or_else(|| {
                    // Fallback: estimate phase from UI time when engine feedback not available
                    let time = ui.ctx().input(|i| i.time);
                    ((time * rate_hz as f64) % 1.0) as f32
                });

            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 140.0) / 2.0); // Center the display

                // Draw the waveform first
                let response = waveform_display(ui, &samples, &config);
                let rect = response.rect;

                if ui.is_rect_visible(rect) {
                    let painter = ui.painter();

                    // Calculate dot position on the waveform
                    let marker_x = rect.left() + phase * rect.width();

                    // Get sample value at current phase by interpolating
                    let sample_index_f = phase * (samples.len() - 1) as f32;
                    let sample_index = sample_index_f as usize;
                    let frac = sample_index_f - sample_index as f32;
                    let sample_value = if sample_index + 1 < samples.len() {
                        samples[sample_index] * (1.0 - frac) + samples[sample_index + 1] * frac
                    } else {
                        samples[sample_index]
                    };

                    // Convert sample value to Y position
                    // Waveform display uses center as 0, with amplitude scaled to half height
                    let amplitude = rect.height() * 0.5 * 0.9; // 0.9 is the default scale in waveform_display
                    let center_y = rect.center().y;
                    let marker_y = center_y - sample_value * amplitude;

                    // Draw glow effect (larger, semi-transparent circle)
                    let glow_color = Color32::from_rgba_unmultiplied(255, 200, 100, 80);
                    painter.circle_filled(
                        egui::Pos2::new(marker_x, marker_y),
                        8.0,
                        glow_color,
                    );

                    // Draw main dot (white with orange tint)
                    let dot_color = Color32::from_rgb(255, 220, 180);
                    painter.circle_filled(
                        egui::Pos2::new(marker_x, marker_y),
                        4.0,
                        dot_color,
                    );

                    // Draw bright center
                    let center_color = Color32::WHITE;
                    painter.circle_filled(
                        egui::Pos2::new(marker_x, marker_y),
                        2.0,
                        center_color,
                    );
                }

                // Request continuous repaint for animation
                ui.ctx().request_repaint();
            });
        }

        // Render horizontal row of knobs if this node has knob parameters
        if !self.knob_params.is_empty() {
            // Add some spacing before the knob row
            ui.add_space(4.0);

            // Draw separator line
            let category_color = self.category.color();
            let separator_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                64,
            );
            ui.painter().hline(
                ui.available_rect_before_wrap().x_range(),
                ui.cursor().top(),
                egui::Stroke::new(1.0, separator_color),
            );
            ui.add_space(4.0);

            // Render knobs in a horizontal layout
            ui.horizontal(|ui| {
                const KNOB_SIZE: f32 = 36.0;

                for knob_param in &self.knob_params {
                    // Find the corresponding input parameter by name
                    if let Some(node) = graph.nodes.get(node_id) {
                        if let Some((_name, input_id)) = node.inputs.iter().find(|(name, _)| *name == knob_param.param_name) {
                            let input = graph.get_input(*input_id);

                            // Calculate param_index by finding the position among editable parameters
                            let current_param_index = node.inputs.iter()
                                .take_while(|(name, _)| *name != knob_param.param_name)
                                .filter(|(_, id)| {
                                    let inp = graph.get_input(*id);
                                    matches!(inp.kind,
                                        egui_node_graph2::InputParamKind::ConstantOnly |
                                        egui_node_graph2::InputParamKind::ConnectionOrConstant)
                                })
                                .count();

                            // Check if this exposed param has a connection
                            // iter_connections returns (InputId, OutputId) - input port and the output it's connected to
                            let is_connected = knob_param.exposed_as_input &&
                                graph.iter_connections().any(|(input, _output)| input == *input_id);

                            // Get input port index for this parameter (for looking up signal value)
                            let input_port_index = if knob_param.exposed_as_input {
                                // Count ConnectionOrConstant and ConnectionOnly inputs before this one
                                node.inputs.iter()
                                    .take_while(|(name, _)| *name != knob_param.param_name)
                                    .filter(|(_, id)| {
                                        let inp = graph.get_input(*id);
                                        matches!(inp.kind,
                                            egui_node_graph2::InputParamKind::ConnectionOnly |
                                            egui_node_graph2::InputParamKind::ConnectionOrConstant)
                                    })
                                    .count()
                            } else {
                                0
                            };

                            // Get signal feedback value from audio engine (if connected and available)
                            let signal_value = if is_connected {
                                engine_node_id.and_then(|eid|
                                    user_state.get_input_value(eid, input_port_index))
                            } else {
                                None
                            };

                            // Get MIDI mapping info for this parameter
                            let midi_mapping = engine_node_id.and_then(|eid|
                                user_state.get_midi_mapping(eid, current_param_index));
                            let is_learn_target = engine_node_id
                                .map(|eid| user_state.is_midi_learn_target(eid, current_param_index))
                                .unwrap_or(false);

                            // Get min/max values for MIDI Learn
                            let (min_value, max_value) = match &input.value {
                                SynthValueType::Scalar { .. } => (0.0, 1.0),
                                SynthValueType::Frequency { min, max, .. } => (*min, *max),
                                SynthValueType::LinearHz { min, max, .. } => (*min, *max),
                                SynthValueType::Time { min, max, .. } => (*min, *max),
                                SynthValueType::LinearRange { min, max, .. } => (*min, *max),
                                SynthValueType::Toggle { .. } => (0.0, 1.0),
                                SynthValueType::Select { options, .. } => (0.0, (options.len() - 1) as f32),
                            };

                            // Build MIDI config for the knob
                            let midi_config = KnobMidiConfig {
                                has_midi_mapping: midi_mapping.is_some(),
                                cc_number: midi_mapping.map(|m| m.cc_number),
                                is_learn_target,
                                min_value,
                                max_value,
                            };

                            // Render the knob based on value type
                            let knob_response = ui.scope(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_min_width(KNOB_SIZE + 8.0);

                                    // Visual indicator for MIDI mapping or learn mode
                                    let show_midi_indicator = midi_config.has_midi_mapping || midi_config.is_learn_target;
                                    let show_connection_indicator = is_connected && !show_midi_indicator;

                                    if show_midi_indicator {
                                        // MIDI CC badge - purple for mapped, blinking for learn mode
                                        let badge_color = if midi_config.is_learn_target {
                                            // Blink effect for learn mode
                                            let time = ui.ctx().input(|i| i.time);
                                            let blink = ((time * 4.0).sin() > 0.0) as u8;
                                            Color32::from_rgba_unmultiplied(180, 100, 200, 128 + blink * 127)
                                        } else {
                                            Color32::from_rgb(180, 100, 200) // Purple for MIDI
                                        };

                                        let dot_size = 8.0;
                                        let available_width = ui.available_width();
                                        let badge_center = egui::pos2(
                                            ui.cursor().left() + available_width / 2.0,
                                            ui.cursor().top() + dot_size / 2.0,
                                        );

                                        // Draw badge background
                                        ui.painter().circle_filled(badge_center, dot_size / 2.0 + 1.0, badge_color);

                                        // Draw "M" letter on badge
                                        let text_pos = badge_center - egui::vec2(3.0, 4.0);
                                        ui.painter().text(
                                            text_pos,
                                            egui::Align2::LEFT_TOP,
                                            "M",
                                            egui::FontId::proportional(8.0),
                                            Color32::WHITE,
                                        );

                                        ui.add_space(dot_size + 2.0);

                                        // Request repaint for blinking effect
                                        if midi_config.is_learn_target {
                                            ui.ctx().request_repaint();
                                        }
                                    } else if show_connection_indicator {
                                        // Orange color for Control signal (matches signal type color)
                                        let indicator_color = if signal_value.is_some() {
                                            Color32::from_rgb(255, 165, 0) // Orange for active signal
                                        } else {
                                            Color32::from_rgb(100, 200, 100) // Green for connected but no signal yet
                                        };
                                        // Draw a small colored dot centered above the knob
                                        let dot_size = 6.0;
                                        let available_width = ui.available_width();
                                        let dot_rect = egui::Rect::from_center_size(
                                            egui::pos2(
                                                ui.cursor().left() + available_width / 2.0,
                                                ui.cursor().top() + dot_size / 2.0,
                                            ),
                                            egui::vec2(dot_size, dot_size),
                                        );
                                        ui.painter().circle_filled(dot_rect.center(), dot_size / 2.0, indicator_color);
                                        ui.add_space(dot_size + 2.0);
                                    }

                                    // Render knob based on the value type
                                    // Note: We need to clone to render since we can't mutate through the graph reference
                                    // The actual parameter change will be handled through the normal widget flow
                                    Self::render_knob_for_value(
                                        ui,
                                        &input.value,
                                        &knob_param.label,
                                        KNOB_SIZE,
                                        is_connected,
                                        node_id,
                                        &knob_param.param_name,
                                        &mut responses,
                                        signal_value,
                                        &midi_config,
                                    );
                                });
                            });

                            // Create an interactive rect over the knob area for context menu
                            let knob_rect = knob_response.response.rect;
                            let interact_response = ui.interact(
                                knob_rect,
                                egui::Id::new(("knob_context", node_id, current_param_index)),
                                egui::Sense::click(),
                            );

                            // Handle right-click context menu for MIDI Learn
                            if let Some(engine_id) = engine_node_id {
                                let menu_response = interact_response.context_menu(|ui| {
                                    if midi_config.has_midi_mapping {
                                        let cc_text = midi_config.cc_number
                                            .map(|cc| format!("CC #{}", cc))
                                            .unwrap_or_else(|| "MIDI".to_string());
                                        ui.label(RichText::new(cc_text).small().weak());
                                        ui.separator();

                                        if ui.button("Clear MIDI").clicked() {
                                            responses.push(NodeResponse::User(SynthResponse::MidiLearnClear {
                                                engine_node_id: engine_id,
                                                param_index: current_param_index,
                                            }));
                                            ui.close_menu();
                                        }
                                        if ui.button("Re-learn MIDI CC").clicked() {
                                            responses.push(NodeResponse::User(SynthResponse::MidiLearnStart {
                                                engine_node_id: engine_id,
                                                param_index: current_param_index,
                                                param_name: knob_param.param_name.clone(),
                                                min_value: midi_config.min_value,
                                                max_value: midi_config.max_value,
                                            }));
                                            ui.close_menu();
                                        }
                                    } else {
                                        if ui.button("Learn MIDI CC").clicked() {
                                            responses.push(NodeResponse::User(SynthResponse::MidiLearnStart {
                                                engine_node_id: engine_id,
                                                param_index: current_param_index,
                                                param_name: knob_param.param_name.clone(),
                                                min_value: midi_config.min_value,
                                                max_value: midi_config.max_value,
                                            }));
                                            ui.close_menu();
                                        }
                                    }
                                });
                                // Set flag if context menu is open to prevent add-node menu
                                if menu_response.is_some() {
                                    user_state.widget_context_menu_open = true;
                                }
                            }
                        }
                    }
                }
            });
        }

        // Render LED indicators if this node has any
        if !self.led_indicators.is_empty() {
            // Add spacing before LEDs
            ui.add_space(4.0);

            // Render LEDs in a horizontal layout
            ui.horizontal(|ui| {
                for led_indicator in &self.led_indicators {
                    // Get the output value from the audio engine
                    let brightness = engine_node_id
                        .and_then(|eid| user_state.get_output_value(eid, led_indicator.output_index))
                        .unwrap_or(0.0);

                    // Render the LED with label
                    ui.vertical(|ui| {
                        ui.set_min_width(20.0);
                        let config = led_indicator.config.clone().with_label(&led_indicator.label);
                        led(ui, brightness, &config);
                    });
                }
            });
        }

        // Show the module category as a small label with subtle styling
        ui.horizontal(|ui| {
            let category_color = self.category.color();
            // Use a dimmed version of the category color for the label
            let label_color = Color32::from_rgba_unmultiplied(
                category_color.r(),
                category_color.g(),
                category_color.b(),
                128, // 50% opacity
            );
            ui.label(RichText::new(self.category.name()).small().color(label_color));
        });

        responses
    }

    fn titlebar_color(
        &self,
        _ui: &egui::Ui,
        _node_id: egui_node_graph2::NodeId,
        _graph: &egui_node_graph2::Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> Option<Color32> {
        // Return the category-based header color
        Some(self.header_color())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_node_data_creation() {
        let data = SynthNodeData::new(
            "osc.sine",
            "Sine Oscillator",
            ModuleCategory::Source,
        );

        assert_eq!(data.module_id, "osc.sine");
        assert_eq!(data.display_name, "Sine Oscillator");
        assert_eq!(data.category, ModuleCategory::Source);
    }

    #[test]
    fn test_header_color() {
        let source = SynthNodeData::new("test", "Test", ModuleCategory::Source);
        let filter = SynthNodeData::new("test", "Test", ModuleCategory::Filter);
        let output = SynthNodeData::new("test", "Test", ModuleCategory::Output);

        // Colors should match the category colors
        assert_eq!(source.header_color(), ModuleCategory::Source.color());
        assert_eq!(filter.header_color(), ModuleCategory::Filter.color());
        assert_eq!(output.header_color(), ModuleCategory::Output.color());
    }

    #[test]
    fn test_category_icons() {
        // Verify each category has a node data struct
        let categories = [
            ModuleCategory::Source,
            ModuleCategory::Filter,
            ModuleCategory::Modulation,
            ModuleCategory::Effect,
            ModuleCategory::Utility,
            ModuleCategory::Output,
        ];

        for category in categories {
            let data = SynthNodeData::new("test", "Test", category);
            // Verify the category is set correctly
            assert_eq!(data.category, category);
        }
    }

    #[test]
    fn test_node_data_clone() {
        let original = SynthNodeData::new("test", "Test Module", ModuleCategory::Utility);
        let cloned = original.clone();

        assert_eq!(original.module_id, cloned.module_id);
        assert_eq!(original.display_name, cloned.display_name);
        assert_eq!(original.category, cloned.category);
    }
}
