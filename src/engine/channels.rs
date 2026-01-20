//! Engine Channels
//!
//! Lock-free communication between the UI thread and audio engine thread.
//! Uses rtrb ring buffers for SPSC (single-producer, single-consumer) queues.

use rtrb::{Consumer, Producer, RingBuffer};

use super::commands::{EngineCommand, EngineEvent};

/// Default buffer size for command queue (UI -> Engine).
pub const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1024;

/// Default buffer size for event queue (Engine -> UI).
pub const DEFAULT_EVENT_BUFFER_SIZE: usize = 256;

/// Holds both directions of communication channels.
/// Split into producer/consumer pairs for the two threads.
pub struct EngineChannels {
    /// Send commands from UI to engine.
    pub command_tx: Producer<EngineCommand>,
    /// Receive commands in engine from UI.
    pub command_rx: Consumer<EngineCommand>,
    /// Send events from engine to UI.
    pub event_tx: Producer<EngineEvent>,
    /// Receive events in UI from engine.
    pub event_rx: Consumer<EngineEvent>,
}

impl EngineChannels {
    /// Create new engine channels with the specified buffer sizes.
    ///
    /// # Arguments
    /// * `command_capacity` - Number of commands the buffer can hold
    /// * `event_capacity` - Number of events the buffer can hold
    pub fn new(command_capacity: usize, event_capacity: usize) -> Self {
        let (command_tx, command_rx) = RingBuffer::new(command_capacity);
        let (event_tx, event_rx) = RingBuffer::new(event_capacity);

        Self {
            command_tx,
            command_rx,
            event_tx,
            event_rx,
        }
    }

    /// Create new channels with default buffer sizes.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_COMMAND_BUFFER_SIZE, DEFAULT_EVENT_BUFFER_SIZE)
    }

    /// Split the channels into UI-side and Engine-side handles.
    /// This consumes self and returns two handles that can be sent to different threads.
    pub fn split(self) -> (UiHandle, EngineHandle) {
        let ui_handle = UiHandle {
            command_tx: self.command_tx,
            event_rx: self.event_rx,
        };
        let engine_handle = EngineHandle {
            command_rx: self.command_rx,
            event_tx: self.event_tx,
        };
        (ui_handle, engine_handle)
    }
}

/// UI-side handle for communicating with the audio engine.
/// Holds the command producer and event consumer.
pub struct UiHandle {
    command_tx: Producer<EngineCommand>,
    event_rx: Consumer<EngineEvent>,
}

impl UiHandle {
    /// Send a command to the audio engine.
    /// Returns Ok(()) if the command was queued, or Err(cmd) if the buffer is full.
    ///
    /// This is a non-blocking operation - it never waits for space.
    pub fn send_command(&mut self, cmd: EngineCommand) -> Result<(), EngineCommand> {
        self.command_tx
            .push(cmd)
            .map_err(|rtrb::PushError::Full(cmd)| cmd)
    }

    /// Try to send a command, dropping it silently if the buffer is full.
    /// Use this for non-critical commands where dropping is acceptable.
    pub fn send_command_lossy(&mut self, cmd: EngineCommand) {
        let _ = self.command_tx.push(cmd);
    }

    /// Receive an event from the audio engine.
    /// Returns Some(event) if available, None if no events pending.
    ///
    /// This is a non-blocking operation.
    pub fn recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx.pop().ok()
    }

    /// Drain all pending events from the engine.
    /// Returns an iterator over all available events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = EngineEvent> + '_ {
        std::iter::from_fn(|| self.recv_event())
    }

    /// Check how many commands can still be queued.
    pub fn command_slots_available(&self) -> usize {
        self.command_tx.slots()
    }

    /// Check if the command buffer is full.
    pub fn is_command_buffer_full(&self) -> bool {
        self.command_tx.is_full()
    }
}

/// Engine-side handle for communicating with the UI.
/// Holds the command consumer and event producer.
///
/// IMPORTANT: All methods are designed to be real-time safe (non-blocking, no allocations).
pub struct EngineHandle {
    command_rx: Consumer<EngineCommand>,
    event_tx: Producer<EngineEvent>,
}

