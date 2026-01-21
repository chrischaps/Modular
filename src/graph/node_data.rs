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
use crate::widgets::{knob, KnobConfig, ParamFormat};
use super::{SynthResponse, SynthValueType};

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
}

impl SynthNodeData {
    /// Create new node data for a module.
    pub fn new(module_id: &'static str, display_name: impl Into<String>, category: ModuleCategory) -> Self {
        Self {
            module_id,
            display_name: display_name.into(),
            category,
            knob_params: Vec::new(),
        }
    }

    /// Builder method to add knob parameters.
    pub fn with_knob_params(mut self, knob_params: Vec<KnobParam>) -> Self {
        self.knob_params = knob_params;
        self
    }

    /// Get the header color for this node based on its category.
    pub fn header_color(&self) -> Color32 {
        self.category.color()
    }

    /// Get the icon character for this module category.
    fn category_icon(&self) -> &'static str {
        match self.category {
            ModuleCategory::Source => "~",      // Wave symbol for oscillators
            ModuleCategory::Filter => "▽",     // Filter symbol
            ModuleCategory::Modulation => "◊",  // Diamond for modulation
            ModuleCategory::Effect => "◈",     // Effect symbol
            ModuleCategory::Utility => "◇",    // Utility symbol
            ModuleCategory::Output => "◉",     // Output symbol (speaker-like)
        }
    }

    /// Get a secondary icon for the right side of the header.
    fn secondary_icon(&self) -> &'static str {
        match self.category {
            ModuleCategory::Source => "∿",      // Another wave symbol
            ModuleCategory::Filter => "◠",     // Curved line for response
            ModuleCategory::Modulation => "↕",  // Up-down arrows
            ModuleCategory::Effect => "◈",     // Effect
            ModuleCategory::Utility => "⚙",    // Gear
            ModuleCategory::Output => "◉",     // Speaker
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
        // Add icons to the top bar
        ui.horizontal(|ui| {
            // Left icon
            ui.label(RichText::new(self.category_icon()).size(14.0).strong());

            // Spacer to push the right icon to the end
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Right icon
                ui.label(RichText::new(self.secondary_icon()).size(12.0));
            });
        });

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

                            // Render the knob based on value type
                            ui.vertical(|ui| {
                                ui.set_min_width(KNOB_SIZE + 8.0);

                                // Visual indicator for connected exposed params with signal feedback
                                if is_connected {
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
                                );
                            });
                        }
                    }
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
        // Verify each category has an icon
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
            // Icons should not be empty
            assert!(!data.category_icon().is_empty());
            assert!(!data.secondary_icon().is_empty());
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
