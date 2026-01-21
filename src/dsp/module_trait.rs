//! The core DspModule trait and supporting types.
//!
//! This module defines the interface that all synthesizer modules must implement,
//! enabling both built-in and user-created modules to work within the audio graph.

use super::context::ProcessContext;
use super::parameter::ParameterDefinition;
use super::port::PortDefinition;
use super::SignalBuffer;
use egui::Color32;
use egui_node_graph2::CategoryTrait;
use std::fmt;

/// Category of a DSP module, used for organization and UI coloring.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModuleCategory {
    /// Sound sources (oscillators, noise generators, samplers).
    Source,
    /// Frequency-shaping modules (filters, EQs).
    Filter,
    /// Modulation sources (envelopes, LFOs).
    Modulation,
    /// Audio effects (delay, reverb, distortion).
    Effect,
    /// Utility modules (mixers, VCAs, math operations).
    Utility,
    /// Output modules (master output, scope, spectrum analyzer).
    Output,
}

impl ModuleCategory {
    /// Returns the color associated with this module category.
    ///
    /// These colors match the concept image aesthetic:
    /// - Source: Blue (#42A5F5) - oscillators, noise generators
    /// - Filter: Teal (#26A69A) - filters, EQs
    /// - Modulation: Orange (#FFB74D) - envelopes, LFOs
    /// - Effect: Cyan (#4DD0E1) - delay, reverb
    /// - Utility: Gray (#9E9E9E) - mixers, VCAs
    /// - Output: Purple (#7E57C2) - master output
    pub fn color(&self) -> Color32 {
        match self {
            ModuleCategory::Source => Color32::from_rgb(66, 165, 245),    // Blue #42A5F5
            ModuleCategory::Filter => Color32::from_rgb(38, 166, 154),    // Teal #26A69A
            ModuleCategory::Modulation => Color32::from_rgb(255, 183, 77), // Orange #FFB74D
            ModuleCategory::Effect => Color32::from_rgb(77, 208, 225),    // Cyan #4DD0E1
            ModuleCategory::Utility => Color32::from_rgb(158, 158, 158),  // Gray #9E9E9E
            ModuleCategory::Output => Color32::from_rgb(126, 87, 194),    // Purple #7E57C2
        }
    }

    /// Returns a human-readable name for the category.
    pub fn name(&self) -> &'static str {
        match self {
            ModuleCategory::Source => "Source",
            ModuleCategory::Filter => "Filter",
            ModuleCategory::Modulation => "Modulation",
            ModuleCategory::Effect => "Effect",
            ModuleCategory::Utility => "Utility",
            ModuleCategory::Output => "Output",
        }
    }
}

impl CategoryTrait for ModuleCategory {
    fn name(&self) -> String {
        self.name().to_string()
    }
}

/// Static information about a DSP module.
///
/// This describes the module's identity and classification,
/// used for display in the UI and module registry.
#[derive(Clone, Debug)]
pub struct ModuleInfo {
    /// Unique identifier for the module type (e.g., "sine_osc", "svf_filter").
    pub id: &'static str,
    /// Human-readable name (e.g., "Sine Oscillator", "SVF Filter").
    pub name: &'static str,
    /// The category this module belongs to.
    pub category: ModuleCategory,
    /// A brief description of what the module does.
    pub description: &'static str,
}

impl ModuleInfo {
    /// Creates a new module info.
    pub fn new(
        id: &'static str,
        name: &'static str,
        category: ModuleCategory,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            name,
            category,
            description,
        }
    }
}

/// Errors that can occur during module operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModuleError {
    /// Failed to deserialize module state.
    DeserializationFailed(String),
    /// Invalid parameter value.
    InvalidParameter { id: String, reason: String },
    /// Module is not in a valid state.
    InvalidState(String),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleError::DeserializationFailed(msg) => {
                write!(f, "Failed to deserialize module state: {}", msg)
            }
            ModuleError::InvalidParameter { id, reason } => {
                write!(f, "Invalid parameter '{}': {}", id, reason)
            }
            ModuleError::InvalidState(msg) => {
                write!(f, "Invalid module state: {}", msg)
            }
        }
    }
}

impl std::error::Error for ModuleError {}

