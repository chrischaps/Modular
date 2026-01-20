//! Node data for the synthesizer graph.
//!
//! Defines the per-node data stored in the graph editor.

use eframe::egui;
use egui_node_graph2::NodeDataTrait;

use crate::dsp::ModuleCategory;
use super::SynthResponse;

/// Data stored per node in the graph.
///
/// This contains information about which module type this node represents
/// and any per-instance display settings.
#[derive(Clone, Debug)]
pub struct SynthNodeData {
    /// The module type identifier (e.g., "sine_osc", "audio_output").
    pub module_id: &'static str,
    /// Display name shown in the node header.
    pub display_name: String,
    /// The category of this module (for header coloring).
    pub category: ModuleCategory,
}

impl SynthNodeData {
    /// Create new node data for a module.
    pub fn new(module_id: &'static str, display_name: impl Into<String>, category: ModuleCategory) -> Self {
        Self {
            module_id,
            display_name: display_name.into(),
            category,
        }
    }

    /// Get the header color for this node based on its category.
    pub fn header_color(&self) -> egui::Color32 {
        self.category.color()
    }
}

impl NodeDataTrait for SynthNodeData {
    type Response = SynthResponse;
    type UserState = super::SynthGraphState;
    type DataType = super::SynthDataType;
    type ValueType = super::SynthValueType;

    fn bottom_ui(
        &self,
        ui: &mut egui::Ui,
        _node_id: egui_node_graph2::NodeId,
        _graph: &egui_node_graph2::Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> Vec<egui_node_graph2::NodeResponse<Self::Response, Self>>
    where
        Self::Response: egui_node_graph2::UserResponseTrait,
    {
        // Show the module category as a small label
        ui.horizontal(|ui: &mut egui::Ui| {
            ui.label(egui::RichText::new(self.category.name()).small().weak());
        });

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_node_data_creation() {
        let data = SynthNodeData::new(
            "sine_osc",
            "Sine Oscillator",
            ModuleCategory::Source,
        );

        assert_eq!(data.module_id, "sine_osc");
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
    fn test_node_data_clone() {
        let original = SynthNodeData::new("test", "Test Module", ModuleCategory::Utility);
        let cloned = original.clone();

        assert_eq!(original.module_id, cloned.module_id);
        assert_eq!(original.display_name, cloned.display_name);
        assert_eq!(original.category, cloned.category);
    }
}
