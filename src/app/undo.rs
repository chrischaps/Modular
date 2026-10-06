//! Undo and redo for patch edits.
//!
//! History doesn't hook each kind of edit. It keeps a [`Snapshot`] of the
//! patch as of the last step, and once a gesture is over (no mouse button
//! held) compares the editor with it. Whatever differs becomes one [`Step`]:
//! adding, deleting, connecting, disconnecting, moving and bypassing modules,
//! and turning their knobs. Nothing is compared while a button is down, so a
//! whole knob turn, node drag or cable repatch is a single step.
//!
//! A step holds both sides of what it changed, so it can be applied in either
//! direction. Applying one edits the editor graph and returns the engine
//! commands for the same edit, so the sound follows. A deleted module comes
//! back under its old engine ID, so its MIDI mappings come back with it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use egui::{Pos2, Vec2};
use egui_node_graph2::{NodeId, NodeTemplateTrait, PanZoom};

use crate::engine::{EngineCommand, NodeId as EngineNodeId};
use crate::graph::{port_mapping, SynthGraphEditorState, SynthGraphState, SynthNodeTemplate};
use super::engine_sync;

/// Most steps kept. The oldest are dropped first.
const MAX_STEPS: usize = 256;

/// Knob changes this close together, to the same knobs, are one step. Knob
/// drags already are; this gathers up anything that edits a value without
/// holding a button, like typing.
const MERGE_WINDOW: Duration = Duration::from_millis(1000);

/// Nodes closer than this to where they were (in unzoomed points) haven't
/// moved. Zooming rescales every position, which leaves rounding behind.
const MOVE_TOLERANCE: f32 = 0.5;

/// Nodes are known by their engine ID, which survives being deleted and
/// brought back. Their graph IDs don't.
type NodeKey = EngineNodeId;

/// One module, as far as undo cares.
#[derive(Clone, Debug, PartialEq)]
struct NodeState {
    template: SynthNodeTemplate,
    /// Editor position in unzoomed points, from [`ViewAnchor`].
    position: Vec2,
    /// Every parameter's value in real units, in parameter order.
    params: Vec<f32>,
    bypassed: bool,
}

/// A cable, by node and by port position on each node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Cable {
    from: NodeKey,
    /// Position in the source node's outputs.
    output: usize,
    to: NodeKey,
    /// Position in the destination node's inputs.
    input: usize,
}

/// The whole patch, as far as undo cares.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    nodes: BTreeMap<NodeKey, NodeState>,
    cables: BTreeSet<Cable>,
}

/// Where the editor's node positions put the origin.
///
/// The editor keeps node positions in zoomed points, and zooming moves them
/// all towards or away from the middle of the view. Steps keep positions
/// that don't depend on the zoom, `(position - origin) / zoom`, with the
/// origin moved exactly as zooming moves every node.
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewAnchor {
    origin: Vec2,
}

impl ViewAnchor {
    fn unzoomed(&self, position: Pos2, zoom: f32) -> Vec2 {
        (position.to_vec2() - self.origin) / zoom
    }

    fn zoomed(&self, position: Vec2, zoom: f32) -> Pos2 {
        (position * zoom + self.origin).to_pos2()
    }

    /// Follows a zoom from `zoom_before` to the current zoom, made while the
    /// view was panned by `pan_before`. It mirrors the editor scaling node
    /// positions about the middle of the view.
    pub fn follow_zoom(&mut self, zoom_before: f32, pan_before: Vec2, pan_zoom: &PanZoom) {
        if pan_zoom.zoom == zoom_before {
            return;
        }
        let scale = pan_zoom.zoom / zoom_before;
        let half_size = pan_zoom.clip_rect.size() / 2.0;
        self.origin = (self.origin - half_size + pan_before) * scale + half_size - pan_before;
    }
}

impl Snapshot {
    /// Takes the editor's patch. Nodes without an engine ID are left out.
    pub fn capture(editor: &SynthGraphEditorState, user_state: &SynthGraphState, anchor: &ViewAnchor) -> Self {
        let graph = &editor.graph;
        let zoom = editor.pan_zoom.zoom;
        let mut snapshot = Self::default();

        for (node_id, node) in graph.nodes.iter() {
            let (Some(key), Some(template)) = (
                user_state.get_engine_node_id(node_id),
                SynthNodeTemplate::from_module_id(node.user_data.module_id),
            ) else {
                continue;
            };
            let position = editor.node_positions.get(node_id).copied().unwrap_or_default();
            let params = port_mapping::parameter_inputs(graph, node_id)
                .into_iter()
                .map(|input| graph.get_input(input).value.actual_value())
                .collect();
            snapshot.nodes.insert(key, NodeState {
                template,
                position: anchor.unzoomed(position, zoom),
                params,
                bypassed: node.user_data.bypassed,
            });
        }

        for (input, output) in graph.iter_connections() {
            let (from, to) = (graph.get_output(output).node, graph.get_input(input).node);
            let cable = (|| Some(Cable {
                from: user_state.get_engine_node_id(from)?,
                output: graph.get_output_index(output)?,
                to: user_state.get_engine_node_id(to)?,
                input: graph.nodes[to].inputs.iter().position(|(_, id)| *id == input)?,
            }))();
            snapshot.cables.extend(cable);
        }
        snapshot
    }

