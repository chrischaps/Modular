//! Moving patches in and out of the editor graph.
//!
//! [`stage_patch`] builds a whole patch into a fresh graph without touching
//! anything live. Callers swap the result in only once it has been built, so
//! a bad patch can never leave a half-loaded graph behind. [`capture_patch`]
//! goes the other way, for saving.
//!
//! Both the editor and headless compilation load through [`stage_patch`], so
//! they agree on how names, IDs and problems are resolved.

use std::collections::HashMap;

use egui_node_graph2::{NodeId, NodeTemplateTrait};

use crate::engine::NodeId as EngineNodeId;
use crate::graph::{
    port_mapping, validate_connection, SynthGraph, SynthGraphState, SynthNodeTemplate, SynthValueType,
};

use super::{
    ConnectionData, MidiMapping, NamedParameter, NodeData, ParameterValue, Patch, PatchError, PATCH_VERSION,
};

/// A node of a staged patch.
#[derive(Debug, Clone, Copy)]
pub struct StagedNode {
    /// The node's ID in the patch file.
    pub patch_id: u64,
    /// The node's ID in [`StagedPatch::graph`].
    pub graph_id: NodeId,
    /// The template the node was built from.
    pub template: SynthNodeTemplate,
    /// Saved editor position.
    pub position: (f32, f32),
}

/// A patch built into its own graph, ready to be swapped in.
pub struct StagedPatch {
    /// The patch's nodes, parameter values and connections.
    pub graph: SynthGraph,
    /// Every node that was built, in patch order. Skipped nodes are missing.
    pub nodes: Vec<StagedNode>,
    /// MIDI mappings whose `param_index` has been resolved by name, but whose
    /// `node_id` is still the patch node ID. See [`StagedPatch::remap_midi_mappings`].
    midi_mappings: Vec<MidiMapping>,
    /// Things that were skipped rather than failing the load.
    pub warnings: Vec<String>,
}

impl StagedPatch {
    /// The staged graph node for a patch node ID, or `None` if it was skipped.
    pub fn graph_id(&self, patch_id: u64) -> Option<NodeId> {
        self.nodes.iter().find(|n| n.patch_id == patch_id).map(|n| n.graph_id)
    }

    /// The patch's MIDI mappings, retargeted at the engine IDs the caller
    /// assigned to the staged nodes.
    ///
    /// Patch node IDs are whatever engine IDs the nodes had when the patch was
    /// saved, so they can't be used as-is: after a load they'd point at the
    /// wrong nodes, or none.
    pub fn remap_midi_mappings(
        &self,
        engine_id: impl Fn(NodeId) -> Option<EngineNodeId>,
    ) -> Vec<MidiMapping> {
        self.midi_mappings
            .iter()
            .filter_map(|mapping| {
                let node_id = engine_id(self.graph_id(mapping.node_id)?)?;
                Some(MidiMapping { node_id, ..mapping.clone() })
            })
            .collect()
    }
}

