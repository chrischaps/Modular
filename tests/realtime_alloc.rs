//! The audio callback must never allocate.
//!
//! A counting global allocator watches `AudioProcessor::process` while it
//! plays a patch using every built-in module, through odd device buffer
//! sizes and while the patch is edited. Edits are compiled on the "UI" side
//! (outside the counted region); installing them happens inside it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use modular_synth::engine::{
    create_module_registry, AudioProcessor, EngineChannels, EngineCommand, NodeId, UiHandle,
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

/// Adds one of every built-in module, chained output-to-input in
/// registration order, with every port monitored.
fn build_big_patch(ui: &mut UiHandle) -> Vec<(NodeId, &'static str)> {
    let registry = create_module_registry();
    let ids: Vec<&'static str> = registry.list_modules().iter().map(|info| info.id).collect();

    let mut nodes = Vec::new();
    let mut previous_output: Option<(NodeId, usize)> = None;
    for (n, module_id) in ids.into_iter().enumerate() {
        let node_id = n as NodeId + 1;
        ui.send_command(EngineCommand::AddModule { node_id, module_id }).unwrap();

        let (inputs, outputs) = ports(module_id);
        if let (Some((from_node, from_port)), Some(&to_port)) = (previous_output, inputs.first()) {
            ui.send_command(EngineCommand::Connect { from_node, from_port, to_node: node_id, to_port })
                .unwrap();
        }
        for input_index in 0..inputs.len() {
            ui.send_command(EngineCommand::MonitorInput { node_id, input_index }).unwrap();
        }
        for output_index in 0..outputs.len() {
            ui.send_command(EngineCommand::MonitorOutput { node_id, output_index }).unwrap();
        }
        if let Some(&port) = outputs.first() {
            previous_output = Some((node_id, port));
        }
        nodes.push((node_id, module_id));
    }
    nodes
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

    let nodes = build_big_patch(&mut ui);
    ui.send_command(EngineCommand::SetPlaying(true)).unwrap();
    assert!(ui.flush());

    // Typical WASAPI/CoreAudio sizes, including ones larger than a block
    let device_buffers = [256, 441, 480, 128, 1024, 64];
    let mut output = vec![0.0_f32; 1024 * 2];
    let mut blocks = 0;
    let mut allocations = 0;

    for round in 0..1000 {
        let frames = device_buffers[round % device_buffers.len()];

        // Edit the patch while it plays: knob moves every block, and a
        // module swapped out every 50 blocks
        ui.send_command(EngineCommand::SetParameter {
            node_id: 1,
            param_index: 0,
            value: 220.0 + round as f32,
        })
        .unwrap();
        if round % 50 == 25 {
            let (node_id, module_id) = nodes[round / 50 % nodes.len()];
            ui.send_command(EngineCommand::RemoveModule { node_id }).unwrap();
            ui.send_command(EngineCommand::AddModule { node_id, module_id }).unwrap();
        }
        ui.flush();
        ui.drain_events().for_each(drop);

        allocations += count_allocations(|| processor.process(&mut output[..frames * 2], 2));
        blocks += 1;
    }

    assert_eq!(blocks, 1000);
    assert!(processor.is_playing());
    assert_eq!(processor.plan().len(), nodes.len());
    assert!(output.iter().all(|s| s.is_finite()));
    assert_eq!(allocations, 0, "audio callback allocated {allocations} times over {blocks} callbacks");
}
