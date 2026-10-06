//! Headless patch compilation.
//!
//! Turns a [`Patch`] into the [`EngineCommand`]s that build it in an audio
//! engine, without a UI. The patch is staged with [`stage_patch`], the same
//! loader the editor uses, and ports and parameters are resolved with the
//! shared [`port_mapping`] helpers, so the indices always match what the
//! editor sends.

use std::collections::HashMap;

use crate::engine::{EngineCommand, NodeId as EngineNodeId};
use crate::graph::port_mapping;

use super::{stage_patch, Patch, PatchError};

/// The engine commands that build a patch, plus bookkeeping for callers.
#[derive(Debug, Default)]
pub struct CompiledPatch {
    /// Commands in the order they must be applied: modules, parameters, connections.
    pub commands: Vec<EngineCommand>,
    /// Engine node ID assigned to each patch node ID.
    pub node_ids: HashMap<u64, EngineNodeId>,
    /// Problems that were skipped rather than failing the whole patch,
    /// e.g. an unknown module or a connection naming a port that no longer exists.
    pub warnings: Vec<String>,
}

/// Compiles a patch into engine commands.
///
/// Fails only on an incompatible version. Anything else that can't be built
/// is skipped and reported in [`CompiledPatch::warnings`], exactly as when the
/// editor loads the patch.
pub fn compile_patch(patch: &Patch) -> Result<CompiledPatch, PatchError> {
    let staged = stage_patch(patch)?;
    let graph = &staged.graph;
    let mut compiled = CompiledPatch {
        warnings: staged.warnings.clone(),
        ..Default::default()
    };
    let mut engine_ids = HashMap::new();

    for (engine_node_id, node) in (0..).zip(&staged.nodes) {
        engine_ids.insert(node.graph_id, engine_node_id);
        compiled.node_ids.insert(node.patch_id, engine_node_id);

        compiled.commands.push(EngineCommand::AddModule {
            node_id: engine_node_id,
            module_id: node.template.module_id(),
        });

        // Send every parameter the way the editor's initial parameter sync does
        let skip = port_mapping::live_input_parameter_count(node.template.module_id());
        for (param_index, input_id) in port_mapping::parameter_inputs(graph, node.graph_id)
            .into_iter()
            .enumerate()
            .skip(skip)
        {
            compiled.commands.push(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index,
                value: graph.get_input(input_id).value.actual_value(),
            });
        }

        if graph[node.graph_id].user_data.bypassed {
            compiled.commands.push(EngineCommand::SetBypass { node_id: engine_node_id, bypassed: true });
        }
    }

    for (input_id, output_id) in graph.iter_connections() {
        let from = graph.get_output(output_id).node;
        let to = graph.get_input(input_id).node;
        // Staging only keeps connections between ports that map to the engine
        let (Some(from_port), Some(to_port)) = (
            port_mapping::output_port_index(graph, from, output_id),
            port_mapping::input_port_index(graph, to, input_id),
        ) else {
            continue;
        };
        compiled.commands.push(EngineCommand::Connect {
            from_node: engine_ids[&from],
            from_port,
            to_node: engine_ids[&to],
            to_port,
        });
    }

    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{ConnectionData, NamedParameter, NodeData, ParameterValue};

    fn osc_to_output() -> Patch {
        let mut patch = Patch::new("test");
        let mut osc = NodeData::new(10, "osc.sine", (0.0, 0.0));
        osc.parameters = vec![
            NamedParameter::new("Octave", ParameterValue::Number(-2.0)),
            NamedParameter::new("Semitone", ParameterValue::Number(9.0)),
            NamedParameter::new("FM Depth", ParameterValue::Number(0.0)),
            NamedParameter::new("Waveform", ParameterValue::Select(1)),
            NamedParameter::new("Pulse Width", ParameterValue::LinearRange(0.5)),
        ];
        patch.nodes.push(osc);
        patch.nodes.push(NodeData::new(20, "output.audio", (200.0, 0.0)));
        patch.connections.push(ConnectionData::new(10, "Out", 20, "Mono"));
        patch
    }

    #[test]
    fn test_compiles_modules_params_and_connections() {
        let compiled = compile_patch(&osc_to_output()).unwrap();
        assert!(compiled.warnings.is_empty(), "{:?}", compiled.warnings);

        let osc = compiled.node_ids[&10];
        let out = compiled.node_ids[&20];

        assert!(compiled.commands.iter().any(|c| matches!(c,
            EngineCommand::AddModule { node_id, module_id: "osc.sine" } if *node_id == osc)));

        // Semitone is parameter 1 and keeps its saved value
        assert!(compiled.commands.iter().any(|c| matches!(c,
            EngineCommand::SetParameter { node_id, param_index: 1, value }
                if *node_id == osc && *value == 9.0)));

        // Oscillator "Out" is port 5 (after 5 inputs); output "Mono" is input port 2
        assert!(compiled.commands.iter().any(|c| matches!(c,
            EngineCommand::Connect { from_node, from_port: 5, to_node, to_port: 2 }
                if *from_node == osc && *to_node == out)));
    }

    #[test]
    fn test_missing_parameters_fall_back_to_defaults() {
        // Patches saved before the Character toggle don't name it; Limiter
        // keeps its saved value and Character falls back to its default (off)
        let mut patch = osc_to_output();
        patch.nodes[1].parameters = vec![
            NamedParameter::new("Volume", ParameterValue::Scalar(0.5)),
            NamedParameter::new("Limiter", ParameterValue::Toggle(false)),
        ];
        let compiled = compile_patch(&patch).unwrap();

        let out = compiled.node_ids[&20];
        let params: Vec<(usize, f32)> = compiled
            .commands
            .iter()
            .filter_map(|c| match c {
                EngineCommand::SetParameter { node_id, param_index, value } if *node_id == out => {
                    Some((*param_index, *value))
                }
                _ => None,
            })
            .collect();
        assert_eq!(params, vec![(0, 0.5), (1, 0.0), (2, 0.0)]);
    }

    #[test]
    fn test_unknown_module_is_skipped_with_a_warning() {
        let mut patch = osc_to_output();
        patch.nodes.push(NodeData::new(30, "does.not.exist", (0.0, 0.0)));
        patch.connections.push(ConnectionData::new(30, "Out", 20, "Left"));
        let compiled = compile_patch(&patch).unwrap();

        // The other nodes and their connection still build
        assert_eq!(compiled.node_ids.len(), 2);
        assert!(!compiled.node_ids.contains_key(&30));
        assert_eq!(
            compiled.commands.iter().filter(|c| matches!(c, EngineCommand::Connect { .. })).count(),
            1
        );
        // One warning for the node, one for its connection
        assert_eq!(compiled.warnings.len(), 2, "{:?}", compiled.warnings);
    }

    #[test]
    fn test_incompatible_version_is_an_error() {
        let mut patch = osc_to_output();
        patch.version = crate::persistence::PATCH_VERSION + 1;
        assert!(matches!(compile_patch(&patch), Err(PatchError::IncompatibleVersion { .. })));
    }

    #[test]
    fn test_bad_port_name_is_a_warning() {
        let mut patch = osc_to_output();
        patch.connections.push(ConnectionData::new(10, "Nope", 20, "Left"));
        let compiled = compile_patch(&patch).unwrap();
        assert_eq!(compiled.warnings.len(), 1);
    }
}
