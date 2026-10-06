//! The audio graph: the patch as modules and cables, edited off the audio thread.
//!
//! [`AudioGraph`] holds the patch description (which modules exist, their
//! parameter values, how they're connected and what the UI is monitoring)
//! and handles the graph editing commands. It never runs audio itself.
//! Whenever the structure changes, [`AudioGraph::compile`] produces a
//! [`GraphPlan`]: the processing order, buffer routing and pre-allocated
//! buffers, resolved up front so the audio thread only has to run it.
//!
//! This runs on the UI thread (inside [`UiHandle`](super::UiHandle)) or
//! directly in the [`OfflineRenderer`](super::OfflineRenderer), so it may
//! allocate freely.

use std::collections::{HashMap, HashSet};

use crate::dsp::{DspModule, ModuleRegistry, PortDefinition, SignalBuffer};
use crate::engine::commands::{EngineCommand, NodeId, PortIndex};
use crate::engine::graph_plan::{
    GraphPlan, InputSource, InputTap, MonitorSource, OutputTap, PlanNode, MAX_INPUTS,
};

/// A connection between two ports in the audio graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    /// Source node ID.
    pub from_node: NodeId,
    /// Output port index on source node.
    pub from_port: PortIndex,
    /// Destination node ID.
    pub to_node: NodeId,
    /// Input port index on destination node.
    pub to_port: PortIndex,
}

impl Connection {
    /// Creates a new connection.
    pub fn new(from_node: NodeId, from_port: PortIndex, to_node: NodeId, to_port: PortIndex) -> Self {
        Self {
            from_node,
            from_port,
            to_node,
            to_port,
        }
    }
}

/// What the graph knows about one module.
struct NodeSpec {
    /// Port definitions, copied from the module so the graph can route
    /// cables after the module itself has moved to the audio thread.
    ports: Vec<PortDefinition>,
    /// Current parameter values (denormalized, ready to pass to process()).
    parameters: Vec<f32>,
    /// A module created and prepared here but not yet handed to a plan.
    /// Once compiled into a plan it lives on the audio thread, and later
    /// plans take it over from their predecessor.
    fresh: Option<Box<dyn DspModule>>,
}

impl NodeSpec {
    fn new(mut module: Box<dyn DspModule>, sample_rate: f32, block_size: usize) -> Self {
        module.prepare(sample_rate, block_size);
        Self {
            ports: module.ports().to_vec(),
            parameters: module.parameters().iter().map(|p| p.default).collect(),
            fresh: Some(module),
        }
    }

    /// Maps a port index to its index among the node's output ports, or
    /// `None` if the port isn't an output.
    fn output_index(&self, port_index: PortIndex) -> Option<usize> {
        let port = self.ports.get(port_index)?;
        port.is_output()
            .then(|| self.ports[..port_index].iter().filter(|p| p.is_output()).count())
    }

    fn output_count(&self) -> usize {
        self.ports.iter().filter(|p| p.is_output()).count()
    }

    /// Input ports as (port index, definition), in port order.
    fn inputs(&self) -> impl Iterator<Item = (PortIndex, &PortDefinition)> {
        self.ports.iter().enumerate().filter(|(_, p)| p.is_input())
    }
}

/// The patch being edited: modules, connections and monitors.
///
/// Edits mark the graph dirty; [`take_plan`](Self::take_plan) then compiles
/// a new [`GraphPlan`] for the audio thread. Every plan compiled must be
/// installed, in order, with [`GraphPlan::take_over`] from the plan before
/// it, since modules carried over between plans exist only in the running one.
pub struct AudioGraph {
    nodes: HashMap<NodeId, NodeSpec>,
    /// All connections in the graph.
    connections: Vec<Connection>,
    /// Processing order (topologically sorted node IDs).
    processing_order: Vec<NodeId>,
    /// Sample rate new modules are prepared at.
    sample_rate: f32,
    /// Largest block a compiled plan will process.
    block_size: usize,
    /// Registry for creating modules by ID.
    registry: Option<ModuleRegistry>,
    /// Whether the graph needs resorting.
    needs_sort: bool,
    /// Whether the structure changed since the last compiled plan.
    dirty: bool,
    /// Inputs that should report values back to UI for knob animation.
    /// Key: (node_id, input_port_index).
    monitored_inputs: HashSet<(NodeId, PortIndex)>,
    /// Outputs that should report values back to UI for LED indicators.
    /// Key: (node_id, output_port_index).
    monitored_outputs: HashSet<(NodeId, PortIndex)>,
}

