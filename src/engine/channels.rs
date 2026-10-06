//! Engine Channels
//!
//! Lock-free communication between the UI thread and audio engine thread.
//! Uses rtrb ring buffers for SPSC (single-producer, single-consumer) queues:
//!
//! - **messages** (UI -> audio): compiled plans, parameter changes, play/stop
//! - **retired plans** (audio -> UI): replaced plans, to be dropped off the
//!   audio thread
//! - **events** (audio -> UI): metering, monitor values, status
//! - **scope frames** (audio -> UI): oscilloscope captures, by value
//!
//! The UI side owns the [`AudioGraph`]. Structural commands edit it there,
//! and [`UiHandle::flush`] ships the result to the audio thread as a
//! compiled [`GraphPlan`], so nothing on the audio side ever allocates.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::sync::Arc;

use rtrb::{Consumer, Producer, PushError, RingBuffer};

use super::audio_graph::AudioGraph;
use super::audio_processor::create_module_registry;
use super::commands::{AudioMessage, EngineCommand, EngineEvent, ScopeFrame};
use super::graph_plan::GraphPlan;

/// Default buffer size for the message queue (UI -> Engine).
pub const DEFAULT_COMMAND_BUFFER_SIZE: usize = 1024;

/// Default buffer size for event queue (Engine -> UI).
pub const DEFAULT_EVENT_BUFFER_SIZE: usize = 256;

/// The most compiled plans that may be on their way to, or held by, the
/// audio thread at once. This bounds the queue that returns retired plans,
/// so the audio thread always has room to hand one back.
pub const MAX_PLANS_IN_FLIGHT: usize = 4;

/// Oscilloscope captures that can wait for the UI.
const SCOPE_FRAME_BUFFER_SIZE: usize = 8;

/// Sample rate and block size, shared so the UI prepares new modules for the
/// settings the audio thread is actually running at.
struct AudioConfig {
    sample_rate_bits: AtomicU32,
    block_size: AtomicUsize,
}

impl AudioConfig {
    fn new(sample_rate: f32, block_size: usize) -> Self {
        Self {
            sample_rate_bits: AtomicU32::new(sample_rate.to_bits()),
            block_size: AtomicUsize::new(block_size),
        }
    }

    fn load(&self) -> (f32, usize) {
        (
            f32::from_bits(self.sample_rate_bits.load(Ordering::Acquire)),
            self.block_size.load(Ordering::Acquire),
        )
    }

    fn store(&self, sample_rate: f32, block_size: usize) {
        self.sample_rate_bits.store(sample_rate.to_bits(), Ordering::Release);
        self.block_size.store(block_size, Ordering::Release);
    }
}

/// Play/stop changes waiting for room in the message queue, collapsed to
/// what the audio thread still needs to see: at most a stop, then a play.
#[derive(Debug, Default)]
struct UnsentTransport {
    /// A stop to deliver before `playing`, so its reset of module state
    /// isn't skipped by a quick stop-then-play.
    stop_first: bool,
    playing: Option<bool>,
}

impl UnsentTransport {
    fn push(&mut self, playing: bool) {
        self.stop_first = playing && (self.stop_first || self.playing == Some(false));
        self.playing = Some(playing);
    }

    fn peek(&self) -> Option<bool> {
        if self.stop_first {
            Some(false)
        } else {
            self.playing
        }
    }

    fn pop(&mut self) {
        if self.stop_first {
            self.stop_first = false;
        } else {
            self.playing = None;
        }
    }
}

/// Both ends of the engine's communication channels.
pub struct EngineChannels {
    ui: UiHandle,
    engine: EngineHandle,
}

