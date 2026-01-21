//! Node data for the synthesizer graph.
//!
//! Defines the per-node data stored in the graph editor.
//!
//! # Custom Rendering
//!
//! This module implements custom node rendering to match the concept image aesthetic:
//! - Colored header bars based on module category
//! - Module icons in the header
//! - Category labels in the footer

use eframe::egui::{self, Color32, RichText};
use egui_node_graph2::{NodeDataTrait, NodeResponse, UserResponseTrait};

use crate::dsp::ModuleCategory;
use super::SynthResponse;

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
        _node_id: egui_node_graph2::NodeId,
        _graph: &egui_node_graph2::Graph<Self, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
    ) -> Vec<NodeResponse<Self::Response, Self>>
    where
        Self::Response: UserResponseTrait,
    {
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

        Vec::new()
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
