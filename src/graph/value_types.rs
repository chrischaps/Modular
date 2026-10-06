//! Value types for node graph parameters.
//!
//! Defines how parameter values are displayed and edited in the graph UI.
//! Values are built from each module's `ParameterDefinition`, so the editor
//! and the DSP agree on range, default, unit and curve.

use eframe::egui;
use egui_node_graph2::WidgetValueTrait;
use crate::dsp::{ParameterDefinition, ParameterDisplay};
use crate::widgets::ParamFormat;
use super::{SynthGraphState, SynthNodeData, SynthResponse};

/// Range, default, unit and curve of a continuous parameter.
///
/// Copied from the module's `ParameterDefinition`, so values are real units
/// (Hz, seconds, dB, ...) on both sides of the UI → engine boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberSpec {
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
}

impl NumberSpec {
    /// Builds the spec for a continuous parameter definition.
    pub fn from_definition(def: &ParameterDefinition) -> Self {
        Self {
            min: def.min,
            max: def.max,
            default: def.default,
            unit: def.display.unit().unwrap_or(""),
            // A log curve needs a positive range
            logarithmic: def.display.is_logarithmic() && def.min > 0.0,
        }
    }

    /// How a knob should print values of this parameter.
    pub fn format(&self) -> ParamFormat {
        let unit_interval = self.min == 0.0 && self.max == 1.0;
        match self.unit {
            "Hz" => ParamFormat::Frequency,
            "s" => ParamFormat::Time,
            "ms" => ParamFormat::Milliseconds,
            "dB" => ParamFormat::Decibels,
            "st" => ParamFormat::Semitones,
            "" | "%" if unit_interval => ParamFormat::Percent,
            "%" | "BPM" => ParamFormat::RawWithUnit { decimals: 0, unit: self.unit },
            "" => ParamFormat::Raw { decimals: if self.logarithmic { 2 } else { 1 } },
            unit => ParamFormat::RawWithUnit { decimals: 1, unit },
        }
    }
}

/// Parameter value types for the synthesizer.
///
/// Each variant represents a different kind of input with its own
/// UI widget and value handling.
#[derive(Clone, Debug, PartialEq)]
pub enum SynthValueType {
    /// An input that only takes a cable and has no value of its own.
    Port,
    /// A continuous value in real units, shown as a knob.
    Number {
        value: f32,
        spec: NumberSpec,
    },
    /// A boolean toggle.
    Toggle {
        value: bool,
        label: String,
    },
    /// A discrete selection from a list of options.
    Select {
        value: usize,
        options: Vec<String>,
        label: String,
    },
}

impl SynthValueType {
    /// Builds the editor value for a parameter, starting at its default.
    ///
    /// `label` is the short label shown next to toggles and dropdowns.
    pub fn from_definition(def: &ParameterDefinition, label: impl Into<String>) -> Self {
        match def.display {
            ParameterDisplay::Toggle { .. } => Self::Toggle {
                value: def.default > 0.5,
                label: label.into(),
            },
            ParameterDisplay::Discrete { labels } => Self::Select {
                value: def.default.round().max(0.0) as usize,
                options: labels.iter().map(|s| s.to_string()).collect(),
                label: label.into(),
            },
            ParameterDisplay::Linear { .. } | ParameterDisplay::Logarithmic { .. } => Self::Number {
                value: def.default,
                spec: NumberSpec::from_definition(def),
            },
        }
    }

    /// Create a continuous value with the given spec.
    pub fn number(value: f32, spec: NumberSpec) -> Self {
        Self::Number { value, spec }
    }

    /// Create a new toggle parameter.
    pub fn toggle(value: bool, label: impl Into<String>) -> Self {
        Self::Toggle {
            value,
            label: label.into(),
        }
    }

    /// Create a new select parameter.
    pub fn select(value: usize, options: Vec<String>, label: impl Into<String>) -> Self {
        Self::Select {
            value,
            options,
            label: label.into(),
        }
    }

    /// The value in real units, as the engine receives it:
    /// Hz, seconds, dB, ... for numbers; 0.0/1.0 for toggles; index for selects.
    pub fn actual_value(&self) -> f32 {
        match self {
            Self::Port => 0.0,
            Self::Number { value, .. } => *value,
            Self::Toggle { value, .. } => if *value { 1.0 } else { 0.0 },
            Self::Select { value, .. } => *value as f32,
        }
    }

    /// Set the value in real units, clamped to the valid range.
    pub fn set_actual_value(&mut self, new_value: f32) {
        match self {
            Self::Port => {}
            Self::Number { value, spec } => *value = new_value.clamp(spec.min, spec.max),
            Self::Toggle { value, .. } => *value = new_value > 0.5,
            Self::Select { value, options, .. } => {
                *value = (new_value.max(0.0) as usize).min(options.len().saturating_sub(1));
            }
        }
    }

    /// The (min, max) range of the value in real units.
    pub fn range(&self) -> (f32, f32) {
        match self {
            Self::Port => (0.0, 1.0),
            Self::Number { spec, .. } => (spec.min, spec.max),
            Self::Toggle { .. } => (0.0, 1.0),
            Self::Select { options, .. } => (0.0, options.len().saturating_sub(1) as f32),
        }
    }
}