/// Builds a patch into a fresh graph.
///
/// Only an incompatible version fails the load. Everything else that can't be
/// restored is skipped and reported in [`StagedPatch::warnings`]:
///
/// - nodes with an unknown module ID
/// - parameters with an unknown name (parameters missing from the patch keep
///   their defaults)
/// - connections to skipped nodes, missing ports, or of incompatible types
/// - MIDI mappings to skipped nodes or unknown parameters
pub fn stage_patch(patch: &Patch) -> Result<StagedPatch, PatchError> {
    if !patch.is_compatible() {
        return Err(PatchError::IncompatibleVersion {
            found: patch.version,
            expected: PATCH_VERSION,
        });
    }

    let mut graph = SynthGraph::default();
    // Templates don't keep anything in the user state while building
    let mut user_state = SynthGraphState::new();
    let mut nodes = Vec::with_capacity(patch.nodes.len());
    let mut warnings = Vec::new();

    for node_data in &patch.nodes {
        let Some(template) = SynthNodeTemplate::from_module_id(&node_data.module_id) else {
            warnings.push(format!(
                "Skipped node {}: unknown module '{}'",
                node_data.id, node_data.module_id
            ));
            continue;
        };

        let graph_id = graph.add_node(
            template.node_graph_label(&mut user_state),
            template.user_data(&mut user_state),
            |graph, node_id| template.build_node(graph, &mut user_state, node_id),
        );
        restore_parameters(&mut graph, graph_id, node_data, &mut warnings);

        nodes.push(StagedNode {
            patch_id: node_data.id,
            graph_id,
            template,
            position: node_data.position,
        });
    }

    let graph_ids: HashMap<u64, NodeId> = nodes.iter().map(|n| (n.patch_id, n.graph_id)).collect();

    for conn in &patch.connections {
        let (Some(&from), Some(&to)) = (graph_ids.get(&conn.from_node), graph_ids.get(&conn.to_node)) else {
            warnings.push(format!(
                "Skipped connection {} -> {}: a node wasn't loaded",
                conn.from_node, conn.to_node
            ));
            continue;
        };

        let output = graph.nodes[from]
            .outputs
            .iter()
            .find(|(name, _)| *name == conn.from_port)
            .map(|(_, id)| *id);
        let input = graph.nodes[to]
            .inputs
            .iter()
            .find(|(name, id)| *name == conn.to_port && port_mapping::is_connectable(graph.get_input(*id).kind))
            .map(|(_, id)| *id);
        let (Some(output), Some(input)) = (output, input) else {
            warnings.push(format!(
                "Skipped connection '{}' -> '{}': no such port",
                conn.from_port, conn.to_port
            ));
            continue;
        };

        let result = validate_connection(graph.get_output(output).typ.0, graph.get_input(input).typ.0);
        if let Some(error) = result.error_message() {
            warnings.push(format!(
                "Skipped connection '{}' -> '{}': {}",
                conn.from_port, conn.to_port, error
            ));
            continue;
        }

        graph.add_connection(output, input, 0);
    }

    let mut midi_mappings = Vec::with_capacity(patch.midi_mappings.len());
    for mapping in &patch.midi_mappings {
        let Some(&graph_id) = graph_ids.get(&mapping.node_id) else {
            warnings.push(format!(
                "Dropped MIDI CC {} mapping: node {} wasn't loaded",
                mapping.cc_number, mapping.node_id
            ));
            continue;
        };
        let Some(param_index) = parameter_index(&graph, graph_id, &mapping.param_name) else {
            warnings.push(format!(
                "Dropped MIDI CC {} mapping: no parameter '{}'",
                mapping.cc_number, mapping.param_name
            ));
            continue;
        };
        midi_mappings.push(MidiMapping { param_index, ..mapping.clone() });
    }

    Ok(StagedPatch { graph, nodes, midi_mappings, warnings })
}

/// Sets a freshly built node's parameters from the patch, by name.
fn restore_parameters(graph: &mut SynthGraph, node_id: NodeId, node_data: &NodeData, warnings: &mut Vec<String>) {
    for saved in &node_data.parameters {
        let input_id = graph.nodes[node_id]
            .inputs
            .iter()
            .find(|(name, id)| *name == saved.name && port_mapping::is_parameter(graph.get_input(*id).kind))
            .map(|(_, id)| *id);
        let Some(input_id) = input_id else {
            warnings.push(format!(
                "{}: ignored unknown parameter '{}'",
                node_data.module_id, saved.name
            ));
            continue;
        };

        // Clamped to the parameter's current range
        graph.inputs[input_id].value.set_actual_value(saved.value.as_f32());
    }
}

/// Index of the parameter called `name` on a node.
fn parameter_index(graph: &SynthGraph, node_id: NodeId, name: &str) -> Option<usize> {
    port_mapping::parameter_inputs(graph, node_id)
        .into_iter()
        .position(|input_id| graph.nodes[node_id].inputs.iter().any(|(n, id)| *id == input_id && n == name))
}

/// The saved form of an editor value.
fn parameter_value(value: &SynthValueType) -> ParameterValue {
    match value {
        SynthValueType::Scalar { value, .. } => ParameterValue::Scalar(*value),
        SynthValueType::Frequency { value, .. } => ParameterValue::Frequency(*value),
        SynthValueType::LinearHz { value, .. } => ParameterValue::LinearHz(*value),
        SynthValueType::Time { value, .. } => ParameterValue::Time(*value),
        SynthValueType::LinearRange { value, .. } => ParameterValue::LinearRange(*value),
        SynthValueType::Toggle { value, .. } => ParameterValue::Toggle(*value),
        SynthValueType::Select { value, .. } => ParameterValue::Select(*value),
    }
}

/// Name of the port with the given ID.
fn port_name<Id: PartialEq>(ports: &[(String, Id)], wanted: Id) -> Option<String> {
    ports.iter().find(|(_, id)| *id == wanted).map(|(name, _)| name.clone())
}

