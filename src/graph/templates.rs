//! Node templates for the synthesizer graph.
//!
//! Defines the available module types that can be added to the graph.

use std::borrow::Cow;
use egui_node_graph2::{Graph, InputParamKind, NodeTemplateIter, NodeTemplateTrait};

use crate::dsp::{ModuleCategory, SignalType};
use super::{SynthDataType, SynthGraphState, SynthNodeData, SynthValueType};

/// Templates for all available synth modules.
///
/// Each template defines how to create a node of that module type,
/// including its ports and initial parameter values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SynthNodeTemplate {
    /// Sine oscillator - basic audio source.
    SineOscillator,
    /// Audio output - final destination in signal chain.
    AudioOutput,
}

impl SynthNodeTemplate {
    /// Get the module ID for this template.
    pub fn module_id(&self) -> &'static str {
        match self {
            SynthNodeTemplate::SineOscillator => "sine_osc",
            SynthNodeTemplate::AudioOutput => "audio_output",
        }
    }

    /// Get the category for this template.
    pub fn category(&self) -> ModuleCategory {
        match self {
            SynthNodeTemplate::SineOscillator => ModuleCategory::Source,
            SynthNodeTemplate::AudioOutput => ModuleCategory::Output,
        }
    }
}

/// Iterator over all available node templates.
pub struct AllNodeTemplates;

impl NodeTemplateIter for AllNodeTemplates {
    type Item = SynthNodeTemplate;

    fn all_kinds(&self) -> Vec<Self::Item> {
        vec![
            SynthNodeTemplate::SineOscillator,
            SynthNodeTemplate::AudioOutput,
        ]
    }
}

impl NodeTemplateTrait for SynthNodeTemplate {
    type NodeData = SynthNodeData;
    type DataType = SynthDataType;
    type ValueType = SynthValueType;
    type UserState = SynthGraphState;
    type CategoryType = ModuleCategory;

    fn node_finder_label(&self, _user_state: &mut Self::UserState) -> Cow<'_, str> {
        match self {
            SynthNodeTemplate::SineOscillator => Cow::Borrowed("Sine Oscillator"),
            SynthNodeTemplate::AudioOutput => Cow::Borrowed("Audio Output"),
        }
    }

    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        vec![self.category()]
    }

    fn node_graph_label(&self, _user_state: &mut Self::UserState) -> String {
        match self {
            SynthNodeTemplate::SineOscillator => "Sine Oscillator".to_string(),
            SynthNodeTemplate::AudioOutput => "Audio Output".to_string(),
        }
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        match self {
            SynthNodeTemplate::SineOscillator => SynthNodeData::new(
                "sine_osc",
                "Sine Oscillator",
                ModuleCategory::Source,
            ),
            SynthNodeTemplate::AudioOutput => SynthNodeData::new(
                "audio_output",
                "Audio Output",
                ModuleCategory::Output,
            ),
        }
    }

    fn build_node(
        &self,
        graph: &mut Graph<Self::NodeData, Self::DataType, Self::ValueType>,
        _user_state: &mut Self::UserState,
        node_id: egui_node_graph2::NodeId,
    ) {
        match self {
            SynthNodeTemplate::SineOscillator => {
                // Input ports (connectable)
                graph.add_input_param(
                    node_id,
                    "Freq CV".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true, // shown inline
                );
                graph.add_input_param(
                    node_id,
                    "FM".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Parameter widgets (with connection capability)
                graph.add_input_param(
                    node_id,
                    "Frequency".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(440.0, 20.0, 20000.0, "Frequency"),
                    InputParamKind::ConnectionOrConstant,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "FM Depth".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, "FM Depth"),
                    InputParamKind::ConstantOnly,
                    true,
                );

                // Output port
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
            }
            SynthNodeTemplate::AudioOutput => {
                // Audio input ports
                graph.add_input_param(
                    node_id,
                    "Left".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "Right".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "Mono".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Parameter widgets
                graph.add_input_param(
                    node_id,
                    "Volume".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.8, "Volume"),
                    InputParamKind::ConstantOnly,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "Limiter".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "Limiter"),
                    InputParamKind::ConstantOnly,
                    true,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_templates() {
        let templates = AllNodeTemplates.all_kinds();
        assert_eq!(templates.len(), 2);
        assert!(templates.contains(&SynthNodeTemplate::SineOscillator));
        assert!(templates.contains(&SynthNodeTemplate::AudioOutput));
    }

    #[test]
    fn test_module_id() {
        assert_eq!(SynthNodeTemplate::SineOscillator.module_id(), "sine_osc");
        assert_eq!(SynthNodeTemplate::AudioOutput.module_id(), "audio_output");
    }

    #[test]
    fn test_category() {
        assert_eq!(SynthNodeTemplate::SineOscillator.category(), ModuleCategory::Source);
        assert_eq!(SynthNodeTemplate::AudioOutput.category(), ModuleCategory::Output);
    }

    #[test]
    fn test_node_finder_label() {
        let mut state = SynthGraphState::default();
        assert_eq!(
            SynthNodeTemplate::SineOscillator.node_finder_label(&mut state),
            "Sine Oscillator"
        );
        assert_eq!(
            SynthNodeTemplate::AudioOutput.node_finder_label(&mut state),
            "Audio Output"
        );
    }
}