impl EngineChannels {
    /// Create new engine channels with the specified buffer sizes.
    ///
    /// # Arguments
    /// * `command_capacity` - Number of messages the UI -> engine queue can hold
    /// * `event_capacity` - Number of events the engine -> UI queue can hold
    pub fn new(command_capacity: usize, event_capacity: usize) -> Self {
        let (message_tx, message_rx) = RingBuffer::new(command_capacity);
        let (retired_tx, retired_rx) = RingBuffer::new(MAX_PLANS_IN_FLIGHT);
        let (event_tx, event_rx) = RingBuffer::new(event_capacity);
        let (scope_tx, scope_rx) = RingBuffer::new(SCOPE_FRAME_BUFFER_SIZE);

        // Placeholder settings until an AudioProcessor reports the real ones
        let graph = AudioGraph::with_registry(44100.0, 256, create_module_registry());
        let (sample_rate, block_size) = (graph.sample_rate(), graph.block_size());
        let config = Arc::new(AudioConfig::new(sample_rate, block_size));

        Self {
            ui: UiHandle {
                graph,
                message_tx,
                retired_rx,
                event_rx,
                scope_rx,
                config: Arc::clone(&config),
                unsent_plan: None,
                unsent_transport: UnsentTransport::default(),
                // The processor starts with an empty plan of its own, which
                // it retires to us like any other
                plans_in_flight: 1,
            },
            engine: EngineHandle {
                message_rx,
                retired_tx,
                event_tx,
                scope_tx,
                config,
            },
        }
    }

    /// Create new channels with default buffer sizes.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_COMMAND_BUFFER_SIZE, DEFAULT_EVENT_BUFFER_SIZE)
    }

    /// Split the channels into UI-side and Engine-side handles.
    /// This consumes self and returns two handles that can be sent to different threads.
    pub fn split(self) -> (UiHandle, EngineHandle) {
        (self.ui, self.engine)
    }
}

/// UI-side handle for communicating with the audio engine.
///
/// Owns the [`AudioGraph`]. Graph edits are applied to it immediately and
/// reach the audio thread on the next [`flush`](Self::flush), batched into a
/// single compiled plan. Parameter changes and play/stop go straight through.
/// Nothing sent through it is dropped when the queue is full.
pub struct UiHandle {
    graph: AudioGraph,
    message_tx: Producer<AudioMessage>,
    retired_rx: Consumer<Box<GraphPlan>>,
    event_rx: Consumer<EngineEvent>,
    scope_rx: Consumer<ScopeFrame>,
    config: Arc<AudioConfig>,
    /// A compiled plan that didn't fit in the queue yet. Plans must arrive in
    /// order, so no newer plan is compiled until this one is sent.
    unsent_plan: Option<Box<GraphPlan>>,
    /// Play/stop changes that didn't fit in the queue yet.
    unsent_transport: UnsentTransport,
    /// Plans sent (or held by the audio thread) and not yet returned.
    plans_in_flight: usize,
}

