//! Node templates for the synthesizer graph.
//!
//! Defines the available module types that can be added to the graph.

use std::borrow::Cow;
use egui_node_graph2::{Graph, InputParamKind, NodeTemplateIter, NodeTemplateTrait};

use crate::dsp::{ModuleCategory, SignalType};
use super::{SynthDataType, SynthGraphState, SynthNodeData, SynthValueType, KnobParam};

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
    /// LFO - low frequency oscillator for modulation.
    Lfo,
    /// State Variable Filter - multi-mode filter with LP, HP, BP outputs.
    SvfFilter,
    /// ADSR Envelope - attack-decay-sustain-release envelope generator.
    AdsrEnvelope,
    /// Clock - periodic gate trigger generator.
    Clock,
}

impl SynthNodeTemplate {
    /// Get the module ID for this template.
    /// These IDs must match the `id` field in the corresponding DspModule::info().
    pub fn module_id(&self) -> &'static str {
        match self {
            SynthNodeTemplate::SineOscillator => "osc.sine",
            SynthNodeTemplate::AudioOutput => "output.audio",
            SynthNodeTemplate::Lfo => "mod.lfo",
            SynthNodeTemplate::SvfFilter => "filter.svf",
            SynthNodeTemplate::AdsrEnvelope => "mod.adsr",
            SynthNodeTemplate::Clock => "util.clock",
        }
    }

    /// Get the category for this template.
    pub fn category(&self) -> ModuleCategory {
        match self {
            SynthNodeTemplate::SineOscillator => ModuleCategory::Source,
            SynthNodeTemplate::AudioOutput => ModuleCategory::Output,
            SynthNodeTemplate::Lfo => ModuleCategory::Modulation,
            SynthNodeTemplate::SvfFilter => ModuleCategory::Filter,
            SynthNodeTemplate::AdsrEnvelope => ModuleCategory::Modulation,
            SynthNodeTemplate::Clock => ModuleCategory::Utility,
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
            SynthNodeTemplate::SvfFilter,
            SynthNodeTemplate::AdsrEnvelope,
            SynthNodeTemplate::Lfo,
            SynthNodeTemplate::Clock,
            SynthNodeTemplate::AudioOutput,
        ]
    }
}

