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
    /// Step Sequencer - 16-step sequencer with pitch, gate, and velocity.
    StepSequencer,
    /// Stereo Delay - delay effect with feedback, filtering, and ping-pong.
    StereoDelay,
    /// Reverb - Freeverb-style stereo reverb effect.
    Reverb,
    /// Parametric EQ - 3-band equalizer with low shelf, mid parametric, high shelf.
    ParametricEq,
    /// Distortion - multi-algorithm distortion/saturation effect.
    Distortion,
    /// Chorus - stereo chorus/flanger with multiple voices.
    Chorus,
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
            SynthNodeTemplate::StepSequencer => "seq.step",
            SynthNodeTemplate::StereoDelay => "fx.delay",
            SynthNodeTemplate::Reverb => "fx.reverb",
            SynthNodeTemplate::ParametricEq => "fx.eq",
            SynthNodeTemplate::Distortion => "fx.distortion",
            SynthNodeTemplate::Chorus => "fx.chorus",
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
            SynthNodeTemplate::StepSequencer => ModuleCategory::Utility,
            SynthNodeTemplate::StereoDelay => ModuleCategory::Effect,
            SynthNodeTemplate::Reverb => ModuleCategory::Effect,
            SynthNodeTemplate::ParametricEq => ModuleCategory::Effect,
            SynthNodeTemplate::Distortion => ModuleCategory::Effect,
            SynthNodeTemplate::Chorus => ModuleCategory::Effect,
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
            SynthNodeTemplate::StepSequencer,
            SynthNodeTemplate::StereoDelay,
            SynthNodeTemplate::Reverb,
            SynthNodeTemplate::ParametricEq,
            SynthNodeTemplate::Distortion,
            SynthNodeTemplate::Chorus,
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
            SynthNodeTemplate::StepSequencer => Cow::Borrowed("Step Sequencer"),
            SynthNodeTemplate::StereoDelay => Cow::Borrowed("Stereo Delay"),
            SynthNodeTemplate::Reverb => Cow::Borrowed("Reverb"),
            SynthNodeTemplate::ParametricEq => Cow::Borrowed("3-Band EQ"),
            SynthNodeTemplate::Distortion => Cow::Borrowed("Distortion"),
            SynthNodeTemplate::Chorus => Cow::Borrowed("Chorus"),
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
            SynthNodeTemplate::StepSequencer => "Step Sequencer".to_string(),
            SynthNodeTemplate::StereoDelay => "Stereo Delay".to_string(),
            SynthNodeTemplate::Reverb => "Reverb".to_string(),
            SynthNodeTemplate::ParametricEq => "3-Band EQ".to_string(),
            SynthNodeTemplate::Distortion => "Distortion".to_string(),
            SynthNodeTemplate::Chorus => "Chorus".to_string(),
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
            SynthNodeTemplate::StepSequencer => SynthNodeData::new(
                "seq.step",
                "Step Sequencer",
                ModuleCategory::Utility,
            ).with_knob_params(vec![
                // Steps: sequence length (1-16)
                KnobParam::knob_only("Steps", "Steps"),
                // Gate Length: percentage of step duration
                KnobParam::knob_only("Gate Length", "Gate"),
            ]).with_led_indicators(vec![
                // Gate output LED
                LedIndicator::gate(1, "Gate"),
                // EOC output LED
                LedIndicator::activity(4, "EOC"),
            ]),
            SynthNodeTemplate::StereoDelay => SynthNodeData::new(
                "fx.delay",
                "Stereo Delay",
                ModuleCategory::Effect,
            ).with_knob_params(vec![
                // Time: exposed (CV input + knob)
                KnobParam::exposed("Time", "Time"),
                // Feedback: exposed (CV input + knob)
                KnobParam::exposed("Feedback", "FB"),
                // Mix: knob-only
                KnobParam::knob_only("Mix", "Mix"),
                // High Cut: knob-only
                KnobParam::knob_only("High Cut", "HiCut"),
                // Low Cut: knob-only
                KnobParam::knob_only("Low Cut", "LoCut"),
            ]),
            SynthNodeTemplate::Reverb => SynthNodeData::new(
                "fx.reverb",
                "Reverb",
                ModuleCategory::Effect,
            ).with_knob_params(vec![
                // Size: knob-only
                KnobParam::knob_only("Size", "Size"),
                // Decay: knob-only
                KnobParam::knob_only("Decay", "Decay"),
                // Damping: knob-only
                KnobParam::knob_only("Damping", "Damp"),
                // Pre-Delay: knob-only
                KnobParam::knob_only("Pre-Delay", "PreD"),
                // Mix: knob-only
                KnobParam::knob_only("Mix", "Mix"),
                // Width: knob-only
                KnobParam::knob_only("Width", "Width"),
            ]),
            SynthNodeTemplate::ParametricEq => SynthNodeData::new(
                "fx.eq",
                "3-Band EQ",
                ModuleCategory::Effect,
            ).with_knob_params(vec![
                // Low shelf controls
                KnobParam::knob_only("Low Freq", "LoFrq"),
                KnobParam::knob_only("Low Gain", "LoGn"),
                // Mid parametric controls
                KnobParam::knob_only("Mid Freq", "MdFrq"),
                KnobParam::knob_only("Mid Gain", "MdGn"),
                KnobParam::knob_only("Mid Q", "MdQ"),
                // High shelf controls
                KnobParam::knob_only("High Freq", "HiFrq"),
                KnobParam::knob_only("High Gain", "HiGn"),
                // Output gain
                KnobParam::knob_only("Output", "Out"),
            ]),
            SynthNodeTemplate::Distortion => SynthNodeData::new(
                "fx.distortion",
                "Distortion",
                ModuleCategory::Effect,
            ).with_knob_params(vec![
                // Drive: exposed param (CV input + knob)
                KnobParam::exposed("Drive", "Drive"),
                // Tone: knob-only (brightness)
                KnobParam::knob_only("Tone", "Tone"),
                // Mix: knob-only (wet/dry)
                KnobParam::knob_only("Mix", "Mix"),
                // Output: knob-only (makeup gain)
                KnobParam::knob_only("Output", "Out"),
            ]),
            SynthNodeTemplate::Chorus => SynthNodeData::new(
                "fx.chorus",
                "Chorus",
                ModuleCategory::Effect,
            ).with_knob_params(vec![
                // Rate: exposed param (CV input + knob)
                KnobParam::exposed("Rate", "Rate"),
                // Depth: exposed param (CV input + knob)
                KnobParam::exposed("Depth", "Depth"),
                // Delay: knob-only (base delay time)
                KnobParam::knob_only("Delay", "Delay"),
                // Feedback: knob-only
                KnobParam::knob_only("Feedback", "FB"),
                // Mix: knob-only (wet/dry)
                KnobParam::knob_only("Mix", "Mix"),
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
            SynthNodeTemplate::StepSequencer => {
                // Clock input port
                graph.add_input_param(
                    node_id,
                    "Clock".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Reset input port
                graph.add_input_param(
                    node_id,
                    "Reset".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Run input port (default high = running)
                graph.add_input_param(
                    node_id,
                    "Run".to_string(),
                    SynthDataType::new(SignalType::Gate),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Steps: knob-only parameter (1-16)
                graph.add_input_param(
                    node_id,
                    "Steps".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(8.0, 1.0, 16.0, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Direction: dropdown selector (shown inline)
                graph.add_input_param(
                    node_id,
                    "Direction".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0, // Forward
                        vec!["Fwd".to_string(), "Bwd".to_string(), "P-P".to_string(), "Rnd".to_string()],
                        "Dir",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
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

                // Per-step parameters (16 steps x 3 params = 48 params, hidden)
                // These are controlled via the custom sequencer UI
                for step in 1..=16 {
                    // Pitch: MIDI note number (0-127)
                    graph.add_input_param(
                        node_id,
                        format!("Step {} Pitch", step),
                        SynthDataType::new(SignalType::Control),
                        SynthValueType::linear_range(60.0, 0.0, 127.0, "", ""),
                        InputParamKind::ConstantOnly,
                        false, // Hidden - controlled via custom UI
                    );

                    // Gate: on/off toggle
                    graph.add_input_param(
                        node_id,
                        format!("Step {} Gate", step),
                        SynthDataType::new(SignalType::Control),
                        SynthValueType::toggle(true, ""),
                        InputParamKind::ConstantOnly,
                        false, // Hidden - controlled via custom UI
                    );

                    // Velocity: 0-127
                    graph.add_input_param(
                        node_id,
                        format!("Step {} Velocity", step),
                        SynthDataType::new(SignalType::Control),
                        SynthValueType::linear_range(100.0, 0.0, 127.0, "", ""),
                        InputParamKind::ConstantOnly,
                        false, // Hidden - controlled via custom UI
                    );
                }

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
                    "Step".to_string(),
                    SynthDataType::new(SignalType::Control),
                );
                graph.add_output_param(
                    node_id,
                    "EOC".to_string(),
                    SynthDataType::new(SignalType::Gate),
                );
            }
            SynthNodeTemplate::StereoDelay => {
                // Left input port
                graph.add_input_param(
                    node_id,
                    "In L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Right input port (normalled from L)
                graph.add_input_param(
                    node_id,
                    "In R".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Time: exposed parameter (CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Time".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(500.0, 1.0, 2000.0, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline
                );

                // Feedback: exposed parameter (CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Feedback".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline
                );

                // Mix: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Mix".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // High Cut: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "High Cut".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(10000.0, 100.0, 20000.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Low Cut: knob-only parameter
                graph.add_input_param(
                    node_id,
                    "Low Cut".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(20.0, 20.0, 2000.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Ping-Pong toggle (shown inline)
                graph.add_input_param(
                    node_id,
                    "Ping-Pong".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::toggle(false, "P-P"),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as checkbox
                );

                // Sync selector (shown inline)
                graph.add_input_param(
                    node_id,
                    "Sync".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0, // Off
                        vec!["Off".to_string(), "1/4".to_string(), "1/8".to_string(), "1/8T".to_string(), "1/16".to_string(), "1/16T".to_string(), "1/32".to_string()],
                        "Sync",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Output ports
                graph.add_output_param(
                    node_id,
                    "Out L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
                graph.add_output_param(
                    node_id,
                    "Out R".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
            }
            SynthNodeTemplate::Reverb => {
                // Left input port
                graph.add_input_param(
                    node_id,
                    "In L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Right input port (normalled from L)
                graph.add_input_param(
                    node_id,
                    "In R".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Size: knob-only parameter (0-1)
                graph.add_input_param(
                    node_id,
                    "Size".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Decay: knob-only parameter (0.1-30s logarithmic)
                graph.add_input_param(
                    node_id,
                    "Decay".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(2.0, 0.1, 30.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Damping: knob-only parameter (0-1)
                graph.add_input_param(
                    node_id,
                    "Damping".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Pre-Delay: knob-only parameter (0-100ms)
                graph.add_input_param(
                    node_id,
                    "Pre-Delay".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, 0.0, 100.0, "ms", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Mix: knob-only parameter (0-1)
                graph.add_input_param(
                    node_id,
                    "Mix".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.3, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Width: knob-only parameter (0-1)
                graph.add_input_param(
                    node_id,
                    "Width".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Output ports
                graph.add_output_param(
                    node_id,
                    "Out L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
                graph.add_output_param(
                    node_id,
                    "Out R".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
            }
            SynthNodeTemplate::ParametricEq => {
                // Audio input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Low shelf controls (knob-only)
                graph.add_input_param(
                    node_id,
                    "Low Freq".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(100.0, 20.0, 500.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );
                graph.add_input_param(
                    node_id,
                    "Low Gain".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -15.0, 15.0, "dB", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Mid parametric controls (knob-only)
                graph.add_input_param(
                    node_id,
                    "Mid Freq".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(1000.0, 100.0, 10000.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );
                graph.add_input_param(
                    node_id,
                    "Mid Gain".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -15.0, 15.0, "dB", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );
                graph.add_input_param(
                    node_id,
                    "Mid Q".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(1.0, 0.1, 10.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // High shelf controls (knob-only)
                graph.add_input_param(
                    node_id,
                    "High Freq".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(8000.0, 2000.0, 20000.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );
                graph.add_input_param(
                    node_id,
                    "High Gain".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -15.0, 15.0, "dB", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Output gain (knob-only)
                graph.add_input_param(
                    node_id,
                    "Output".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -12.0, 12.0, "dB", ""),
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
            SynthNodeTemplate::Distortion => {
                // Audio input port
                graph.add_input_param(
                    node_id,
                    "In".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Drive: exposed parameter (CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Drive".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline
                );

                // Tone: knob-only parameter (brightness)
                graph.add_input_param(
                    node_id,
                    "Tone".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Type: distortion algorithm selector (shown inline)
                graph.add_input_param(
                    node_id,
                    "Type".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        0, // Soft
                        vec!["Soft".to_string(), "Hard".to_string(), "Fold".to_string(), "Bit".to_string()],
                        "Type",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Mix: knob-only parameter (wet/dry)
                graph.add_input_param(
                    node_id,
                    "Mix".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(1.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Output: knob-only parameter (makeup gain)
                graph.add_input_param(
                    node_id,
                    "Output".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -12.0, 12.0, "dB", ""),
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
            SynthNodeTemplate::Chorus => {
                // Left input port
                graph.add_input_param(
                    node_id,
                    "In L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Right input port (normalled from L)
                graph.add_input_param(
                    node_id,
                    "In R".to_string(),
                    SynthDataType::new(SignalType::Audio),
                    SynthValueType::scalar(0.0, ""),
                    InputParamKind::ConnectionOnly,
                    true,
                );

                // Rate: exposed parameter (CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Rate".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::frequency(1.0, 0.1, 10.0, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline
                );

                // Depth: exposed parameter (CV input + knob at bottom)
                graph.add_input_param(
                    node_id,
                    "Depth".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConnectionOrConstant,
                    true, // Port shown inline
                );

                // Delay: knob-only parameter (base delay time)
                graph.add_input_param(
                    node_id,
                    "Delay".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::time(10.0, 1.0, 30.0, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Feedback: knob-only parameter (-0.5 to +0.5)
                graph.add_input_param(
                    node_id,
                    "Feedback".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::linear_range(0.0, -0.5, 0.5, "", ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Voices: selector (shown inline)
                graph.add_input_param(
                    node_id,
                    "Voices".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::select(
                        1, // 2 voices default
                        vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()],
                        "Voices",
                    ),
                    InputParamKind::ConstantOnly,
                    true, // Shown inline as dropdown
                );

                // Mix: knob-only parameter (wet/dry)
                graph.add_input_param(
                    node_id,
                    "Mix".to_string(),
                    SynthDataType::new(SignalType::Control),
                    SynthValueType::scalar(0.5, ""),
                    InputParamKind::ConstantOnly,
                    false, // Hidden inline - shown in bottom knob row
                );

                // Output ports
                graph.add_output_param(
                    node_id,
                    "Out L".to_string(),
                    SynthDataType::new(SignalType::Audio),
                );
                graph.add_output_param(
                    node_id,
                    "Out R".to_string(),
                    SynthDataType::new(SignalType::Audio),
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
        assert_eq!(templates.len(), 19);
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
        assert!(templates.contains(&SynthNodeTemplate::StepSequencer));
        assert!(templates.contains(&SynthNodeTemplate::StereoDelay));
        assert!(templates.contains(&SynthNodeTemplate::Distortion));
        assert!(templates.contains(&SynthNodeTemplate::Reverb));
        assert!(templates.contains(&SynthNodeTemplate::ParametricEq));
        assert!(templates.contains(&SynthNodeTemplate::Chorus));
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
        assert_eq!(SynthNodeTemplate::StepSequencer.module_id(), "seq.step");
        assert_eq!(SynthNodeTemplate::StereoDelay.module_id(), "fx.delay");
        assert_eq!(SynthNodeTemplate::Reverb.module_id(), "fx.reverb");
        assert_eq!(SynthNodeTemplate::ParametricEq.module_id(), "fx.eq");
        assert_eq!(SynthNodeTemplate::Distortion.module_id(), "fx.distortion");
        assert_eq!(SynthNodeTemplate::Chorus.module_id(), "fx.chorus");
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
        assert_eq!(SynthNodeTemplate::StepSequencer.category(), ModuleCategory::Utility);
        assert_eq!(SynthNodeTemplate::StereoDelay.category(), ModuleCategory::Effect);
        assert_eq!(SynthNodeTemplate::Reverb.category(), ModuleCategory::Effect);
        assert_eq!(SynthNodeTemplate::ParametricEq.category(), ModuleCategory::Effect);
        assert_eq!(SynthNodeTemplate::Distortion.category(), ModuleCategory::Effect);
        assert_eq!(SynthNodeTemplate::Chorus.category(), ModuleCategory::Effect);
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
        assert_eq!(
            SynthNodeTemplate::StepSequencer.node_finder_label(&mut state),
            "Step Sequencer"
        );
        assert_eq!(
            SynthNodeTemplate::StereoDelay.node_finder_label(&mut state),
            "Stereo Delay"
        );
        assert_eq!(
            SynthNodeTemplate::Reverb.node_finder_label(&mut state),
            "Reverb"
        );
        assert_eq!(
            SynthNodeTemplate::ParametricEq.node_finder_label(&mut state),
            "3-Band EQ"
        );
        assert_eq!(
            SynthNodeTemplate::Distortion.node_finder_label(&mut state),
            "Distortion"
        );
        assert_eq!(
            SynthNodeTemplate::Chorus.node_finder_label(&mut state),
            "Chorus"
        );
    }
}