    /// Records a value MIDI CC set, so it isn't mistaken for an edit.
    fn set_param(&mut self, key: NodeKey, param_index: usize, value: f32) {
        if let Some(param) = self.nodes.get_mut(&key).and_then(|n| n.params.get_mut(param_index)) {
            *param = value;
        }
    }
}

/// How one node changed in a step. `None` is a node that isn't there.
#[derive(Clone, Debug)]
struct NodeDiff {
    key: NodeKey,
    before: Option<NodeState>,
    after: Option<NodeState>,
}

/// A cable a step added or removed.
#[derive(Clone, Copy, Debug)]
struct CableDiff {
    cable: Cable,
    added: bool,
}

/// One undoable edit, which may touch several modules and cables.
#[derive(Clone, Debug)]
pub struct Step {
    nodes: Vec<NodeDiff>,
    cables: Vec<CableDiff>,
    label: String,
}

fn moved(a: Vec2, b: Vec2) -> bool {
    (a - b).length() > MOVE_TOLERANCE
}

/// Positions of the parameters that differ between two states of a node.
fn changed_params<'a>(a: &'a NodeState, b: &'a NodeState) -> impl Iterator<Item = usize> + 'a {
    a.params.iter().zip(&b.params).enumerate().filter(|(_, (x, y))| x != y).map(|(i, _)| i)
}

impl NodeDiff {
    /// Whether the node is the same in substance on both sides.
    fn is_noop(&self) -> bool {
        match (&self.before, &self.after) {
            (Some(a), Some(b)) => {
                !moved(a.position, b.position) && a.bypassed == b.bypassed && changed_params(a, b).next().is_none()
            }
            (None, None) => true,
            _ => false,
        }
    }

    /// The parameters a step changed on a node that stayed, if that's all
    /// it changed.
    fn params_only(&self) -> Option<Vec<usize>> {
        match (&self.before, &self.after) {
            (Some(a), Some(b)) if !moved(a.position, b.position) && a.bypassed == b.bypassed => {
                Some(changed_params(a, b).collect())
            }
            _ => None,
        }
    }

    fn name(&self) -> &'static str {
        self.before.as_ref().or(self.after.as_ref()).map_or("module", |n| n.template.name())
    }
}

impl Step {
    /// What changed from `before` to `after`, or `None` if nothing did.
    pub fn between(before: &Snapshot, after: &Snapshot) -> Option<Self> {
        let keys: BTreeSet<NodeKey> = before.nodes.keys().chain(after.nodes.keys()).copied().collect();
        let nodes: Vec<NodeDiff> = keys
            .into_iter()
            .map(|key| NodeDiff {
                key,
                before: before.nodes.get(&key).cloned(),
                after: after.nodes.get(&key).cloned(),
            })
            .filter(|diff| !diff.is_noop())
            .collect();
        let cables: Vec<CableDiff> = before.cables.difference(&after.cables)
            .map(|&cable| CableDiff { cable, added: false })
            .chain(after.cables.difference(&before.cables).map(|&cable| CableDiff { cable, added: true }))
            .collect();

        if nodes.is_empty() && cables.is_empty() {
            return None;
        }
        let label = describe(&nodes, &cables, before, after);
        Some(Self { nodes, cables, label })
    }

    /// What the step did, for the Edit buttons and status bar,
    /// e.g. "Move Oscillator" or "Set SVF Filter Cutoff".
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The step that undoes this one.
    fn inverse(&self) -> Self {
        Self {
            nodes: self.nodes.iter()
                .map(|diff| NodeDiff { key: diff.key, before: diff.after.clone(), after: diff.before.clone() })
                .collect(),
            cables: self.cables.iter().map(|diff| CableDiff { added: !diff.added, ..*diff }).collect(),
            label: self.label.clone(),
        }
    }

    /// Folds `next` into this step, if both only turned the same knobs.
    fn absorb(&mut self, next: &Step) -> bool {
        let same_knobs = self.cables.is_empty()
            && next.cables.is_empty()
            && self.nodes.len() == next.nodes.len()
            && self.nodes.iter().zip(&next.nodes).all(|(a, b)| {
                a.key == b.key && a.params_only().is_some() && a.params_only() == b.params_only()
            });
        if !same_knobs {
            return false;
        }
        for (mine, theirs) in self.nodes.iter_mut().zip(&next.nodes) {
            mine.after = theirs.after.clone();
        }
        true
    }