impl Default for SynthValueType {
    fn default() -> Self {
        Self::Port
    }
}

impl WidgetValueTrait for SynthValueType {
    type Response = SynthResponse;
    type UserState = SynthGraphState;
    type NodeData = SynthNodeData;

    fn value_widget(
        &mut self,
        param_name: &str,
        _node_id: egui_node_graph2::NodeId,
        ui: &mut egui::Ui,
        _user_state: &mut Self::UserState,
        _node_data: &Self::NodeData,
    ) -> Vec<Self::Response> {
        // Design Philosophy: Inputs vs Knobs
        // ==================================
        // - Inputs (left side ports): Connection points for external signals. No inline widgets.
        // - Knobs (bottom section): Manual user controls for parameter values.
        // - Some parameters are "exposed": they have both an input AND a knob.
        //   When disconnected, the knob controls the value. When connected, the
        //   external signal takes over and the knob becomes a read-only display.
        //
        // Therefore, inline widgets for inputs should be minimal - just labels for
        // most types. Only Toggle and Select get inline widgets since they're not
        // suitable for knobs.
        match self {
            Self::Port | Self::Number { .. } => {
                ui.label(param_name);
            }
            Self::Toggle { value, label } => {
                // Toggle gets an inline checkbox - not suitable for knob
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    ui.add_space(4.0);
                    ui.checkbox(value, "");
                });
            }
            Self::Select { value, options, label } => {
                // Select gets an inline ComboBox - discrete choices need dropdown
                let zoom = _user_state.zoom;
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    egui::ComboBox::from_id_salt(param_name)
                        .width(60.0 * zoom)
                        .selected_text(options.get(*value).map(|s| s.as_str()).unwrap_or(""))
                        .show_ui(ui, |ui: &mut egui::Ui| {
                            for (i, option) in options.iter().enumerate() {
                                ui.selectable_value(value, i, option);
                            }
                        });
                });
            }
        }

        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_from_definition_keeps_range_default_and_curve() {
        let def = ParameterDefinition::frequency("cutoff", "Cutoff", 20.0, 20000.0, 1000.0);
        let value = SynthValueType::from_definition(&def, "");
        assert_eq!(
            value,
            SynthValueType::Number {
                value: 1000.0,
                spec: NumberSpec { min: 20.0, max: 20000.0, default: 1000.0, unit: "Hz", logarithmic: true },
            }
        );
    }

    #[test]
    fn test_toggle_and_select_from_definition() {
        let toggle = SynthValueType::from_definition(&ParameterDefinition::toggle("run", "Run", true), "Run");
        assert_eq!(toggle, SynthValueType::toggle(true, "Run"));

        let def = ParameterDefinition::choice("wave", "Waveform", &["Sine", "Saw"], 1);
        let select = SynthValueType::from_definition(&def, "Wave");
        assert_eq!(select, SynthValueType::select(1, vec!["Sine".into(), "Saw".into()], "Wave"));
    }

    #[test]
    fn test_set_actual_value_clamps_to_definition_range() {
        let def = ParameterDefinition::new("drive", "Drive", 1.0, 10.0, 1.0, ParameterDisplay::linear("x"));
        let mut value = SynthValueType::from_definition(&def, "");
        value.set_actual_value(0.5);
        assert_eq!(value.actual_value(), 1.0);
        value.set_actual_value(4.0);
        assert_eq!(value.actual_value(), 4.0);
        value.set_actual_value(50.0);
        assert_eq!(value.actual_value(), 10.0);
    }

    #[test]
    fn test_log_curve_needs_positive_minimum() {
        let def = ParameterDefinition::new("x", "X", 0.0, 1.0, 0.5, ParameterDisplay::logarithmic(""));
        assert!(!NumberSpec::from_definition(&def).logarithmic);
    }

    #[test]
    fn test_format_follows_unit() {
        let spec = |min, max, unit| NumberSpec { min, max, default: min, unit, logarithmic: false };
        assert_eq!(spec(1.0, 2000.0, "ms").format().format(500.0), "500 ms");
        assert_eq!(spec(0.001, 10.0, "s").format().format(0.25), "250 ms");
        assert_eq!(spec(0.0, 1.0, "").format().format(0.5), "50%");
        assert_eq!(spec(1.0, 99.0, "%").format().format(50.0), "50 %");
        assert_eq!(spec(1.0, 10.0, "x").format().format(2.0), "2.0 x");
    }

    #[test]
    fn test_toggle_and_select_values() {
        let mut select = SynthValueType::select(0, vec!["A".into(), "B".into()], "");
        select.set_actual_value(7.0);
        assert_eq!(select.actual_value(), 1.0);
        assert_eq!(select.range(), (0.0, 1.0));

        let mut toggle = SynthValueType::toggle(false, "");
        toggle.set_actual_value(1.0);
        assert_eq!(toggle.actual_value(), 1.0);
    }

    #[test]
    fn test_default_is_port() {
        assert_eq!(SynthValueType::default(), SynthValueType::Port);
    }
}
