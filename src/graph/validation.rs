//! Connection validation for the node graph.
//!
//! Implements signal type compatibility checking to ensure only valid
//! connections can be made between ports.

use crate::dsp::SignalType;
use super::SynthDataType;

/// Result of a connection validation check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationResult {
    /// Connection is valid.
    Valid,
    /// Connection is invalid with a reason.
    Invalid(ConnectionError),
}

impl ValidationResult {
    /// Returns true if the connection is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    /// Returns the error message if invalid, None if valid.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            ValidationResult::Valid => None,
            ValidationResult::Invalid(err) => Some(err.message()),
        }
    }
}

/// Errors that can occur when attempting to connect ports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConnectionError {
    /// Attempting to connect incompatible signal types.
    IncompatibleTypes {
        from_type: SignalType,
        to_type: SignalType,
    },
    /// Attempting to connect a node to itself.
    SelfConnection,
    /// Connection already exists.
    DuplicateConnection,
}

impl ConnectionError {
    /// Returns a human-readable error message.
    pub fn message(&self) -> &'static str {
        match self {
            ConnectionError::IncompatibleTypes { from_type, to_type } => {
                match (from_type, to_type) {
                    (SignalType::Midi, _) => "MIDI requires a converter module",
                    (_, SignalType::Midi) => "MIDI requires a converter module",
                    (SignalType::Gate, SignalType::Audio) => "Gate cannot connect directly to Audio",
                    (SignalType::Audio, SignalType::Gate) => "Audio cannot connect to Gate",
                    (SignalType::Control, SignalType::Gate) => "Control cannot connect to Gate",
                    _ => "Incompatible signal types",
                }
            }
            ConnectionError::SelfConnection => "Cannot connect a node to itself",
            ConnectionError::DuplicateConnection => "Connection already exists",
        }
    }
}

/// Validates whether a connection between two signal types is allowed.
///
/// # Connection Rules
///
/// | From → To      | Allowed | Reason                        |
/// |----------------|---------|-------------------------------|
/// | Audio → Audio  | ✓       | Same type                     |
/// | Audio → Control| ✓       | Audio-rate modulation         |
/// | Control → Audio| ✓       | LFO to audio mixer            |
/// | Control → Control| ✓     | Same type                     |
/// | Gate → Control | ✓       | On/off modulation             |
/// | Gate → Gate    | ✓       | Same type                     |
/// | MIDI → MIDI    | ✓       | Same type                     |
/// | MIDI → Others  | ✗       | Needs converter module        |
/// | Others → MIDI  | ✗       | Needs converter module        |
/// | Gate → Audio   | ✗       | Gate needs envelope/converter |
/// | Audio → Gate   | ✗       | Needs comparator module       |
/// | Control → Gate | ✗       | Needs comparator module       |
///
pub fn validate_connection(
    from_type: SignalType,
    to_type: SignalType,
) -> ValidationResult {
    if from_type.can_connect_to(to_type) {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(ConnectionError::IncompatibleTypes {
            from_type,
            to_type,
        })
    }
}

/// Checks if two data types are compatible for connection (symmetric check).
///
/// This is used for visual feedback during connection dragging, where we don't
/// know which direction the connection will be made. Returns true if a connection
/// could be valid in either direction.
pub fn types_compatible(a: &SynthDataType, b: &SynthDataType) -> bool {
    a.0.can_connect_to(b.0) || b.0.can_connect_to(a.0)
}