impl EngineHandle {
    /// Receive a command from the UI.
    /// Returns Some(cmd) if available, None if no commands pending.
    ///
    /// REAL-TIME SAFE: Non-blocking operation.
    pub fn recv_command(&mut self) -> Option<EngineCommand> {
        self.command_rx.pop().ok()
    }

    /// Process all pending commands with the given handler.
    /// The handler is called for each command in order.
    ///
    /// REAL-TIME SAFE: Non-blocking, no allocations.
    pub fn process_commands<F>(&mut self, mut handler: F)
    where
        F: FnMut(EngineCommand),
    {
        while let Some(cmd) = self.recv_command() {
            handler(cmd);
        }
    }

    /// Send an event to the UI.
    /// Returns Ok(()) if the event was queued, or Err(event) if the buffer is full.
    ///
    /// REAL-TIME SAFE: Non-blocking operation.
    pub fn send_event(&mut self, event: EngineEvent) -> Result<(), EngineEvent> {
        self.event_tx
            .push(event)
            .map_err(|rtrb::PushError::Full(event)| event)
    }

    /// Try to send an event, dropping it silently if the buffer is full.
    /// Use this for metering data where dropping old values is acceptable.
    ///
    /// REAL-TIME SAFE: Non-blocking, no allocations.
    pub fn send_event_lossy(&mut self, event: EngineEvent) {
        let _ = self.event_tx.push(event);
    }

    /// Check how many events can still be queued.
    pub fn event_slots_available(&self) -> usize {
        self.event_tx.slots()
    }

    /// Check how many commands are pending.
    pub fn commands_pending(&self) -> usize {
        self.command_rx.slots()
    }
}

// Safety: The handles use rtrb which is designed for safe SPSC cross-thread use.
// Each handle only contains one producer and one consumer, ensuring single-thread access.
unsafe impl Send for UiHandle {}
unsafe impl Send for EngineHandle {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_creation() {
        let channels = EngineChannels::new(64, 32);
        assert_eq!(channels.command_tx.slots(), 64);
        assert_eq!(channels.event_tx.slots(), 32);
    }

    #[test]
    fn test_default_channels() {
        let channels = EngineChannels::with_defaults();
        assert_eq!(channels.command_tx.slots(), DEFAULT_COMMAND_BUFFER_SIZE);
        assert_eq!(channels.event_tx.slots(), DEFAULT_EVENT_BUFFER_SIZE);
    }

    #[test]
    fn test_command_send_receive() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Send command from UI
        let result = ui.send_command(EngineCommand::SetPlaying(true));
        assert!(result.is_ok());

