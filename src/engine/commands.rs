//! Engine Commands and Events
//!
//! Defines the messages that flow between the UI thread and the audio engine thread.
//! All types here must be Send + 'static for safe cross-thread communication.

/// Unique identifier for a node in the audio graph.
/// Maps to the node ID from egui_node_graph2.
pub type NodeId = u64;

/// Index of a port on a module.
pub type PortIndex = usize;

/// Commands sent from the UI thread to the audio engine.
/// These are processed non-blocking in the audio callback.
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// Add a new module instance to the audio graph.
    AddModule {
        /// Unique identifier for this node instance.
        node_id: NodeId,
        /// Static string ID of the module type (from ModuleRegistry).
        module_id: &'static str,
    },

    /// Remove a module from the audio graph.
    RemoveModule {
        /// The node to remove.
        node_id: NodeId,
    },

    /// Connect two ports in the audio graph.
    Connect {
        /// Source node.
        from_node: NodeId,
        /// Output port index on source node.
        from_port: PortIndex,
        /// Destination node.
        to_node: NodeId,
        /// Input port index on destination node.
        to_port: PortIndex,
    },

    /// Disconnect a specific connection.
    Disconnect {
        /// Node with the connection to remove.
        node_id: NodeId,
        /// Port index to disconnect.
        port: PortIndex,
        /// Whether this is an input port (true) or output port (false).
        is_input: bool,
    },

    /// Set a parameter value on a module.
    SetParameter {
        /// Target node.
        node_id: NodeId,
        /// Parameter index.
        param_index: usize,
        /// New value (normalized 0.0-1.0).
        value: f32,
    },

    /// Start or stop audio processing.
    SetPlaying(bool),

    /// Clear the entire audio graph.
    ClearGraph,

    /// Start monitoring an input port for UI feedback.
    /// The engine will send InputValue events with the signal values.
    MonitorInput {
        /// The node containing the input.
        node_id: NodeId,
        /// The input port index to monitor.
        input_index: PortIndex,
    },

    /// Stop monitoring an input port.
    UnmonitorInput {
        /// The node containing the input.
        node_id: NodeId,
        /// The input port index to stop monitoring.
        input_index: PortIndex,
    },

    /// Start monitoring an output port for UI feedback (e.g., LED indicators).
    /// The engine will send OutputValue events with the signal values.
    MonitorOutput {
        /// The node containing the output.
        node_id: NodeId,
        /// The output port index to monitor.
        output_index: PortIndex,
    },

    /// Stop monitoring an output port.
    UnmonitorOutput {
        /// The node containing the output.
        node_id: NodeId,
        /// The output port index to stop monitoring.
        output_index: PortIndex,
    },
}

/// Events sent from the audio engine to the UI thread.
/// These provide feedback for metering and status display.
#[derive(Debug, Clone, Copy)]
pub enum EngineEvent {
    /// Current output levels for metering display.
    OutputLevel {
        /// Left channel peak level (0.0-1.0+).
        left: f32,
        /// Right channel peak level (0.0-1.0+).
        right: f32,
    },

    /// Current CPU load of the audio processing.
    CpuLoad(f32),

    /// Audio processing started.
    Started,

    /// Audio processing stopped.
    Stopped,

    /// An error occurred in the audio engine.
    Error,

    /// Reports the current value at a monitored input port.
    /// Used for animating knobs when their input is connected.
    InputValue {
        /// The node containing the input.
        node_id: NodeId,
        /// The input port index.
        input_index: PortIndex,
        /// The sampled value (typically first sample or average of block).
        value: f32,
    },

    /// Reports the current value at a monitored output port.
    /// Used for LED indicators and other output visualizations.
    OutputValue {
        /// The node containing the output.
        node_id: NodeId,
        /// The output port index.
        output_index: PortIndex,
        /// The sampled value (typically first sample or max of block).
        value: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_debug() {
        let cmd = EngineCommand::SetPlaying(true);
        assert!(format!("{:?}", cmd).contains("SetPlaying"));
    }

    #[test]
    fn test_command_clone() {
        let cmd = EngineCommand::AddModule {
            node_id: 42,
            module_id: "sine_osc",
        };
        let cloned = cmd.clone();
        if let EngineCommand::AddModule { node_id, module_id } = cloned {
            assert_eq!(node_id, 42);
            assert_eq!(module_id, "sine_osc");
        } else {
            panic!("Clone failed");
        }
    }

    #[test]
    fn test_event_copy() {
        let event = EngineEvent::OutputLevel {
            left: 0.5,
            right: 0.7,
        };
        let copied = event;
        if let EngineEvent::OutputLevel { left, right } = copied {
            assert!((left - 0.5).abs() < f32::EPSILON);
            assert!((right - 0.7).abs() < f32::EPSILON);
        } else {
            panic!("Copy failed");
        }
    }

    #[test]
    fn test_connect_command() {
        let cmd = EngineCommand::Connect {
            from_node: 1,
            from_port: 0,
            to_node: 2,
            to_port: 1,
        };
        if let EngineCommand::Connect {
            from_node,
            from_port,
            to_node,
            to_port,
        } = cmd
        {
            assert_eq!(from_node, 1);
            assert_eq!(from_port, 0);
            assert_eq!(to_node, 2);
            assert_eq!(to_port, 1);
        }
    }

    #[test]
    fn test_set_parameter_command() {
        let cmd = EngineCommand::SetParameter {
            node_id: 5,
            param_index: 2,
            value: 0.75,
        };
        if let EngineCommand::SetParameter {
            node_id,
            param_index,
            value,
        } = cmd
        {
            assert_eq!(node_id, 5);
            assert_eq!(param_index, 2);
            assert!((value - 0.75).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn test_command_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EngineCommand>();
    }

    #[test]
    fn test_event_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EngineEvent>();
    }
}