    /// Makes the editor match the `after` side of the step, and returns the
    /// engine commands that do the same to the audio graph.
    fn apply(&self, editor: &mut SynthGraphEditorState, user_state: &mut SynthGraphState, anchor: &ViewAnchor) -> Vec<EngineCommand> {
        let mut commands = Vec::new();
        let zoom = editor.pan_zoom.zoom;
        let mut graph_ids: HashMap<NodeKey, NodeId> =
            user_state.node_id_map.iter().map(|(&graph_id, &key)| (key, graph_id)).collect();

        // Cables come out first, so neither a removed module nor a cable
        // about to take their input is still attached to them
        for diff in self.cables.iter().filter(|d| !d.added) {
            let graph = &mut editor.graph;
            if let Some((output, input)) = cable_ports(graph, &graph_ids, diff.cable) {
                if graph.remove_connection(input, output) {
                    commands.extend(engine_sync::cable_disconnected(graph, user_state, output, input));
                }
            }
        }

        // Modules that go
        for diff in self.nodes.iter().filter(|d| d.after.is_none()) {
            let Some(node_id) = graph_ids.remove(&diff.key) else { continue };
            let (_, cut) = editor.graph.remove_node(node_id);
            for (input, output) in cut {
                commands.extend(engine_sync::cable_disconnected(&editor.graph, user_state, output, input));
            }
            editor.node_positions.remove(node_id);
            editor.node_order.retain(|id| *id != node_id);
            editor.selected_nodes.retain(|id| *id != node_id);
            user_state.remove_node(node_id);
            commands.push(EngineCommand::RemoveModule { node_id: diff.key });
        }

        // Modules that come (back), under the engine ID they had
        for diff in self.nodes.iter().filter(|d| d.before.is_none()) {
            let Some(state) = &diff.after else { continue };
            let template = state.template;
            let node_id = editor.graph.add_node(
                template.node_graph_label(user_state),
                template.user_data(user_state),
                |graph, node_id| template.build_node(graph, user_state, node_id),
            );
            editor.graph[node_id].user_data.bypassed = state.bypassed;
            editor.node_positions.insert(node_id, anchor.zoomed(state.position, zoom));
            editor.node_order.push(node_id);
            user_state.assign_engine_node_id(node_id, diff.key);
            graph_ids.insert(diff.key, node_id);
            commands.extend(engine_sync::add_module(&editor.graph, node_id, diff.key));

            // Every value goes to the new module, since it starts at defaults
            let all = 0..state.params.len();
            commands.extend(set_params(editor, node_id, diff.key, state, all));
        }

        // Modules that stay but changed
        for diff in &self.nodes {
            let (Some(before), Some(after)) = (&diff.before, &diff.after) else { continue };
            let Some(&node_id) = graph_ids.get(&diff.key) else { continue };
            let changed: Vec<usize> = changed_params(before, after).collect();
            commands.extend(set_params(editor, node_id, diff.key, after, changed));
            if moved(before.position, after.position) {
                editor.node_positions.insert(node_id, anchor.zoomed(after.position, zoom));
            }
            if before.bypassed != after.bypassed {
                editor.graph[node_id].user_data.bypassed = after.bypassed;
                commands.push(EngineCommand::SetBypass { node_id: diff.key, bypassed: after.bypassed });
            }
        }

        // Cables go in last, once both ends exist
        for diff in self.cables.iter().filter(|d| d.added) {
            let graph = &mut editor.graph;
            if let Some((output, input)) = cable_ports(graph, &graph_ids, diff.cable) {
                graph.add_connection(output, input, 0);
                commands.extend(engine_sync::cable_connected(graph, user_state, output, input));
            }
        }
        commands
    }
}

/// The graph ports at the ends of a cable, if both nodes and ports exist.
fn cable_ports(
    graph: &crate::graph::SynthGraph,
    graph_ids: &HashMap<NodeKey, NodeId>,
    cable: Cable,
) -> Option<(egui_node_graph2::OutputId, egui_node_graph2::InputId)> {
    let from = graph.nodes.get(*graph_ids.get(&cable.from)?)?;
    let to = graph.nodes.get(*graph_ids.get(&cable.to)?)?;
    Some((from.outputs.get(cable.output)?.1, to.inputs.get(cable.input)?.1))
}

