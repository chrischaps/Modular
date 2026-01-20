//! Module registry for managing DSP module types.
//!
//! The registry provides a central catalog of available module types,
//! enabling the node graph to instantiate modules by their ID.

use std::collections::HashMap;

use super::module_trait::{DspModule, ModuleInfo};

/// Factory function type for creating module instances.
///
/// Returns a boxed trait object for type erasure.
pub type ModuleFactory = fn() -> Box<dyn DspModule>;

/// Central registry of available DSP module types.
///
/// The registry stores factory functions and module information,
/// allowing modules to be instantiated by their string ID.
///
/// # Example
///
/// ```ignore
/// let mut registry = ModuleRegistry::new();
/// registry.register::<SineOscillator>();
/// registry.register::<AudioOutput>();
///
/// // Later, create instances by ID
/// if let Some(osc) = registry.create("osc.sine") {
///     // Use the oscillator...
/// }
/// ```
pub struct ModuleRegistry {
    /// Map of module ID to factory function.
    factories: HashMap<&'static str, ModuleFactory>,
    /// Cached module information for listing.
    infos: Vec<ModuleInfo>,
}

impl ModuleRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            infos: Vec::new(),
        }
    }

    /// Registers a module type with the registry.
    ///
    /// The module type must implement `DspModule` and `Default`.
    /// A temporary instance is created to extract the module's info,
    /// which is then stored along with a factory function.
    ///
    /// # Type Parameters
    ///
    /// * `M` - The module type to register
    ///
    /// # Panics
    ///
    /// Panics if a module with the same ID is already registered.
    ///
    /// # Example
    ///
    /// ```ignore
    /// registry.register::<SineOscillator>();
    /// ```
    pub fn register<M: DspModule + Default + 'static>(&mut self) {
        // Create a temporary instance to get module info
        let temp = M::default();
        let info = temp.info().clone();
        let id = info.id;

        // Check for duplicate registration
        if self.factories.contains_key(id) {
            panic!("Module '{}' is already registered", id);
        }

        // Store factory function and info
        self.factories.insert(id, create_module::<M>);
        self.infos.push(info);
    }

    /// Creates a new instance of a module by its ID.
    ///
    /// Returns `None` if no module with the given ID is registered.
    ///
    /// # Arguments
    ///
    /// * `id` - The unique identifier of the module type
    ///
    /// # Example
    ///
    /// ```ignore
    /// let module = registry.create("osc.sine");
    /// assert!(module.is_some());
    /// ```
    pub fn create(&self, id: &str) -> Option<Box<dyn DspModule>> {
        self.factories.get(id).map(|factory| factory())
    }

    /// Returns a list of all registered module types.
    ///
    /// The returned slice contains module info for each registered type,
    /// useful for displaying available modules in the UI.
    pub fn list_modules(&self) -> &[ModuleInfo] {
        &self.infos
    }

    /// Returns the number of registered modules.
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    /// Returns true if no modules are registered.
    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }

    /// Checks if a module with the given ID is registered.
    pub fn contains(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal factory function for creating module instances.
///
/// This is a generic function that can be stored as a function pointer.
fn create_module<M: DspModule + Default + 'static>() -> Box<dyn DspModule> {
    Box::new(M::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{
        ParameterDefinition, PortDefinition, ProcessContext, SignalBuffer, SignalType,
    };
    use crate::dsp::module_trait::ModuleCategory;

    /// A simple test module for registry testing.
    struct TestOscillator {
        ports: Vec<PortDefinition>,
        parameters: Vec<ParameterDefinition>,
    }

    impl Default for TestOscillator {
        fn default() -> Self {
            Self {
                ports: vec![PortDefinition::output("out", "Output", SignalType::Audio)],
                parameters: vec![ParameterDefinition::frequency(
                    "freq",
                    "Frequency",
                    20.0,
                    20000.0,
                    440.0,
                )],
            }
        }
    }

    impl DspModule for TestOscillator {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.oscillator",
                name: "Test Oscillator",
                category: ModuleCategory::Source,
                description: "A test oscillator for registry testing",
            };
            &INFO
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
            _inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            // Fill with silence for testing
            for sample in outputs[0].samples.iter_mut() {
                *sample = 0.0;
            }
        }

        fn reset(&mut self) {}
    }

    /// Another test module to verify multiple registrations.
    struct TestFilter {
        ports: Vec<PortDefinition>,
        parameters: Vec<ParameterDefinition>,
    }

    impl Default for TestFilter {
        fn default() -> Self {
            Self {
                ports: vec![
                    PortDefinition::input("in", "Input", SignalType::Audio),
                    PortDefinition::output("out", "Output", SignalType::Audio),
                ],
                parameters: vec![ParameterDefinition::frequency(
                    "cutoff",
                    "Cutoff",
                    20.0,
                    20000.0,
                    1000.0,
                )],
            }
        }
    }

    impl DspModule for TestFilter {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.filter",
                name: "Test Filter",
                category: ModuleCategory::Filter,
                description: "A test filter for registry testing",
            };
            &INFO
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
            // Passthrough for testing
            outputs[0].samples.copy_from_slice(&inputs[0].samples);
        }

        fn reset(&mut self) {}
    }

    #[test]
    fn test_registry_creation() {
        let registry = ModuleRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_register_module() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains("test.oscillator"));
    }

    #[test]
    fn test_register_multiple_modules() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();
        registry.register::<TestFilter>();

        assert_eq!(registry.len(), 2);
        assert!(registry.contains("test.oscillator"));
        assert!(registry.contains("test.filter"));
    }

    #[test]
    fn test_create_module() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();

        let module = registry.create("test.oscillator");
        assert!(module.is_some());

        let module = module.unwrap();
        assert_eq!(module.info().id, "test.oscillator");
        assert_eq!(module.info().name, "Test Oscillator");
        assert_eq!(module.info().category, ModuleCategory::Source);
    }

    #[test]
    fn test_create_unknown_module() {
        let registry = ModuleRegistry::new();
        let module = registry.create("unknown.module");
        assert!(module.is_none());
    }

    #[test]
    fn test_list_modules() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();
        registry.register::<TestFilter>();

        let modules = registry.list_modules();
        assert_eq!(modules.len(), 2);

        // Check that both modules are in the list
        let ids: Vec<&str> = modules.iter().map(|m| m.id).collect();
        assert!(ids.contains(&"test.oscillator"));
        assert!(ids.contains(&"test.filter"));
    }

    #[test]
    fn test_list_modules_empty() {
        let registry = ModuleRegistry::new();
        assert!(registry.list_modules().is_empty());
    }

    #[test]
    fn test_created_module_is_functional() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestFilter>();

        let mut module = registry.create("test.filter").unwrap();

        // Prepare and process
        module.prepare(44100.0, 128);

        let mut input = SignalBuffer::audio(4);
        input.samples.copy_from_slice(&[1.0, 0.5, -0.5, -1.0]);

        let mut outputs = vec![SignalBuffer::audio(4)];
        let ctx = ProcessContext::default();

        module.process(&[&input], &mut outputs, &[1000.0], &ctx);

        // TestFilter is a passthrough, so output should match input
        assert_eq!(outputs[0].samples, input.samples);
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn test_duplicate_registration_panics() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();
        registry.register::<TestOscillator>(); // Should panic
    }

    #[test]
    fn test_default_implementation() {
        let registry = ModuleRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_module_info_preserved() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();

        let modules = registry.list_modules();
        let info = &modules[0];

        assert_eq!(info.id, "test.oscillator");
        assert_eq!(info.name, "Test Oscillator");
        assert_eq!(info.category, ModuleCategory::Source);
        assert_eq!(info.description, "A test oscillator for registry testing");
    }
}
