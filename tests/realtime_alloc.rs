//! The audio callback must never allocate.
//!
//! A counting global allocator watches `AudioProcessor::process` while it
//! plays a patch of more than 30 modules (every built-in module, some
//! twice), through odd device buffer sizes, while the patch is edited and
//! its Clock changes tempo under a tempo-synced Delay and its effects are
//! bypassed and brought back, and live MIDI plays its MIDI Note modules.
//! Edits are compiled on the "UI" side (outside the counted region);
//! installing them happens inside it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use modular_synth::engine::{
    create_module_registry, AudioProcessor, EngineChannels, EngineCommand, MidiEvent, NodeId,
    TimestampedMidiEvent, UiHandle,
};

struct CountingAllocator;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

fn note_allocation() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note_allocation();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note_allocation();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Counts allocations and frees made by `f` on this thread.
fn count_allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    ALLOCATIONS.load(Ordering::Relaxed) - before
}

/// Port layout of a registered module: (input port indices, output port indices).
fn ports(module_id: &str) -> (Vec<usize>, Vec<usize>) {
    let module = create_module_registry().create(module_id).unwrap();
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for (index, port) in module.ports().iter().enumerate() {
        if port.is_input() {
            inputs.push(index);
        } else {
            outputs.push(index);
        }
    }
    (inputs, outputs)
}

/// The patch size the guard plays.
const PATCH_SIZE: usize = 32;

/// Adds `count` modules, cycling through every built-in module in
/// registration order, chained output-to-input, with every port monitored.
fn build_big_patch(ui: &mut UiHandle, count: usize) -> Vec<(NodeId, &'static str)> {
    let registry = create_module_registry();
    let ids: Vec<&'static str> = registry.list_modules().iter().map(|info| info.id).collect();

    let mut nodes = Vec::new();
    let mut previous_output: Option<(NodeId, usize)> = None;
    for (n, module_id) in ids.into_iter().cycle().take(count).enumerate() {
        let node_id = n as NodeId + 1;
        ui.send_command(EngineCommand::AddModule { node_id, module_id });

        let (inputs, outputs) = ports(module_id);
        if let (Some((from_node, from_port)), Some(&to_port)) = (previous_output, inputs.first()) {
            ui.send_command(EngineCommand::Connect { from_node, from_port, to_node: node_id, to_port });
        }
        for input_index in 0..inputs.len() {
            ui.send_command(EngineCommand::MonitorInput { node_id, input_index });
        }
        for output_index in 0..outputs.len() {
            ui.send_command(EngineCommand::MonitorOutput { node_id, output_index });
        }
        if let Some(&port) = outputs.first() {
            previous_output = Some((node_id, port));
        }
        nodes.push((node_id, module_id));
    }
    nodes
}

/// The first node running `module_id`.
fn node_of(nodes: &[(NodeId, &'static str)], module_id: &str) -> NodeId {
    nodes.iter().find(|(_, id)| *id == module_id).map(|&(node_id, _)| node_id).unwrap()
}

#[test]
fn harness_counts_allocations() {
    let counted = count_allocations(|| {
        std::hint::black_box(Vec::<u8>::with_capacity(16));
    });
    assert_eq!(counted, 2, "one allocation and one free");
}

#[test]
fn audio_callback_never_allocates() {
    let (mut ui, engine) = EngineChannels::with_defaults().split();
    let mut processor = AudioProcessor::new(48000.0, 256, engine);
    let (mut midi, midi_input) = rtrb::RingBuffer::new(512);
    processor.set_midi_input(midi_input);

    let nodes = build_big_patch(&mut ui, PATCH_SIZE);
    assert!(nodes.len() >= 30);

    // Sync every Delay to the patch tempo, which the first Clock sets
    let clock = node_of(&nodes, "util.clock");
    for &(node_id, _) in nodes.iter().filter(|(_, id)| *id == "fx.delay") {
        ui.send_command(EngineCommand::SetParameter { node_id, param_index: 6, value: 2.0 });
    }
    ui.send_command(EngineCommand::SetPlaying(true));
    assert!(ui.flush());

    // Typical WASAPI/CoreAudio sizes, including ones larger than a block
    let device_buffers = [256, 441, 480, 128, 1024, 64];
    let mut output = vec![0.0_f32; 1024 * 2];
    let mut blocks = 0;
    let mut allocations = 0;

    for round in 0..1000 {
        let frames = device_buffers[round % device_buffers.len()];

        // Edit the patch while it plays: knob moves and tempo changes every
        // block, and a module swapped out every 50 blocks
        ui.send_command(EngineCommand::SetParameter {
            node_id: 1,
            param_index: 0,
            value: 220.0 + round as f32,
        });
        ui.send_command(EngineCommand::SetParameter {
            node_id: clock,
            param_index: 0,
            value: 60.0 + (round % 120) as f32,
        });
        // Every few blocks, bypass every filter and effect or bring them all
        // back: crossfades, fully bypassed rests, and resets on return
        if round % 8 == 3 {
            for &(node_id, _) in &nodes {
                ui.send_command(EngineCommand::SetBypass { node_id, bypassed: round % 16 == 3 });
            }
        }
        if round % 50 == 25 {
            let (node_id, module_id) = nodes[round / 50 % nodes.len()];
            ui.send_command(EngineCommand::RemoveModule { node_id });
            ui.send_command(EngineCommand::AddModule { node_id, module_id });
        }
        // Play: notes on and off every block, with bends and pressure. The
        // queue would overflow (and the push panic) unless the callback drains it
        let note = 48 + (round % 24) as u8;
        let events = [
            MidiEvent::NoteOn { channel: 0, note, velocity: 100 },
            MidiEvent::PitchBend { channel: 0, value: (round as i16 % 64) * 128 - 4096 },
            MidiEvent::ChannelPressure { channel: 0, pressure: (round % 128) as u8 },
            MidiEvent::NoteOff { channel: 0, note: note.wrapping_sub(3), velocity: 0 },
        ];
        for event in events {
            midi.push(TimestampedMidiEvent::now(event)).unwrap();
        }
        ui.flush();
        ui.drain_events().for_each(drop);

        allocations += count_allocations(|| processor.process(&mut output[..frames * 2], 2));
        blocks += 1;
    }

    assert_eq!(blocks, 1000);
    assert!(processor.is_playing());
    assert_eq!(processor.plan().len(), nodes.len());
    assert!(processor.plan().tempo_bpm().is_some(), "the Clock sets the patch tempo");
    assert!(output.iter().all(|s| s.is_finite()));
    assert_eq!(allocations, 0, "audio callback allocated {allocations} times over {blocks} callbacks");
}