/// Sets some of a node's parameters to `state`'s values, and returns the
/// SetParameter commands for them. Live parameters (a Keyboard's Note and
/// Gate) are the player's, so they're left alone.
fn set_params(
    editor: &mut SynthGraphEditorState,
    node_id: NodeId,
    key: NodeKey,
    state: &NodeState,
    indices: impl IntoIterator<Item = usize>,
) -> Vec<EngineCommand> {
    let inputs = port_mapping::parameter_inputs(&editor.graph, node_id);
    let live = state.template.live_parameter_count();
    indices
        .into_iter()
        .filter(|&i| i >= live)
        .filter_map(|param_index| {
            let input = editor.graph.inputs.get_mut(*inputs.get(param_index)?)?;
            input.value.set_actual_value(*state.params.get(param_index)?);
            Some(EngineCommand::SetParameter { node_id: key, param_index, value: input.value.actual_value() })
        })
        .collect()
}

/// Names a step after what it did.
fn describe(nodes: &[NodeDiff], cables: &[CableDiff], before: &Snapshot, after: &Snapshot) -> String {
    let added: Vec<&NodeDiff> = nodes.iter().filter(|d| d.before.is_none()).collect();
    let removed: Vec<&NodeDiff> = nodes.iter().filter(|d| d.after.is_none()).collect();
    let changed: Vec<&NodeDiff> = nodes.iter().filter(|d| d.before.is_some() && d.after.is_some()).collect();
    let modules = |n: usize| if n == 1 { "module".to_string() } else { format!("{n} modules") };

    if !added.is_empty() || !removed.is_empty() {
        return match (added.as_slice(), removed.as_slice()) {
            ([one], []) => format!("Add {}", one.name()),
            (many, []) => format!("Add {}", modules(many.len())),
            ([], [one]) => format!("Delete {}", one.name()),
            ([], many) => format!("Delete {}", modules(many.len())),
            _ => "Edit patch".to_string(),
        };
    }

    if changed.is_empty() {
        let name = |key: NodeKey| {
            after.nodes.get(&key).or(before.nodes.get(&key)).map_or("module", |n| n.template.name())
        };
        let ends = |c: &Cable| format!("{} → {}", name(c.from), name(c.to));
        let made: Vec<&Cable> = cables.iter().filter(|d| d.added).map(|d| &d.cable).collect();
        let cut: Vec<&Cable> = cables.iter().filter(|d| !d.added).map(|d| &d.cable).collect();
        return match (made.as_slice(), cut.as_slice()) {
            ([one], []) => format!("Connect {}", ends(one)),
            ([], [one]) => format!("Disconnect {}", ends(one)),
            ([one], [_]) => format!("Repatch {}", ends(one)),
            _ => "Repatch cables".to_string(),
        };
    }
    if !cables.is_empty() {
        return "Edit patch".to_string();
    }

    let pairs = || changed.iter().filter_map(|d| Some((d, d.before.as_ref()?, d.after.as_ref()?)));
    if pairs().all(|(_, a, b)| changed_params(a, b).next().is_none() && a.bypassed == b.bypassed) {
        return match changed.as_slice() {
            [one] => format!("Move {}", one.name()),
            many => format!("Move {}", modules(many.len())),
        };
    }
    if pairs().all(|(_, a, b)| changed_params(a, b).next().is_none() && !moved(a.position, b.position)) {
        return match (changed.as_slice(), pairs().next()) {
            ([one], Some((_, _, b))) if b.bypassed => format!("Bypass {}", one.name()),
            ([one], Some(_)) => format!("Switch on {}", one.name()),
            _ => format!("Bypass {}", modules(changed.len())),
        };
    }
    if let ([one], Some((_, a, b))) = (changed.as_slice(), pairs().next()) {
        let params: Vec<usize> = changed_params(a, b).collect();
        if let ([param], false, true) = (params.as_slice(), moved(a.position, b.position), a.bypassed == b.bypassed) {
            if let Some(param_name) = b.template.parameter_names().get(*param) {
                return format!("Set {} {}", one.name(), param_name);
            }
        }
        return format!("Change {}", one.name());
    }
    "Change modules".to_string()
}

/// What undo or redo did: the step's label, and the engine commands that
/// make the audio graph match.
pub struct Applied {
    pub label: String,
    pub commands: Vec<EngineCommand>,
}

/// The undo and redo stacks, and the patch as of the last step.
#[derive(Default)]
pub struct History {
    baseline: Snapshot,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// When the last step was recorded, while it can still absorb more knob
    /// changes. Undo and redo close it.
    open_since: Option<Instant>,
    anchor: ViewAnchor,
}

impl History {
    /// Forgets every step and starts over from the editor's patch, e.g.
    /// after loading one.
    pub fn reset(&mut self, editor: &SynthGraphEditorState, user_state: &SynthGraphState) {
        self.undo.clear();
        self.redo.clear();
        self.open_since = None;
        self.baseline = Snapshot::capture(editor, user_state, &self.anchor);
    }

    /// Starts tracking zoom afresh, for when the editor's pan and zoom are
    /// reset to their defaults.
    pub fn reset_view(&mut self) {
        self.anchor = ViewAnchor::default();
    }

