//! What the DSP modules declare about themselves, gathered once for the editor.
//!
//! Node templates are generated from these specs, so the editor's ports,
//! parameters, ranges and defaults can't drift from the `DspModule` that
//! actually processes the audio.

use std::sync::OnceLock;

use crate::dsp::{ModuleInfo, ParameterDefinition, PortDefinition};
use crate::engine::create_module_registry;

/// A registered module's identity, ports and parameters.
#[derive(Debug)]
pub struct ModuleSpec {
    pub info: ModuleInfo,
    pub ports: Vec<PortDefinition>,
    pub parameters: Vec<ParameterDefinition>,
}

impl ModuleSpec {
    /// Input ports, in port-index order.
    pub fn inputs(&self) -> impl Iterator<Item = &PortDefinition> {
        self.ports.iter().filter(|p| p.is_input())
    }

    /// Output ports, in port-index order.
    pub fn outputs(&self) -> impl Iterator<Item = &PortDefinition> {
        self.ports.iter().filter(|p| p.is_output())
    }

    /// Index of the parameter that an input port carries CV for, if any.
    ///
    /// A port pairs with the parameter of the same name ("Cutoff") or with
    /// that name plus " CV" ("Time CV" → "Time"). A paired port and parameter
    /// become one editor input with both a jack and a knob.
    pub fn paired_parameter(&self, port: &PortDefinition) -> Option<usize> {
        if !port.is_input() {
            return None;
        }
        let name = port.name.strip_suffix(" CV").unwrap_or(port.name);
        self.parameters.iter().position(|p| p.name == name)
    }

    /// Whether some input port carries CV for this parameter.
    pub fn has_cv_input(&self, param_index: usize) -> bool {
        self.inputs().any(|port| self.paired_parameter(port) == Some(param_index))
    }
}

/// Every registered module, in registration (menu) order.
pub fn modules() -> &'static [ModuleSpec] {
    static CATALOG: OnceLock<Vec<ModuleSpec>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let registry = create_module_registry();
        registry
            .list_modules()
            .iter()
            .map(|info| {
                let module = registry.create(info.id).expect("registered module");
                ModuleSpec {
                    info: info.clone(),
                    ports: module.ports().to_vec(),
                    parameters: module.parameters().to_vec(),
                }
            })
            .collect()
    })
}

/// The spec for a module ID, if it's registered.
pub fn module(module_id: &str) -> Option<&'static ModuleSpec> {
    modules().iter().find(|spec| spec.info.id == module_id)
}