/// Captures a graph as a patch.
///
/// Nodes are saved under their engine IDs (`engine_id`); nodes without one
/// are left out. `position` gives each node's canonical editor position.
/// Nodes and connections are sorted so that saving the same graph twice
/// gives the same file.
pub fn capture_patch(
    name: &str,
    graph: &SynthGraph,
    engine_id: impl Fn(NodeId) -> Option<EngineNodeId>,
    position: impl Fn(NodeId) -> (f32, f32),
    midi_mappings: &[MidiMapping],
) -> Patch {
    let mut patch = Patch::new(name);

    for (node_id, node) in graph.nodes.iter() {
        let Some(id) = engine_id(node_id) else {
            continue;
        };
        let mut node_data = NodeData::new(id, node.user_data.module_id, position(node_id));
        node_data.parameters = node
            .inputs
            .iter()
            .filter(|(_, input_id)| port_mapping::is_parameter(graph.get_input(*input_id).kind))
            .map(|(name, input_id)| NamedParameter::new(name.clone(), parameter_value(&graph.get_input(*input_id).value)))
            .collect();
        patch.nodes.push(node_data);
    }
    patch.nodes.sort_by_key(|n| n.id);

    for (input_id, output_id) in graph.iter_connections() {
        let from = graph.get_output(output_id).node;
        let to = graph.get_input(input_id).node;
        let (Some(from_id), Some(to_id)) = (engine_id(from), engine_id(to)) else {
            continue;
        };
        let (Some(from_port), Some(to_port)) = (
            port_name(&graph.nodes[from].outputs, output_id),
            port_name(&graph.nodes[to].inputs, input_id),
        ) else {
            continue;
        };
        patch.connections.push(ConnectionData::new(from_id, from_port, to_id, to_port));
    }
    patch.connections.sort_by(|a, b| {
        (a.from_node, &a.from_port, a.to_node, &a.to_port).cmp(&(b.from_node, &b.from_port, b.to_node, &b.to_port))
    });

    // Mappings to deleted nodes would only come back as load warnings
    patch.midi_mappings = midi_mappings
        .iter()
        .filter(|m| patch.nodes.iter().any(|n| n.id == m.node_id))
        .cloned()
        .collect();
    patch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::patch_from_json;

    const V2_FIXTURE: &str = include_str!("fixtures/patch_v2.json");

    /// Loads a patch the way the editor does (engine IDs handed out in node
    /// order from `first_engine_id`, MIDI mappings remapped), then saves it.
    fn reload(patch: &Patch, first_engine_id: EngineNodeId) -> (Patch, Vec<String>) {
        let staged = stage_patch(patch).unwrap();
        let engine_ids: HashMap<NodeId, EngineNodeId> =
            staged.nodes.iter().zip(first_engine_id..).map(|(n, id)| (n.graph_id, id)).collect();
        let engine_id = |graph_id| engine_ids.get(&graph_id).copied();
        let mappings = staged.remap_midi_mappings(engine_id);
        let position = |graph_id| staged.nodes.iter().find(|n| n.graph_id == graph_id).unwrap().position;
        let saved = capture_patch(&patch.name, &staged.graph, engine_id, position, &mappings);
        (saved, staged.warnings)
    }

    fn node<'a>(patch: &'a Patch, module_id: &str) -> &'a NodeData {
        patch.nodes.iter().find(|n| n.module_id == module_id).unwrap()
    }

    fn param(patch: &Patch, module_id: &str, name: &str) -> f32 {
        node(patch, module_id).parameters.iter().find(|p| p.name == name).unwrap().value.as_f32()
    }

    #[test]
    fn test_round_trip_keeps_graph_params_and_midi_mappings() {
        let (first, warnings) = reload(&patch_from_json(V2_FIXTURE).unwrap(), 0);
        assert!(warnings.is_empty(), "{:?}", warnings);

        // Load it again under different engine IDs, as a second load in the
        // same session would
        let (second, warnings) = reload(&first, 100);
        assert!(warnings.is_empty(), "{:?}", warnings);

        let mut expected = first.clone();
        for n in &mut expected.nodes {
            n.id += 100;
        }
        for c in &mut expected.connections {
            c.from_node += 100;
            c.to_node += 100;
        }
        for m in &mut expected.midi_mappings {
            m.node_id += 100;
        }
        assert_eq!(second, expected);

        // And through JSON
        let json = serde_json::to_string_pretty(&second).unwrap();
        assert_eq!(patch_from_json(&json).unwrap(), second);
    }

    #[test]
    fn test_v2_fixture_loads() {
        let patch = patch_from_json(V2_FIXTURE).unwrap();
        assert_eq!(patch.version, PATCH_VERSION);
        let (saved, warnings) = reload(&patch, 40);
        assert!(warnings.is_empty(), "{:?}", warnings);

        assert_eq!(saved.nodes.len(), 4);
        assert_eq!(saved.connections.len(), 3);

        assert_eq!(param(&saved, "osc.sine", "Frequency"), 110.0);
        assert_eq!(param(&saved, "osc.sine", "Waveform"), 1.0);
        assert_eq!(param(&saved, "osc.sine", "Pulse Width"), 0.3);
        assert_eq!(param(&saved, "filter.svf", "Cutoff"), 640.0);
        assert_eq!(param(&saved, "filter.svf", "Resonance"), 0.8);
        assert_eq!(param(&saved, "filter.svf", "Drive"), 0.25);
        assert_eq!(param(&saved, "mod.adsr", "Sustain"), 0.2);
        assert_eq!(param(&saved, "mod.adsr", "Release"), 0.4);
        assert_eq!(param(&saved, "output.audio", "Volume"), 0.6);
        assert_eq!(param(&saved, "output.audio", "Limiter"), 1.0);
        // Saved before Character existed, so it keeps its default
        assert_eq!(param(&saved, "output.audio", "Character"), 0.0);

        // The mappings follow the filter to its new engine ID
        let filter_id = node(&saved, "filter.svf").id;
        assert_eq!(filter_id, 41);
        let targets: Vec<_> = saved.midi_mappings.iter().map(|m| (m.cc_number, m.node_id, m.param_index)).collect();
        assert_eq!(targets, vec![(74, filter_id, 0), (71, filter_id, 1)]);
    }

    #[test]
    fn test_unknown_module_keeps_the_rest_of_the_patch() {
        let mut patch = patch_from_json(V2_FIXTURE).unwrap();
        patch.nodes.push(NodeData::new(20, "fx.from_the_future", (0.0, 0.0)));
        patch.connections.push(ConnectionData::new(20, "Out", 15, "Left"));
        patch.midi_mappings.push(MidiMapping::new(1, 0, 20, 0, "Wobble", 0.0, 1.0));

        let staged = stage_patch(&patch).unwrap();
        assert_eq!(staged.nodes.len(), 4);
        assert_eq!(staged.graph.iter_connections().count(), 3);
        assert!(staged.graph_id(20).is_none());
        assert_eq!(staged.warnings.len(), 3, "{:?}", staged.warnings);
        assert!(staged.warnings[0].contains("fx.from_the_future"));

        let mappings = staged.remap_midi_mappings(|_| Some(0));
        assert_eq!(mappings.len(), 2);
    }

    #[test]
    fn test_parameters_restore_by_name_not_position() {
        let mut patch = Patch::new("reordered");
        let mut filter = NodeData::new(1, "filter.svf", (0.0, 0.0));
        filter.parameters = vec![
            NamedParameter::new("Drive", ParameterValue::Scalar(0.9)),
            NamedParameter::new("Retired Knob", ParameterValue::Scalar(0.1)),
            NamedParameter::new("Cutoff", ParameterValue::Frequency(300.0)),
        ];
        patch.nodes.push(filter);

        let (saved, warnings) = reload(&patch, 0);
        assert_eq!(param(&saved, "filter.svf", "Cutoff"), 300.0);
        assert_eq!(param(&saved, "filter.svf", "Drive"), 0.9);
        // Missing from the patch, so it keeps its default
        assert_eq!(param(&saved, "filter.svf", "Resonance"), 0.5);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("Retired Knob"));
    }

    #[test]
    fn test_midi_mapping_follows_parameter_name() {
        // A stale index must not win over the name
        let mut patch = patch_from_json(V2_FIXTURE).unwrap();
        patch.midi_mappings[1].param_index = 2;
        patch.midi_mappings.push(MidiMapping::new(5, 0, 7, 0, "Retired Knob", 0.0, 1.0));

        let staged = stage_patch(&patch).unwrap();
        let mappings = staged.remap_midi_mappings(|_| Some(0));
        let indices: Vec<_> = mappings.iter().map(|m| (m.cc_number, m.param_index)).collect();
        assert_eq!(indices, vec![(74, 0), (71, 1)]);
        assert_eq!(staged.warnings.len(), 1, "{:?}", staged.warnings);
    }

    #[test]
    fn test_capture_drops_mappings_to_deleted_nodes() {
        let mut patch = patch_from_json(V2_FIXTURE).unwrap();
        patch.midi_mappings.push(MidiMapping::new(9, 0, 999, 0, "Cutoff", 0.0, 1.0));
        // Bypass staging so the stale mapping reaches capture, as it would
        // after the node was deleted in the editor
        let staged = stage_patch(&patch).unwrap();
        let ids: HashMap<NodeId, u64> = staged.nodes.iter().map(|n| (n.graph_id, n.patch_id)).collect();
        let saved = capture_patch("x", &staged.graph, |g| ids.get(&g).copied(), |_| (0.0, 0.0), &patch.midi_mappings);
        assert_eq!(saved.midi_mappings.len(), 2);
    }
}
