//! Patch serialization for save/load functionality.
//!
//! This module defines the data structures for serializing synthesizer patches
//! to JSON files. A patch captures the complete state of the node graph including
//! all nodes, their positions, parameter values, and connections.

use serde::{Deserialize, Serialize};

/// Current patch format version.
/// Increment this when making breaking changes to the format.
pub const PATCH_VERSION: u32 = 1;

/// A complete synthesizer patch.
///
/// Contains all the information needed to recreate a graph configuration:
/// nodes with their positions and parameters, and all connections between them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Human-readable name for the patch.
    pub name: String,
    /// Patch format version for future compatibility.
    pub version: u32,
    /// All nodes in the patch.
    pub nodes: Vec<NodeData>,
    /// All connections between nodes.
    pub connections: Vec<ConnectionData>,
}

impl Patch {
    /// Create a new empty patch with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: PATCH_VERSION,
            nodes: Vec::new(),
            connections: Vec::new(),
        }
    }

    /// Check if this patch version is compatible with the current format.
    pub fn is_compatible(&self) -> bool {
        self.version <= PATCH_VERSION
    }
}

impl Default for Patch {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Serialized data for a single node in the patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeData {
    /// Unique identifier for this node within the patch.
    /// Used for referencing in connections.
    pub id: u64,
    /// Module type identifier (e.g., "osc.sine", "filter.svf").
    /// Must match a registered module ID.
    pub module_id: String,
    /// Node position in the graph editor (x, y).
    pub position: (f32, f32),
    /// Parameter values in order they appear in the node.
    /// These are the actual values (Hz for frequency, seconds for time, etc.).
    pub parameters: Vec<ParameterValue>,
}

impl NodeData {
    /// Create new node data.
    pub fn new(id: u64, module_id: impl Into<String>, position: (f32, f32)) -> Self {
        Self {
            id,
            module_id: module_id.into(),
            position,
            parameters: Vec::new(),
        }
    }
}

/// A parameter value that preserves type information for proper restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ParameterValue {
    /// Scalar value (0.0-1.0 range).
    Scalar(f32),
    /// Frequency value in Hz.
    Frequency(f32),
    /// Linear Hz value.
    LinearHz(f32),
    /// Time value in seconds.
    Time(f32),
    /// Linear range value (stored as raw value).
    LinearRange(f32),
    /// Boolean toggle.
    Toggle(bool),
    /// Selection index.
    Select(usize),
}

impl ParameterValue {
    /// Get the value as f32 for engine parameter setting.
    pub fn as_f32(&self) -> f32 {
        match self {
            Self::Scalar(v) => *v,
            Self::Frequency(v) => *v,
            Self::LinearHz(v) => *v,
            Self::Time(v) => *v,
            Self::LinearRange(v) => *v,
            Self::Toggle(v) => if *v { 1.0 } else { 0.0 },
            Self::Select(v) => *v as f32,
        }
    }
}

/// Serialized data for a connection between two nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionData {
    /// Source node ID.
    pub from_node: u64,
    /// Output port name on source node.
    pub from_port: String,
    /// Destination node ID.
    pub to_node: u64,
    /// Input port name on destination node.
    pub to_port: String,
}

impl ConnectionData {
    /// Create new connection data.
    pub fn new(
        from_node: u64,
        from_port: impl Into<String>,
        to_node: u64,
        to_port: impl Into<String>,
    ) -> Self {
        Self {
            from_node,
            from_port: from_port.into(),
            to_node,
            to_port: to_port.into(),
        }
    }
}

/// Error type for patch operations.
#[derive(Debug)]
pub enum PatchError {
    /// File I/O error.
    IoError(std::io::Error),
    /// JSON serialization/deserialization error.
    SerializationError(serde_json::Error),
    /// Incompatible patch version.
    IncompatibleVersion { found: u32, expected: u32 },
    /// Unknown module type in patch.
    UnknownModule(String),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "File error: {}", e),
            Self::SerializationError(e) => write!(f, "Serialization error: {}", e),
            Self::IncompatibleVersion { found, expected } => {
                write!(f, "Incompatible patch version: found {}, expected <= {}", found, expected)
            }
            Self::UnknownModule(id) => write!(f, "Unknown module type: {}", id),
        }
    }
}

impl std::error::Error for PatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(e) => Some(e),
            Self::SerializationError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PatchError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError(err)
    }
}

impl From<serde_json::Error> for PatchError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError(err)
    }
}

/// Save a patch to a JSON file.
pub fn save_to_file(patch: &Patch, path: &std::path::Path) -> Result<(), PatchError> {
    let json = serde_json::to_string_pretty(patch)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load a patch from a JSON file.
pub fn load_from_file(path: &std::path::Path) -> Result<Patch, PatchError> {
    let json = std::fs::read_to_string(path)?;
    let patch: Patch = serde_json::from_str(&json)?;

    // Version check
    if !patch.is_compatible() {
        return Err(PatchError::IncompatibleVersion {
            found: patch.version,
            expected: PATCH_VERSION,
        });
    }

    Ok(patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_creation() {
        let patch = Patch::new("Test Patch");
        assert_eq!(patch.name, "Test Patch");
        assert_eq!(patch.version, PATCH_VERSION);
        assert!(patch.nodes.is_empty());
        assert!(patch.connections.is_empty());
    }

    #[test]
    fn test_patch_serialization() {
        let mut patch = Patch::new("Test");
        patch.nodes.push(NodeData {
            id: 1,
            module_id: "osc.sine".to_string(),
            position: (100.0, 200.0),
            parameters: vec![
                ParameterValue::Frequency(440.0),
                ParameterValue::Scalar(0.5),
            ],
        });
        patch.connections.push(ConnectionData::new(1, "Out", 2, "In"));

        let json = serde_json::to_string(&patch).unwrap();
        let loaded: Patch = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.nodes.len(), 1);
        assert_eq!(loaded.connections.len(), 1);
    }

    #[test]
    fn test_version_compatibility() {
        let patch = Patch::new("Test");
        assert!(patch.is_compatible());

        let future_patch = Patch {
            name: "Future".to_string(),
            version: PATCH_VERSION + 1,
            nodes: vec![],
            connections: vec![],
        };
        assert!(!future_patch.is_compatible());
    }

    #[test]
    fn test_parameter_value_as_f32() {
        assert!((ParameterValue::Scalar(0.5).as_f32() - 0.5).abs() < f32::EPSILON);
        assert!((ParameterValue::Frequency(440.0).as_f32() - 440.0).abs() < f32::EPSILON);
        assert!((ParameterValue::Toggle(true).as_f32() - 1.0).abs() < f32::EPSILON);
        assert!((ParameterValue::Toggle(false).as_f32()).abs() < f32::EPSILON);
        assert!((ParameterValue::Select(2).as_f32() - 2.0).abs() < f32::EPSILON);
    }
}
