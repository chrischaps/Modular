//! Audio graph for managing and processing DSP modules.
//!
//! The AudioGraph holds module instances and their connections, determining
//! the correct processing order via topological sort. It handles all
//! graph manipulation commands from the UI thread in a real-time safe manner.

use std::collections::HashMap;

use crate::dsp::{DspModule, ModuleRegistry, ProcessContext, SignalBuffer, SignalType};
use crate::engine::buffer_pool::BufferPool;
use crate::engine::commands::{EngineCommand, NodeId, PortIndex};

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

/// Stored module data including the instance and parameter values.
struct ModuleData {
    /// The DSP module instance.
    module: Box<dyn DspModule>,
    /// Current parameter values (denormalized, ready to pass to process()).
    parameters: Vec<f32>,
}

impl ModuleData {
    fn new(mut module: Box<dyn DspModule>, sample_rate: f32, block_size: usize) -> Self {
        // Extract default parameter values
        let parameters: Vec<f32> = module
            .parameters()
            .iter()
            .map(|p| p.default)
            .collect();

        // Prepare the module
        module.prepare(sample_rate, block_size);

        Self { module, parameters }
    }
}

/// The audio graph that manages modules and their connections.
///
/// The graph maintains:
/// - A collection of module instances
/// - Connections between module ports
/// - A topologically sorted processing order
/// - Pre-allocated buffers for all signals
pub struct AudioGraph {
    /// Modules indexed by their node ID.
    modules: HashMap<NodeId, ModuleData>,
    /// All connections in the graph.
    connections: Vec<Connection>,
    /// Processing order (topologically sorted node IDs).
    processing_order: Vec<NodeId>,
    /// Pre-allocated signal buffers.
    buffers: BufferPool,
    /// Current sample rate.
    sample_rate: f32,
    /// Current block size.
    block_size: usize,
    /// Reference to the module registry for creating modules.
    /// Note: This is an Option because we may not always have a registry.
    registry: Option<ModuleRegistry>,
    /// Whether the graph needs resorting.
    needs_sort: bool,
}

