# Modular Synth - Claude Instructions

## Project Overview

A **node-based modular audio synthesizer** in Rust. Unlike VCV Rack's hardware skeuomorphism, this uses a clean node-graph approach (like Blender nodes) while retaining rich visual feedback.

**Key characteristics:**
- Node-graph UI (not virtual hardware)
- Color-coded signal types and modules
- Lock-free audio architecture (UI and audio threads communicate via ring buffers)
- Learning project for Rust

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  PRESENTATION: egui + egui_node_graph2                  │
│  - Node graph editor with custom rendering              │
│  - Custom widgets (knobs, faders, waveform displays)    │
└───────────────────────────┬─────────────────────────────┘
                            │ Lock-free ring buffer (rtrb)
┌───────────────────────────┴─────────────────────────────┐
│  ENGINE: Audio graph processor (audio thread)           │
│  - Topological sort for processing order                │
│  - Pre-allocated buffers (no allocations in audio)      │
└───────────────────────────┬─────────────────────────────┘
                            │ cpal audio callback
┌───────────────────────────┴─────────────────────────────┐
│  AUDIO I/O: cpal stream to hardware                     │
└─────────────────────────────────────────────────────────┘
```

## Signal Types (Color-Coded)

| Type    | Color  | Range                | Purpose            |
|---------|--------|----------------------|--------------------|
| Audio   | Blue   | -1.0 to 1.0          | Sample streams     |
| Control | Orange | 0.0-1.0 or -1.0-1.0  | Modulation CV      |
| Gate    | Green  | 0.0 or 1.0           | On/off triggers    |
| MIDI    | Purple | Structured events    | Note/CC data       |

## Module Categories (Color-Coded Headers)

- **Blue**: Sources (Oscillators)
- **Green**: Filters
- **Orange**: Utilities (Envelopes, LFOs)
- **Purple**: Output

## Directory Structure

```
src/
├── main.rs              # Entry point, egui app setup
├── lib.rs               # Module declarations
├── app/                 # UI application, theme
├── graph/               # Node graph (egui_node_graph2 integration)
├── widgets/             # Custom controls (knob, fader, displays)
├── engine/              # Audio engine, graph processor, commands
├── dsp/                 # DspModule trait, ports, parameters, signals
├── modules/             # Built-in modules (osc, filter, env, lfo, output)
└── persistence/         # Patch save/load (JSON)
```

## Tech Stack

- **eframe/egui 0.29**: Immediate-mode GUI
- **egui_node_graph2 0.5**: Node editor foundation
- **cpal 0.15**: Cross-platform audio I/O
- **rtrb 0.3**: Lock-free ring buffer for thread communication
- **serde/serde_json**: Patch serialization

## Development Phases

Issues are organized into 6 phases:
1. **Foundation**: Project setup, core types, basic audio, minimal window
2. **Module System**: Registry, oscillator, output, command channel, audio graph
3. **Node Graph UI**: egui_node_graph2 integration, templates, connections
4. **Custom Rendering**: Styled nodes, cables, widgets
5. **Complete Modules**: Filter, envelope, LFO, waveforms
6. **Polish**: Save/load, smoothing, keyboard input, CPU metering

---

## Issue Implementation Workflow

When asked to implement the next issue, follow this workflow:

### 1. Find the Next Issue

```bash
gh issue list --state open --limit 30
```

The **lowest issue number** is the next to implement. Issues are ordered by phase and dependency.

### 2. Read the Issue Details

```bash
gh issue view <issue_number>
```

Understand:
- What files need to be created/modified
- Acceptance criteria
- Dependencies on other issues
- Testing instructions

### 3. Implement the Issue

- Read existing code before modifying
- Follow Rust idioms and the patterns established in PLAN.md
- Keep the lock-free constraint in mind for audio code (no allocations in audio thread)
- Use the signal type colors consistently

### 4. Verify the Implementation

Run the verification commands from the issue. At minimum:

```bash
cargo check
cargo build
cargo run  # If the issue adds visible functionality
cargo test # If tests exist
```

For audio-related issues, verify sound output works.

### 5. Commit the Changes

```bash
git add -A
git status  # Review what's being committed
git commit -m "$(cat <<'EOF'
[Phase X] Issue title

- Bullet point of what was done
- Another change

Closes #<issue_number>

Co-Authored-By: Claude <noreply@anthropic.com>
EOF
)"
git push
```

### 6. Close the Issue

```bash
gh issue close <issue_number> --comment "Completed!

<Brief summary of what was implemented>

Verified: cargo build ✓, cargo run ✓"
```

---

## Common Commands

```bash
# Build and run
cargo build
cargo run
cargo check

# Run with release optimizations (for audio performance testing)
cargo run --release

# GitHub CLI
gh issue list --state open
gh issue view <number>
gh issue close <number> --comment "Done"

# Git
git status
git diff
git add -A
git commit -m "message"
git push
```

## Important Constraints

1. **No allocations in audio thread** - All buffers must be pre-allocated
2. **Lock-free communication** - Use rtrb ring buffers between UI and audio
3. **Type-safe connections** - Signal types must be validated when connecting ports
4. **Real-time safe** - Audio callback must never block

## Design Philosophy: Inputs vs Knobs

This synthesizer follows a clear separation between **inputs** and **knobs**:

### Inputs (Left Side Ports)
- **Purpose**: Connection points for external signals from other modules
- **Display**: Label only, no inline widget (except Toggle/Select which need special UI)
- **Behavior**: Receive signals from connected modules

### Knobs (Bottom Section)
- **Purpose**: Manual user controls for parameter values
- **Display**: Rotary knob with value readout
- **Behavior**: User can drag to adjust value

### Exposed Parameters (Both Input AND Knob)
Some parameters can be controlled both manually and externally. These are called "exposed" parameters:

1. **When disconnected**: The knob controls the value normally
2. **When connected**: The external signal takes over:
   - Knob becomes read-only (dimmed, non-interactive)
   - Knob position animates to show the incoming signal value
   - Visual indicator (orange dot) shows active external control
3. **When disconnected again**: Returns to manual control

This mirrors real analog modular synthesizers where patching a cable into a parameter jack overrides the manual control.

### Implementation
- `KnobParam::knob_only(...)` - Parameter with knob only, no input port
- `KnobParam::exposed(...)` - Parameter with both input port AND knob
- Signal feedback from audio engine enables knob animation when connected

## Visual Design Notes

From the concept image Chris provided:
- Modules have rounded rectangles with colored header bars
- Ports are metallic circular jacks on module edges
- Cables are bezier curves with signal-type coloring and subtle glow
- Knobs are 3D-style with value displays
- Background is dark with subtle grid pattern

## References

- **PLAN.md**: Detailed implementation plan with all specifications
- **Claude's Vault**: `~\dev\Claude's Vault\Modular Synth Project - 2026-01-20.md` has project context