impl AudioGraph {
    /// Creates a new audio graph with the given sample rate and block size.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            nodes: HashMap::new(),
            connections: Vec::new(),
            processing_order: Vec::new(),
            sample_rate,
            block_size,
            registry: None,
            needs_sort: false,
            dirty: false,
            monitored_inputs: HashSet::new(),
            monitored_outputs: HashSet::new(),
        }
    }

    /// Creates a new audio graph with a module registry.
    pub fn with_registry(sample_rate: f32, block_size: usize, registry: ModuleRegistry) -> Self {
        Self {
            registry: Some(registry),
            ..Self::new(sample_rate, block_size)
        }
    }

    /// Sets the module registry.
    pub fn set_registry(&mut self, registry: ModuleRegistry) {
        self.registry = Some(registry);
    }

    /// The sample rate new modules are prepared at.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// The largest block compiled plans will process.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Sets the sample rate and block size that modules are prepared for and
    /// plans are compiled with. Modules not yet handed to a plan are
    /// re-prepared; modules already running are the audio side's to update
    /// (see [`GraphPlan::set_sample_rate`]).
    pub fn set_audio_config(&mut self, sample_rate: f32, block_size: usize) {
        if sample_rate == self.sample_rate && block_size == self.block_size {
            return;
        }
        if block_size != self.block_size {
            // Running plans hold buffers sized for the old block
            self.dirty = true;
        }
        self.sample_rate = sample_rate;
        self.block_size = block_size;
        for module in self.nodes.values_mut().filter_map(|spec| spec.fresh.as_mut()) {
            module.prepare(sample_rate, block_size);
        }
    }

    /// Returns a reference to the processing order.
    pub fn processing_order(&self) -> &[NodeId] {
        &self.processing_order
    }

    /// Returns the number of modules in the graph.
    pub fn module_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the number of connections in the graph.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Checks if a module exists in the graph.
    pub fn contains_module(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(&node_id)
    }

    /// Returns the connections in the graph.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Returns a module's current parameter values.
    pub fn parameters(&self, node_id: NodeId) -> Option<&[f32]> {
        self.nodes.get(&node_id).map(|spec| spec.parameters.as_slice())
    }

    /// Returns true if the structure changed since the last compiled plan.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Forces the next [`take_plan`](Self::take_plan) to compile, e.g. to
    /// deliver a parameter change whose own message couldn't be queued.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // ========================================================================
    // Graph Modification Methods
    // ========================================================================

    /// Adds a module to the graph using the registry.
    ///
    /// Returns true if the module was added successfully.
    pub fn add_module(&mut self, node_id: NodeId, module_id: &str) -> bool {
        // Check if node already exists
        if self.nodes.contains_key(&node_id) {
            return false;
        }

        let module = match self.registry.as_ref().and_then(|r| r.create(module_id)) {
            Some(m) => m,
            None => return false,
        };

        self.add_module_instance(node_id, module);
        true
    }

    /// Adds a pre-created module instance to the graph, preparing it.
    pub fn add_module_instance(&mut self, node_id: NodeId, module: Box<dyn DspModule>) {
        let spec = NodeSpec::new(module, self.sample_rate, self.block_size);
        self.nodes.insert(node_id, spec);
        self.needs_sort = true;
        self.dirty = true;
    }

    /// Removes a module from the graph.
    ///
    /// Also removes all connections to/from this module.
    pub fn remove_module(&mut self, node_id: NodeId) -> bool {
        if self.nodes.remove(&node_id).is_none() {
            return false;
        }

        // Remove all connections involving this node
        self.connections.retain(|conn| {
            conn.from_node != node_id && conn.to_node != node_id
        });

        self.needs_sort = true;
        self.dirty = true;
        true
    }

    /// Connects two ports.
    ///
    /// Returns true if the connection was made successfully.
    pub fn connect(
        &mut self,
        from_node: NodeId,
        from_port: PortIndex,
        to_node: NodeId,
        to_port: PortIndex,
    ) -> bool {
        // Check that both nodes exist
        if !self.nodes.contains_key(&from_node) || !self.nodes.contains_key(&to_node) {
            return false;
        }

        // Check for duplicate connection
        let new_conn = Connection::new(from_node, from_port, to_node, to_port);
        if self.connections.contains(&new_conn) {
            return false;
        }

        // An input takes one cable: a new connection replaces any existing one
        let replaced = self
            .connections
            .iter()
            .position(|c| c.to_node == to_node && c.to_port == to_port)
            .map(|index| self.connections.remove(index));

        // Check that we're not creating a cycle
        // (We'll do a full topological sort to verify)
        self.connections.push(new_conn);

        // Try to sort - if it fails, we have a cycle
        if self.has_cycle() {
            self.connections.pop(); // Remove the connection that caused the cycle
            self.connections.extend(replaced); // Keep the cable that was there
            return false;
        }

        self.needs_sort = true;
        self.dirty = true;
        true
    }

    /// Disconnects a specific port.
    ///
    /// If `is_input` is true, removes the connection TO this port.
    /// If `is_input` is false, removes all connections FROM this port.
    pub fn disconnect(&mut self, node_id: NodeId, port: PortIndex, is_input: bool) -> bool {
        let original_len = self.connections.len();

        if is_input {
            // Remove connection TO this input port
            self.connections.retain(|conn| {
                !(conn.to_node == node_id && conn.to_port == port)
            });
        } else {
            // Remove all connections FROM this output port
            self.connections.retain(|conn| {
                !(conn.from_node == node_id && conn.from_port == port)
            });
        }

        self.connections_changed(original_len)
    }

    /// Disconnects a specific connection.
    pub fn disconnect_connection(
        &mut self,
        from_node: NodeId,
        from_port: PortIndex,
        to_node: NodeId,
        to_port: PortIndex,
    ) -> bool {
        let original_len = self.connections.len();
        self.connections.retain(|conn| {
            !(conn.from_node == from_node
                && conn.from_port == from_port
                && conn.to_node == to_node
                && conn.to_port == to_port)
        });

        self.connections_changed(original_len)
    }

    /// Marks the graph for resorting if connections were removed.
    fn connections_changed(&mut self, original_len: usize) -> bool {
        let removed = self.connections.len() < original_len;
        if removed {
            self.needs_sort = true;
            self.dirty = true;
        }
        removed
    }

    /// Sets a parameter value on a module.
    ///
    /// This updates the graph's copy, which the next compiled plan carries.
    /// A running plan is updated separately with [`GraphPlan::set_parameter`].
    pub fn set_parameter(&mut self, node_id: NodeId, param_index: usize, value: f32) -> bool {
        match self.nodes.get_mut(&node_id).and_then(|spec| spec.parameters.get_mut(param_index)) {
            Some(param) => {
                *param = value;
                true
            }
            None => false,
        }
    }

    /// Clears the entire graph.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.connections.clear();
        self.processing_order.clear();
        self.needs_sort = false;
        self.dirty = true;
        self.monitored_inputs.clear();
        self.monitored_outputs.clear();
    }

    /// Start monitoring an input port for UI feedback.
    pub fn monitor_input(&mut self, node_id: NodeId, input_index: PortIndex) {
        self.dirty |= self.monitored_inputs.insert((node_id, input_index));
    }

    /// Stop monitoring an input port.
    pub fn unmonitor_input(&mut self, node_id: NodeId, input_index: PortIndex) {
        self.dirty |= self.monitored_inputs.remove(&(node_id, input_index));
    }

    /// Start monitoring an output port for UI feedback (e.g., LED indicators).
    pub fn monitor_output(&mut self, node_id: NodeId, output_index: PortIndex) {
        self.dirty |= self.monitored_outputs.insert((node_id, output_index));
    }

    /// Stop monitoring an output port.
    pub fn unmonitor_output(&mut self, node_id: NodeId, output_index: PortIndex) {
        self.dirty |= self.monitored_outputs.remove(&(node_id, output_index));
    }

    // ========================================================================
    // Topological Sort
    // ========================================================================

    /// Checks if the current graph has a cycle.
    fn has_cycle(&self) -> bool {
        // Use Kahn's algorithm - if we can't process all nodes, there's a cycle
        let sorted = self.compute_topological_order();
        sorted.len() != self.nodes.len()
    }

    /// Computes the topological order using Kahn's algorithm.
    fn compute_topological_order(&self) -> Vec<NodeId> {
        // Build in-degree map
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();

        // Initialize all nodes with 0 in-degree
        for &node_id in self.nodes.keys() {
            in_degree.insert(node_id, 0);
        }

        // Count incoming edges for each node
        for conn in &self.connections {
            if let Some(degree) = in_degree.get_mut(&conn.to_node) {
                *degree += 1;
            }
        }

        // Start with nodes that have no incoming edges
        let mut queue: Vec<NodeId> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(&node_id, _)| node_id)
            .collect();

        // Sort the queue for deterministic ordering
        queue.sort();

        let mut result = Vec::with_capacity(self.nodes.len());

        while let Some(node_id) = queue.pop() {
            result.push(node_id);

            // Find all nodes that depend on this one
            for conn in &self.connections {
                if conn.from_node == node_id {
                    if let Some(degree) = in_degree.get_mut(&conn.to_node) {
                        *degree -= 1;
                        if *degree == 0 {
                            // Insert in sorted position for determinism
                            let insert_pos = queue.binary_search(&conn.to_node).unwrap_or_else(|p| p);
                            queue.insert(insert_pos, conn.to_node);
                        }
                    }
                }
            }
        }

        result
    }

    /// Updates the processing order if needed.
    pub fn update_processing_order(&mut self) {
        if self.needs_sort {
            self.processing_order = self.compute_topological_order();
            self.needs_sort = false;
        }
    }

    // ========================================================================
    // Command Handling
    // ========================================================================

    /// Handles an engine command.
    ///
    /// Returns true if the command was handled successfully.
    pub fn handle_command(&mut self, command: EngineCommand) -> bool {
        match command {
            EngineCommand::AddModule { node_id, module_id } => {
                self.add_module(node_id, module_id)
            }
            EngineCommand::RemoveModule { node_id } => {
                // Also remove any monitored inputs/outputs for this node
                self.monitored_inputs.retain(|(n, _)| *n != node_id);
                self.monitored_outputs.retain(|(n, _)| *n != node_id);
                self.remove_module(node_id)
            }
            EngineCommand::Connect {
                from_node,
                from_port,
                to_node,
                to_port,
            } => self.connect(from_node, from_port, to_node, to_port),
            EngineCommand::Disconnect {
                node_id,
                port,
                is_input,
            } => self.disconnect(node_id, port, is_input),
            EngineCommand::SetParameter {
                node_id,
                param_index,
                value,
            } => self.set_parameter(node_id, param_index, value),
            EngineCommand::SetPlaying(_) => {
                // Handled on the audio side
                true
            }
            EngineCommand::ClearGraph => {
                self.clear();
                true
            }
            EngineCommand::MonitorInput { node_id, input_index } => {
                self.monitor_input(node_id, input_index);
                true
            }
            EngineCommand::UnmonitorInput { node_id, input_index } => {
                self.unmonitor_input(node_id, input_index);
                true
            }
            EngineCommand::MonitorOutput { node_id, output_index } => {
                self.monitor_output(node_id, output_index);
                true
            }
            EngineCommand::UnmonitorOutput { node_id, output_index } => {
                self.unmonitor_output(node_id, output_index);
                true
            }
        }
    }

    // ========================================================================
    // Plan Compilation
    // ========================================================================

    /// Compiles a plan if the structure changed since the last one.
    pub fn take_plan(&mut self) -> Option<Box<GraphPlan>> {
        self.dirty.then(|| self.compile())
    }

    /// Compiles the graph into a [`GraphPlan`] for the audio thread.
    ///
    /// Modules created since the last plan move into this one; modules
    /// already running are left for [`GraphPlan::take_over`] to carry across.
    pub fn compile(&mut self) -> Box<GraphPlan> {
        self.update_processing_order();
        self.dirty = false;

        let block_size = self.block_size;
        let mut plan = Box::new(GraphPlan::empty(block_size));

        // Where each node's outputs start in the plan's output buffers
        let mut output_base: HashMap<NodeId, usize> = HashMap::with_capacity(self.nodes.len());
        // Which cable feeds each input
        let feeds: HashMap<(NodeId, PortIndex), &Connection> = self
            .connections
            .iter()
            .map(|conn| ((conn.to_node, conn.to_port), conn))
            .collect();

        // Resolves the buffer an upstream output port writes to
        let source_buffer = |conn: &Connection, output_base: &HashMap<NodeId, usize>| {
            let base = output_base.get(&conn.from_node)?;
            let source = self.nodes.get(&conn.from_node)?;
            Some(base + source.output_index(conn.from_port)?)
        };

        for &node_id in &self.processing_order {
            let Some(spec) = self.nodes.get(&node_id) else {
                continue;
            };

            let mut inputs = Vec::new();
            for (port_index, port) in spec.inputs() {
                // Sources precede this node in processing order, so their
                // output buffers are already laid out
                let source = feeds
                    .get(&(node_id, port_index))
                    .and_then(|conn| source_buffer(conn, &output_base));
                inputs.push(match source {
                    Some(buffer) => InputSource::Output(buffer),
                    None => {
                        let mut buffer = SignalBuffer::unconnected(block_size, port.signal_type);
                        buffer.fill(port.default_value);
                        plan.defaults.push(buffer);
                        plan.default_values.push(port.default_value);
                        InputSource::Default(plan.defaults.len() - 1)
                    }
                });
            }
            debug_assert!(inputs.len() <= MAX_INPUTS, "module has more than {MAX_INPUTS} inputs");
            inputs.truncate(MAX_INPUTS);

            let start = plan.outputs.len();
            output_base.insert(node_id, start);
            plan.outputs.extend(
                spec.ports
                    .iter()
                    .filter(|p| p.is_output())
                    .map(|p| SignalBuffer::new(block_size, p.signal_type)),
            );

            plan.nodes.push(PlanNode {
                node_id,
                module: None,
                params: spec.parameters.clone(),
                inputs,
                outputs: start..plan.outputs.len(),
            });
        }

        // Monitor taps, in a stable order
        let mut monitored_inputs: Vec<_> = self.monitored_inputs.iter().copied().collect();
        monitored_inputs.sort_unstable();
        for (node_id, input_index) in monitored_inputs {
            let Some(spec) = self.nodes.get(&node_id) else {
                continue;
            };
            let source = match feeds.get(&(node_id, input_index)) {
                Some(conn) => source_buffer(conn, &output_base).map(MonitorSource::Output),
                None => spec
                    .inputs()
                    .nth(input_index)
                    .map(|(_, port)| MonitorSource::Constant(port.default_value)),
            };
            if let Some(source) = source {
                plan.input_taps.push(InputTap { node_id, input_index, source });
            }
        }

        let mut monitored_outputs: Vec<_> = self.monitored_outputs.iter().copied().collect();
        monitored_outputs.sort_unstable();
        for (node_id, output_index) in monitored_outputs {
            let (Some(spec), Some(&base)) = (self.nodes.get(&node_id), output_base.get(&node_id)) else {
                continue;
            };
            if output_index < spec.output_count() {
                plan.output_taps.push(OutputTap { node_id, output_index, buffer: base + output_index });
            }
        }

        // Hand over modules that haven't run yet
        for node in &mut plan.nodes {
            if let Some(spec) = self.nodes.get_mut(&node.node_id) {
                node.module = spec.fresh.take();
            }
        }

        plan
    }
}

