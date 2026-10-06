//! Engine commands for edits to the editor graph.
//!
//! The editor graph and the audio engine each keep their own copy of the
//! patch. Every edit to the graph, whether the user made it or undo replayed
//! it, has to be mirrored to the engine. These helpers build the commands for
//! each kind of edit, so the live editor and undo always send the same ones.

use egui_node_graph2::{InputId, NodeId, OutputId};

use crate::engine::{EngineCommand, NodeId as EngineNodeId};
use crate::graph::{port_mapping, SynthGraph, SynthGraphState};

/// Commands that create a node's module and start monitoring the outputs it
/// shows (LEDs, lit ports, displays). Parameter values follow with the next
/// parameter sync.
pub fn add_module(graph: &SynthGraph, node_id: NodeId, engine_node_id: EngineNodeId) -> Vec<EngineCommand> {
    let Some(node) = graph.nodes.get(node_id) else {
        return Vec::new();
    };
    let mut commands = vec![EngineCommand::AddModule {
        node_id: engine_node_id,
        module_id: node.user_data.module_id,
    }];
    if node.user_data.bypassed {
        commands.push(EngineCommand::SetBypass { node_id: engine_node_id, bypassed: true });
    }
    let monitored = node.user_data.led_indicators.iter()
        .map(|led| led.output_index)
        .chain(node.user_data.monitored_outputs.iter().copied());
    commands.extend(monitored.map(|output_index| EngineCommand::MonitorOutput {
        node_id: engine_node_id,
        output_index,
    }));
    commands
}

/// Commands for a cable the editor has just made.
///
/// The input is disconnected first: when a cable lands on an input that
/// already had one, the editor drops the old cable without saying so.
pub fn cable_connected(
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    output: OutputId,
    input: InputId,
) -> Vec<EngineCommand> {
    let mut commands: Vec<EngineCommand> = disconnect(graph, user_state, input).into_iter().collect();
    if let Some(connect) = connect(graph, user_state, output, input) {
        commands.push(connect);
        // An exposed knob follows the cable's signal
        commands.extend(monitor_input(graph, user_state, input, true));
        // The cable animates with the output's level
        commands.extend(monitor_output(graph, user_state, output, true));
    }
    commands
}

/// Commands for a cable the editor has just removed.
///
/// Either end may already be gone, when the cable went with a deleted node.
pub fn cable_disconnected(
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    output: OutputId,
    input: InputId,
) -> Vec<EngineCommand> {
    let mut commands = Vec::new();
    if let Some(disconnect) = disconnect(graph, user_state, input) {
        commands.push(disconnect);
        commands.extend(monitor_input(graph, user_state, input, false));
    }

    // Stop watching the output once its last cable is gone, unless the node
    // always shows it (a lit port, say)
    let still_patched = graph.iter_connections().any(|(_, o)| o == output);
    let always_monitored = graph.outputs.get(output)
        .and_then(|out| {
            let node = graph.nodes.get(out.node)?;
            let output_index = graph.get_output_index(output)?;
            Some(node.user_data.monitored_outputs.contains(&output_index))
        })
        .unwrap_or(false);
    if !still_patched && !always_monitored {
        commands.extend(monitor_output(graph, user_state, output, false));
    }
    commands
}

/// A Connect command from graph port IDs.
fn connect(graph: &SynthGraph, user_state: &SynthGraphState, output: OutputId, input: InputId) -> Option<EngineCommand> {
    let output_data = graph.outputs.get(output)?;
    let input_data = graph.inputs.get(input)?;
    Some(EngineCommand::Connect {
        from_node: user_state.get_engine_node_id(output_data.node)?,
        from_port: port_mapping::output_port_index(graph, output_data.node, output)?,
        to_node: user_state.get_engine_node_id(input_data.node)?,
        to_port: port_mapping::input_port_index(graph, input_data.node, input)?,
    })
}

/// A Disconnect command for whatever cable feeds `input`. The engine
/// ignores it if nothing does.
fn disconnect(graph: &SynthGraph, user_state: &SynthGraphState, input: InputId) -> Option<EngineCommand> {
    // A stale ID (the node was deleted) has nothing left to disconnect
    let input_data = graph.inputs.get(input)?;
    Some(EngineCommand::Disconnect {
        node_id: user_state.get_engine_node_id(input_data.node)?,
        port: port_mapping::input_port_index(graph, input_data.node, input)?,
        is_input: true,
    })
}

/// Starts or stops monitoring an input, if it's an exposed parameter (both
/// a jack and a knob). Its knob animates with the incoming signal.
fn monitor_input(
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    input: InputId,
    monitor: bool,
) -> Option<EngineCommand> {
    let input_data = graph.inputs.get(input)?;
    let node = graph.nodes.get(input_data.node)?;
    let input_name = node.inputs.iter()
        .find(|(_, id)| *id == input)
        .map(|(name, _)| name)?;
    let is_exposed_param = node.user_data.knob_params.iter()
        .any(|kp| kp.param_name == *input_name && kp.has_input_port());
    if !is_exposed_param {
        return None;
    }

    let node_id = user_state.get_engine_node_id(input_data.node)?;
    let input_index = port_mapping::input_port_index(graph, input_data.node, input)?;
    Some(if monitor {
        EngineCommand::MonitorInput { node_id, input_index }
    } else {
        EngineCommand::UnmonitorInput { node_id, input_index }
    })
}

/// Starts or stops monitoring an output, which drives cable animation.
fn monitor_output(
    graph: &SynthGraph,
    user_state: &SynthGraphState,
    output: OutputId,
    monitor: bool,
) -> Option<EngineCommand> {
    let output_data = graph.outputs.get(output)?;
    let node_id = user_state.get_engine_node_id(output_data.node)?;
    let output_index = graph.get_output_index(output)?;
    Some(if monitor {
        EngineCommand::MonitorOutput { node_id, output_index }
    } else {
        EngineCommand::UnmonitorOutput { node_id, output_index }
    })
}