impl AllNodeTemplates {
    /// Returns all templates grouped by category.
    ///
    /// Categories are returned in a logical display order:
    /// Sources, Filters, Modulation, Effects, Utilities, Output.
    /// Only includes categories that have at least one template.
    pub fn by_category() -> Vec<(ModuleCategory, Vec<SynthNodeTemplate>)> {
        use std::collections::HashMap;

        // Collect templates by category
        let mut map: HashMap<ModuleCategory, Vec<SynthNodeTemplate>> = HashMap::new();
        for template in Self.all_kinds() {
            map.entry(template.category())
                .or_default()
                .push(template);
        }

        // Define display order for categories
        let category_order = [
            ModuleCategory::Source,
            ModuleCategory::Filter,
            ModuleCategory::Modulation,
            ModuleCategory::Effect,
            ModuleCategory::Utility,
            ModuleCategory::Output,
        ];

        // Build result in display order, excluding empty categories
        category_order
            .into_iter()
            .filter_map(|cat| map.remove(&cat).map(|templates| (cat, templates)))
            .collect()
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
            SynthNodeTemplate::Lfo => Cow::Borrowed("LFO"),
            SynthNodeTemplate::SvfFilter => Cow::Borrowed("SVF Filter"),
            SynthNodeTemplate::AdsrEnvelope => Cow::Borrowed("ADSR Envelope"),
            SynthNodeTemplate::Clock => Cow::Borrowed("Clock"),
        }
    }

    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        vec![self.category()]
    }

    fn node_graph_label(&self, _user_state: &mut Self::UserState) -> String {
        match self {
            SynthNodeTemplate::SineOscillator => "Sine Oscillator".to_string(),
            SynthNodeTemplate::AudioOutput => "Audio Output".to_string(),
            SynthNodeTemplate::Lfo => "LFO".to_string(),
            SynthNodeTemplate::SvfFilter => "SVF Filter".to_string(),
            SynthNodeTemplate::AdsrEnvelope => "ADSR Envelope".to_string(),
            SynthNodeTemplate::Clock => "Clock".to_string(),
        }
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        match self {
            SynthNodeTemplate::SineOscillator => SynthNodeData::new(
                "osc.sine",
                "Sine Oscillator",
                ModuleCategory::Source,
            ).with_knob_params(vec![
                // Frequency: exposed param with input port AND bottom knob
                // When connected, knob shows incoming value and is disabled
                KnobParam::exposed("Frequency", "Freq"),
                // FM Depth: knob-only, no input port
                KnobParam::knob_only("FM Depth", "FM Dpth"),
            ]),
            SynthNodeTemplate::AudioOutput => SynthNodeData::new(
                "output.audio",
                "Audio Output",
                ModuleCategory::Output,
            ).with_knob_params(vec![
                // Volume is knob-only
                KnobParam::knob_only("Volume", "Vol"),
            ]),
            SynthNodeTemplate::Lfo => SynthNodeData::new(
                "mod.lfo",
                "LFO",
                ModuleCategory::Modulation,
            ).with_knob_params(vec![
                // Rate: knob-only parameter
                KnobParam::knob_only("Rate", "Rate"),
            ]),
            SynthNodeTemplate::SvfFilter => SynthNodeData::new(
                "filter.svf",
                "SVF Filter",
                ModuleCategory::Filter,
            ).with_knob_params(vec![
                // Cutoff: exposed param (input port + knob)
                KnobParam::exposed("Cutoff", "Cutoff"),
                // Resonance: exposed param (input port + knob)
                KnobParam::exposed("Resonance", "Res"),
                // Drive: knob-only
                KnobParam::knob_only("Drive", "Drive"),
            ]),
            SynthNodeTemplate::AdsrEnvelope => SynthNodeData::new(
                "mod.adsr",
                "ADSR Envelope",
                ModuleCategory::Modulation,
            ).with_knob_params(vec![
                // All ADSR parameters are knob-only (no CV input ports)
                KnobParam::knob_only("Attack", "Atk"),
                KnobParam::knob_only("Decay", "Dec"),
                KnobParam::knob_only("Sustain", "Sus"),
                KnobParam::knob_only("Release", "Rel"),
            ]),
            SynthNodeTemplate::Clock => SynthNodeData::new(
                "util.clock",
                "Clock",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Tempo and Gate Length as knobs
                KnobParam::knob_only("Tempo", "BPM"),
                KnobParam::knob_only("Gate Length", "Gate"),
            ]),
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
                // Pure input ports (connection only, no knob)
                graph.add_input_param(
                    node_id,
                    "Add Freq".to_string(),  // Renamed from "Freq CV"
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true, // shown inline (just the port)
                );
                graph.add_input_param(
                    node_id,
                    "FM".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Frequency parameter: input port for external control + knob at bottom
                // ConnectionOrConstant allows both external modulation and manual control
                // The inline widget is skipped (see value_widget) since we have the bottom knob
                graph.add_input_param(
                    node_id,
                    "Frequency".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(440.0, 20.0, 20000.0, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline, widget skipped via knob_params check
                );

                // Knob-only parameter: no input port, knob at bottom
                // ConstantOnly + hidden inline = knob only appears at bottom
                graph.add_input_param(
                    node_id,
                    "FM Depth".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_hz(0.0, 0.0, 1000.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown only in bottom knob row
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

                // Knob-only parameter: Volume control
                graph.add_input_param(
                    node_id,
                    "Volume".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.8, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Limiter toggle - keep inline for now (not a knob type)
                graph.add_input_param(
                    node_id,
                    "Limiter".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "Limiter"),
                    InputParamKind::ConstantOnly,
                    true,
                );
            }
            SynthNodeTemplate::Lfo => {
                // Rate: knob-only parameter for LFO speed
                graph.add_input_param(
                    node_id,
                    "Rate".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_hz(1.0, 0.01, 20.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Waveform selector - shown inline
                graph.add_input_param(
                    node_id,
                    "Waveform".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec!["Sine".to_string(), "Triangle".to_string(), "Square".to_string(), "Saw".to_string()],
                        "Wave",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline
                );

                // Output port - Control signal
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::SvfFilter => {
                // Audio input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Cutoff: exposed param (input port + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Cutoff".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(1000.0, 20.0, 20000.0, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline, widget skipped via knob_params check
                );

                // Resonance: exposed param (input port + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Resonance".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline, widget skipped via knob_params check
                );

                // Drive: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Drive".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""), // 0-1 maps to 1-10x in DSP
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown only in bottom knob row
                );

                // Output ports - all three filter types
                graph.add_output_param(
                    node_id,
                    "LowPass".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
                graph.add_output_param(
                    node_id,
                    "HighPass".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
                graph.add_output_param(
                    node_id,
                    "BandPass".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
            }
            SynthNodeTemplate::AdsrEnvelope => {
                // Gate input port
                graph.add_input_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Retrigger input port
                graph.add_input_param(
                    node_id,
                    "Retrig".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Attack: knob-only parameter (logarithmic time)
                graph.add_input_param(
                    node_id,
                    "Attack".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(0.01, 0.001, 10.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Decay: knob-only parameter (logarithmic time)
                graph.add_input_param(
                    node_id,
                    "Decay".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(0.1, 0.001, 10.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Sustain: knob-only parameter (0-1 level)
                graph.add_input_param(
                    node_id,
                    "Sustain".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.7, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Release: knob-only parameter (logarithmic time)
                graph.add_input_param(
                    node_id,
                    "Release".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(0.3, 0.001, 10.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Output port - Control signal
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::Clock => {
                // Sync input port
                graph.add_input_param(
                    node_id,
                    "Sync".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Tempo: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Tempo".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(120.0 / 300.0, ""), // Normalized: 120 BPM in 20-300 range
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Gate Length: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Gate Length".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""), // 50%
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Division: discrete selection (shown inline)
                graph.add_input_param(
                    node_id,
                    "Division".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        2, // Quarter note default
                        vec!["1".to_string(), "1/2".to_string(), "1/4".to_string(), "1/8".to_string(), "1/16".to_string()],
                        "Div",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Run toggle (shown inline)
                graph.add_input_param(
                    node_id,
                    "Run".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "Run"),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as checkbox
                );

                // Gate output port
                graph.add_output_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Gate),
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
        assert_eq!(templates.len(), 6);
        assert!(templates.contains(&SynthNodeTemplate::SineOscillator));
        assert!(templates.contains(&SynthNodeTemplate::AudioOutput));
        assert!(templates.contains(&SynthNodeTemplate::Lfo));
        assert!(templates.contains(&SynthNodeTemplate::SvfFilter));
        assert!(templates.contains(&SynthNodeTemplate::AdsrEnvelope));
        assert!(templates.contains(&SynthNodeTemplate::Clock));
    }

    #[test]
    fn test_module_id() {
        assert_eq!(SynthNodeTemplate::SineOscillator.module_id(), "osc.sine");
        assert_eq!(SynthNodeTemplate::AudioOutput.module_id(), "output.audio");
        assert_eq!(SynthNodeTemplate::Lfo.module_id(), "mod.lfo");
        assert_eq!(SynthNodeTemplate::SvfFilter.module_id(), "filter.svf");
        assert_eq!(SynthNodeTemplate::AdsrEnvelope.module_id(), "mod.adsr");
        assert_eq!(SynthNodeTemplate::Clock.module_id(), "util.clock");
    }

    #[test]
    fn test_category() {
        assert_eq!(SynthNodeTemplate::SineOscillator.category(), ModuleCategory::Source);
        assert_eq!(SynthNodeTemplate::AudioOutput.category(), ModuleCategory::Output);
        assert_eq!(SynthNodeTemplate::Lfo.category(), ModuleCategory::Modulation);
        assert_eq!(SynthNodeTemplate::SvfFilter.category(), ModuleCategory::Filter);
        assert_eq!(SynthNodeTemplate::AdsrEnvelope.category(), ModuleCategory::Modulation);
        assert_eq!(SynthNodeTemplate::Clock.category(), ModuleCategory::Utility);
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
        assert_eq!(
            SynthNodeTemplate::Lfo.node_finder_label(&mut state),
            "LFO"
        );
        assert_eq!(
            SynthNodeTemplate::SvfFilter.node_finder_label(&mut state),
            "SVF Filter"
        );
        assert_eq!(
            SynthNodeTemplate::AdsrEnvelope.node_finder_label(&mut state),
            "ADSR Envelope"
        );
        assert_eq!(
            SynthNodeTemplate::Clock.node_finder_label(&mut state),
            "Clock"
        );
    }
}
