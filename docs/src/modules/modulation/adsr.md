# ADSR Envelope

**Module ID**: `mod.adsr`
**Category**: Modulation
**Header Color**: Orange

![ADSR Envelope Module](../../images/module-adsr.png)
*The ADSR Envelope module*

## Description

The ADSR Envelope generates a control signal that shapes how a sound evolves over time. When triggered by a gate signal (like pressing a key), it produces a predictable voltage curve through four stages: Attack, Decay, Sustain, and Release.

Envelopes are essential for:
- Controlling amplitude (volume shape) via VCA
- Modulating filter cutoff for timbral changes
- Adding dynamic movement to any parameter

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Gate** | Gate (Green) | Trigger input. Rising edge starts Attack, falling edge starts Release |
| **Retrig** | Gate (Green) | Retrigger input. Rising edge restarts Attack without waiting for Release |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Env** | Control (Orange) | Main envelope output (0.0 to 1.0) |
| **Inv** | Control (Orange) | Inverted envelope output (1.0 to 0.0) |
| **EOC** | Gate (Green) | End of Cycle - outputs pulse when envelope completes Release |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Attack** | 0.1 ms - 10 s | 10 ms | Time to rise from 0 to peak |
| **Decay** | 0.1 ms - 10 s | 100 ms | Time to fall from peak to Sustain level |
| **Sustain** | 0.0 - 1.0 | 0.7 | Level held while gate is high |
| **Release** | 0.1 ms - 10 s | 200 ms | Time to fall from Sustain to 0 after gate low |

## Envelope Stages

![ADSR Diagram](../../images/envelope-adsr-diagram.png)
*The four stages of an ADSR envelope*

### Attack

The **Attack** phase begins when the gate goes high (key pressed). The envelope rises from 0 to its peak value (1.0).

- **Short attack** (0.1-10 ms): Instant, percussive start (drums, plucks)
- **Medium attack** (10-100 ms): Soft start (strings, pads)
- **Long attack** (100 ms+): Gradual swell (ambient, swells)

### Decay

The **Decay** phase begins immediately after Attack reaches peak. The envelope falls from peak (1.0) to the Sustain level.

- **Short decay** (10-50 ms): Percussive, plucky sounds
- **Medium decay** (50-200 ms): Piano-like sounds
- **Long decay** (200 ms+): Smooth, gradual transition

### Sustain

The **Sustain** phase holds at a fixed level while the gate remains high (key held). Unlike the other parameters (which are times), Sustain is a **level** from 0.0 to 1.0.

- **Low sustain** (0.0-0.3): Percussive, the sound dies away while key is held
- **Medium sustain** (0.3-0.7): Balanced, natural decay to held level
- **High sustain** (0.7-1.0): Full, organ-like sustained sound

### Release

The **Release** phase begins when the gate goes low (key released). The envelope falls from the current level to 0.

- **Short release** (10-50 ms): Abrupt stop, staccato
- **Medium release** (50-300 ms): Natural fade
- **Long release** (300 ms+): Lingering, ambient tails

## Usage Tips

### Basic Volume Envelope

Connect envelope to VCA for note-shaped volume:

```
[Keyboard] ──Gate──> [ADSR] ──> [VCA CV]
[Oscillator] ──> [VCA In] ──> [Output]
```

### Filter Envelope

Create dynamic timbral changes:

```
[Keyboard] ──Gate──> [ADSR] ──> [Filter Cutoff CV]
```

- Fast attack + fast decay = "plucky" brightness
- Slow attack = gradual brightening
- Combine with VCA envelope for complex shapes

### Dual Envelopes

Use separate envelopes for amplitude and filter:

```
[Keyboard Gate] ──> [ADSR 1] ──> [VCA CV] (volume shape)
                ──> [ADSR 2] ──> [Filter CV] (timbre shape)
```

This allows independent control:
- VCA envelope: Long release for sustained notes
- Filter envelope: Short decay for initial brightness

### Inverted Envelope

Use the **Inv** output for "reversed" modulation:

```
[ADSR Inv] ──> [Filter Cutoff CV]
```

- Filter opens as note releases
- Creates unusual, "backwards" effects

### End of Cycle Triggering

Use **EOC** output to trigger other events:

```
[ADSR EOC] ──> [Another ADSR Gate]
```

- Chain envelopes for complex shapes
- Trigger samples when envelope completes
- Create automatic sequences

### Retrigger Behavior

The **Retrig** input restarts the envelope from Attack:

```
[LFO Square] ──> [ADSR Retrig]
```

- Creates rhythmic retriggering
- Useful for tremolo-like effects
- Each trigger restarts the Attack phase

### Looping Envelope

Create a looping envelope by connecting EOC to Gate:

```
[ADSR EOC] ──> [ADSR Gate]
```

This creates a repeating cycle (like a complex LFO). Adjust A, D, S, R to shape the loop.

## Common Envelope Shapes

### Pluck (Piano, Guitar)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 1 ms | 200 ms | 0.0 | 100 ms |

Fast attack, immediate decay to silence, short release.

### Pad (Strings, Ambient)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 500 ms | 1 s | 0.7 | 2 s |

Slow attack, gradual decay, sustained level, long release.

### Organ (Sustained)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 1 ms | 10 ms | 1.0 | 50 ms |

Instant attack, no decay, full sustain, quick release.

### Brass (Soft Attack)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 100 ms | 300 ms | 0.8 | 200 ms |

Moderate attack (breath), slight decay, high sustain.

### Percussion (Drum, Pluck)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 0.1 ms | 100 ms | 0.0 | 50 ms |

Instant attack, quick decay, no sustain.

## Connection Examples

### Standard Synth Voice
```
[Keyboard] ──V/Oct──> [Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
           ──Gate───> [ADSR 1] ─────────────────────┘
                      [ADSR 2] ──> [Filter Cutoff CV]
```

### Triggered Drone
```
[Clock] ──> [ADSR Gate]
            [ADSR] ──> [VCA CV]
[Oscillator] ──> [VCA] ──> [Output]
```

### Envelope Following
```
[ADSR] ──> [Attenuverter] ──> [Multiple Destinations]
```

## Related Modules

- [VCA](../utilities/vca.md) - Control amplitude with envelope
- [SVF Filter](../filters/svf-filter.md) - Modulate filter with envelope
- [Keyboard Input](../midi/keyboard.md) - Gate source for envelope
- [LFO](./lfo.md) - Alternative modulation source
