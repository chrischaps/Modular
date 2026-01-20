//! Parameter definitions for DSP modules.
//!
//! Parameters are the controllable values on modules (knobs, sliders, switches).

/// How a parameter value should be displayed and interpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterDisplay {
    /// Linear scaling with a unit suffix (e.g., "Hz", "ms", "%").
    Linear { unit: &'static str },
    /// Logarithmic scaling, common for frequency and gain controls.
    Logarithmic { unit: &'static str },
    /// Discrete steps with named values.
    Discrete { labels: &'static [&'static str] },
    /// On/off toggle switch.
    Toggle {
        off_label: &'static str,
        on_label: &'static str,
    },
}

impl ParameterDisplay {
    /// Creates a linear display with the given unit.
    pub fn linear(unit: &'static str) -> Self {
        Self::Linear { unit }
    }

    /// Creates a logarithmic display with the given unit.
    pub fn logarithmic(unit: &'static str) -> Self {
        Self::Logarithmic { unit }
    }

    /// Creates a discrete display with named steps.
    pub fn discrete(labels: &'static [&'static str]) -> Self {
        Self::Discrete { labels }
    }

    /// Creates a toggle display with custom labels.
    pub fn toggle(off_label: &'static str, on_label: &'static str) -> Self {
        Self::Toggle {
            off_label,
            on_label,
        }
    }

    /// Creates an on/off toggle.
    pub fn on_off() -> Self {
        Self::Toggle {
            off_label: "Off",
            on_label: "On",
        }
    }

    /// Returns the unit string, if applicable.
    pub fn unit(&self) -> Option<&'static str> {
        match self {
            Self::Linear { unit } | Self::Logarithmic { unit } => Some(unit),
            _ => None,
        }
    }

    /// Returns true if this is a logarithmic parameter.
    pub fn is_logarithmic(&self) -> bool {
        matches!(self, Self::Logarithmic { .. })
    }
}

/// Definition of a parameter on a DSP module.
///
/// Parameters represent user-controllable values like knobs and switches.
/// Each parameter has a unique ID, display name, valid range, and default value.
#[derive(Clone, Debug)]
pub struct ParameterDefinition {
    /// Unique identifier for this parameter within the module.
    pub id: &'static str,
    /// Human-readable name displayed in the UI.
    pub name: &'static str,
    /// Minimum value of the parameter.
    pub min: f32,
    /// Maximum value of the parameter.
    pub max: f32,
    /// Default value when the module is created.
    pub default: f32,
    /// How to display and interpret the parameter value.
    pub display: ParameterDisplay,
}

impl ParameterDefinition {
    /// Creates a new parameter definition.
    pub fn new(
        id: &'static str,
        name: &'static str,
        min: f32,
        max: f32,
        default: f32,
        display: ParameterDisplay,
    ) -> Self {
        Self {
            id,
            name,
            min,
            max,
            default,
            display,
        }
    }

    /// Creates a normalized parameter (0.0 to 1.0) with linear display.
    pub fn normalized(id: &'static str, name: &'static str, default: f32) -> Self {
        Self {
            id,
            name,
            min: 0.0,
            max: 1.0,
            default,
            display: ParameterDisplay::linear("%"),
        }
    }

    /// Creates a frequency parameter with logarithmic scaling.
    pub fn frequency(
        id: &'static str,
        name: &'static str,
        min: f32,
        max: f32,
        default: f32,
    ) -> Self {
        Self {
            id,
            name,
            min,
            max,
            default,
            display: ParameterDisplay::logarithmic("Hz"),
        }
    }

    /// Creates a toggle (boolean) parameter.
    pub fn toggle(id: &'static str, name: &'static str, default: bool) -> Self {
        Self {
            id,
            name,
            min: 0.0,
            max: 1.0,
            default: if default { 1.0 } else { 0.0 },
            display: ParameterDisplay::on_off(),
        }
    }

    /// Creates a discrete choice parameter.
    pub fn choice(
        id: &'static str,
        name: &'static str,
        labels: &'static [&'static str],
        default_index: usize,
    ) -> Self {
        Self {
            id,
            name,
            min: 0.0,
            max: (labels.len().saturating_sub(1)) as f32,
            default: default_index as f32,
            display: ParameterDisplay::discrete(labels),
        }
    }

    /// Clamps a value to this parameter's valid range.
    pub fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }

    /// Normalizes a value from the parameter's range to 0.0-1.0.
    pub fn normalize(&self, value: f32) -> f32 {
        if (self.max - self.min).abs() < f32::EPSILON {
            0.0
        } else {
            (value - self.min) / (self.max - self.min)
        }
    }

    /// Denormalizes a 0.0-1.0 value to the parameter's range.
    pub fn denormalize(&self, normalized: f32) -> f32 {
        self.min + normalized * (self.max - self.min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_display_linear() {
        let display = ParameterDisplay::linear("Hz");
        assert_eq!(display.unit(), Some("Hz"));
        assert!(!display.is_logarithmic());
    }

    #[test]
    fn test_parameter_display_logarithmic() {
        let display = ParameterDisplay::logarithmic("dB");
        assert_eq!(display.unit(), Some("dB"));
        assert!(display.is_logarithmic());
    }

    #[test]
    fn test_parameter_display_discrete() {
        let labels: &[&str] = &["Sine", "Square", "Saw"];
        let display = ParameterDisplay::discrete(labels);
        assert_eq!(display.unit(), None);
    }

    #[test]
    fn test_parameter_display_toggle() {
        let display = ParameterDisplay::on_off();
        assert_eq!(display.unit(), None);
    }

    #[test]
    fn test_parameter_clamp() {
        let param = ParameterDefinition::new(
            "test",
            "Test",
            0.0,
            100.0,
            50.0,
            ParameterDisplay::linear(""),
        );
        assert_eq!(param.clamp(-10.0), 0.0);
        assert_eq!(param.clamp(50.0), 50.0);
        assert_eq!(param.clamp(150.0), 100.0);
    }

    #[test]
    fn test_parameter_normalize_denormalize() {
        let param = ParameterDefinition::frequency("freq", "Frequency", 20.0, 20000.0, 440.0);

        let normalized = param.normalize(440.0);
        let denormalized = param.denormalize(normalized);
        assert!((denormalized - 440.0).abs() < 0.001);

        assert_eq!(param.normalize(20.0), 0.0);
        assert_eq!(param.normalize(20000.0), 1.0);
        assert_eq!(param.denormalize(0.0), 20.0);
        assert_eq!(param.denormalize(1.0), 20000.0);
    }

    #[test]
    fn test_normalized_parameter() {
        let param = ParameterDefinition::normalized("mix", "Mix", 0.5);
        assert_eq!(param.min, 0.0);
        assert_eq!(param.max, 1.0);
        assert_eq!(param.default, 0.5);
    }

    #[test]
    fn test_toggle_parameter() {
        let param = ParameterDefinition::toggle("bypass", "Bypass", false);
        assert_eq!(param.default, 0.0);

        let param_on = ParameterDefinition::toggle("bypass", "Bypass", true);
        assert_eq!(param_on.default, 1.0);
    }

    #[test]
    fn test_choice_parameter() {
        let param = ParameterDefinition::choice(
            "waveform",
            "Waveform",
            &["Sine", "Square", "Saw", "Triangle"],
            0,
        );
        assert_eq!(param.min, 0.0);
        assert_eq!(param.max, 3.0);
        assert_eq!(param.default, 0.0);
    }
}