impl AudioGraph {
    /// Creates a new audio graph with the given sample rate and block size.
    pub fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            modules: HashMap::new(),
            connections: Vec::new(),
            processing_order: Vec::new(),
            buffers: BufferPool::new(block_size),
            sample_rate,
            block_size,
            registry: None,
            needs_sort: false,
        }
    }

    /// Creates a new audio graph with a module registry.
    pub fn with_registry(sample_rate: f32, block_size: usize, registry: ModuleRegistry) -> Self {
        Self {
            modules: HashMap::new(),
            connections: Vec::new(),
            processing_order: Vec::new(),
            buffers: BufferPool::new(block_size),
            sample_rate,
            block_size,
            registry: Some(registry),
            needs_sort: false,
        }
    }

    /// Sets the module registry.
    pub fn set_registry(&mut self, registry: ModuleRegistry) {
        self.registry = Some(registry);
    }

    /// Returns a reference to the processing order.
    pub fn processing_order(&self) -> &[NodeId] {
        &self.processing_order
    }

    /// Returns the number of modules in the graph.
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Returns the number of connections in the graph.
    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// Checks if a module exists in the graph.
    pub fn contains_module(&self, node_id: NodeId) -> bool {
        self.modules.contains_key(&node_id)
    }

    /// Returns a reference to a module by node ID.
    pub fn get_module(&self, node_id: NodeId) -> Option<&dyn DspModule> {
        self.modules.get(&node_id).map(|data| data.module.as_ref())
    }

    /// Returns a mutable reference to a module by node ID.
    pub fn get_module_mut(&mut self, node_id: NodeId) -> Option<&mut Box<dyn DspModule>> {
        self.modules.get_mut(&node_id).map(|data| &mut data.module)
    }

    /// Returns the connections in the graph.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    // ========================================================================
    // Graph Modification Methods
    // ========================================================================

    /// Adds a module to the graph using the registry.
    ///
    /// Returns true if the module was added successfully.
    pub fn add_module(&mut self, node_id: NodeId, module_id: &str) -> bool {
        // Check if node already exists
        if self.modules.contains_key(&node_id) {
            return false;
        }

        // Create module from registry
        let module = match &self.registry {
            Some(registry) => registry.create(module_id),
            None => return false,
        };

        let module = match module {
            Some(m) => m,
            None => return false,
        };

        self.add_module_instance(node_id, module);
        true
    }

    /// Adds a pre-created module instance to the graph.
    pub fn add_module_instance(&mut self, node_id: NodeId, module: Box<dyn DspModule>) {
        // Allocate buffers for output ports
        let ports = module.ports();
        let mut output_index = 0;
        for port in ports {
            if port.is_output() {
                self.buffers.allocate(node_id, output_index, port.signal_type);
                output_index += 1;
            }
        }

        // Create module data with default parameters
        let data = ModuleData::new(module, self.sample_rate, self.block_size);
        self.modules.insert(node_id, data);

        self.needs_sort = true;
    }

    /// Removes a module from the graph.
    ///
    /// Also removes all connections to/from this module.
    pub fn remove_module(&mut self, node_id: NodeId) -> bool {
        if self.modules.remove(&node_id).is_none() {
            return false;
        }

        // Remove all connections involving this node
        self.connections.retain(|conn| {
            conn.from_node != node_id && conn.to_node != node_id
        });

        // Deallocate buffers
        self.buffers.deallocate_node(node_id);

        self.needs_sort = true;
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
        if !self.modules.contains_key(&from_node) || !self.modules.contains_key(&to_node) {
            return false;
        }

        // Check for duplicate connection
        let new_conn = Connection::new(from_node, from_port, to_node, to_port);
        if self.connections.contains(&new_conn) {
            return false;
        }

        // Check that we're not creating a cycle
        // (We'll do a full topological sort to verify)
        self.connections.push(new_conn);

        // Try to sort - if it fails, we have a cycle
        if self.has_cycle() {
            self.connections.pop(); // Remove the connection that caused the cycle
            return false;
        }

        self.needs_sort = true;
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

        let removed = self.connections.len() < original_len;
        if removed {
            self.needs_sort = true;
        }
        removed
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

        let removed = self.connections.len() < original_len;
        if removed {
            self.needs_sort = true;
        }
        removed
    }

    /// Sets a parameter value on a module.
    pub fn set_parameter(&mut self, node_id: NodeId, param_index: usize, value: f32) -> bool {
        if let Some(data) = self.modules.get_mut(&node_id) {
            if param_index < data.parameters.len() {
                data.parameters[param_index] = value;
                return true;
            }
        }
        false
    }

    /// Clears the entire graph.
    pub fn clear(&mut self) {
        self.modules.clear();
        self.connections.clear();
        self.processing_order.clear();
        self.buffers.clear_pool();
        self.needs_sort = false;
    }

    // ========================================================================
    // Topological Sort
    // ========================================================================

    /// Checks if the current graph has a cycle.
    fn has_cycle(&self) -> bool {
        // Use Kahn's algorithm - if we can't process all nodes, there's a cycle
        let sorted = self.compute_topological_order();
        sorted.len() != self.modules.len()
    }

    /// Computes the topological order using Kahn's algorithm.
    fn compute_topological_order(&self) -> Vec<NodeId> {
        // Build in-degree map
        let mut in_degree: HashMap<NodeId, usize> = HashMap::new();

        // Initialize all nodes with 0 in-degree
        for &node_id in self.modules.keys() {
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

        let mut result = Vec::with_capacity(self.modules.len());

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
                // Handled at a higher level
                true
            }
            EngineCommand::ClearGraph => {
                self.clear();
                true
            }
        }
    }

    // ========================================================================
    // Audio Processing
    // ========================================================================

    /// Processes a block of audio through the graph.
    ///
    /// This is the main audio processing method, called from the audio callback.
    /// It processes all modules in topological order.
    pub fn process(&mut self, context: &ProcessContext) {
        // Ensure processing order is up to date
        self.update_processing_order();

        // Clear all output buffers
        self.buffers.clear_all();

        // Process modules in topological order
        for &node_id in &self.processing_order.clone() {
            self.process_module(node_id, context);
        }
    }

    /// Processes a single module.
    fn process_module(&mut self, node_id: NodeId, context: &ProcessContext) {
        // Gather input buffers for this module
        let input_buffers = self.gather_inputs(node_id);

        // Get module data
        let data = match self.modules.get_mut(&node_id) {
            Some(d) => d,
            None => return,
        };

        // Count output ports (for potential future use)
        let _output_port_count = data.module.ports().iter().filter(|p| p.is_output()).count();

        // Create output buffer references
        // We need to collect the signal types first
        let output_types: Vec<SignalType> = data
            .module
            .ports()
            .iter()
            .filter(|p| p.is_output())
            .map(|p| p.signal_type)
            .collect();

        // Create temporary output buffers
        let mut output_buffers: Vec<SignalBuffer> = output_types
            .iter()
            .map(|&t| SignalBuffer::new(context.block_size, t))
            .collect();

        // Get parameter values
        let params = data.parameters.clone();

        // Process the module
        data.module.process(
            &input_buffers.iter().collect::<Vec<_>>(),
            &mut output_buffers,
            &params,
            context,
        );

        // Copy output buffers to the buffer pool
        for (i, output_buf) in output_buffers.into_iter().enumerate() {
            if let Some(pool_buf) = self.buffers.get_mut(node_id, i) {
                pool_buf.samples.copy_from_slice(&output_buf.samples);
            }
        }
    }

    /// Gathers input buffers for a module based on its connections.
    fn gather_inputs(&self, node_id: NodeId) -> Vec<SignalBuffer> {
        let data = match self.modules.get(&node_id) {
            Some(d) => d,
            None => return Vec::new(),
        };

        // Count input ports
        let input_ports: Vec<_> = data
            .module
            .ports()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.is_input())
            .collect();

        let mut inputs = Vec::with_capacity(input_ports.len());

        for (port_idx, port_def) in input_ports {
            // Find connection to this input port
            let connection = self.connections.iter().find(|conn| {
                conn.to_node == node_id && conn.to_port == port_idx
            });

            if let Some(conn) = connection {
                // Get the output buffer from the source module
                // We need to map the from_port (which is a port index) to an output index
                if let Some(source_data) = self.modules.get(&conn.from_node) {
                    let output_idx = self.port_to_output_index(source_data, conn.from_port);
                    if let Some(buf) = self.buffers.get(conn.from_node, output_idx) {
                        inputs.push(buf.clone());
                        continue;
                    }
                }
            }

            // No connection or buffer not found - use default
            let default_value = port_def.default_value;
            let mut buf = SignalBuffer::new(self.block_size, port_def.signal_type);
            buf.fill(default_value);
            inputs.push(buf);
        }

        inputs
    }

    /// Converts a port index to an output buffer index.
    fn port_to_output_index(&self, data: &ModuleData, port_index: PortIndex) -> usize {
        // Count how many output ports come before this port index
        data.module
            .ports()
            .iter()
            .take(port_index)
            .filter(|p| p.is_output())
            .count()
    }

    /// Gets the final output from the audio output module (if present).
    ///
    /// Returns the stereo output buffer as (left, right) slices.
    pub fn get_output(&self) -> Option<(&[f32], &[f32])> {
        // Find the audio output module
        for (&node_id, data) in &self.modules {
            if data.module.info().id == "output.audio" {
                // The AudioOutput module has its own internal buffer
                // We need to access it through a downcast
                // For now, return None - this would need the module to expose its buffer
                // through the trait or a specific accessor
                let _ = node_id; // Suppress unused warning
            }
        }
        None
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
    use crate::dsp::{ModuleCategory, ModuleInfo, ParameterDefinition, PortDefinition};

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

        fn prepare(&mut self, _sample_rate: f32, _max_block_size: usize) {}

        fn process(
            &mut self,
            inputs: &[&SignalBuffer],
            _outputs: &mut [SignalBuffer],
            _params: &[f32],
            _context: &ProcessContext,
        ) {
            if !inputs.is_empty() {
                self.received = inputs[0].samples.clone();
            }
        }

        fn reset(&mut self) {
            self.received.clear();
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
    fn test_clear_graph() {
        let mut graph = AudioGraph::new(44100.0, 256);

        graph.add_module_instance(1, Box::new(TestOscillator::default()));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        graph.clear();

        assert_eq!(graph.module_count(), 0);
        assert_eq!(graph.connection_count(), 0);
        assert!(graph.processing_order().is_empty());
    }

    #[test]
    fn test_process_simple() {
        let mut graph = AudioGraph::new(44100.0, 4);

        graph.add_module_instance(1, Box::new(TestOscillator::new(0.75)));
        graph.add_module_instance(2, Box::new(TestOutput::default()));
        graph.connect(1, 0, 2, 0);

        let ctx = ProcessContext::new(44100.0, 4);
        graph.process(&ctx);

        // The output buffer should have been written to
        // (We can't easily verify this without accessing the internal buffer)
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
        let ctx = ProcessContext::new(44100.0, 4);
        graph.process(&ctx);
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
}
