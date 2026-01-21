//! Node templates for the synthesizer graph.
//!
//! Defines the available module types that can be added to the graph.

use std::borrow::Cow;
use egui_node_graph2::{Graph, InputParamKind, NodeTemplateIter, NodeTemplateTrait};

use crate::dsp::{ModuleCategory, SignalType};
use super::{SynthDataType, SynthGraphState, SynthNodeData, SynthValueType, KnobParam, LedIndicator};

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
    /// VCA - voltage controlled amplifier for amplitude shaping.
    Vca,
    /// Attenuverter - scale and invert control signals.
    Attenuverter,
    /// Keyboard - virtual keyboard for playing notes from computer keyboard.
    Keyboard,
    /// MIDI Monitor - display incoming MIDI events.
    MidiMonitor,
    /// MIDI Note - convert MIDI note events to CV signals.
    MidiNote,
    /// Sample & Hold - sample input on trigger, hold until next trigger.
    SampleHold,
    /// Oscilloscope - real-time waveform visualization.
    Oscilloscope,
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
            SynthNodeTemplate::Vca => "util.vca",
            SynthNodeTemplate::Attenuverter => "util.attenuverter",
            SynthNodeTemplate::Keyboard => "input.keyboard",
            SynthNodeTemplate::MidiMonitor => "util.midi_monitor",
            SynthNodeTemplate::MidiNote => "input.midi_note",
            SynthNodeTemplate::SampleHold => "util.sample_hold",
            SynthNodeTemplate::Oscilloscope => "util.oscilloscope",
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
            SynthNodeTemplate::Vca => ModuleCategory::Utility,
            SynthNodeTemplate::Attenuverter => ModuleCategory::Utility,
            SynthNodeTemplate::Keyboard => ModuleCategory::Source,
            SynthNodeTemplate::MidiMonitor => ModuleCategory::Utility,
            SynthNodeTemplate::MidiNote => ModuleCategory::Source,
            SynthNodeTemplate::SampleHold => ModuleCategory::Utility,
            SynthNodeTemplate::Oscilloscope => ModuleCategory::Utility,
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
            SynthNodeTemplate::Keyboard,
            SynthNodeTemplate::MidiNote,
            SynthNodeTemplate::SvfFilter,
            SynthNodeTemplate::AdsrEnvelope,
            SynthNodeTemplate::Lfo,
            SynthNodeTemplate::Clock,
            SynthNodeTemplate::Vca,
            SynthNodeTemplate::Attenuverter,
            SynthNodeTemplate::SampleHold,
            SynthNodeTemplate::Oscilloscope,
            SynthNodeTemplate::MidiMonitor,
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
            SynthNodeTemplate::SineOscillator => Cow::Borrowed("Oscillator"),
            SynthNodeTemplate::AudioOutput => Cow::Borrowed("Audio Output"),
            SynthNodeTemplate::Lfo => Cow::Borrowed("LFO"),
            SynthNodeTemplate::SvfFilter => Cow::Borrowed("SVF Filter"),
            SynthNodeTemplate::AdsrEnvelope => Cow::Borrowed("ADSR Envelope"),
            SynthNodeTemplate::Clock => Cow::Borrowed("Clock"),
            SynthNodeTemplate::Vca => Cow::Borrowed("VCA"),
            SynthNodeTemplate::Attenuverter => Cow::Borrowed("Attenuverter"),
            SynthNodeTemplate::Keyboard => Cow::Borrowed("Keyboard"),
            SynthNodeTemplate::MidiMonitor => Cow::Borrowed("MIDI Monitor"),
            SynthNodeTemplate::MidiNote => Cow::Borrowed("MIDI Note"),
            SynthNodeTemplate::SampleHold => Cow::Borrowed("Sample & Hold"),
            SynthNodeTemplate::Oscilloscope => Cow::Borrowed("Oscilloscope"),
        }
    }

    fn node_finder_categories(&self, _user_state: &mut Self::UserState) -> Vec<Self::CategoryType> {
        vec![self.category()]
    }

    fn node_graph_label(&self, _user_state: &mut Self::UserState) -> String {
        match self {
            SynthNodeTemplate::SineOscillator => "Oscillator".to_string(),
            SynthNodeTemplate::AudioOutput => "Audio Output".to_string(),
            SynthNodeTemplate::Lfo => "LFO".to_string(),
            SynthNodeTemplate::SvfFilter => "SVF Filter".to_string(),
            SynthNodeTemplate::AdsrEnvelope => "ADSR Envelope".to_string(),
            SynthNodeTemplate::Clock => "Clock".to_string(),
            SynthNodeTemplate::Vca => "VCA".to_string(),
            SynthNodeTemplate::Attenuverter => "Attenuverter".to_string(),
            SynthNodeTemplate::Keyboard => "Keyboard".to_string(),
            SynthNodeTemplate::MidiMonitor => "MIDI Monitor".to_string(),
            SynthNodeTemplate::MidiNote => "MIDI Note".to_string(),
            SynthNodeTemplate::SampleHold => "Sample & Hold".to_string(),
            SynthNodeTemplate::Oscilloscope => "Oscilloscope".to_string(),
        }
    }

    fn user_data(&self, _user_state: &mut Self::UserState) -> Self::NodeData {
        match self {
            SynthNodeTemplate::SineOscillator => SynthNodeData::new(
                "osc.sine",
                "Oscillator",
                ModuleCategory::Source,
            ).with_knob_params(vec![
                // Frequency: exposed param with input port AND bottom knob
                // When connected, knob shows incoming value and is disabled
                KnobParam::exposed("Frequency", "Freq"),
                // FM Depth: knob-only, no input port
                KnobParam::knob_only("FM Depth", "FM Dpth"),
                // Pulse Width: knob-only, for square wave duty cycle
                KnobParam::knob_only("Pulse Width", "PW"),
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
                // Rate: exposed parameter (Rate CV input + knob)
                KnobParam::exposed("Rate", "Rate"),
                // Phase: knob-only parameter
                KnobParam::knob_only("Phase", "Phase"),
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
            ]).with_led_indicators(vec![
                // Gate output LED indicator
                LedIndicator::gate(0, "Gate"),
            ]),
            SynthNodeTemplate::Vca => SynthNodeData::new(
                "util.vca",
                "VCA",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Level and CV Amount as knobs
                KnobParam::knob_only("Level", "Level"),
                KnobParam::knob_only("CV Amount", "CV Amt"),
            ]),
            SynthNodeTemplate::Attenuverter => SynthNodeData::new(
                "util.attenuverter",
                "Attenuverter",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Amount: -1 to +1 bipolar scaling
                KnobParam::knob_only("Amount", "Amt"),
                // Offset: DC offset
                KnobParam::knob_only("Offset", "Offset"),
            ]),
            SynthNodeTemplate::Keyboard => SynthNodeData::new(
                "input.keyboard",
                "Keyboard",
                ModuleCategory::Source,
            ).with_knob_params(vec![
                // Octave shift: -2 to +2
                KnobParam::knob_only("Octave", "Oct"),
                // Velocity: 0-1
                KnobParam::knob_only("Velocity", "Vel"),
            ]).with_led_indicators(vec![
                // Gate output LED indicator
                LedIndicator::gate(0, "Gate"),
            ]),
            SynthNodeTemplate::MidiMonitor => SynthNodeData::new(
                "util.midi_monitor",
                "MIDI Monitor",
                ModuleCategory::Utility,
            ),
            // Note: MidiMonitor has no knob_params - it uses custom rendering for the event log
            SynthNodeTemplate::MidiNote => SynthNodeData::new(
                "input.midi_note",
                "MIDI Note",
                ModuleCategory::Source,
            ).with_knob_params(vec![
                // Octave shift: -4 to +4
                KnobParam::knob_only("Octave", "Oct"),
            ]).with_led_indicators(vec![
                // Gate output LED indicator (output index 1 = Gate port)
                LedIndicator::gate(1, "Gate"),
            ]),
            SynthNodeTemplate::SampleHold => SynthNodeData::new(
                "util.sample_hold",
                "Sample & Hold",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Slew: glide time to new value (0-1s)
                KnobParam::knob_only("Slew", "Slew"),
            ]),
            SynthNodeTemplate::Oscilloscope => SynthNodeData::new(
                "util.oscilloscope",
                "Oscilloscope",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Trigger Level: threshold for triggering
                KnobParam::knob_only("Trigger Level", "Trig"),
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
                // V/Oct: 1V/Octave pitch CV input (exponential scaling)
                graph.add_input_param(
                    node_id,
                    "V/Oct".to_string(),
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

                // PWM: Pulse width modulation for square wave
                graph.add_input_param(
                    node_id,
                    "PWM".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
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

                // Waveform selector - shown inline
                graph.add_input_param(
                    node_id,
                    "Waveform".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec!["Sine".to_string(), "Saw".to_string(), "Square".to_string(), "Tri".to_string()],
                        "Wave",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Pulse Width: knob-only parameter (0.1-0.9)
                graph.add_input_param(
                    node_id,
                    "Pulse Width".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.5, 0.1, 0.9, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
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
                // Rate: exposed parameter (Rate CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Rate".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(1.0, 0.01, 100.0, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline, widget skipped via knob_params check
                );

                // Sync input port
                graph.add_input_param(
                    node_id,
                    "Sync".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
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
                    true, // Shown inline as dropdown
                );

                // Phase: knob-only parameter (0-360 degrees)
                graph.add_input_param(
                    node_id,
                    "Phase".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, 0.0, 360.0, "°", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Bipolar toggle - shown inline
                graph.add_input_param(
                    node_id,
                    "Bipolar".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "Bipolar"),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as checkbox
                );

                // Single output port
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

                // Tempo: knob-only parameter (20-300 BPM)
                graph.add_input_param(
                    node_id,
                    "Tempo".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(120.0, 20.0, 300.0, "BPM", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Gate Length: knob-only parameter (1-99%)
                graph.add_input_param(
                    node_id,
                    "Gate Length".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(50.0, 1.0, 99.0, "%", ""),
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
            SynthNodeTemplate::Vca => {
                // Audio input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // CV input port
                graph.add_input_param(
                    node_id,
                    "CV".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Level: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Level".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // CV Amount: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "CV Amount".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Audio output port
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
            }
            SynthNodeTemplate::Attenuverter => {
                // Control input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Amount: knob-only parameter (-1 to +1)
                graph.add_input_param(
                    node_id,
                    "Amount".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(1.0, -1.0, 1.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Offset: knob-only parameter (-1 to +1)
                graph.add_input_param(
                    node_id,
                    "Offset".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -1.0, 1.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Control output port
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::Keyboard => {
                // Note: MIDI note number (0-127), controlled by UI keyboard events
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Note".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(60.0, 0.0, 127.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by keyboard events
                );

                // Gate: controlled by UI keyboard events
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(false, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by keyboard events
                );

                // Octave: shift keyboard up/down (-2 to +2)
                graph.add_input_param(
                    node_id,
                    "Octave".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -2.0, 2.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Velocity: fixed velocity for all notes
                graph.add_input_param(
                    node_id,
                    "Velocity".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Priority: key priority mode (shown inline as dropdown)
                graph.add_input_param(
                    node_id,
                    "Priority".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec!["Last".to_string(), "Lowest".to_string(), "Highest".to_string()],
                        "Priority",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Output ports
                graph.add_output_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Gate),
                );
                graph.add_output_param(
                    node_id,
                    "Pitch".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
                graph.add_output_param(
                    node_id,
                    "Velocity".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::MidiMonitor => {
                // Channel filter (0 = all, 1-16 = specific channel)
                graph.add_input_param(
                    node_id,
                    "Channel".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec![
                            "All".to_string(), "1".to_string(), "2".to_string(), "3".to_string(),
                            "4".to_string(), "5".to_string(), "6".to_string(), "7".to_string(),
                            "8".to_string(), "9".to_string(), "10".to_string(), "11".to_string(),
                            "12".to_string(), "13".to_string(), "14".to_string(), "15".to_string(),
                            "16".to_string(),
                        ],
                        "Ch",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Toggle filters for event types
                graph.add_input_param(
                    node_id,
                    "Notes".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "Notes"),
                    InputParamKind::ConstantOnly,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "CC".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "CC"),
                    InputParamKind::ConstantOnly,
                    true,
                );
                graph.add_input_param(
                    node_id,
                    "Pitch Bend".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(true, "PB"),
                    InputParamKind::ConstantOnly,
                    true,
                );

                // No output ports - this is display-only
            }
            SynthNodeTemplate::MidiNote => {
                // Note: MIDI note number (0-127), controlled by MIDI events
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Note".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(60.0, 0.0, 127.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by MIDI events
                );

                // Gate: controlled by MIDI events
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(false, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by MIDI events
                );

                // Velocity: controlled by MIDI events (0-127)
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Velocity".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(100.0, 0.0, 127.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by MIDI events
                );

                // Aftertouch: controlled by MIDI events (0-127)
                // Hidden parameter - not shown inline
                graph.add_input_param(
                    node_id,
                    "Aftertouch".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, 0.0, 127.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden - controlled by MIDI events
                );

                // Channel filter (0=Omni, 1-16=specific)
                graph.add_input_param(
                    node_id,
                    "Channel".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec![
                            "Omni".to_string(), "1".to_string(), "2".to_string(), "3".to_string(),
                            "4".to_string(), "5".to_string(), "6".to_string(), "7".to_string(),
                            "8".to_string(), "9".to_string(), "10".to_string(), "11".to_string(),
                            "12".to_string(), "13".to_string(), "14".to_string(), "15".to_string(),
                            "16".to_string(),
                        ],
                        "Ch",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Octave shift: -4 to +4
                graph.add_input_param(
                    node_id,
                    "Octave".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -4.0, 4.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Priority: voice priority mode (shown inline as dropdown)
                graph.add_input_param(
                    node_id,
                    "Priority".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0,
                        vec!["Last".to_string(), "Low".to_string(), "High".to_string()],
                        "Priority",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Retrigger toggle (shown inline)
                graph.add_input_param(
                    node_id,
                    "Retrigger".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(false, "Retrig"),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as checkbox
                );

                // Output ports
                graph.add_output_param(
                    node_id,
                    "Pitch".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
                graph.add_output_param(
                    node_id,
                    "Gate".to_string(),
                    SynthDataType::new(SignalType::Gate),
                );
                graph.add_output_param(
                    node_id,
                    "Velocity".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
                graph.add_output_param(
                    node_id,
                    "Aftertouch".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::SampleHold => {
                // Signal input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Trigger input port
                graph.add_input_param(
                    node_id,
                    "Trig".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Slew: knob-only parameter (0-1s)
                graph.add_input_param(
                    node_id,
                    "Slew".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(0.0, 0.0, 1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Control output port
                graph.add_output_param(
                    node_id,
                    "Out".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
            }
            SynthNodeTemplate::Oscilloscope => {
                // Input 1: Primary signal (audio or control)
                graph.add_input_param(
                    node_id,
                    "In 1".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Input 2: Secondary signal (audio or control)
                graph.add_input_param(
                    node_id,
                    "In 2".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // External trigger input
                graph.add_input_param(
                    node_id,
                    "Trig".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Trigger Mode: dropdown selector (shown inline)
                graph.add_input_param(
                    node_id,
                    "Mode".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0, // Auto
                        vec!["Auto".to_string(), "Normal".to_string(), "Single".to_string(), "Free".to_string()],
                        "Mode",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Trigger Level: knob-only parameter (-1 to +1)
                graph.add_input_param(
                    node_id,
                    "Trigger Level".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -1.0, 1.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // No output ports - display only
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
        assert_eq!(templates.len(), 13);
        assert!(templates.contains(&SynthNodeTemplate::SineOscillator));
        assert!(templates.contains(&SynthNodeTemplate::AudioOutput));
        assert!(templates.contains(&SynthNodeTemplate::Lfo));
        assert!(templates.contains(&SynthNodeTemplate::SvfFilter));
        assert!(templates.contains(&SynthNodeTemplate::AdsrEnvelope));
        assert!(templates.contains(&SynthNodeTemplate::Clock));
        assert!(templates.contains(&SynthNodeTemplate::Attenuverter));
        assert!(templates.contains(&SynthNodeTemplate::Vca));
        assert!(templates.contains(&SynthNodeTemplate::Keyboard));
        assert!(templates.contains(&SynthNodeTemplate::MidiMonitor));
        assert!(templates.contains(&SynthNodeTemplate::MidiNote));
        assert!(templates.contains(&SynthNodeTemplate::SampleHold));
        assert!(templates.contains(&SynthNodeTemplate::Oscilloscope));
    }

    #[test]
    fn test_module_id() {
        assert_eq!(SynthNodeTemplate::SineOscillator.module_id(), "osc.sine");
        assert_eq!(SynthNodeTemplate::AudioOutput.module_id(), "output.audio");
        assert_eq!(SynthNodeTemplate::Lfo.module_id(), "mod.lfo");
        assert_eq!(SynthNodeTemplate::SvfFilter.module_id(), "filter.svf");
        assert_eq!(SynthNodeTemplate::AdsrEnvelope.module_id(), "mod.adsr");
        assert_eq!(SynthNodeTemplate::Clock.module_id(), "util.clock");
        assert_eq!(SynthNodeTemplate::Vca.module_id(), "util.vca");
        assert_eq!(SynthNodeTemplate::Attenuverter.module_id(), "util.attenuverter");
        assert_eq!(SynthNodeTemplate::Keyboard.module_id(), "input.keyboard");
        assert_eq!(SynthNodeTemplate::MidiMonitor.module_id(), "util.midi_monitor");
        assert_eq!(SynthNodeTemplate::MidiNote.module_id(), "input.midi_note");
        assert_eq!(SynthNodeTemplate::SampleHold.module_id(), "util.sample_hold");
        assert_eq!(SynthNodeTemplate::Oscilloscope.module_id(), "util.oscilloscope");
    }

    #[test]
    fn test_category() {
        assert_eq!(SynthNodeTemplate::SineOscillator.category(), ModuleCategory::Source);
        assert_eq!(SynthNodeTemplate::AudioOutput.category(), ModuleCategory::Output);
        assert_eq!(SynthNodeTemplate::Lfo.category(), ModuleCategory::Modulation);
        assert_eq!(SynthNodeTemplate::SvfFilter.category(), ModuleCategory::Filter);
        assert_eq!(SynthNodeTemplate::AdsrEnvelope.category(), ModuleCategory::Modulation);
        assert_eq!(SynthNodeTemplate::Clock.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::Vca.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::Attenuverter.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::Keyboard.category(), ModuleCategory::Source);
        assert_eq!(SynthNodeTemplate::MidiMonitor.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::MidiNote.category(), ModuleCategory::Source);
        assert_eq!(SynthNodeTemplate::SampleHold.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::Oscilloscope.category(), ModuleCategory::Utility);
    }

    #[test]
    fn test_node_finder_label() {
        let mut state = SynthGraphState::default();
        assert_eq!(
            SynthNodeTemplate::SineOscillator.node_finder_label(&mut state),
            "Oscillator"
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
        assert_eq!(
            SynthNodeTemplate::Vca.node_finder_label(&mut state),
            "VCA"
        );
        assert_eq!(
            SynthNodeTemplate::Attenuverter.node_finder_label(&mut state),
            "Attenuverter"
        );
        assert_eq!(
            SynthNodeTemplate::Keyboard.node_finder_label(&mut state),
            "Keyboard"
        );
        assert_eq!(
            SynthNodeTemplate::MidiMonitor.node_finder_label(&mut state),
            "MIDI Monitor"
        );
        assert_eq!(
            SynthNodeTemplate::MidiNote.node_finder_label(&mut state),
            "MIDI Note"
        );
        assert_eq!(
            SynthNodeTemplate::SampleHold.node_finder_label(&mut state),
            "Sample & Hold"
        );
        assert_eq!(
            SynthNodeTemplate::Oscilloscope.node_finder_label(&mut state),
            "Oscilloscope"
        );
    }
}