        // Receive in engine
        let cmd = engine.recv_command();
        assert!(cmd.is_some());
        assert!(matches!(cmd.unwrap(), EngineCommand::SetPlaying(true)));
    }

    #[test]
    fn test_event_send_receive() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Send event from engine
        let result = engine.send_event(EngineEvent::OutputLevel {
            left: 0.5,
            right: 0.6,
        });
        assert!(result.is_ok());

        // Receive in UI
        let event = ui.recv_event();
        assert!(event.is_some());
        if let EngineEvent::OutputLevel { left, right } = event.unwrap() {
            assert!((left - 0.5).abs() < f32::EPSILON);
            assert!((right - 0.6).abs() < f32::EPSILON);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_buffer_full_handling() {
        let channels = EngineChannels::new(2, 2);
        let (mut ui, _engine) = channels.split();

        // Fill the buffer
        assert!(ui.send_command(EngineCommand::SetPlaying(true)).is_ok());
        assert!(ui.send_command(EngineCommand::SetPlaying(false)).is_ok());

        // Buffer should be full now
        assert!(ui.is_command_buffer_full());

        // Next send should fail
        let result = ui.send_command(EngineCommand::ClearGraph);
        assert!(result.is_err());

        // The returned command should be the one we tried to send
        if let Err(cmd) = result {
            assert!(matches!(cmd, EngineCommand::ClearGraph));
        }
    }

    #[test]
    fn test_lossy_send() {
        let channels = EngineChannels::new(1, 1);
        let (mut ui, mut engine) = channels.split();

        // Fill buffers
        ui.send_command_lossy(EngineCommand::SetPlaying(true));
        ui.send_command_lossy(EngineCommand::SetPlaying(false)); // Should be dropped

        engine.send_event_lossy(EngineEvent::CpuLoad(0.5));
        engine.send_event_lossy(EngineEvent::CpuLoad(0.6)); // Should be dropped

        // Should only receive one of each
        assert!(engine.recv_command().is_some());
        assert!(engine.recv_command().is_none());

        assert!(ui.recv_event().is_some());
        assert!(ui.recv_event().is_none());
    }

    #[test]
    fn test_process_commands() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Send multiple commands
        ui.send_command_lossy(EngineCommand::SetPlaying(true));
        ui.send_command_lossy(EngineCommand::SetParameter {
            node_id: 1,
            param_index: 0,
            value: 0.5,
        });
        ui.send_command_lossy(EngineCommand::ClearGraph);

        // Process all commands
        let mut count = 0;
        engine.process_commands(|_cmd| {
            count += 1;
        });

        assert_eq!(count, 3);

        // No more commands
        assert!(engine.recv_command().is_none());
    }

    #[test]
    fn test_drain_events() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Send multiple events
        engine.send_event_lossy(EngineEvent::Started);
        engine.send_event_lossy(EngineEvent::CpuLoad(0.3));
        engine.send_event_lossy(EngineEvent::OutputLevel {
            left: 0.1,
            right: 0.2,
        });

        // Drain all events
        let events: Vec<_> = ui.drain_events().collect();
        assert_eq!(events.len(), 3);

        // No more events
        assert!(ui.recv_event().is_none());
    }

    #[test]
    fn test_slots_available() {
        let channels = EngineChannels::new(10, 10);
        let (mut ui, mut engine) = channels.split();

        assert_eq!(ui.command_slots_available(), 10);
        assert_eq!(engine.event_slots_available(), 10);

        ui.send_command_lossy(EngineCommand::SetPlaying(true));
        engine.send_event_lossy(EngineEvent::Started);

        assert_eq!(ui.command_slots_available(), 9);
        assert_eq!(engine.event_slots_available(), 9);
    }

    #[test]
    fn test_commands_pending() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, engine) = channels.split();

        assert_eq!(engine.commands_pending(), 0);

        ui.send_command_lossy(EngineCommand::SetPlaying(true));
        ui.send_command_lossy(EngineCommand::ClearGraph);

        // commands_pending shows items available to read (items in buffer)
        assert_eq!(engine.commands_pending(), 2);
    }

    #[test]
    fn test_handles_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UiHandle>();
        assert_send::<EngineHandle>();
    }

    #[test]
    fn test_multiple_command_types() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Send various command types
        ui.send_command_lossy(EngineCommand::AddModule {
            node_id: 1,
            module_id: "sine_osc",
        });
        ui.send_command_lossy(EngineCommand::Connect {
            from_node: 1,
            from_port: 0,
            to_node: 2,
            to_port: 0,
        });
        ui.send_command_lossy(EngineCommand::Disconnect {
            node_id: 2,
            port: 0,
            is_input: true,
        });
        ui.send_command_lossy(EngineCommand::RemoveModule { node_id: 1 });

        // Verify all received
        let mut commands = Vec::new();
        engine.process_commands(|cmd| commands.push(cmd));

        assert_eq!(commands.len(), 4);
        assert!(matches!(commands[0], EngineCommand::AddModule { .. }));
        assert!(matches!(commands[1], EngineCommand::Connect { .. }));
        assert!(matches!(commands[2], EngineCommand::Disconnect { .. }));
        assert!(matches!(commands[3], EngineCommand::RemoveModule { .. }));
    }

    #[test]
    fn test_empty_receive() {
        let channels = EngineChannels::new(64, 64);
        let (mut ui, mut engine) = channels.split();

        // Nothing sent yet
        assert!(engine.recv_command().is_none());
        assert!(ui.recv_event().is_none());
    }
}