impl UiHandle {
    /// Send a command to the audio engine.
    ///
    /// No command is ever lost, and this never waits for space:
    /// - Graph edits are applied to the UI-side graph and delivered by
    ///   [`flush`](Self::flush) as a compiled plan, held back until the queue
    ///   has room.
    /// - Parameter changes are queued immediately. If the queue is full the
    ///   value is still recorded in the graph, and travels with the next plan.
    /// - Play/stop is queued immediately, or kept until `flush` finds room.
    pub fn send_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::SetParameter { node_id, param_index, value } => {
                self.graph.set_parameter(node_id, param_index, value);
                let message = AudioMessage::SetParameter { node_id, param_index, value };
                if self.message_tx.push(message).is_err() {
                    // Make sure the value travels with the next plan instead
                    self.graph.mark_dirty();
                }
            }
            EngineCommand::SetPlaying(playing) => {
                self.unsent_transport.push(playing);
                self.send_transport();
            }
            other => {
                self.graph.handle_command(other);
            }
        }
    }

    /// Sends any play/stop changes that are waiting for room in the queue.
    /// Returns true if none are left waiting.
    fn send_transport(&mut self) -> bool {
        while let Some(playing) = self.unsent_transport.peek() {
            if self.message_tx.push(AudioMessage::SetPlaying(playing)).is_err() {
                return false;
            }
            self.unsent_transport.pop();
        }
        true
    }

    /// Delivers pending graph edits to the audio thread as a compiled plan,
    /// and drops plans the audio thread has retired.
    ///
    /// Call once per UI frame, after sending that frame's commands. Returns
    /// true if everything sent so far is on its way. If it returns false, the
    /// audio thread is behind (or not running): call again soon, and the
    /// held-back edits follow as soon as there is room.
    pub fn flush(&mut self) -> bool {
        while let Ok(retired) = self.retired_rx.pop() {
            drop(retired);
            self.plans_in_flight = self.plans_in_flight.saturating_sub(1);
        }

        let (sample_rate, block_size) = self.config.load();
        self.graph.set_audio_config(sample_rate, block_size);

        if !self.send_transport() {
            return false;
        }

        loop {
            if self.plans_in_flight >= MAX_PLANS_IN_FLIGHT {
                break;
            }
            let Some(plan) = self.unsent_plan.take().or_else(|| self.graph.take_plan()) else {
                break;
            };
            match self.message_tx.push(AudioMessage::InstallPlan(plan)) {
                Ok(()) => self.plans_in_flight += 1,
                Err(PushError::Full(AudioMessage::InstallPlan(plan))) => {
                    self.unsent_plan = Some(plan);
                    break;
                }
                Err(PushError::Full(_)) => unreachable!("pushed a plan"),
            }
        }

        self.unsent_plan.is_none() && !self.graph.is_dirty()
    }

    /// The UI-side graph: the patch as the engine will play it.
    pub fn graph(&self) -> &AudioGraph {
        &self.graph
    }

    /// Receive an event from the audio engine.
    /// Returns Some(event) if available, None if no events pending.
    ///
    /// This is a non-blocking operation.
    pub fn recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx
            .pop()
            .ok()
            .or_else(|| self.scope_rx.pop().ok().map(ScopeFrame::into_event))
    }

    /// Drain all pending events from the engine.
    /// Returns an iterator over all available events.
    pub fn drain_events(&mut self) -> impl Iterator<Item = EngineEvent> + '_ {
        std::iter::from_fn(|| self.recv_event())
    }

    /// Check how many messages can still be queued.
    pub fn command_slots_available(&self) -> usize {
        self.message_tx.slots()
    }

    /// Check if the message buffer is full.
    pub fn is_command_buffer_full(&self) -> bool {
        self.message_tx.is_full()
    }
}

/// Engine-side handle for communicating with the UI.
///
/// IMPORTANT: All methods are designed to be real-time safe (non-blocking, no allocations).
pub struct EngineHandle {
    message_rx: Consumer<AudioMessage>,
    retired_tx: Producer<Box<GraphPlan>>,
    event_tx: Producer<EngineEvent>,
    scope_tx: Producer<ScopeFrame>,
    config: Arc<AudioConfig>,
}

impl EngineHandle {
    /// Receive a message from the UI.
    /// Returns Some(message) if available, None if none are pending.
    ///
    /// REAL-TIME SAFE: Non-blocking operation.
    pub fn recv_message(&mut self) -> Option<AudioMessage> {
        self.message_rx.pop().ok()
    }

    /// Hands a replaced plan back to the UI thread to be dropped there.
    ///
    /// REAL-TIME SAFE: The UI never has more than [`MAX_PLANS_IN_FLIGHT`]
    /// plans out, so there is always room. Should that ever fail, the plan is
    /// dropped here rather than lost.
    pub fn retire_plan(&mut self, plan: Box<GraphPlan>) {
        if let Err(PushError::Full(plan)) = self.retired_tx.push(plan) {
            debug_assert!(false, "retired plan queue full");
            drop(plan);
        }
    }

