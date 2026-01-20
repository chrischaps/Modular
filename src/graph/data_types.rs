//! Data types for the node graph.
//!
//! Wraps SignalType to implement egui_node_graph2's DataTypeTrait.
//!
//! # Connection Compatibility
//!
//! The `PartialEq` implementation for `SynthDataType` is customized to support
//! signal type compatibility rules rather than strict equality. This allows the
//! node graph library to show visual feedback for compatible port types during
//! connection dragging.
//!
//! Two types are considered "equal" for connection purposes if they can connect
//! in either direction (symmetric check). This enables:
//! - Audio ↔ Control (interchangeable for modulation)
//! - Gate → Control (on/off modulation)
//! - Same types always compatible
//!
//! MIDI is isolated and only "equals" itself.

use std::borrow::Cow;
use eframe::egui::Color32;
use egui_node_graph2::DataTypeTrait;

use crate::dsp::SignalType;
use super::SynthGraphState;

/// Wrapper around SignalType for the node graph library.
///
/// This implements DataTypeTrait to define how signal types are displayed
/// and how connections are validated in the graph editor.
///
/// # Equality Semantics
///
/// `PartialEq` is implemented to check **connection compatibility** rather than
/// strict type equality. Two `SynthDataType` values are "equal" if a connection
/// could be valid between them in either direction. This allows the graph UI to
/// show compatible ports during dragging.
///
/// For strict type equality, compare the inner `SignalType` values directly:
/// ```ignore
/// if a.signal_type() == b.signal_type() { /* exact match */ }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct SynthDataType(pub SignalType);

impl PartialEq for SynthDataType {
    /// Checks connection compatibility rather than strict equality.
    ///
    /// Returns true if these signal types can be connected in either direction.
    /// This is symmetric: `a == b` implies `b == a`.
    fn eq(&self, other: &Self) -> bool {
        // For the node graph library, "equal" means "can connect".
        // We check both directions since dragging can start from either end.
        self.0.can_connect_to(other.0) || other.0.can_connect_to(self.0)
    }
}

impl Eq for SynthDataType {}

impl SynthDataType {
    /// Create a new SynthDataType from a SignalType.
    pub fn new(signal_type: SignalType) -> Self {
        Self(signal_type)
    }

    /// Get the underlying SignalType.
    pub fn signal_type(&self) -> SignalType {
        self.0
    }
}

impl From<SignalType> for SynthDataType {
    fn from(signal_type: SignalType) -> Self {
        Self(signal_type)
    }
}

impl From<SynthDataType> for SignalType {
    fn from(data_type: SynthDataType) -> Self {
        data_type.0
    }
}

impl DataTypeTrait<SynthGraphState> for SynthDataType {
    /// Returns the color for this data type in the graph editor.
    ///
    /// Uses the colors defined in SignalType::color().
    fn data_type_color(&self, _user_state: &mut SynthGraphState) -> Color32 {
        self.0.color()
    }

    /// Returns the human-readable name for this data type.
    fn name(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.0.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synth_data_type_creation() {
        let audio = SynthDataType::new(SignalType::Audio);
        assert_eq!(audio.signal_type(), SignalType::Audio);

        let control = SynthDataType(SignalType::Control);
        assert_eq!(control.0, SignalType::Control);
    }

    #[test]
    fn test_from_signal_type() {
        let data_type: SynthDataType = SignalType::Gate.into();
        assert_eq!(data_type.signal_type(), SignalType::Gate);
    }

    #[test]
    fn test_into_signal_type() {
        let data_type = SynthDataType::new(SignalType::Midi);
        let signal_type: SignalType = data_type.into();
        assert_eq!(signal_type, SignalType::Midi);
    }

    #[test]
    fn test_data_type_name() {
        assert_eq!(SynthDataType::new(SignalType::Audio).name(), "Audio");
        assert_eq!(SynthDataType::new(SignalType::Control).name(), "Control");
        assert_eq!(SynthDataType::new(SignalType::Gate).name(), "Gate");
        assert_eq!(SynthDataType::new(SignalType::Midi).name(), "MIDI");
    }

    #[test]
    fn test_data_type_colors_are_unique() {
        let mut state = SynthGraphState::default();

        let colors = [
            SynthDataType::new(SignalType::Audio).data_type_color(&mut state),
            SynthDataType::new(SignalType::Control).data_type_color(&mut state),
            SynthDataType::new(SignalType::Gate).data_type_color(&mut state),
            SynthDataType::new(SignalType::Midi).data_type_color(&mut state),
        ];

        for i in 0..colors.len() {
            for j in (i + 1)..colors.len() {
                assert_ne!(colors[i], colors[j], "Data type colors should be unique");
            }
        }
    }

    // Tests for connection compatibility (PartialEq semantics)

    #[test]
    fn test_same_types_are_compatible() {
        let audio1 = SynthDataType::new(SignalType::Audio);
        let audio2 = SynthDataType::new(SignalType::Audio);
        assert_eq!(audio1, audio2);

        let control1 = SynthDataType::new(SignalType::Control);
        let control2 = SynthDataType::new(SignalType::Control);
        assert_eq!(control1, control2);

        let gate1 = SynthDataType::new(SignalType::Gate);
        let gate2 = SynthDataType::new(SignalType::Gate);
        assert_eq!(gate1, gate2);

        let midi1 = SynthDataType::new(SignalType::Midi);
        let midi2 = SynthDataType::new(SignalType::Midi);
        assert_eq!(midi1, midi2);
    }

    #[test]
    fn test_audio_control_compatible() {
        let audio = SynthDataType::new(SignalType::Audio);
        let control = SynthDataType::new(SignalType::Control);

        // Audio and Control are interchangeable
        assert_eq!(audio, control);
        assert_eq!(control, audio);
    }

    #[test]
    fn test_gate_control_compatible() {
        let gate = SynthDataType::new(SignalType::Gate);
        let control = SynthDataType::new(SignalType::Control);

        // Gate can connect to Control (asymmetric, but PartialEq is symmetric)
        assert_eq!(gate, control);
        assert_eq!(control, gate);
    }

    #[test]
    fn test_gate_audio_incompatible() {
        let gate = SynthDataType::new(SignalType::Gate);
        let audio = SynthDataType::new(SignalType::Audio);

        // Gate cannot connect to Audio in either direction
        assert_ne!(gate, audio);
        assert_ne!(audio, gate);
    }

    #[test]
    fn test_midi_isolated() {
        let midi = SynthDataType::new(SignalType::Midi);
        let audio = SynthDataType::new(SignalType::Audio);
        let control = SynthDataType::new(SignalType::Control);
        let gate = SynthDataType::new(SignalType::Gate);

        // MIDI is only compatible with itself
        assert_ne!(midi, audio);
        assert_ne!(midi, control);
        assert_ne!(midi, gate);
        assert_ne!(audio, midi);
        assert_ne!(control, midi);
        assert_ne!(gate, midi);
    }

    #[test]
    fn test_strict_equality_via_signal_type() {
        // For cases where strict equality is needed, compare signal_type()
        let audio = SynthDataType::new(SignalType::Audio);
        let control = SynthDataType::new(SignalType::Control);

        // These are "compatible" (PartialEq)
        assert_eq!(audio, control);

        // But not strictly equal
        assert_ne!(audio.signal_type(), control.signal_type());
    }
}