    /// Follows a zoom of the editor. See [`ViewAnchor::follow_zoom`].
    pub fn follow_zoom(&mut self, zoom_before: f32, pan_before: Vec2, pan_zoom: &PanZoom) {
        self.anchor.follow_zoom(zoom_before, pan_before, pan_zoom);
    }

    /// Records anything that changed since the last step as a new step.
    /// Call it once a frame, after every edit. While `gesture_held` (a mouse
    /// button is down) it waits, so the gesture becomes one step.
    pub fn record(&mut self, editor: &SynthGraphEditorState, user_state: &SynthGraphState, gesture_held: bool, now: Instant) {
        if gesture_held {
            return;
        }
        let current = Snapshot::capture(editor, user_state, &self.anchor);
        let Some(step) = Step::between(&self.baseline, &current) else {
            return;
        };
        self.baseline = current;
        self.redo.clear();

        let merges = self.open_since.is_some_and(|since| now.duration_since(since) < MERGE_WINDOW);
        if !(merges && self.undo.last_mut().is_some_and(|last| last.absorb(&step))) {
            self.undo.push(step);
            if self.undo.len() > MAX_STEPS {
                self.undo.remove(0);
            }
        }
        self.open_since = Some(now);
    }

    /// Notes a value set by MIDI CC: playing a controller isn't an edit.
    pub fn absorb_param(&mut self, key: EngineNodeId, param_index: usize, value: f32) {
        self.baseline.set_param(key, param_index, value);
    }

    /// Undoes the last step, if there is one.
    pub fn undo(&mut self, editor: &mut SynthGraphEditorState, user_state: &mut SynthGraphState) -> Option<Applied> {
        // A gesture still in progress is finished first, so it's what gets undone
        self.record(editor, user_state, false, Instant::now());
        let step = self.undo.pop()?;
        let applied = self.replay(&step.inverse(), editor, user_state);
        self.redo.push(step);
        Some(applied)
    }

    /// Redoes the last undone step, if there is one.
    pub fn redo(&mut self, editor: &mut SynthGraphEditorState, user_state: &mut SynthGraphState) -> Option<Applied> {
        self.record(editor, user_state, false, Instant::now());
        let step = self.redo.pop()?;
        let applied = self.replay(&step, editor, user_state);
        self.undo.push(step);
        Some(applied)
    }

    fn replay(&mut self, step: &Step, editor: &mut SynthGraphEditorState, user_state: &mut SynthGraphState) -> Applied {
        let commands = step.apply(editor, user_state, &self.anchor);
        self.baseline = Snapshot::capture(editor, user_state, &self.anchor);
        self.open_since = None;
        Applied { label: step.label.clone(), commands }
    }

