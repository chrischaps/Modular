//! Headless patch compilation.
//!
//! Turns a [`Patch`] into the [`EngineCommand`]s that build it in an audio
//! engine, without a UI. Nodes are built through the same editor templates the
//! UI uses, and ports and parameters are resolved with the shared
//! [`port_mapping`] helpers, so the indices always match what the editor sends.

use std::collections::HashMap;

use crate::engine::{EngineCommand, NodeId as EngineNodeId};
use crate::graph::{port_mapping, SynthGraph, SynthGraphState, SynthNodeTemplate};
use egui_node_graph2::NodeTemplateTrait;

use super::{Patch, PatchError};

/// The engine commands that build a patch, plus bookkeeping for callers.
#[derive(Debug, Default)]
pub struct CompiledPatch {
    /// Commands in the order they must be applied: modules, parameters, connections.
    pub commands: Vec<EngineCommand>,
    /// Engine node ID assigned to each patch node ID.
    pub node_ids: HashMap<u64, EngineNodeId>,
    /// Problems that were skipped rather than failing the whole patch,
    /// e.g. a connection naming a port that no longer exists.
    pub warnings: Vec<String>,
}

/// Compiles a patch into engine commands.
///
/// Fails on an unknown module (the patch can't be represented faithfully).
/// Connections to missing nodes or ports are skipped and reported in
/// [`CompiledPatch::warnings`], matching how the editor loads patches.
pub fn compile_patch(patch: &Patch) -> Result<CompiledPatch, PatchError> {
    if !patch.is_compatible() {
        return Err(PatchError::IncompatibleVersion {
            found: patch.version,
            expected: super::PATCH_VERSION,
        });
    }

    let mut graph = SynthGraph::default();
    let mut user_state = SynthGraphState::new();
    let mut compiled = CompiledPatch::default();
    let mut graph_ids = HashMap::new();

    for node_data in &patch.nodes {
        let template = SynthNodeTemplate::from_module_id(&node_data.module_id)
            .ok_or_else(|| PatchError::UnknownModule(node_data.module_id.clone()))?;

        let graph_node_id = graph.add_node(
            template.node_graph_label(&mut user_state),
            template.user_data(&mut user_state),
            |graph, node_id| template.build_node(graph, &mut user_state, node_id),
        );
        let engine_node_id = user_state.allocate_engine_node_id(graph_node_id);
        graph_ids.insert(node_data.id, graph_node_id);
        compiled.node_ids.insert(node_data.id, engine_node_id);

        compiled.commands.push(EngineCommand::AddModule {
            node_id: engine_node_id,
            module_id: template.module_id(),
        });

        // Restore saved values (by position, as the editor does), then send
        // every parameter the way the editor's initial parameter sync does.
        let skip = port_mapping::live_input_parameter_count(template.module_id());
        for (param_index, input_id) in port_mapping::parameter_inputs(&graph, graph_node_id)
            .into_iter()
            .enumerate()
        {
            let Some(input) = graph.inputs.get_mut(input_id) else {
                continue;
            };
            if let Some(saved) = node_data.parameters.get(param_index) {
                input.value.set_actual_value(saved.as_f32());
            }
            if param_index < skip {
                continue;
            }
            compiled.commands.push(EngineCommand::SetParameter {
                node_id: engine_node_id,
                param_index,
                value: input.value.actual_value(),
            });
        }
    }

    for conn in &patch.connections {
        let (Some(&from), Some(&to)) = (graph_ids.get(&conn.from_node), graph_ids.get(&conn.to_node)) else {
            compiled.warnings.push(format!(
                "connection {} -> {} references a missing node",
                conn.from_node, conn.to_node
            ));
            continue;
        };

        let output_id = graph.nodes[from]
            .outputs
            .iter()
            .find(|(name, _)| *name == conn.from_port)
            .map(|(_, id)| *id);
        let input_id = graph.nodes[to]
            .inputs
            .iter()
            .find(|(name, _)| *name == conn.to_port)
            .map(|(_, id)| *id);

        let ports = output_id.zip(input_id).and_then(|(output_id, input_id)| {
            Some((
                port_mapping::output_port_index(&graph, from, output_id)?,
                port_mapping::input_port_index(&graph, to, input_id)?,
            ))
        });
        let Some((from_port, to_port)) = ports else {
            compiled.warnings.push(format!(
                "connection '{}' -> '{}' names a port that doesn't exist",
                conn.from_port, conn.to_port
            ));
            continue;
        };

        compiled.commands.push(EngineCommand::Connect {
            from_node: user_state.get_engine_node_id(from).expect("allocated above"),
            from_port,
            to_node: user_state.get_engine_node_id(to).expect("allocated above"),
            to_port,
        });
    }

    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{ConnectionData, NodeData, ParameterValue};

    fn osc_to_output() -> Patch {
        let mut patch = Patch::new("test");
        let mut osc = NodeData::new(10, "osc.sine", (0.0, 0.0));
        osc.parameters = vec![
            ParameterValue::Frequency(110.0),
            ParameterValue::LinearHz(0.0),
            ParameterValue::Select(1),
            ParameterValue::Scalar(0.5),
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

        // Frequency is parameter 0 and keeps its saved value
        assert!(compiled.commands.iter().any(|c| matches!(c,
            EngineCommand::SetParameter { node_id, param_index: 0, value }
                if *node_id == osc && (*value - 110.0).abs() < 1e-3)));

        // Oscillator "Out" is port 4 (after 4 inputs); output "Mono" is input port 2
        assert!(compiled.commands.iter().any(|c| matches!(c,
            EngineCommand::Connect { from_node, from_port: 4, to_node, to_port: 2 }
                if *from_node == osc && *to_node == out)));
    }

    #[test]
    fn test_unknown_module_is_an_error() {
        let mut patch = osc_to_output();
        patch.nodes.push(NodeData::new(30, "does.not.exist", (0.0, 0.0)));
        assert!(matches!(compile_patch(&patch), Err(PatchError::UnknownModule(_))));
    }

    #[test]
    fn test_bad_port_name_is_a_warning() {
        let mut patch = osc_to_output();
        patch.connections.push(ConnectionData::new(10, "Nope", 20, "Left"));
        let compiled = compile_patch(&patch).unwrap();
        assert_eq!(compiled.warnings.len(), 1);
    }
}
