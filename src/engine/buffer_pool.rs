//! Pre-allocated buffer pool for real-time audio processing.
//!
//! The BufferPool manages a collection of pre-allocated SignalBuffers
//! that can be assigned to module ports. This avoids memory allocation
//! in the audio thread, which is critical for glitch-free playback.

use crate::dsp::signal::{SignalBuffer, SignalType};
use crate::engine::commands::NodeId;

/// A buffer slot identified by node and port.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferSlot {
    /// The node that owns this buffer.
    pub node_id: NodeId,
    /// The port index on the node (output ports only).
    pub port_index: usize,
}

impl BufferSlot {
    /// Creates a new buffer slot identifier.
    pub fn new(node_id: NodeId, port_index: usize) -> Self {
        Self { node_id, port_index }
    }
}

/// A pre-allocated pool of signal buffers for the audio graph.
///
/// Each output port of each module gets its own buffer. Input ports
/// read from the buffers of the output ports they're connected to.
///
/// The pool handles:
/// - Pre-allocation of all buffers at startup
/// - Resizing buffers when block size changes
/// - Clearing all buffers at the start of each processing block
pub struct BufferPool {
    /// All allocated buffers, indexed by (node_id, port_index).
    /// Using a Vec of tuples for simplicity and cache locality.
    buffers: Vec<(BufferSlot, SignalBuffer)>,
    /// Current block size.
    block_size: usize,
}

impl BufferPool {
    /// Creates a new buffer pool with the given initial block size.
    pub fn new(block_size: usize) -> Self {
        Self {
            buffers: Vec::new(),
            block_size,
        }
    }

    /// Allocates a buffer for a specific output port.
    ///
    /// If a buffer already exists for this slot, it is replaced.
    pub fn allocate(&mut self, node_id: NodeId, port_index: usize, signal_type: SignalType) {
        let slot = BufferSlot::new(node_id, port_index);

        // Check if buffer already exists for this slot
        if let Some(pos) = self.buffers.iter().position(|(s, _)| *s == slot) {
            self.buffers[pos].1 = SignalBuffer::new(self.block_size, signal_type);
        } else {
            self.buffers.push((slot, SignalBuffer::new(self.block_size, signal_type)));
        }
    }

    /// Removes all buffers associated with a node.
    pub fn deallocate_node(&mut self, node_id: NodeId) {
        self.buffers.retain(|(slot, _)| slot.node_id != node_id);
    }

    /// Gets a reference to a buffer by slot.
    pub fn get(&self, node_id: NodeId, port_index: usize) -> Option<&SignalBuffer> {
        let slot = BufferSlot::new(node_id, port_index);
        self.buffers
            .iter()
            .find(|(s, _)| *s == slot)
            .map(|(_, buf)| buf)
    }

    /// Gets a mutable reference to a buffer by slot.
    pub fn get_mut(&mut self, node_id: NodeId, port_index: usize) -> Option<&mut SignalBuffer> {
        let slot = BufferSlot::new(node_id, port_index);
        self.buffers
            .iter_mut()
            .find(|(s, _)| *s == slot)
            .map(|(_, buf)| buf)
    }

    /// Clears all buffers (sets all samples to zero).
    ///
    /// This should be called at the start of each processing block.
    pub fn clear_all(&mut self) {
        for (_, buffer) in &mut self.buffers {
            buffer.clear();
        }
    }

    /// Resizes all buffers to a new block size.
    ///
    /// Called when the audio engine's block size changes.
    pub fn resize_all(&mut self, new_block_size: usize) {
        self.block_size = new_block_size;
        for (_, buffer) in &mut self.buffers {
            buffer.resize(new_block_size);
        }
    }

    /// Returns the current block size.
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Returns the total number of allocated buffers.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Returns true if no buffers are allocated.
    pub fn is_empty(&self) -> bool {
        self.buffers.is_empty()
    }

    /// Clears all buffers from the pool.
    pub fn clear_pool(&mut self) {
        self.buffers.clear();
    }
}