    /// What Undo would undo.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(Step::label)
    }

    /// What Redo would redo.
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(Step::label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, vec2, Rect};
    use egui_node_graph2::GraphEditorState;

    /// An editor without a window: the graph, its user state and undo
    /// history, edited the way the editor and its responses edit them.
    struct Rig {
        editor: SynthGraphEditorState,
        user_state: SynthGraphState,
        history: History,
        /// The clock steps are recorded at
        now: Instant,
    }

    impl Rig {
        fn new() -> Self {
            let mut rig = Self {
                editor: GraphEditorState::new(1.0),
                user_state: SynthGraphState::new(),
                history: History::default(),
                now: Instant::now(),
            };
            rig.history.reset(&rig.editor, &rig.user_state);
            rig
        }

        /// Adds a module, as the add-module menu does.
        fn add(&mut self, module_id: &str, at: Pos2) -> NodeId {
            let template = SynthNodeTemplate::from_module_id(module_id).unwrap();
            let user_state = &mut self.user_state;
            let node_id = self.editor.graph.add_node(
                template.node_graph_label(user_state),
                template.user_data(user_state),
                |graph, node_id| template.build_node(graph, user_state, node_id),
            );
            self.editor.node_positions.insert(node_id, at);
            self.editor.node_order.push(node_id);
            self.user_state.allocate_engine_node_id(node_id);
            node_id
        }

        /// Deletes a module, as its close button does.
        fn delete(&mut self, node_id: NodeId) {
            self.editor.graph.remove_node(node_id);
            self.editor.node_positions.remove(node_id);
            self.editor.node_order.retain(|id| *id != node_id);
            self.user_state.remove_node(node_id);
        }

        fn ports(&self, from: NodeId, output: &str, to: NodeId, input: &str) -> (egui_node_graph2::OutputId, egui_node_graph2::InputId) {
            let graph = &self.editor.graph;
            (graph[from].get_output(output).unwrap(), graph[to].get_input(input).unwrap())
        }

        /// Patches a cable, replacing any already in the input.
        fn connect(&mut self, from: NodeId, output: &str, to: NodeId, input: &str) {
            let (output, input) = self.ports(from, output, to, input);
            self.editor.graph.add_connection(output, input, 0);
        }

        fn disconnect(&mut self, from: NodeId, output: &str, to: NodeId, input: &str) {
            let (output, input) = self.ports(from, output, to, input);
            assert!(self.editor.graph.remove_connection(input, output));
        }

        fn drag(&mut self, node_id: NodeId, by: Vec2) {
            self.editor.node_positions[node_id] += by;
        }

        fn param(&self, node_id: NodeId, name: &str) -> f32 {
            self.editor.graph.get_input(self.editor.graph[node_id].get_input(name).unwrap()).value.actual_value()
        }

        fn set(&mut self, node_id: NodeId, name: &str, value: f32) {
            let input = self.editor.graph[node_id].get_input(name).unwrap();
            self.editor.graph.inputs[input].value.set_actual_value(value);
        }

        fn engine_id(&self, node_id: NodeId) -> EngineNodeId {
            self.user_state.get_engine_node_id(node_id).unwrap()
        }

        /// The graph node now standing for an engine ID.
        fn node(&self, key: EngineNodeId) -> NodeId {
            self.user_state.node_id_map.iter().find(|(_, &k)| k == key).map(|(&id, _)| id).unwrap()
        }

        /// Ends a gesture, a second after the last.
        fn record(&mut self) {
            self.now += Duration::from_secs(1);
            self.history.record(&self.editor, &self.user_state, false, self.now);
        }

        /// A frame in the middle of a gesture.
        fn record_held(&mut self) {
            self.now += Duration::from_millis(16);
            self.history.record(&self.editor, &self.user_state, true, self.now);
        }

        fn snapshot(&self) -> Snapshot {
            Snapshot::capture(&self.editor, &self.user_state, &self.history.anchor)
        }

        fn undo(&mut self) -> Applied {
            self.history.undo(&mut self.editor, &mut self.user_state).expect("something to undo")
        }

        fn redo(&mut self) -> Applied {
            self.history.redo(&mut self.editor, &mut self.user_state).expect("something to redo")
        }

        /// Zooms about the middle of the view, as scrolling does.
        fn zoom(&mut self, scale: f32) {
            let pan_zoom = &mut self.editor.pan_zoom;
            pan_zoom.clip_rect = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0));
            let (zoom_before, pan) = (pan_zoom.zoom, pan_zoom.pan);
            pan_zoom.zoom *= scale;
            let half_size = pan_zoom.clip_rect.size() / 2.0;
            for (_, pos) in self.editor.node_positions.iter_mut() {
                *pos = ((pos.to_vec2() - half_size + pan) * scale + half_size - pan).to_pos2();
            }
            self.history.follow_zoom(zoom_before, pan, &self.editor.pan_zoom);
        }
    }

    fn same(a: &Snapshot, b: &Snapshot) -> bool {
        Step::between(a, b).is_none()
    }

    /// Records an edit, then checks undo puts back what was there before
    /// it, and redo what was there after.
    fn round_trip(rig: &mut Rig, edit: impl FnOnce(&mut Rig)) -> (Applied, Applied) {
        let before = rig.snapshot();
        edit(rig);
        rig.record();
        let after = rig.snapshot();
        assert!(!same(&before, &after), "the edit changed nothing");

        let undone = rig.undo();
        assert!(same(&rig.snapshot(), &before), "undo left {:?}", Step::between(&before, &rig.snapshot()));
        let redone = rig.redo();
        assert!(same(&rig.snapshot(), &after), "redo left {:?}", Step::between(&after, &rig.snapshot()));
        (undone, redone)
    }

    /// An oscillator into a filter into the output.
    fn voice(rig: &mut Rig) -> (NodeId, NodeId, NodeId) {
        let osc = rig.add("osc.sine", pos2(100.0, 100.0));
        let filter = rig.add("filter.svf", pos2(400.0, 100.0));
        let out = rig.add("output.audio", pos2(700.0, 100.0));
        rig.connect(osc, "Out", filter, "In");
        rig.connect(filter, "LowPass", out, "Left");
        rig.record();
        (osc, filter, out)
    }

    fn count(commands: &[EngineCommand], wanted: impl Fn(&EngineCommand) -> bool) -> usize {
        commands.iter().filter(|c| wanted(c)).count()
    }

    #[test]
    fn test_add_round_trips() {
        let mut rig = Rig::new();
        let (undone, redone) = round_trip(&mut rig, |rig| {
            rig.add("osc.sine", pos2(50.0, 60.0));
        });
        assert_eq!(undone.label, "Add Oscillator");
        assert!(matches!(undone.commands[..], [EngineCommand::RemoveModule { node_id: 0 }]));
        // It comes back as the same engine node, where it was
        assert!(matches!(redone.commands[0], EngineCommand::AddModule { node_id: 0, module_id: "osc.sine" }));
        assert_eq!(rig.editor.node_positions[rig.node(0)], pos2(50.0, 60.0));
    }

    #[test]
    fn test_delete_brings_back_values_cables_and_engine_id() {
        let mut rig = Rig::new();
        let (_, filter, _) = voice(&mut rig);
        rig.set(filter, "Cutoff", 440.0);
        rig.record();
        let key = rig.engine_id(filter);

        let (undone, redone) = round_trip(&mut rig, |rig| rig.delete(filter));
        assert_eq!(undone.label, "Delete SVF Filter");

        let commands = &undone.commands;
        assert!(matches!(commands[0], EngineCommand::AddModule { node_id, .. } if node_id == key));
        // A fresh module starts at defaults, so the edited value is sent
        assert_eq!(count(commands, |c| matches!(c,
            EngineCommand::SetParameter { node_id, param_index: 0, value } if *node_id == key && *value == 440.0)), 1);
        assert_eq!(count(commands, |c| matches!(c, EngineCommand::Connect { .. })), 2);
        assert_eq!(count(&redone.commands, |c| matches!(c, EngineCommand::RemoveModule { node_id } if *node_id == key)), 1);

        rig.undo();
        assert_eq!(rig.param(rig.node(key), "Cutoff"), 440.0);
    }

    #[test]
    fn test_connect_round_trips() {
        let mut rig = Rig::new();
        let (osc, filter, out) = voice(&mut rig);
        let (undone, redone) = round_trip(&mut rig, |rig| rig.connect(osc, "Sub", out, "Right"));
        assert_eq!(undone.label, "Connect Oscillator → Audio Output");
        assert_eq!(count(&undone.commands, |c| matches!(c, EngineCommand::Disconnect { .. })), 1);
        assert_eq!(count(&redone.commands, |c| matches!(c, EngineCommand::Connect { .. })), 1);

        // A cable into an exposed knob's jack makes the knob follow it
        let (undone, redone) = round_trip(&mut rig, |rig| rig.connect(osc, "Out", filter, "Cutoff"));
        assert_eq!(count(&undone.commands, |c| matches!(c, EngineCommand::UnmonitorInput { .. })), 1);
        assert_eq!(count(&redone.commands, |c| matches!(c, EngineCommand::MonitorInput { .. })), 1);
    }

    #[test]
    fn test_disconnect_round_trips() {
        let mut rig = Rig::new();
        let (osc, filter, _) = voice(&mut rig);
        let (undone, _) = round_trip(&mut rig, |rig| rig.disconnect(osc, "Out", filter, "In"));
        assert_eq!(undone.label, "Disconnect Oscillator → SVF Filter");
        assert_eq!(count(&undone.commands, |c| matches!(c, EngineCommand::Connect { .. })), 1);
    }

    #[test]
    fn test_repatch_is_one_step() {
        let mut rig = Rig::new();
        let (osc, filter, out) = voice(&mut rig);
        // Dropping a cable on an input that already had one
        let (undone, _) = round_trip(&mut rig, |rig| rig.connect(osc, "Out", out, "Left"));
        assert_eq!(undone.label, "Repatch Oscillator → Audio Output");

        // Dragging a cable's end from one input to another
        let (undone, _) = round_trip(&mut rig, |rig| {
            rig.disconnect(osc, "Out", filter, "In");
            rig.record_held();
            rig.connect(osc, "Out", filter, "Cutoff");
        });
        assert_eq!(undone.label, "Repatch Oscillator → SVF Filter");
    }

    #[test]
    fn test_move_round_trips() {
        let mut rig = Rig::new();
        let (osc, filter, _) = voice(&mut rig);
        let (undone, _) = round_trip(&mut rig, |rig| rig.drag(osc, vec2(30.0, -12.0)));
        assert_eq!(undone.label, "Move Oscillator");
        assert!(undone.commands.is_empty());

        // A selection dragged together
        let (undone, _) = round_trip(&mut rig, |rig| {
            rig.drag(osc, vec2(5.0, 5.0));
            rig.drag(filter, vec2(5.0, 5.0));
        });
        assert_eq!(undone.label, "Move 2 modules");
    }

    #[test]
    fn test_move_undoes_to_the_same_place_after_zooming() {
        let mut rig = Rig::new();
        let (osc, filter, _) = voice(&mut rig);
        rig.editor.pan_zoom.pan = vec2(-80.0, 35.0);
        rig.drag(osc, vec2(200.0, 150.0));
        rig.record();

        // Zooming isn't an edit
        rig.zoom(1.7);
        rig.editor.pan_zoom.pan = vec2(20.0, -60.0);
        rig.zoom(0.8);
        rig.record();
        assert_eq!(rig.history.undo_label(), Some("Move Oscillator"));

        rig.undo();
        // Back beside the filter, at this zoom
        let gap = rig.editor.node_positions[filter] - rig.editor.node_positions[osc];
        let zoom = rig.editor.pan_zoom.zoom;
        assert!((gap - vec2(300.0, 0.0) * zoom).length() < 0.01, "{gap:?}");
    }

    #[test]
    fn test_param_round_trips() {
        let mut rig = Rig::new();
        let (_, filter, _) = voice(&mut rig);
        let key = rig.engine_id(filter);
        let (undone, redone) = round_trip(&mut rig, |rig| rig.set(filter, "Cutoff", 300.0));
        assert_eq!(undone.label, "Set SVF Filter Cutoff");
        assert!(matches!(undone.commands[..],
            [EngineCommand::SetParameter { node_id, param_index: 0, value }] if node_id == key && value == 1000.0));
        assert!(matches!(redone.commands[..], [EngineCommand::SetParameter { value, .. }] if value == 300.0));
    }

    #[test]
    fn test_bypass_round_trips() {
        let mut rig = Rig::new();
        let delay = rig.add("fx.delay", pos2(0.0, 0.0));
        rig.record();
        let (undone, redone) = round_trip(&mut rig, |rig| rig.editor.graph[delay].user_data.bypassed = true);
        assert_eq!(undone.label, "Bypass Stereo Delay");
        assert!(matches!(undone.commands[..], [EngineCommand::SetBypass { bypassed: false, .. }]));
        assert!(matches!(redone.commands[..], [EngineCommand::SetBypass { bypassed: true, .. }]));
    }

    #[test]
    fn test_knob_drag_is_one_step() {
        let mut rig = Rig::new();
        let (_, filter, _) = voice(&mut rig);
        let steps = rig.history.undo.len();
        for cutoff in [900.0, 700.0, 500.0, 350.0] {
            rig.set(filter, "Cutoff", cutoff);
            rig.record_held();
        }
        rig.record();
        assert_eq!(rig.history.undo.len(), steps + 1);

        rig.undo();
        assert_eq!(rig.param(filter, "Cutoff"), 1000.0);
    }

    #[test]
    fn test_quick_changes_to_one_knob_merge() {
        let mut rig = Rig::new();
        let (_, filter, _) = voice(&mut rig);
        let steps = rig.history.undo.len();
        for cutoff in [900.0, 800.0, 700.0] {
            rig.set(filter, "Cutoff", cutoff);
            rig.now += Duration::from_millis(200);
            rig.history.record(&rig.editor, &rig.user_state, false, rig.now);
        }
        assert_eq!(rig.history.undo.len(), steps + 1);

        // Another knob is another step
        rig.set(filter, "Resonance", 0.9);
        rig.now += Duration::from_millis(200);
        rig.history.record(&rig.editor, &rig.user_state, false, rig.now);
        assert_eq!(rig.history.undo.len(), steps + 2);

        rig.undo();
        rig.undo();
        assert_eq!(rig.param(filter, "Cutoff"), 1000.0);
    }

    #[test]
    fn test_midi_cc_is_not_an_edit() {
        let mut rig = Rig::new();
        let (_, filter, _) = voice(&mut rig);
        let steps = rig.history.undo.len();
        rig.set(filter, "Cutoff", 2000.0);
        rig.history.absorb_param(rig.engine_id(filter), 0, 2000.0);
        rig.record();
        assert_eq!(rig.history.undo.len(), steps);
    }

    #[test]
    fn test_new_edit_clears_redo() {
        let mut rig = Rig::new();
        let (osc, _, _) = voice(&mut rig);
        rig.drag(osc, vec2(10.0, 0.0));
        rig.record();
        rig.undo();
        assert!(rig.history.redo_label().is_some());
        rig.drag(osc, vec2(0.0, 10.0));
        rig.record();
        assert!(rig.history.redo_label().is_none());
    }

    #[test]
    fn test_undo_walks_back_to_an_empty_patch() {
        let mut rig = Rig::new();
        let empty = rig.snapshot();
        let (osc, filter, _) = voice(&mut rig);
        rig.set(filter, "Cutoff", 250.0);
        rig.record();
        rig.drag(osc, vec2(0.0, 40.0));
        rig.record();
        rig.delete(filter);
        rig.record();
        let full = rig.snapshot();

        while rig.history.undo(&mut rig.editor, &mut rig.user_state).is_some() {}
        assert!(same(&rig.snapshot(), &empty));
        assert!(rig.editor.graph.nodes.is_empty());
        assert!(rig.editor.node_order.is_empty());

        while rig.history.redo(&mut rig.editor, &mut rig.user_state).is_some() {}
        assert!(same(&rig.snapshot(), &full));
    }
}
