//! Port definitions for DSP modules.
//!
//! Ports are the connection points on modules where signals flow in and out.

use super::SignalType;

/// Direction of a port on a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PortDirection {
    /// An input port that receives signals.
    Input,
    /// An output port that sends signals.
    Output,
}

impl PortDirection {
    /// Returns a human-readable name for the port direction.
    pub fn name(&self) -> &'static str {
        match self {
            PortDirection::Input => "Input",
            PortDirection::Output => "Output",
        }
    }
}

/// Definition of a port on a DSP module.
///
/// Each port has a unique ID within the module, a display name,
/// a direction (input/output), and a signal type.
#[derive(Clone, Debug)]
pub struct PortDefinition {
    /// Unique identifier for this port within the module.
    pub id: &'static str,
    /// Human-readable name displayed in the UI.
    pub name: &'static str,
    /// Whether this is an input or output port.
    pub direction: PortDirection,
    /// The type of signal this port accepts or produces.
    pub signal_type: SignalType,
    /// Default value when no connection is made (for inputs only).
    /// For outputs, this is ignored.
    pub default_value: f32,
}

impl PortDefinition {
    /// Creates a new input port definition.
    pub fn input(id: &'static str, name: &'static str, signal_type: SignalType) -> Self {
        Self {
            id,
            name,
            direction: PortDirection::Input,
            signal_type,
            default_value: 0.0,
        }
    }

    /// Creates a new input port definition with a custom default value.
    pub fn input_with_default(
        id: &'static str,
        name: &'static str,
        signal_type: SignalType,
        default_value: f32,
    ) -> Self {
        Self {
            id,
            name,
            direction: PortDirection::Input,
            signal_type,
            default_value,
        }
    }

    /// Creates a new output port definition.
    pub fn output(id: &'static str, name: &'static str, signal_type: SignalType) -> Self {
        Self {
            id,
            name,
            direction: PortDirection::Output,
            signal_type,
            default_value: 0.0,
        }
    }

    /// Returns true if this is an input port.
    pub fn is_input(&self) -> bool {
        self.direction == PortDirection::Input
    }

    /// Returns true if this is an output port.
    pub fn is_output(&self) -> bool {
        self.direction == PortDirection::Output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_direction_names() {
        assert_eq!(PortDirection::Input.name(), "Input");
        assert_eq!(PortDirection::Output.name(), "Output");
    }

    #[test]
    fn test_input_port_creation() {
        let port = PortDefinition::input("audio_in", "Audio In", SignalType::Audio);
        assert_eq!(port.id, "audio_in");
        assert_eq!(port.name, "Audio In");
        assert!(port.is_input());
        assert!(!port.is_output());
        assert_eq!(port.signal_type, SignalType::Audio);
        assert_eq!(port.default_value, 0.0);
    }

    #[test]
    fn test_input_port_with_default() {
        let port =
            PortDefinition::input_with_default("freq", "Frequency", SignalType::Control, 0.5);
        assert_eq!(port.default_value, 0.5);
        assert!(port.is_input());
    }

    #[test]
    fn test_output_port_creation() {
        let port = PortDefinition::output("audio_out", "Audio Out", SignalType::Audio);
        assert_eq!(port.id, "audio_out");
        assert!(port.is_output());
        assert!(!port.is_input());
    }
}