/// The core trait that all DSP modules must implement.
///
/// This trait defines the interface for audio processing modules in the synthesizer.
/// Modules receive input signals, process them according to their parameters,
/// and produce output signals.
///
/// # Thread Safety
///
/// `DspModule` requires `Send + 'static` because modules may be moved between
/// threads (from the UI thread to the audio thread) after construction.
///
/// # Example
///
/// ```ignore
/// struct GainModule {
///     info: ModuleInfo,
///     ports: Vec<PortDefinition>,
///     parameters: Vec<ParameterDefinition>,
/// }
///
/// impl DspModule for GainModule {
///     fn info(&self) -> &ModuleInfo { &self.info }
///     fn ports(&self) -> &[PortDefinition] { &self.ports }
///     fn parameters(&self) -> &[ParameterDefinition] { &self.parameters }
///
///     fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}
///
///     fn process(
///         &mut self,
///         inputs: &[&SignalBuffer],
///         outputs: &mut [SignalBuffer],
///         params: &[f32],
///         _context: &ProcessContext,
///     ) {
///         let gain = params[0];
///         for (i, sample) in inputs[0].samples.iter().enumerate() {
///             outputs[0].samples[i] = sample * gain;
///         }
///     }
///
///     fn reset(&mut self) {}
/// }
/// ```
pub trait DspModule: Send + 'static {
    /// Returns static information about this module.
    fn info(&self) -> &ModuleInfo;

    /// Returns the port definitions for this module.
    ///
    /// The order of ports determines their indices for the `process` method.
    /// Input ports should come before output ports by convention.
    fn ports(&self) -> &[PortDefinition];

    /// Returns the parameter definitions for this module.
    ///
    /// The order of parameters determines their indices in the `params` slice
    /// passed to the `process` method.
    fn parameters(&self) -> &[ParameterDefinition];

    /// Prepares the module for processing.
    ///
    /// Called before audio processing begins, and whenever the sample rate
    /// or maximum block size changes. Modules should allocate any buffers
    /// and precompute any values that depend on sample rate here.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - The audio sample rate in Hz
    /// * `max_block_size` - The maximum number of samples per process call
    fn prepare(&mut self, sample_rate: f32, max_block_size: usize);

    /// Processes a block of audio.
    ///
    /// This is the main audio processing method. It receives input signals,
    /// current parameter values, and must fill the output buffers.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input signal buffers, indexed by input port order
    /// * `outputs` - Output signal buffers to fill, indexed by output port order
    /// * `params` - Current parameter values, indexed by parameter order
    /// * `context` - Processing context with sample rate, block size, and transport
    ///
    /// # Real-time Constraints
    ///
    /// This method runs on the audio thread and must not:
    /// - Allocate memory
    /// - Acquire locks
    /// - Perform I/O operations
    /// - Call any functions that might block
    fn process(
        &mut self,
        inputs: &[&SignalBuffer],
        outputs: &mut [SignalBuffer],
        params: &[f32],
        context: &ProcessContext,
    );

    /// Resets the module to its initial state.
    ///
    /// Called when playback stops or when the user explicitly resets the module.
    /// Modules should clear any internal state (filters, delay lines, envelopes).
    fn reset(&mut self);

    /// Serializes the module's internal state for saving patches.
    ///
    /// Returns `None` if the module has no state beyond its parameters.
    /// The default implementation returns `None`.
    fn serialize_state(&self) -> Option<Vec<u8>> {
        None
    }

    /// Restores the module's internal state from saved data.
    ///
    /// The default implementation accepts any data and returns `Ok(())`.
    fn deserialize_state(&mut self, _data: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Returns the final audio output for output modules.
    ///
    /// This is used by the audio engine to extract stereo audio from
    /// AudioOutput modules. Returns `None` for non-output modules.
    ///
    /// The returned slices are (left_channel, right_channel).
    fn get_audio_output(&self) -> Option<(&[f32], &[f32])> {
        None
    }

    /// Returns peak levels for metering (for output modules).
    ///
    /// Returns (left_peak, right_peak) in the range 0.0 to 1.0+.
    /// Returns `None` for non-output modules.
    fn get_peak_levels(&self) -> Option<(f32, f32)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{PortDefinition, SignalBuffer, SignalType};

    /// A minimal test module that passes audio through unchanged.
    struct PassthroughModule {
        info: ModuleInfo,
        ports: Vec<PortDefinition>,
        parameters: Vec<ParameterDefinition>,
    }

    impl PassthroughModule {
        fn new() -> Self {
            Self {
                info: ModuleInfo::new(
                    "passthrough",
                    "Passthrough",
                    ModuleCategory::Utility,
                    "Passes audio through unchanged",
                ),
                ports: vec![
                    PortDefinition::input("in", "Input", SignalType::Audio),
                    PortDefinition::output("out", "Output", SignalType::Audio),
                ],
                parameters: vec![],
            }
        }
    }

    impl DspModule for PassthroughModule {
        fn info(&self) -> &ModuleInfo {
            &self.info
        }

        fn ports(&self) -> &[PortDefinition] {
            &self.ports
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &self.parameters
        }

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            outputs[0].samples.copy_from_slice(&inputs[0].samples);
        }

        fn reset(&mut self) {}
    }

    /// A test module with parameters.
    struct GainModule {
        info: ModuleInfo,
        ports: Vec<PortDefinition>,
        parameters: Vec<ParameterDefinition>,
    }

    impl GainModule {
        fn new() -> Self {
            use crate::dsp::ParameterDefinition;

            Self {
                info: ModuleInfo::new(
                    "gain",
                    "Gain",
                    ModuleCategory::Utility,
                    "Adjusts signal amplitude",
                ),
                ports: vec![
                    PortDefinition::input("in", "Input", SignalType::Audio),
                    PortDefinition::output("out", "Output", SignalType::Audio),
                ],
                parameters: vec![ParameterDefinition::normalized("gain", "Gain", 1.0)],
            }
        }
    }

    impl DspModule for GainModule {
        fn info(&self) -> &ModuleInfo {
            &self.info
        }

        fn ports(&self) -> &[PortDefinition] {
            &self.ports
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &self.parameters
        }

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            params: &[f32],
            _context: &ProcessContext,
        ) {
            let gain = params[0];
            for (out, &inp) in outputs[0].samples.iter_mut().zip(inputs[0].samples.iter()) {
                *out = inp * gain;
            }
        }

        fn reset(&mut self) {}
    }

    #[test]
    fn test_module_category_colors() {
        // Verify all categories have distinct colors
        let categories = [
            ModuleCategory::Source,
            ModuleCategory::Filter,
            ModuleCategory::Modulation,
            ModuleCategory::Effect,
            ModuleCategory::Utility,
            ModuleCategory::Output,
        ];

        for i in 0..categories.len() {
            for j in (i + 1)..categories.len() {
                assert_ne!(
                    categories[i].color(),
                    categories[j].color(),
                    "Categories {:?} and {:?} have the same color",
                    categories[i],
                    categories[j]
                );
            }
        }
    }

    #[test]
    fn test_module_category_names() {
        assert_eq!(ModuleCategory::Source.name(), "Source");
        assert_eq!(ModuleCategory::Filter.name(), "Filter");
        assert_eq!(ModuleCategory::Modulation.name(), "Modulation");
        assert_eq!(ModuleCategory::Effect.name(), "Effect");
        assert_eq!(ModuleCategory::Utility.name(), "Utility");
        assert_eq!(ModuleCategory::Output.name(), "Output");
    }

    #[test]
    fn test_module_info_creation() {
        let info = ModuleInfo::new(
            "test_mod",
            "Test Module",
            ModuleCategory::Effect,
            "A test module",
        );
        assert_eq!(info.id, "test_mod");
        assert_eq!(info.name, "Test Module");
        assert_eq!(info.category, ModuleCategory::Effect);
        assert_eq!(info.description, "A test module");
    }

    #[test]
    fn test_module_error_display() {
        let err = ModuleError::DeserializationFailed("invalid data".to_string());
        assert!(err.to_string().contains("invalid data"));

        let err = ModuleError::InvalidParameter {
            id: "freq".to_string(),
            reason: "out of range".to_string(),
        };
        assert!(err.to_string().contains("freq"));
        assert!(err.to_string().contains("out of range"));

        let err = ModuleError::InvalidState("not prepared".to_string());
        assert!(err.to_string().contains("not prepared"));
    }

    #[test]
    fn test_passthrough_module() {
        let mut module = PassthroughModule::new();

        assert_eq!(module.info().id, "passthrough");
        assert_eq!(module.ports().len(), 2);
        assert!(module.parameters().is_empty());

        // Test processing
        module.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(4);
        input.samples.copy_from_slice(&[1.0, 0.5, -0.5, -1.0]);

        let mut output = SignalBuffer::audio(4);
        let ctx = ProcessContext::default();

        module.process(&[&input], &mut [output.clone()], &[], &ctx);

        // For this test we need to manually copy since we can't borrow mutably twice
        for (i, &sample) in input.samples.iter().enumerate() {
            output.samples[i] = sample;
        }

        assert_eq!(output.samples, input.samples);
    }

    #[test]
    fn test_gain_module() {
        let mut module = GainModule::new();

        assert_eq!(module.info().id, "gain");
        assert_eq!(module.info().name, "Gain");
        assert_eq!(module.ports().len(), 2);
        assert_eq!(module.parameters().len(), 1);
        assert_eq!(module.parameters()[0].id, "gain");

        module.prepare(44100.0, 256);

        let mut input = SignalBuffer::audio(4);
        input.samples.copy_from_slice(&[1.0, 0.5, -0.5, -1.0]);

        let mut outputs = vec![SignalBuffer::audio(4)];
        let ctx = ProcessContext::default();

        // Test with gain = 0.5
        module.process(&[&input], &mut outputs, &[0.5], &ctx);

        // Verify output
        let expected = [0.5, 0.25, -0.25, -0.5];
        assert_eq!(outputs[0].samples, expected);
    }

    #[test]
    fn test_default_serialization() {
        let module = PassthroughModule::new();

        // Default implementation returns None
        assert!(module.serialize_state().is_none());
    }

    #[test]
    fn test_default_deserialization() {
        let mut module = PassthroughModule::new();

        // Default implementation accepts any data
        assert!(module.deserialize_state(&[1, 2, 3, 4]).is_ok());
    }

    #[test]
    fn test_module_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PassthroughModule>();
        assert_send::<GainModule>();
    }
}