    /// Records the settings the audio thread runs at, so the UI prepares new
    /// modules for them.
    pub fn set_audio_config(&self, sample_rate: f32, block_size: usize) {
        self.config.store(sample_rate, block_size);
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

    /// Send an oscilloscope capture, dropping it if the UI is behind.
    ///
    /// REAL-TIME SAFE: Non-blocking, no allocations.
    pub fn send_scope_frame_lossy(&mut self, frame: ScopeFrame) {
        let _ = self.scope_tx.push(frame);
    }

    /// Check how many events can still be queued.
    pub fn event_slots_available(&self) -> usize {
        self.event_tx.slots()
    }

    /// Check how many messages are pending.
    pub fn messages_pending(&self) -> usize {
        self.message_rx.slots()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::OutputLevels;

    fn add(node_id: u64, module_id: &'static str) -> EngineCommand {
        EngineCommand::AddModule { node_id, module_id }
    }

    /// Plays the audio thread's part: installs each plan, retiring the old one.
    fn run_audio_side(engine: &mut EngineHandle, current: &mut Box<GraphPlan>) -> Vec<&'static str> {
        let mut seen = Vec::new();
        while let Some(message) = engine.recv_message() {
            match message {
                AudioMessage::InstallPlan(mut plan) => {
                    plan.take_over(current);
                    let old = std::mem::replace(current, plan);
                    engine.retire_plan(old);
                    seen.push("plan");
                }
                AudioMessage::SetParameter { node_id, param_index, value } => {
                    current.set_parameter(node_id, param_index, value);
                    seen.push("param");
                }
                AudioMessage::SetPlaying(playing) => seen.push(if playing { "play" } else { "stop" }),
            }
        }
        seen
    }

    #[test]
    fn test_default_channels() {
        let (ui, engine) = EngineChannels::with_defaults().split();
        assert_eq!(ui.command_slots_available(), DEFAULT_COMMAND_BUFFER_SIZE);
        assert_eq!(engine.event_slots_available(), DEFAULT_EVENT_BUFFER_SIZE);
    }

    #[test]
    fn test_graph_edits_are_batched_into_one_plan() {
        let (mut ui, mut engine) = EngineChannels::with_defaults().split();
        let mut plan = Box::new(GraphPlan::empty(256));

        ui.send_command(add(1, "osc.sine"));
        ui.send_command(add(2, "output.audio"));
        ui.send_command(EngineCommand::Connect { from_node: 1, from_port: 5, to_node: 2, to_port: 2 });
        assert_eq!(engine.messages_pending(), 0, "nothing sent before flush");

        assert!(ui.flush());
        assert_eq!(run_audio_side(&mut engine, &mut plan), vec!["plan"]);
        assert_eq!(plan.len(), 2);

        // Nothing changed: nothing more to send
        assert!(ui.flush());
        assert_eq!(engine.messages_pending(), 0);
    }

    #[test]
    fn test_parameters_go_straight_through() {
        let (mut ui, mut engine) = EngineChannels::with_defaults().split();
        let mut plan = Box::new(GraphPlan::empty(256));
        ui.send_command(add(1, "osc.sine"));
        ui.flush();
        run_audio_side(&mut engine, &mut plan);

        ui.send_command(EngineCommand::SetParameter { node_id: 1, param_index: 0, value: 220.0 });
        ui.send_command(EngineCommand::SetPlaying(true));
        assert_eq!(run_audio_side(&mut engine, &mut plan), vec!["param", "play"]);
        assert_eq!(plan.nodes[0].params[0], 220.0);
        assert_eq!(ui.graph().parameters(1).unwrap()[0], 220.0);
    }

    #[test]
    fn test_parameter_for_new_node_arrives_with_its_plan() {
        let (mut ui, mut engine) = EngineChannels::with_defaults().split();
        let mut plan = Box::new(GraphPlan::empty(256));

        // The parameter message overtakes the (unflushed) plan; the plan
        // still carries the value
        ui.send_command(add(1, "osc.sine"));
        ui.send_command(EngineCommand::SetParameter { node_id: 1, param_index: 0, value: 330.0 });
        ui.flush();
        assert_eq!(run_audio_side(&mut engine, &mut plan), vec!["param", "plan"]);
        assert_eq!(plan.nodes[0].params[0], 330.0);
    }

    #[test]
    fn test_retired_plans_return_to_ui() {
        let (mut ui, mut engine) = EngineChannels::with_defaults().split();
        let mut plan = Box::new(GraphPlan::empty(256));

        // Many edits, each flushed and installed, never exhaust the plan budget
        for id in 0..(MAX_PLANS_IN_FLIGHT as u64 * 3) {
            ui.send_command(add(id, "osc.sine"));
            assert!(ui.flush(), "plan {id} was held back");
            run_audio_side(&mut engine, &mut plan);
        }
        assert_eq!(plan.len(), MAX_PLANS_IN_FLIGHT * 3);
        assert!(plan.nodes.iter().all(|n| n.module.is_some()));
    }

    #[test]
    fn test_plans_wait_while_audio_thread_is_away() {
        let (mut ui, mut engine) = EngineChannels::with_defaults().split();
        let mut plan = Box::new(GraphPlan::empty(256));

        // With no audio callbacks, at most MAX_PLANS_IN_FLIGHT plans go out
        // (one of which is the processor's initial plan)
        for id in 0..10 {
            ui.send_command(add(id, "osc.sine"));
            ui.flush();
        }
        assert_eq!(engine.messages_pending(), MAX_PLANS_IN_FLIGHT - 1);

        // Once the audio thread catches up, the held-back edits follow
        run_audio_side(&mut engine, &mut plan);
        assert!(ui.flush());
        run_audio_side(&mut engine, &mut plan);
        assert_eq!(plan.len(), 10);
        assert!(plan.nodes.iter().all(|n| n.module.is_some()));
    }

    #[test]
    fn test_full_queue_holds_plan_until_space() {
        let (mut ui, mut engine) = EngineChannels::new(2, 8).split();
        let mut plan = Box::new(GraphPlan::empty(256));

        ui.send_command(EngineCommand::SetPlaying(true));
        ui.send_command(EngineCommand::SetPlaying(false));
        assert!(ui.is_command_buffer_full());
        // Held, not dropped
        ui.send_command(EngineCommand::SetPlaying(true));

        ui.send_command(add(1, "osc.sine"));
        assert!(!ui.flush(), "no room for the plan yet");

        assert_eq!(run_audio_side(&mut engine, &mut plan), vec!["play", "stop"]);
        assert!(ui.flush());
        assert_eq!(run_audio_side(&mut engine, &mut plan), vec!["play", "plan"]);
        assert_eq!(plan.len(), 1);
    }

    #[test]
    fn test_held_transport_collapses_but_keeps_the_stop() {
        let (mut ui, mut engine) = EngineChannels::new(1, 8).split();
        let mut plan = Box::new(GraphPlan::empty(256));

        ui.send_command(EngineCommand::SetParameter { node_id: 1, param_index: 0, value: 1.0 });
        assert!(ui.is_command_buffer_full());

        // A burst of toggles while the queue is full: the stop still has to
        // reach the audio thread, since stopping clears tails
        for playing in [false, true, false, true] {
            ui.send_command(EngineCommand::SetPlaying(playing));
        }
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.extend(run_audio_side(&mut engine, &mut plan));
            ui.flush();
        }
        assert_eq!(seen, vec!["param", "stop", "play"]);

        // Ending on a stop needs only the stop
        ui.send_command(EngineCommand::SetParameter { node_id: 1, param_index: 0, value: 2.0 });
        for playing in [true, false, true, false] {
            ui.send_command(EngineCommand::SetPlaying(playing));
        }
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.extend(run_audio_side(&mut engine, &mut plan));
            ui.flush();
        }
        assert_eq!(seen, vec!["param", "stop"]);
    }

    #[test]
    fn test_large_patch_load_never_loses_nodes() {
        // A tiny queue, an audio thread that only wakes every few frames, and
        // a stream of knob moves competing for room with the patch
        let (mut ui, mut engine) = EngineChannels::new(4, 8).split();
        let mut plan = Box::new(GraphPlan::empty(256));
        const NODES: u64 = 50;

        ui.send_command(EngineCommand::ClearGraph);
        for id in 1..=NODES {
            let module_id = if id % 2 == 0 { "osc.sine" } else { "fx.delay" };
            ui.send_command(add(id, module_id));
            if id > 1 {
                // Oscillator "Out" is port 5, delay "Out L" port 4
                let from_port = if (id - 1) % 2 == 0 { 5 } else { 4 };
                ui.send_command(EngineCommand::Connect { from_node: id - 1, from_port, to_node: id, to_port: 0 });
            }
            ui.send_command(EngineCommand::SetParameter { node_id: id, param_index: 0, value: id as f32 });
            ui.send_command(EngineCommand::SetPlaying(true));
            if id % 10 == 0 {
                ui.flush();
            }
            if id % 15 == 0 {
                run_audio_side(&mut engine, &mut plan);
            }
        }

        // Keep the UI's frames going until it has sent everything
        let mut frames = 0;
        while !ui.flush() {
            run_audio_side(&mut engine, &mut plan);
            frames += 1;
            assert!(frames < 100, "edits still held back after {frames} frames");
        }
        run_audio_side(&mut engine, &mut plan);

        assert_eq!(plan.len(), NODES as usize, "nodes were lost");
        assert!(plan.nodes.iter().all(|n| n.module.is_some()));
        for node in &plan.nodes {
            assert_eq!(node.params[0], node.node_id as f32, "node {} lost its parameter", node.node_id);
        }
        assert_eq!(plan.processing_order().collect::<Vec<_>>(), (1..=NODES).collect::<Vec<_>>());
    }

    #[test]
    fn test_event_send_receive() {
        let (mut ui, mut engine) = EngineChannels::new(64, 64).split();

        // Send event from engine
        let result = engine.send_event(EngineEvent::OutputLevel(OutputLevels {
            post: [0.5, 0.6],
            ..Default::default()
        }));
        assert!(result.is_ok());

        // Receive in UI
        let event = ui.recv_event();
        assert!(event.is_some());
        if let EngineEvent::OutputLevel(levels) = event.unwrap() {
            assert_eq!(levels.post, [0.5, 0.6]);
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_scope_frames_arrive_as_events() {
        let (mut ui, mut engine) = EngineChannels::new(64, 64).split();
        engine.send_scope_frame_lossy(ScopeFrame::new(7, &[0.1, 0.2, 0.3], &[], true));

        match ui.recv_event() {
            Some(EngineEvent::ScopeBuffer { node_id, channel1, channel2, triggered }) => {
                assert_eq!(node_id, 7);
                assert_eq!(&*channel1, &[0.1, 0.2, 0.3]);
                assert!(channel2.is_empty());
                assert!(triggered);
            }
            other => panic!("expected a scope buffer, got {other:?}"),
        }
    }

    #[test]
    fn test_lossy_events() {
        let (mut ui, mut engine) = EngineChannels::new(1, 1).split();

        engine.send_event_lossy(EngineEvent::CpuLoad(0.5));
        engine.send_event_lossy(EngineEvent::CpuLoad(0.6)); // Should be dropped

        assert!(ui.recv_event().is_some());
        assert!(ui.recv_event().is_none());
    }

    #[test]
    fn test_drain_events() {
        let (mut ui, mut engine) = EngineChannels::new(64, 64).split();

        // Send multiple events
        engine.send_event_lossy(EngineEvent::Started);
        engine.send_event_lossy(EngineEvent::CpuLoad(0.3));
        engine.send_event_lossy(EngineEvent::OutputLevel(OutputLevels::default()));

        // Drain all events
        let events: Vec<_> = ui.drain_events().collect();
        assert_eq!(events.len(), 3);

        // No more events
        assert!(ui.recv_event().is_none());
    }

    #[test]
    fn test_slots_available() {
        let (mut ui, mut engine) = EngineChannels::new(10, 10).split();

        assert_eq!(ui.command_slots_available(), 10);
        assert_eq!(engine.event_slots_available(), 10);

        ui.send_command(EngineCommand::SetPlaying(true));
        engine.send_event_lossy(EngineEvent::Started);

        assert_eq!(ui.command_slots_available(), 9);
        assert_eq!(engine.event_slots_available(), 9);
    }

    #[test]
    fn test_handles_are_send() {
        fn assert_send<T: Send>() {}
        assert_send::<UiHandle>();
        assert_send::<EngineHandle>();
    }

    #[test]
    fn test_audio_config_reaches_ui_graph() {
        let (mut ui, engine) = EngineChannels::with_defaults().split();
        engine.set_audio_config(48000.0, 128);
        ui.flush();
        assert_eq!(ui.graph().sample_rate(), 48000.0);
        assert_eq!(ui.graph().block_size(), 128);
    }

    #[test]
    fn test_empty_receive() {
        let (mut ui, mut engine) = EngineChannels::new(64, 64).split();

        // Nothing sent yet
        assert!(engine.recv_message().is_none());
        assert!(ui.recv_event().is_none());
    }
}
