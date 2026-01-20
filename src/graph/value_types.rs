//! Value types for node graph parameters.
//!
//! Defines how parameter values are displayed and edited in the graph UI.

use eframe::egui;
use egui_node_graph2::WidgetValueTrait;
use super::{SynthGraphState, SynthNodeData, SynthResponse};

/// Parameter value types for the synthesizer.
///
/// Each variant represents a different kind of parameter with its own
/// UI widget and value handling.
#[derive(Clone, Debug, PartialEq)]
pub enum SynthValueType {
    /// A scalar value in 0.0-1.0 range (normalized parameter).
    Scalar {
        value: f32,
        label: String,
    },
    /// A frequency value in Hz, displayed logarithmically.
    Frequency {
        value: f32,
        min: f32,
        max: f32,
        label: String,
    },
    /// A time value in seconds or milliseconds.
    Time {
        value: f32,
        min: f32,
        max: f32,
        label: String,
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
    /// Create a new scalar parameter.
    pub fn scalar(value: f32, label: impl Into<String>) -> Self {
        Self::Scalar {
            value,
            label: label.into(),
        }
    }

    /// Create a new frequency parameter.
    pub fn frequency(value: f32, min: f32, max: f32, label: impl Into<String>) -> Self {
        Self::Frequency {
            value,
            min,
            max,
            label: label.into(),
        }
    }

    /// Create a new time parameter.
    pub fn time(value: f32, min: f32, max: f32, label: impl Into<String>) -> Self {
        Self::Time {
            value,
            min,
            max,
            label: label.into(),
        }
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

    /// Get the current value as a normalized f32 (0.0-1.0).
    pub fn normalized_value(&self) -> f32 {
        match self {
            Self::Scalar { value, .. } => *value,
            Self::Frequency { value, min, max, .. } => {
                // Logarithmic normalization for frequency
                let log_min = min.ln();
                let log_max = max.ln();
                let log_val = value.ln();
                (log_val - log_min) / (log_max - log_min)
            }
            Self::Time { value, min, max, .. } => {
                // Linear normalization for time
                (value - min) / (max - min)
            }
            Self::Toggle { value, .. } => if *value { 1.0 } else { 0.0 },
            Self::Select { value, options, .. } => {
                if options.is_empty() {
                    0.0
                } else {
                    *value as f32 / (options.len() - 1) as f32
                }
            }
        }
    }
}

impl Default for SynthValueType {
    fn default() -> Self {
        Self::Scalar {
            value: 0.0,
            label: String::new(),
        }
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
        match self {
            Self::Scalar { value, label } => {
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    ui.add(egui::Slider::new(value, 0.0..=1.0).show_value(true));
                });
            }
            Self::Frequency { value, min, max, label } => {
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    ui.add(
                        egui::Slider::new(value, *min..=*max)
                            .logarithmic(true)
                            .suffix(" Hz")
                            .show_value(true),
                    );
                });
            }
            Self::Time { value, min, max, label } => {
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    // Show as ms if under 1 second
                    if *max < 1.0 {
                        let ms = *value * 1000.0;
                        let mut ms_val = ms;
                        ui.add(
                            egui::Slider::new(&mut ms_val, *min * 1000.0..=*max * 1000.0)
                                .suffix(" ms")
                                .show_value(true),
                        );
                        *value = ms_val / 1000.0;
                    } else {
                        ui.add(
                            egui::Slider::new(value, *min..=*max)
                                .suffix(" s")
                                .show_value(true),
                        );
                    }
                });
            }
            Self::Toggle { value, label } => {
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    ui.checkbox(value, "");
                });
            }
            Self::Select { value, options, label } => {
                ui.horizontal(|ui: &mut egui::Ui| {
                    ui.label(if label.is_empty() { param_name } else { label });
                    egui::ComboBox::from_id_salt(param_name)
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
    fn test_scalar_creation() {
        let scalar = SynthValueType::scalar(0.5, "Volume");
        match scalar {
            SynthValueType::Scalar { value, label } => {
                assert!((value - 0.5).abs() < f32::EPSILON);
                assert_eq!(label, "Volume");
            }
            _ => panic!("Expected Scalar variant"),
        }
    }

    #[test]
    fn test_frequency_creation() {
        let freq = SynthValueType::frequency(440.0, 20.0, 20000.0, "Frequency");
        match freq {
            SynthValueType::Frequency { value, min, max, label } => {
                assert!((value - 440.0).abs() < f32::EPSILON);
                assert!((min - 20.0).abs() < f32::EPSILON);
                assert!((max - 20000.0).abs() < f32::EPSILON);
                assert_eq!(label, "Frequency");
            }
            _ => panic!("Expected Frequency variant"),
        }
    }

    #[test]
    fn test_toggle_creation() {
        let toggle = SynthValueType::toggle(true, "Enable");
        match toggle {
            SynthValueType::Toggle { value, label } => {
                assert!(value);
                assert_eq!(label, "Enable");
            }
            _ => panic!("Expected Toggle variant"),
        }
    }

    #[test]
    fn test_select_creation() {
        let select = SynthValueType::select(
            1,
            vec!["Sine".to_string(), "Saw".to_string(), "Square".to_string()],
            "Waveform",
        );
        match select {
            SynthValueType::Select { value, options, label } => {
                assert_eq!(value, 1);
                assert_eq!(options.len(), 3);
                assert_eq!(label, "Waveform");
            }
            _ => panic!("Expected Select variant"),
        }
    }

    #[test]
    fn test_normalized_scalar() {
        let scalar = SynthValueType::scalar(0.75, "Test");
        assert!((scalar.normalized_value() - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn test_normalized_toggle() {
        let on = SynthValueType::toggle(true, "Test");
        let off = SynthValueType::toggle(false, "Test");
        assert!((on.normalized_value() - 1.0).abs() < f32::EPSILON);
        assert!(off.normalized_value().abs() < f32::EPSILON);
    }

    #[test]
    fn test_default() {
        let default = SynthValueType::default();
        match default {
            SynthValueType::Scalar { value, label } => {
                assert!(value.abs() < f32::EPSILON);
                assert!(label.is_empty());
            }
            _ => panic!("Expected Scalar default"),
        }
    }
}
