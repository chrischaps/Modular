# LFO

**Module ID**: `mod.lfo`
**Category**: Modulation
**Header Color**: Orange

![LFO Module](../../images/module-lfo.png)
*The LFO module*

## Description

The **Low Frequency Oscillator** (LFO) generates slow, cyclic waveforms used for modulation rather than audio. LFOs add movement and animation to your patches by continuously varying parameters like filter cutoff, oscillator pitch, or amplitude.

While structurally similar to audio oscillators, LFOs typically operate at sub-audio frequencies (0.01 Hz to ~20 Hz), creating effects like vibrato, tremolo, and filter sweeps.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Rate CV** | Control (Orange) | Modulation input for LFO rate |
| **Sync** | Gate (Green) | Reset phase on rising edge |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Main** | Control (Orange) | Primary output (unipolar 0-1 or bipolar -1 to +1) |
| **Inv** | Control (Orange) | Inverted output |
| **Square** | Control (Orange) | Square wave output (regardless of waveform setting) |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Waveform** | Sine/Triangle/Square/Saw | Sine | Shape of the LFO wave |
| **Rate** | 0.01 Hz - 20 Hz | 1 Hz | Speed of oscillation |
| **Bipolar** | On/Off | Off | Off = 0 to 1, On = -1 to +1 |
| **Phase** | 0° - 360° | 0° | Starting phase of waveform |

## Waveforms

### Sine

![LFO Sine](../../images/lfo-sine.png)

Smooth, continuous modulation with no sharp edges.

**Best for:**
- Vibrato (pitch modulation)
- Gentle filter sweeps
- Smooth tremolo
- Natural-sounding movement

### Triangle

![LFO Triangle](../../images/lfo-triangle.png)

Linear up/down movement with turnaround points.

**Best for:**
- Similar to sine but with more "edge"
- Classic synthesizer modulation
- Steady back-and-forth motion

### Square

![LFO Square](../../images/lfo-square.png)

Instant switching between minimum and maximum.

**Best for:**
- Rhythmic on/off effects
- Hard tremolo/chopping
- Sample & Hold-like stepping
- Gate-like modulation

### Sawtooth

![LFO Saw](../../images/lfo-saw.png)

Gradual rise followed by instant reset.

**Best for:**
- Rising filter sweeps
- Rhythmic builds
- Asymmetric modulation

## Usage Tips

### Vibrato

Classic pitch wobble effect:

```
[LFO] ──> [Oscillator FM]
```

- **Rate**: 5-7 Hz for natural vibrato
- **Waveform**: Sine or Triangle
- **Depth**: Low FM Amount on oscillator
- **Bipolar**: On (pitch goes up AND down)

### Tremolo

Volume modulation effect:

```
[LFO] ──> [VCA CV]
```

- **Rate**: 4-8 Hz for classic tremolo
- **Waveform**: Sine (smooth) or Triangle
- **Bipolar**: Off (volume only goes down from max)

### Filter Sweep (Wobble)

Rhythmic filter movement:

```
[LFO] ──> [Filter Cutoff CV]
```

- **Rate**: Sync to tempo for rhythmic effect
- **Waveform**:
  - Sine/Triangle = smooth wobble
  - Square = choppy rhythm
  - Saw = rising sweeps
- **Bipolar**: Usually Off

### PWM (Pulse Width Modulation)

Animate square wave timbre:

```
[LFO] ──> [Oscillator PWM]
```

- **Rate**: 0.5-3 Hz for subtle animation
- **Waveform**: Triangle or Sine
- Creates chorus-like thickening

### Tempo Sync

Lock LFO to musical time by triggering sync from a clock:

```
[Clock] ──> [LFO Sync]
```

The LFO resets its phase on each clock pulse, synchronizing to the tempo.

### Phase Offset

Use the Phase parameter when running multiple LFOs:

```
[LFO 1 (Phase: 0°)] ──> [Osc 1 FM]
[LFO 2 (Phase: 180°)] ──> [Osc 2 FM]
```

This creates movement that's related but not identical.

### Using the Square Output

The dedicated Square output is useful for:
- Triggering envelopes rhythmically
- Creating rhythmic gates
- Sample & Hold clock

```
[LFO Square] ──> [ADSR Gate]
```

### Unipolar vs Bipolar

**Unipolar (0 to 1)**:
- Filter cutoff (always positive)
- Volume (can't go negative)
- Most parameters

**Bipolar (-1 to +1)**:
- Pitch modulation (up AND down)
- Pan modulation (left AND right)
- Any parameter where negative makes sense

### Modulating the Rate

Create evolving modulation by controlling LFO speed:

```
[LFO 2 (slow)] ──> [LFO 1 Rate CV]
```

The modulation speed itself varies over time.

## Common Settings

### Subtle Vibrato
| Rate | Waveform | Bipolar |
|------|----------|---------|
| 6 Hz | Sine | On |

### Dramatic Wobble
| Rate | Waveform | Bipolar |
|------|----------|---------|
| 2 Hz | Triangle | Off |

### Rhythmic Chop
| Rate | Waveform | Bipolar |
|------|----------|---------|
| Synced | Square | Off |

### Slow Evolution
| Rate | Waveform | Bipolar |
|------|----------|---------|
| 0.1 Hz | Sine | On |

### PWM Animation
| Rate | Waveform | Bipolar |
|------|----------|---------|
| 0.5 Hz | Triangle | Off |

## Connection Examples

### Multi-Destination Modulation
```
[LFO] ──> [Filter Cutoff CV]
      ──> [Oscillator PWM]
      ──> [VCA CV] (via Attenuverter)
```

### Stereo Movement
```
[LFO (Phase: 0°)] ──> [Left Channel Parameter]
[LFO (Phase: 90°)] ──> [Right Channel Parameter]
```

### Stepped Random (with S&H)
```
[LFO] ──> [Sample & Hold Input]
[LFO Square] ──> [Sample & Hold Trigger]
[S&H Output] ──> [Modulation Destination]
```

## LFO vs Envelope

| LFO | Envelope |
|-----|----------|
| Continuous, cyclic | One-shot, triggered |
| Constant motion | Responds to events |
| Same shape always | ADSR shape |
| Time-based | Event-based |

Use LFO for ongoing animation, envelopes for note-shaped modulation.

## Related Modules

- [ADSR Envelope](./adsr.md) - Event-triggered modulation
- [Clock](./clock.md) - For LFO sync
- [VCA](../utilities/vca.md) - Tremolo destination
- [SVF Filter](../filters/svf-filter.md) - Filter modulation destination
- [Oscillator](../sources/oscillator.md) - Vibrato/FM destination
