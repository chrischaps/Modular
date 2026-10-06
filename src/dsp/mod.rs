//! DSP module
//!
//! Core DSP traits and types.
//! Defines the DspModule trait, ports, parameters, and signal types.

pub mod analysis;
pub mod context;
pub mod denormal;
pub mod dynamics;
pub mod module_trait;
pub mod parameter;
pub mod port;
pub mod primitives;
pub mod registry;
pub mod signal;
pub mod smoothed_value;

// Re-export commonly used types
pub use context::{ProcessContext, TransportState};
pub use module_trait::{DspModule, ModuleCategory, ModuleError, ModuleInfo, OutputLevels};
pub use parameter::{ParameterDefinition, ParameterDisplay};
pub use port::{PortDefinition, PortDirection};
pub use registry::{ModuleFactory, ModuleRegistry};
pub use signal::{connected_input, MidiEvent, MidiMessage, SignalBuffer, SignalType};
pub use smoothed_value::SmoothedValue;
