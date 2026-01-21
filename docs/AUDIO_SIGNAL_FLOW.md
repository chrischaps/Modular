# Audio Signal Flow Documentation

This document describes how audio signals flow through the Modular Synth system, from UI interaction to speaker output. Understanding this flow is essential for debugging audio issues.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         UI THREAD                                    │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────────┐    │
│  │ egui_node_  │───▶│  SynthApp    │───▶│   EngineCommand     │    │
│  │ graph2      │    │              │    │   (via rtrb)        │    │
│  └─────────────┘    └──────────────┘    └──────────┬──────────┘    │
└─────────────────────────────────────────────────────┼───────────────┘
                                                      │ Lock-free
                                                      │ ring buffer
┌─────────────────────────────────────────────────────┼───────────────┐
│                       AUDIO THREAD                  ▼               │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────────┐    │
│  │ cpal        │◀───│ AudioProc-   │◀───│    AudioGraph       │    │
│  │ callback    │    │ essor        │    │                     │    │
│  └──────┬──────┘    └──────────────┘    └─────────────────────┘    │
└─────────┼───────────────────────────────────────────────────────────┘
          │
          ▼
    ┌───────────┐
    │  Speakers │
    └───────────┘
```

## Thread Model

### UI Thread
- Runs the egui event loop
- Handles user interactions (adding nodes, making connections, adjusting parameters)
- Sends commands to audio thread via lock-free ring buffer (rtrb)
- **Never blocks on audio thread**

### Audio Thread
- Runs in cpal's audio callback
- Processes audio in real-time with strict timing requirements
- Receives commands from UI thread
- **Must never allocate memory or block**

## Signal Flow Step-by-Step

### 1. User Interaction → Commands

When the user interacts with the UI:

```
User Action              →  EngineCommand
─────────────────────────────────────────
Add node                 →  AddModule { node_id, module_id }
Delete node              →  RemoveModule { node_id }
Connect ports            →  Connect { from_node, from_port, to_node, to_port }
Disconnect ports         →  Disconnect { node_id, port, is_input }
Adjust parameter         →  SetParameter { node_id, param_index, value }
Click Play               →  SetPlaying(true)
Click Stop               →  SetPlaying(false)
```

**Key file:** `src/app/synth_app.rs`
- `sync_parameters()` - Sends parameter changes
- Node response handlers - Send add/remove/connect commands

### 2. Command Channel (UI → Audio)

Commands flow through a lock-free ring buffer:

```rust
// UI side (src/engine/channels.rs)
ui_handle.send_command(EngineCommand::SetParameter { ... })

// Audio side (src/engine/audio_processor.rs)
while let Some(cmd) = engine_handle.recv_command() {
    // Process command
}
```

**Key file:** `src/engine/channels.rs`
- `UIHandle` - UI thread's interface
- `EngineHandle` - Audio thread's interface
- Uses `rtrb` crate for lock-free communication

### 3. Audio Callback

cpal calls our audio callback ~100 times per second (at 48kHz with 480 sample blocks):

```rust
// src/engine/audio_engine.rs - start_with_processor()
move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
    if let Ok(mut proc) = processor.try_lock() {
        proc.process(data, channels);
    }
}
```

The `data` buffer is what cpal will send to the speakers.

### 4. AudioProcessor::process()

Main audio processing entry point:

```rust
// src/engine/audio_processor.rs
pub fn process(&mut self, output: &mut [f32], channels: usize) {
    // 1. Process pending commands from UI
    self.process_commands();

    // 2. Clear output buffer
    for sample in output.iter_mut() {
        *sample = 0.0;
    }

    // 3. Early return if not playing
    if !self.is_playing {
        return;
    }

    // 4. Process the audio graph
    self.graph.process(&self.context);

    // 5. Extract output from AudioOutput module
    self.extract_output(output, channels, num_frames);
}
```

### 5. AudioGraph::process()

Processes all modules in topological order:

```rust
// src/engine/audio_graph.rs
pub fn process(&mut self, context: &ProcessContext) {
    // Ensure processing order is up to date
    self.update_processing_order();

    // Clear all output buffers in the pool
    self.buffers.clear_all();

    // Process modules in topological order
    for &node_id in &self.processing_order.clone() {
        self.process_module(node_id, context);
    }
}
```

### 6. Module Processing

Each module is processed individually:

```rust
// src/engine/audio_graph.rs - process_module()
fn process_module(&mut self, node_id: NodeId, context: &ProcessContext) {
    // 1. Gather input buffers from connected modules
    let input_buffers = self.gather_inputs(node_id);

    // 2. Create temporary output buffers
    let mut output_buffers: Vec<SignalBuffer> = ...;

    // 3. Get parameter values
    let params = data.parameters.clone();

    // 4. Call module's process function
    data.module.process(
        &input_buffers.iter().collect::<Vec<_>>(),
        &mut output_buffers,
        &params,
        context,
    );

    // 5. Copy outputs to buffer pool for downstream modules
    for (i, output_buf) in output_buffers.into_iter().enumerate() {
        if let Some(pool_buf) = self.buffers.get_mut(node_id, i) {
            pool_buf.samples.copy_from_slice(&output_buf.samples);
        }
    }
}
```

### 7. Input Gathering

`gather_inputs()` resolves connections to get input data:

```rust
// src/engine/audio_graph.rs
fn gather_inputs(&self, node_id: NodeId) -> Vec<SignalBuffer> {
    for (port_idx, port_def) in input_ports {
        // Find connection to this input port
        let connection = self.connections.iter().find(|conn| {
            conn.to_node == node_id && conn.to_port == port_idx
        });

        if let Some(conn) = connection {
            // Get buffer from source module's output
            let output_idx = self.port_to_output_index(...);
            if let Some(buf) = self.buffers.get(conn.from_node, output_idx) {
                inputs.push(buf.clone());
                continue;
            }
        }

        // No connection - use default value
        let mut buf = SignalBuffer::new(...);
        buf.fill(port_def.default_value);
        inputs.push(buf);
    }
}
```

### 8. Output Extraction

After all modules are processed, audio is extracted from AudioOutput:

```rust
// src/engine/audio_processor.rs - extract_output()
fn extract_output(&mut self, output: &mut [f32], channels: usize, num_frames: usize) {
    for node_id in self.graph.processing_order() {
        if let Some(module) = self.graph.get_module(node_id) {
            // AudioOutput implements get_audio_output()
            if let Some((left, right)) = module.get_audio_output() {
                // Write interleaved stereo to cpal buffer
                for (i, frame) in output.chunks_mut(channels).enumerate() {
                    frame[0] = left[i];   // Left channel
                    frame[1] = right[i];  // Right channel
                }
                break;
            }
        }
    }
}
```

## Buffer Management

### BufferPool

Pre-allocated buffers for module outputs:

```
BufferPool
├── (node_id=0, output_idx=0) → SignalBuffer [480 samples]
├── (node_id=1, output_idx=0) → SignalBuffer [480 samples]
└── ...
```

**Key file:** `src/engine/buffer_pool.rs`

### SignalBuffer

Holds audio/control/gate samples:

```rust
pub struct SignalBuffer {
    pub samples: Vec<f32>,      // The actual sample data
    pub signal_type: SignalType, // Audio, Control, Gate, or MIDI
}
```

**Key file:** `src/dsp/signal.rs`

## Parameter Flow

Parameters flow from UI sliders to module processing:

```
UI Slider (SynthValueType)
    │
    ▼ actual_value() ← IMPORTANT: Returns Hz, not normalized!