/// Checks if two data types are exactly equal.
///
/// This is the strict equality check, useful for cases where we need exact
/// type matching rather than compatibility.
pub fn types_equal(a: &SynthDataType, b: &SynthDataType) -> bool {
    a.0 == b.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_type_connections() {
        assert!(validate_connection(SignalType::Audio, SignalType::Audio).is_valid());
        assert!(validate_connection(SignalType::Control, SignalType::Control).is_valid());
        assert!(validate_connection(SignalType::Gate, SignalType::Gate).is_valid());
        assert!(validate_connection(SignalType::Midi, SignalType::Midi).is_valid());
    }

    #[test]
    fn test_audio_control_bidirectional() {
        // Audio and Control can connect in both directions
        assert!(validate_connection(SignalType::Audio, SignalType::Control).is_valid());
        assert!(validate_connection(SignalType::Control, SignalType::Audio).is_valid());
    }

    #[test]
    fn test_gate_to_control() {
        // Gate can feed into Control (on/off modulation)
        assert!(validate_connection(SignalType::Gate, SignalType::Control).is_valid());
    }

    #[test]
    fn test_control_to_gate_invalid() {
        // Control cannot feed into Gate (needs comparator)
        let result = validate_connection(SignalType::Control, SignalType::Gate);
        assert!(!result.is_valid());
        assert!(result.error_message().is_some());
    }

    #[test]
    fn test_gate_to_audio_invalid() {
        // Gate cannot connect directly to Audio
        let result = validate_connection(SignalType::Gate, SignalType::Audio);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_audio_to_gate_invalid() {
        // Audio cannot connect to Gate
        let result = validate_connection(SignalType::Audio, SignalType::Gate);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_midi_isolation() {
        // MIDI can only connect to MIDI
        assert!(!validate_connection(SignalType::Midi, SignalType::Audio).is_valid());
        assert!(!validate_connection(SignalType::Midi, SignalType::Control).is_valid());
        assert!(!validate_connection(SignalType::Midi, SignalType::Gate).is_valid());

        assert!(!validate_connection(SignalType::Audio, SignalType::Midi).is_valid());
        assert!(!validate_connection(SignalType::Control, SignalType::Midi).is_valid());
        assert!(!validate_connection(SignalType::Gate, SignalType::Midi).is_valid());
    }

    #[test]
    fn test_types_compatible_symmetric() {
        let audio = SynthDataType::new(SignalType::Audio);
        let control = SynthDataType::new(SignalType::Control);
        let gate = SynthDataType::new(SignalType::Gate);
        let midi = SynthDataType::new(SignalType::Midi);

        // Audio and Control are compatible (both directions work)
        assert!(types_compatible(&audio, &control));
        assert!(types_compatible(&control, &audio));

        // Gate and Control are compatible (Gate→Control works)
        assert!(types_compatible(&gate, &control));
        assert!(types_compatible(&control, &gate));

        // Same types are always compatible
        assert!(types_compatible(&audio, &audio));
        assert!(types_compatible(&control, &control));
        assert!(types_compatible(&gate, &gate));
        assert!(types_compatible(&midi, &midi));

        // MIDI is not compatible with others
        assert!(!types_compatible(&midi, &audio));
        assert!(!types_compatible(&midi, &control));
        assert!(!types_compatible(&midi, &gate));

        // Gate and Audio are not compatible (neither direction works)
        assert!(!types_compatible(&gate, &audio));
        assert!(!types_compatible(&audio, &gate));
    }

    #[test]
    fn test_validation_result_methods() {
        let valid = ValidationResult::Valid;
        assert!(valid.is_valid());
        assert!(valid.error_message().is_none());

        let invalid = ValidationResult::Invalid(ConnectionError::SelfConnection);
        assert!(!invalid.is_valid());
        assert!(invalid.error_message().is_some());
    }

    #[test]
    fn test_connection_error_messages() {
        assert_eq!(
            ConnectionError::SelfConnection.message(),
            "Cannot connect a node to itself"
        );
        assert_eq!(
            ConnectionError::DuplicateConnection.message(),
            "Connection already exists"
        );

        // MIDI errors
        let midi_to_audio = ConnectionError::IncompatibleTypes {
            from_type: SignalType::Midi,
            to_type: SignalType::Audio,
        };
        assert!(midi_to_audio.message().contains("MIDI"));

        // Gate to Audio error
        let gate_to_audio = ConnectionError::IncompatibleTypes {
            from_type: SignalType::Gate,
            to_type: SignalType::Audio,
        };
        assert!(gate_to_audio.message().contains("Gate"));
    }
}
