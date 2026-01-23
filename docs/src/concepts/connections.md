# Connections

Connections are the cables that link modules together, allowing signals to flow through your patch. Understanding connection rules helps you build effective synthesizer patches.

## Basic Rules

### Outputs to Inputs

Connections always flow from **output ports** (right side of modules) to **input ports** (left side of modules).

![Connection Direction](../images/connection-direction.png)
*Signals flow from outputs (right) to inputs (left)*

You cannot:
- Connect output to output
- Connect input to input
- Create circular connections that would cause feedback loops (the system prevents this)

### One Input, Many Outputs

**Inputs** accept only one connection at a time. Connecting a new cable to an already-connected input will replace the existing connection.

**Outputs** can feed multiple inputs. The signal is copied to each destination without reduction in level.

![Multiple Connections](../images/connection-multiple.png)
*One output feeding multiple inputs*

---

## Making Connections

### Creating a Connection

1. **Click and hold** on an output port
2. **Drag** toward the destination input
3. The cursor will show valid connection points
4. **Release** over an input port to complete

![Creating Connection](../images/connection-creating.png)
*Dragging to create a connection*

### Visual Feedback

While dragging:
- **Valid inputs** highlight to show they can accept the connection
- **Invalid inputs** (wrong signal type or already connected) may dim
- The cable preview shows the signal type color

### Quick Connect

Double-click an output port to start a connection, then single-click an input to complete it. This is useful for long-distance connections.

---

## Removing Connections

### Right-Click Method

**Right-click** on a cable to delete it immediately.

### Disconnect from Port

Click on a connected input port, then press **Escape** to disconnect.

### Delete Module

Deleting a module automatically removes all its connections.

---

## Signal Type Matching

### Preferred Connections

For best results, match signal types:

| Connection | Result |
|------------|--------|
| Audio → Audio | Full-bandwidth sound signal |
| Control → Control | Modulation and CV |
| Gate → Gate | Trigger and timing |
| MIDI → MIDI | MIDI message passing |

### Cross-Type Connections

Some cross-type connections are useful:

| Connection | Use Case |
|------------|----------|
| Audio → Control | Audio-rate modulation (FM synthesis) |
| Gate → Control | Simple 0/1 control signal |
| Control → Audio | Slow modulation mixed as audio |

The system allows most cross-type connections, treating the signal according to the destination's expectations.

### MIDI Special Case

MIDI signals are structured differently and generally only connect to MIDI-specific inputs. Use the MIDI Note module to convert MIDI to CV signals (V/Oct, Gate, Velocity) for standard modules.

---

## Cable Colors

Cables are colored by signal type for easy visual identification:

| Color | Signal Type |
|-------|-------------|
| **Blue** | Audio |
| **Orange** | Control/CV |
| **Green** | Gate/Trigger |
| **Purple** | MIDI |

![Cable Colors](../images/connection-colors.png)
*Cables colored by signal type*

This coloring helps you:
- Trace signal flow through complex patches
- Identify signal types at a glance
- Debug routing issues

---

## Connection Tips

### Keep It Organized

- Position modules so signal flows left-to-right
- Group related modules together
- Use the canvas space to prevent cable crossings

![Organized Patch](../images/connection-organized.png)
*A well-organized patch with clear signal flow*

### Modulation Routing

Control signals often "reach across" the main signal flow:

```
[LFO] ────────────────────────┐
                              ↓ (CV)
[Osc] ──> [Filter] ──> [VCA] ──> [Out]
            ↑
[Envelope] ─┘
```

Position modulation sources (LFOs, envelopes) above or below the main signal path.

### Check Signal Flow

If you're not getting sound:

1. **Trace from output backward** - Is the Audio Output connected?
2. **Check control signals** - Is the VCA getting a CV signal?
3. **Verify gates** - Is the envelope receiving a gate?
4. **Look at signal types** - Are the right types connected?

---

## Exposed Parameters

Some module parameters can be controlled via connections. These are called "exposed" parameters.

### How It Works

When a parameter is exposed:
- It has both a **knob** for manual control AND an **input port** for external control
- When **disconnected**: The knob controls the parameter normally
- When **connected**: The external signal takes over

### Visual Indicators

![Exposed Parameter](../images/connection-exposed.png)
*An exposed parameter showing external control*

When externally controlled:
- The knob becomes **read-only** (dimmed)
- The knob **animates** to show the incoming signal value
- An **orange indicator dot** shows external control is active

### Combining Manual and Modulation

The external signal often adds to the knob's base value:
- **Knob** sets the center/base value
- **Input** adds modulation on top

For example, a filter cutoff:
- Knob at 1000 Hz
- LFO input swinging ±500 Hz
- Result: Cutoff sweeps between 500-1500 Hz

---

## Advanced Topics

### Audio-Rate Modulation

Control inputs can accept audio-rate signals for special effects:

- **FM Synthesis**: Oscillator frequency modulated at audio rate
- **AM/Ring Mod**: Amplitude modulated at audio rate
- **Filter FM**: Cutoff modulated at audio rate

### Feedback Loops

The system prevents direct feedback loops (output connecting back to earlier input in the same signal path). This is necessary to maintain stable, real-time audio processing.

For delay-based feedback effects, use the Delay module's built-in feedback control.

### DC Offset

Control signals may have DC offset (a constant value added to the signal). Some modules include DC blocking or offset controls to manage this.

---

## Troubleshooting

### No Sound

1. Check that Audio Output is connected
2. Verify VCA is receiving CV or is set to pass audio
3. Ensure oscillator is running (not waiting for trigger)
4. Check system audio settings

### Unexpected Sound

1. Look for unintended connections
2. Check signal levels (may need attenuation)
3. Verify signal types match expectations

### Clicking or Popping

1. Ensure control signals are smoothed
2. Check for abrupt gate transitions
3. Add slight attack/release to envelopes

---

## Next Steps

- **[Module Reference](../modules/README.md)** - See connection details for each module
- **[Your First Patch](../getting-started/your-first-patch.md)** - Practice making connections