SetParameter { value: 440.0 }
    │
    ▼ (via ring buffer)
AudioGraph::set_parameter()
    │
    ▼ (stored in ModuleData)
module.process(..., params: &[f32], ...)
    │
    ▼
let base_freq = params[0]; // 440.0 Hz
```

**Critical:** Parameters are stored and sent as actual values (Hz, seconds, etc.), not normalized 0-1 values. The `SynthValueType::actual_value()` method handles this conversion.

## Port Index Mapping

Ports are indexed differently in different contexts:

### In egui_node_graph2
- All ports have sequential indices
- Example: SineOscillator ports [0: freq_cv, 1: fm, 2: out]

### In DspModule
- Input ports and output ports are separate
- `gather_inputs()` only iterates input ports
- `port_to_output_index()` converts port index to output buffer index

### Example: SineOscillator
```
Port Definition:
  [0] freq_cv  (Input, Control)
  [1] fm       (Input, Control)
  [2] out      (Output, Audio)

Input indices:  [0: freq_cv, 1: fm]
Output indices: [0: out]

When connecting osc.out (port 2) → output.mono (port 2):
  - Connection stores: from_port=2, to_port=2
  - port_to_output_index(2) returns 0 (first output)
```

## Common Debugging Points

### No Sound - Checklist

1. **Is playing?** Check `AudioProcessor::is_playing`
2. **Modules added?** Check `graph.module_count()`
3. **Processing order?** Check `graph.processing_order()` - should not be empty
4. **Connections made?** Check `graph.connection_count()`
5. **Parameters correct?** Verify frequency is in Hz, not normalized (0-1)
6. **Output module exists?** Look for module with `get_audio_output()` returning `Some`
7. **Buffers have data?** Log peak values at each stage

### Debug Logging Locations

Add temporary logging at these points:

```rust
// 1. Command receipt
// src/engine/audio_processor.rs - process_commands()
eprintln!("Received: {:?}", cmd);

// 2. Module processing
// src/engine/audio_graph.rs - process_module()
eprintln!("Processing module: {}", module.info().id);

// 3. Input gathering
// src/engine/audio_graph.rs - gather_inputs()
eprintln!("Input {} connected: {}", port_idx, connection.is_some());

// 4. Output extraction
// src/engine/audio_processor.rs - extract_output()
eprintln!("Output peak: L={:.4}, R={:.4}", peak_l, peak_r);

// 5. Final buffer
// src/engine/audio_engine.rs - callback
eprintln!("Buffer max: {:.4}", data.iter().fold(0.0f32, |a, &b| a.max(b.abs())));
```

## Files Reference

| File | Purpose |
|------|---------|
| `src/engine/audio_engine.rs` | cpal setup, audio callback |
| `src/engine/audio_processor.rs` | Main processing loop, command handling |
| `src/engine/audio_graph.rs` | Module graph, topological sort, processing |
| `src/engine/buffer_pool.rs` | Pre-allocated output buffers |
| `src/engine/channels.rs` | Lock-free command/event channels |
| `src/engine/commands.rs` | Command and event definitions |
| `src/dsp/module_trait.rs` | DspModule trait definition |
| `src/dsp/signal.rs` | SignalBuffer, SignalType |
| `src/modules/oscillator.rs` | SineOscillator implementation |
| `src/modules/output.rs` | AudioOutput implementation |
| `src/app/synth_app.rs` | UI, parameter sync, command sending |
| `src/graph/value_types.rs` | Parameter value types (actual_value!) |