impl Default for AudioGraph {
    fn default() -> Self {
        Self::new(44100.0, 256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::{ModuleCategory, ModuleInfo, ParameterDefinition, ProcessContext, SignalType};

    // ========================================================================
    // Test Module Implementations
    // ========================================================================

    /// A simple test oscillator that outputs a constant value.
    struct TestOscillator {
        value: f32,
    }

    impl TestOscillator {
        fn new(value: f32) -> Self {
            Self { value }
        }
    }

    impl Default for TestOscillator {
        fn default() -> Self {
            Self::new(0.5)
        }
    }

    impl DspModule for TestOscillator {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.osc",
                name: "Test Oscillator",
                category: ModuleCategory::Source,
                description: "Test oscillator",
            };
            &INFO
        }

        fn ports(&self) -> &[PortDefinition] {
            static PORTS: &[PortDefinition] = &[PortDefinition {
                id: "out",
                name: "Output",
                signal_type: SignalType::Audio,
                direction: crate::dsp::PortDirection::Output,
                default_value: 0.0,
            }];
            PORTS
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &[]
        }

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            _inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            outputs[0].fill(self.value);
        }

        fn reset(&mut self) {}
    }

    /// A simple test output module.
    struct TestOutput {
        received: Vec<f32>,
    }

    impl Default for TestOutput {
        fn default() -> Self {
            Self { received: Vec::new() }
        }
    }

    impl DspModule for TestOutput {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.output",
                name: "Test Output",
                category: ModuleCategory::Output,
                description: "Test output",
            };
            &INFO
        }

        fn ports(&self) -> &[PortDefinition] {
            static PORTS: &[PortDefinition] = &[PortDefinition {
                id: "in",
                name: "Input",
                signal_type: SignalType::Audio,
                direction: crate::dsp::PortDirection::Input,
                default_value: 0.0,
            }];
            PORTS
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &[]
        }

        fn prepare(&mut self, _sample_rate: f32, max_block_size: usize) {
            self.received = vec![0.0; max_block_size];
        }

        fn process(
            &mut self,
            inputs: &[&SignalBuffer],
            _outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            if let Some(input) = inputs.first() {
                self.received[..input.len()].copy_from_slice(&input.samples);
            }
        }

        fn reset(&mut self) {
            self.received.fill(0.0);
        }

        fn get_audio_output(&self) -> Option<(&[f32], &[f32])> {
            Some((&self.received, &self.received))
        }
    }

    /// A passthrough module for testing chains.
    struct TestPassthrough;

    impl Default for TestPassthrough {
        fn default() -> Self {
            Self
        }
    }

    impl DspModule for TestPassthrough {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.passthrough",
                name: "Test Passthrough",
                category: ModuleCategory::Utility,
                description: "Test passthrough",
            };
            &INFO
        }

        fn ports(&self) -> &[PortDefinition] {
            static PORTS: &[PortDefinition] = &[
                PortDefinition {
                    id: "in",
                    name: "Input",
                    signal_type: SignalType::Audio,
                    direction: crate::dsp::PortDirection::Input,
                    default_value: 0.0,
                },
                PortDefinition {
                    id: "out",
                    name: "Output",
                    signal_type: SignalType::Audio,
                    direction: crate::dsp::PortDirection::Output,
                    default_value: 0.0,
                },
            ];
            PORTS
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &[]
        }

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            if !inputs.is_empty() && !outputs.is_empty() {
                outputs[0].samples.copy_from_slice(&inputs[0].samples);
            }
        }

        fn reset(&mut self) {}
    }

    /// Counts the blocks it has processed and outputs the count, so tests can
    /// tell whether a module kept its state across plans.
    #[derive(Default)]
    struct TestCounter {
        blocks: f32,
    }

    impl DspModule for TestCounter {
        fn info(&self) -> &ModuleInfo {
            static INFO: ModuleInfo = ModuleInfo {
                id: "test.counter",
                name: "Test Counter",
                category: ModuleCategory::Source,
                description: "Counts blocks",
            };
            &INFO
        }

        fn ports(&self) -> &[PortDefinition] {
            static PORTS: &[PortDefinition] = &[PortDefinition {
                id: "out",
                name: "Output",
                signal_type: SignalType::Audio,
                direction: crate::dsp::PortDirection::Output,
                default_value: 0.0,
            }];
            PORTS
        }

        fn parameters(&self) -> &[ParameterDefinition] {
            &[]
        }

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            _inputs: &[&SignalBuffer],
            outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            self.blocks += 1.0;
            outputs[0].fill(self.blocks);
        }

        fn reset(&mut self) {
            self.blocks = 0.0;
        }
    }

    /// Compiles `graph` and runs one block of `block_size` samples.
    fn run_block(graph: &mut AudioGraph, block_size: usize) -> Box<GraphPlan> {
        let mut plan = graph.compile();
        plan.process(&ProcessContext::new(44100.0, block_size));
        plan
    }

    /// Installs the graph's next plan in place of `plan`, as the audio thread does.
    fn install(graph: &mut AudioGraph, plan: &mut Box<GraphPlan>) {
        let mut next = graph.take_plan().expect("graph changed");
        next.take_over(plan);
        *plan = next;
    }

    // ========================================================================
    // Tests
    // ========================================================================

    #[test]
    fn test_graph_creation() {
        let graph = AudioGraph::new(44100.0, 256);
        assert_eq!(graph.module_count(), 0);
        assert_eq!(graph.connection_count(), 0);
        assert!(graph.processing_order().is_empty());
    }

    #[test]
    fn test_add_module_instance() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));

        assert_eq!(graph.module_count(), 1);
        assert!(graph.contains_module(1));
        assert!(!graph.contains_module(2));
    }

    #[test]
    fn test_add_multiple_modules() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        assert_eq!(graph.module_count(), 2);
        assert!(graph.contains_module(1));
        assert!(graph.contains_module(2));
    }

    #[test]
    fn test_remove_module() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        assert!(graph.remove_module(1));
        assert_eq!(graph.module_count(), 1);
        assert!(!graph.contains_module(1));
        assert!(graph.contains_module(2));

        // Removing non-existent module returns false
        assert!(!graph.remove_module(999));
    }

    #[test]
    fn test_connect_modules() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        assert!(graph.connect(1, 0, 2, 0));
        assert_eq!(graph.connection_count(), 1);

        let conns = graph.connections();
        assert_eq!(conns[0].from_node, 1);
        assert_eq!(conns[0].from_port, 0);
        assert_eq!(conns[0].to_node, 2);
        assert_eq!(conns[0].to_port, 0);
    }

    #[test]
    fn test_connect_nonexistent_fails() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));

        // Can't connect to non-existent node
        assert!(!graph.connect(1, 0, 999, 0));
        assert!(!graph.connect(999, 0, 1, 0));
        assert_eq!(graph.connection_count(), 0);
    }

    #[test]
    fn test_duplicate_connection_fails() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        assert!(graph.connect(1, 0, 2, 0));
        assert!(!graph.connect(1, 0, 2, 0)); // Duplicate
        assert_eq!(graph.connection_count(), 1);
    }

    #[test]
    fn test_disconnect() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        assert!(graph.disconnect(2, 0, true)); // Disconnect input
        assert_eq!(graph.connection_count(), 0);
    }

    #[test]
    fn test_remove_module_removes_connections() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));
        graph.add_module_instance(3, Box::new(TestOutput::default()));

        graph.connect(1, 0, 2, 0);
        graph.connect(2, 1, 3, 0);

        assert_eq!(graph.connection_count(), 2);

        // Remove middle module
        graph.remove_module(2);

        assert_eq!(graph.connection_count(), 0);
    }

    #[test]
    fn test_processing_order_simple() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        graph.update_processing_order();

        let order = graph.processing_order();
        assert_eq!(order.len(), 2);

        // Oscillator should come before output
        let osc_pos = order.iter().position(|&id| id == 1).unwrap();
        let out_pos = order.iter().position(|&id| id == 2).unwrap();
        assert!(osc_pos < out_pos);
    }

    #[test]
    fn test_processing_order_chain() {
        let mut graph = AudioGraph::new(44100.0, 256);

        // Create a chain: osc -> pass -> output
        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));
        graph.add_module_instance(3, Box::new(TestOutput::default()));

        graph.connect(1, 0, 2, 0);
        graph.connect(2, 1, 3, 0);

        graph.update_processing_order();

        let order = graph.processing_order();
        assert_eq!(order.len(), 3);

        let pos1 = order.iter().position(|&id| id == 1).unwrap();
        let pos2 = order.iter().position(|&id| id == 2).unwrap();
        let pos3 = order.iter().position(|&id| id == 3).unwrap();

        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestPassthrough::default()));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));

        // Create a valid connection
        assert!(graph.connect(1, 1, 2, 0));

        // Try to create a cycle - should fail
        assert!(!graph.connect(2, 1, 1, 0));
        assert_eq!(graph.connection_count(), 1);
    }

    #[test]
    fn test_new_cable_replaces_existing_one() {
        let mut graph = AudioGraph::new(44100.0, 256);
        for id in 1..=3 {
            graph.add_module_instance(id, Box::new(TestPassthrough::default()));
        }

        assert!(graph.connect(1, 1, 3, 0));
        assert!(graph.connect(2, 1, 3, 0));

        // An input holds one cable: the second connection replaced the first
        assert_eq!(graph.connection_count(), 1);
        assert_eq!(graph.connections()[0].from_node, 2);
    }

    #[test]
    fn test_rejected_replacement_keeps_existing_cable() {
        let mut graph = AudioGraph::new(44100.0, 256);
        for id in 1..=3 {
            graph.add_module_instance(id, Box::new(TestPassthrough::default()));
        }
        assert!(graph.connect(1, 1, 2, 0)); // 1 -> 2
        assert!(graph.connect(3, 1, 1, 0)); // 3 -> 1

        // Replacing 3 -> 1 with 2 -> 1 would form a cycle, so it is refused
        assert!(!graph.connect(2, 1, 1, 0));
        assert_eq!(graph.connection_count(), 2);
        assert!(graph.connections().iter().any(|c| c.from_node == 3 && c.to_node == 1));
    }

    #[test]
    fn test_unconnected_inputs_are_marked() {
        let mut graph = AudioGraph::new(44100.0, 64);
        graph.add_module_instance(1, Box::new(TestPassthrough::default()));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));

        let plan = graph.compile();
        let node = plan.nodes.iter().find(|n| n.node_id == 2).unwrap();
        let InputSource::Default(index) = node.inputs[0] else {
            panic!("nothing plugged in yet");
        };
        assert!(!plan.defaults[index].is_connected());

        graph.connect(1, 1, 2, 0);
        let plan = graph.compile();
        let node = plan.nodes.iter().find(|n| n.node_id == 2).unwrap();
        let InputSource::Output(index) = node.inputs[0] else {
            panic!("cable plugged in");
        };
        assert!(plan.outputs[index].is_connected());
    }

    #[test]
    fn test_clear_graph() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        graph.clear();

        assert_eq!(graph.module_count(), 0);
        assert_eq!(graph.connection_count(), 0);
        assert!(graph.processing_order().is_empty());
        assert!(graph.compile().is_empty());
    }

    #[test]
    fn test_process_simple() {
        let mut graph = AudioGraph::new(44100.0, 4);

        graph.add_module_instance(1, Box::new(TestOscillator::new(0.75)));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        let plan = run_block(&mut graph, 4);
        let (left, _) = plan.audio_output().expect("output module");
        assert_eq!(left, &[0.75; 4]);
    }

    #[test]
    fn test_short_block_reaches_modules() {
        let mut graph = AudioGraph::new(44100.0, 8);
        graph.add_module_instance(1, Box::new(TestOscillator::new(0.5)));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));
        graph.connect(1, 0, 2, 0);

        // A block shorter than the plan's capacity: modules see that length
        let mut plan = graph.compile();
        plan.process(&ProcessContext::new(44100.0, 3));
        assert!(plan.outputs.iter().all(|b| b.samples == [0.5; 3]));
        assert!(plan.defaults.iter().all(|b| b.len() == 3));

        plan.process(&ProcessContext::new(44100.0, 8));
        assert!(plan.outputs.iter().all(|b| b.samples == [0.5; 8]));
    }

    #[test]
    fn test_unpatched_input_holds_port_default() {
        struct DefaultedInput;
        impl DspModule for DefaultedInput {
            fn info(&self) -> &ModuleInfo {
                static INFO: ModuleInfo = ModuleInfo {
                    id: "test.defaulted",
                    name: "Test Defaulted Input",
                    category: ModuleCategory::Utility,
                    description: "Checks its unpatched input",
                };
                &INFO
            }
            fn ports(&self) -> &[PortDefinition] {
                static PORTS: &[PortDefinition] = &[PortDefinition {
                    id: "in",
                    name: "Input",
                    signal_type: SignalType::Control,
                    direction: crate::dsp::PortDirection::Input,
                    default_value: 0.25,
                }];
                PORTS
            }
            fn parameters(&self) -> &[ParameterDefinition] {
                &[]
            }
            fn prepare(&mut self, _: f32, _: usize) {}
            fn process(&mut self, inputs: &[&SignalBuffer], _: &mut [SignalBuffer], _: &[f32], ctx: &ProcessContext) {
                assert!(!inputs[0].is_connected());
                assert_eq!(inputs[0].samples, vec![0.25; ctx.block_size]);
            }
            fn reset(&mut self) {}
        }

        let mut graph = AudioGraph::new(44100.0, 16);
        graph.add_module_instance(1, Box::new(DefaultedInput));
        let mut plan = graph.compile();
        // Shrinking then growing the block must refill with the default
        for block in [16, 5, 16] {
            plan.process(&ProcessContext::new(44100.0, block));
        }
    }

    #[test]
    fn test_running_modules_survive_recompile() {
        let mut graph = AudioGraph::new(44100.0, 4);
        graph.add_module_instance(1, Box::new(TestCounter::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        let mut plan = run_block(&mut graph, 4);
        plan.process(&ProcessContext::new(44100.0, 4));

        // Editing the patch builds a new plan; the counter keeps counting
        graph.add_module_instance(3, Box::new(TestOscillator::default()));
        install(&mut graph, &mut plan);
        plan.process(&ProcessContext::new(44100.0, 4));

        let (left, _) = plan.audio_output().unwrap();
        assert_eq!(left, &[3.0; 4], "counter was carried over, not recreated");
    }

    #[test]
    fn test_removed_module_stays_with_old_plan() {
        let mut graph = AudioGraph::new(44100.0, 4);
        graph.add_module_instance(1, Box::new(TestCounter::default()));
        graph.add_module_instance(2, Box::new(TestCounter::default()));
        let mut plan = graph.compile();

        graph.remove_module(2);
        let mut next = graph.take_plan().unwrap();
        next.take_over(&mut plan);

        assert_eq!(next.len(), 1);
        assert!(next.nodes[0].module.is_some());
        // The removed module is still owned by the old plan, to drop off-thread
        let leftover: Vec<_> = plan.nodes.iter().filter(|n| n.module.is_some()).map(|n| n.node_id).collect();
        assert_eq!(leftover, vec![2]);
    }

    #[test]
    fn test_readded_node_gets_a_fresh_module() {
        let mut graph = AudioGraph::new(44100.0, 4);
        graph.add_module_instance(1, Box::new(TestCounter::default()));
        let mut plan = run_block(&mut graph, 4);

        // Same node ID, new module, in one batch: the old state must not leak in
        graph.remove_module(1);
        graph.add_module_instance(1, Box::new(TestCounter::default()));
        install(&mut graph, &mut plan);
        plan.process(&ProcessContext::new(44100.0, 4));

        assert_eq!(plan.outputs[0].samples, [1.0; 4]);
    }

    #[test]
    fn test_take_plan_only_when_dirty() {
        let mut graph = AudioGraph::new(44100.0, 4);
        assert!(graph.take_plan().is_none());

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        assert!(graph.take_plan().is_some());
        assert!(graph.take_plan().is_none());

        // Parameter changes travel separately and need no new plan
        graph.set_parameter(1, 0, 0.5);
        assert!(graph.take_plan().is_none());

        graph.monitor_output(1, 0);
        assert!(graph.take_plan().is_some());
        graph.monitor_output(1, 0);
        assert!(graph.take_plan().is_none(), "already monitored");
    }

    #[test]
    fn test_monitor_taps() {
        let mut graph = AudioGraph::new(44100.0, 4);
        graph.add_module_instance(1, Box::new(TestOscillator::new(-0.5)));
        graph.add_module_instance(2, Box::new(TestPassthrough::default()));
        graph.monitor_output(1, 0);
        graph.monitor_input(2, 0);

        // Unpatched input reports the port default
        let mut plan = run_block(&mut graph, 4);
        assert_eq!(plan.output_values().collect::<Vec<_>>(), vec![(1, 0, -0.5)]);
        assert_eq!(plan.input_values().collect::<Vec<_>>(), vec![(2, 0, 0.0)]);

        // Patched input reports the incoming signal
        graph.connect(1, 0, 2, 0);
        install(&mut graph, &mut plan);
        plan.process(&ProcessContext::new(44100.0, 4));
        assert_eq!(plan.input_values().collect::<Vec<_>>(), vec![(2, 0, -0.5)]);
    }

    #[test]
    fn test_set_parameter() {
        let mut graph = AudioGraph::new(44100.0, 256);

        // Create a module with parameters
        struct ParamModule {
            params: Vec<ParameterDefinition>,
        }

        impl Default for ParamModule {
            fn default() -> Self {
                Self {
                    params: vec![ParameterDefinition::normalized("gain", "Gain", 0.5)],
                }
            }
        }

        impl DspModule for ParamModule {
            fn info(&self) -> &ModuleInfo {
                static INFO: ModuleInfo = ModuleInfo {
                    id: "test.param",
                    name: "Test Param",
                    category: ModuleCategory::Utility,
                    description: "Test",
                };
                &INFO
            }
            fn ports(&self) -> &[PortDefinition] {
                &[]
            }
            fn parameters(&self) -> &[ParameterDefinition] {
                &self.params
            }
            fn prepare(&mut self, _: f32, _: usize) {}
            fn process(&mut self, _: &[&SignalBuffer], _: &mut [SignalBuffer], _: &[f32], _: &ProcessContext) {}
            fn reset(&mut self) {}
        }

        graph.add_module_instance(1, Box::new(ParamModule::default()));

        assert!(graph.set_parameter(1, 0, 0.8));
        assert!(!graph.set_parameter(1, 5, 0.5)); // Invalid index
        assert!(!graph.set_parameter(999, 0, 0.5)); // Invalid node

        // The compiled plan carries the value, and can be updated in place
        let mut plan = graph.compile();
        assert_eq!(plan.nodes[0].params, vec![0.8]);
        assert!(plan.set_parameter(1, 0, 0.3));
        assert!(!plan.set_parameter(1, 5, 0.3));
        assert_eq!(plan.nodes[0].params, vec![0.3]);
    }

    #[test]
    fn test_handle_command_add_remove() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();
        registry.register::<TestOutput>();

        let mut graph = AudioGraph::with_registry(44100.0, 256, registry);

        // Add via command
        assert!(graph.handle_command(EngineCommand::AddModule {
            node_id: 1,
            module_id: "test.osc",
        }));
        assert_eq!(graph.module_count(), 1);

        // Remove via command
        assert!(graph.handle_command(EngineCommand::RemoveModule { node_id: 1 }));
        assert_eq!(graph.module_count(), 0);
    }

    #[test]
    fn test_handle_command_connect_disconnect() {
        let mut registry = ModuleRegistry::new();
        registry.register::<TestOscillator>();
        registry.register::<TestOutput>();

        let mut graph = AudioGraph::with_registry(44100.0, 256, registry);

        graph.handle_command(EngineCommand::AddModule {
            node_id: 1,
            module_id: "test.osc",
        });
        graph.handle_command(EngineCommand::AddModule {
            node_id: 2,
            module_id: "test.output",
        });

        // Connect via command
        assert!(graph.handle_command(EngineCommand::Connect {
            from_node: 1,
            from_port: 0,
            to_node: 2,
            to_port: 0,
        }));
        assert_eq!(graph.connection_count(), 1);

        // Disconnect via command
        assert!(graph.handle_command(EngineCommand::Disconnect {
            node_id: 2,
            port: 0,
            is_input: true,
        }));
        assert_eq!(graph.connection_count(), 0);
    }

    #[test]
    fn test_handle_command_clear() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        graph.handle_command(EngineCommand::ClearGraph);

        assert_eq!(graph.module_count(), 0);
    }

    #[test]
    fn test_default() {
        let graph = AudioGraph::default();
        assert_eq!(graph.module_count(), 0);
    }

    #[test]
    fn test_unconnected_modules_process() {
        let mut graph = AudioGraph::new(44100.0, 4);

        // Add modules without connecting them
        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));

        // Should not panic
        run_block(&mut graph, 4);
    }

    #[test]
    fn test_parallel_modules() {
        let mut graph = AudioGraph::new(44100.0, 4);

        // Add two independent oscillators (no connections between them)
        graph.add_module_instance(1, Box::new(TestOscillator::new(0.3)));
        graph.add_module_instance(2, Box::new(TestOscillator::new(0.7)));

        graph.update_processing_order();

        // Both should be in the processing order
        let order = graph.processing_order();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn test_connection_struct() {
        let conn1 = Connection::new(1, 0, 2, 1);
        let conn2 = Connection::new(1, 0, 2, 1);
        let conn3 = Connection::new(1, 0, 3, 1);

        assert_eq!(conn1, conn2);
        assert_ne!(conn1, conn3);
    }

    #[test]
    fn test_builtin_modules_fit_input_array() {
        let registry = crate::engine::create_module_registry();
        for info in registry.list_modules() {
            let (id, module) = (info.id, registry.create(info.id).unwrap());
            let inputs = module.ports().iter().filter(|p| p.is_input()).count();
            assert!(inputs <= MAX_INPUTS, "{id} has {inputs} inputs");
        }
    }
}
