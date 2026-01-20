//! DSP module
//!
//! Core DSP traits and types.
//! Defines the DspModule trait, ports, parameters, and signal types.

pub mod context;
pub mod module_trait;
pub mod parameter;
pub mod port;
pub mod registry;
pub mod signal;

// Re-export commonly used types
pub use context::{ProcessContext, TransportState};
pub use module_trait::{DspModule, ModuleCategory, ModuleError, ModuleInfo};
pub use parameter::{ParameterDefinition, ParameterDisplay};
pub use port::{PortDefinition, PortDirection};
pub use registry::{ModuleFactory, ModuleRegistry};
pub use signal::{MidiEvent, MidiMessage, SignalBuffer, SignalType};