impl Default for BufferPool {
    fn default() -> Self {
        Self::new(256)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_creation() {
        let pool = BufferPool::new(512);
        assert_eq!(pool.block_size(), 512);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_buffer_allocation() {
        let mut pool = BufferPool::new(256);

        pool.allocate(1, 0, SignalType::Audio);
        pool.allocate(1, 1, SignalType::Control);
        pool.allocate(2, 0, SignalType::Audio);

        assert_eq!(pool.len(), 3);
        assert!(!pool.is_empty());

        // Check buffer types and sizes
        let buf1 = pool.get(1, 0).unwrap();
        assert_eq!(buf1.signal_type, SignalType::Audio);
        assert_eq!(buf1.len(), 256);

        let buf2 = pool.get(1, 1).unwrap();
        assert_eq!(buf2.signal_type, SignalType::Control);

        let buf3 = pool.get(2, 0).unwrap();
        assert_eq!(buf3.signal_type, SignalType::Audio);
    }

    #[test]
    fn test_buffer_get_nonexistent() {
        let pool = BufferPool::new(256);
        assert!(pool.get(999, 0).is_none());
    }

    #[test]
    fn test_buffer_deallocation() {
        let mut pool = BufferPool::new(256);

        pool.allocate(1, 0, SignalType::Audio);
        pool.allocate(1, 1, SignalType::Audio);
        pool.allocate(2, 0, SignalType::Audio);

        assert_eq!(pool.len(), 3);

        // Deallocate all buffers for node 1
        pool.deallocate_node(1);

        assert_eq!(pool.len(), 1);
        assert!(pool.get(1, 0).is_none());
        assert!(pool.get(1, 1).is_none());
        assert!(pool.get(2, 0).is_some());
    }

    #[test]
    fn test_buffer_clear_all() {
        let mut pool = BufferPool::new(4);

        pool.allocate(1, 0, SignalType::Audio);

        // Write some data
        if let Some(buf) = pool.get_mut(1, 0) {
            buf.fill(0.5);
        }

        // Verify data was written
        assert!(pool.get(1, 0).unwrap().samples.iter().all(|&s| s == 0.5));

        // Clear all buffers
        pool.clear_all();

        // Verify buffers are cleared
        assert!(pool.get(1, 0).unwrap().samples.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_buffer_resize_all() {
        let mut pool = BufferPool::new(256);

        pool.allocate(1, 0, SignalType::Audio);
        pool.allocate(2, 0, SignalType::Audio);

        assert_eq!(pool.get(1, 0).unwrap().len(), 256);

        pool.resize_all(512);

        assert_eq!(pool.block_size(), 512);
        assert_eq!(pool.get(1, 0).unwrap().len(), 512);
        assert_eq!(pool.get(2, 0).unwrap().len(), 512);
    }

    #[test]
    fn test_buffer_slot_equality() {
        let slot1 = BufferSlot::new(1, 0);
        let slot2 = BufferSlot::new(1, 0);
        let slot3 = BufferSlot::new(1, 1);
        let slot4 = BufferSlot::new(2, 0);

        assert_eq!(slot1, slot2);
        assert_ne!(slot1, slot3);
        assert_ne!(slot1, slot4);
    }

    #[test]
    fn test_buffer_reallocation() {
        let mut pool = BufferPool::new(256);

        pool.allocate(1, 0, SignalType::Audio);

        // Write some data
        if let Some(buf) = pool.get_mut(1, 0) {
            buf.fill(0.5);
        }

        // Re-allocate the same slot
        pool.allocate(1, 0, SignalType::Control);

        // Buffer should be replaced (new type, cleared data)
        let buf = pool.get(1, 0).unwrap();
        assert_eq!(buf.signal_type, SignalType::Control);
        assert!(buf.samples.iter().all(|&s| s == 0.0));

        // Should still be only 1 buffer
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_clear_pool() {
        let mut pool = BufferPool::new(256);

        pool.allocate(1, 0, SignalType::Audio);
        pool.allocate(2, 0, SignalType::Audio);

        assert_eq!(pool.len(), 2);

        pool.clear_pool();

        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_default() {
        let pool = BufferPool::default();
        assert_eq!(pool.block_size(), 256);
        assert!(pool.is_empty());
    }
}
