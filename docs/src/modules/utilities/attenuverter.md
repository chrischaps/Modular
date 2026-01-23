# Attenuverter

**Module ID**: `util.attenuverter`
**Category**: Utilities
**Header Color**: Yellow

![Attenuverter Module](../../images/module-attenuverter.png)
*The Attenuverter module*

## Description

The Attenuverter is a utility module that scales, inverts, and offsets signals. The name combines "attenuate" (reduce) and "invert" (flip). It's an essential tool for adapting modulation signals to fit the needs of your destination parameters.

**Key functions:**
- Reduce signal strength (attenuation)
- Flip signal polarity (inversion)
- Shift signal baseline (offset)

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Any (matches input) | Signal to be processed |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Any (matches input) | Processed signal |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Amount** | -1.0 to +1.0 | 1.0 | Scale factor (negative = inverted) |
| **Offset** | -1.0 to +1.0 | 0.0 | DC offset added to output |

## How It Works

The attenuverter applies this formula:

```
Output = (Input × Amount) + Offset
```

### Amount Knob

- **+1.0**: Full positive (signal unchanged)
- **+0.5**: Half strength
- **0.0**: Signal canceled (only offset remains)
- **-0.5**: Half strength, inverted
- **-1.0**: Full negative (signal inverted)

### Offset Knob

Adds a constant value to shift the signal:

- **+0.5**: Shifts signal up by 0.5
- **0.0**: No shift
- **-0.5**: Shifts signal down by 0.5

## Usage Tips

### Reducing Modulation Depth

An LFO may be too strong for subtle vibrato:

```
[LFO] ──> [Attenuverter] ──> [Oscillator FM]
           (Amount: 0.2)
```

Only 20% of the LFO reaches the oscillator.

### Inverting Modulation

Flip the direction of modulation:

```
[Envelope] ──> [Attenuverter] ──> [Filter Cutoff CV]
                (Amount: -1.0)
```

Instead of the filter opening on attack, it closes.

### Converting Unipolar to Bipolar

An envelope (0 to 1) needs to swing both ways:

```
[Envelope] ──> [Attenuverter] ──> [Pitch CV]
                (Amount: 2.0, Offset: -1.0)
```

- Original: 0 to 1
- After: -1 to +1 (centered around zero)

### Converting Bipolar to Unipolar

An LFO (-1 to +1) needs to stay positive:

```
[LFO] ──> [Attenuverter] ──> [CV Destination]
           (Amount: 0.5, Offset: 0.5)
```

- Original: -1 to +1
- After: 0 to 1

### Creating a Fixed Voltage

With no input connected, the offset becomes a constant:

```
[Attenuverter] ──> [Parameter CV]
(Input: none, Offset: 0.7)
```

Outputs constant 0.7—useful for manual CV sources.

### Scaling for Range Matching

Match an envelope's range to a parameter:

```
[Envelope (0-1)] ──> [Attenuverter] ──> [Filter Cutoff]
                      (Amount: 0.6, Offset: 0.2)
```

- Output ranges from 0.2 to 0.8
- Never fully closed, never fully open

### Ducking/Sidechain Effect

Invert an envelope for ducking:

```
[Kick Gate] ──> [Envelope] ──> [Attenuverter] ──> [Pad VCA CV]
                                (Amount: -1.0, Offset: 1.0)
```

- Kick hits → Envelope rises → Attenuverter inverts → Pad ducks
- Offset keeps pad at full volume when envelope is zero

### Precise Modulation Amount

Many parameters don't have their own "modulation depth" control. Use attenuverter:

```
[LFO] ──> [Attenuverter (Amount: 0.3)] ──> [Filter Cutoff CV]
```

Now you have precise control over modulation depth.

## Visual Understanding

### Positive Amount
```
Input:  ╱╲╱╲╱╲   (LFO)
Output: ╱╲╱╲╱╲   (Same direction, possibly smaller)
```

### Negative Amount (Inverted)
```
Input:  ╱╲╱╲╱╲
Output: ╲╱╲╱╲╱   (Flipped upside down)
```

### With Offset
```
Input:  ╱╲╱╲╱╲   (Centered at 0)
Output: ¯╱╲╱╲╱╲¯ (Shifted up by offset)
```

## Common Configurations

| Use Case | Amount | Offset |
|----------|--------|--------|
| Full pass | +1.0 | 0.0 |
| Invert | -1.0 | 0.0 |
| Half strength | +0.5 | 0.0 |
| Inverted half | -0.5 | 0.0 |
| Unipolar to bipolar | +2.0 | -1.0 |
| Bipolar to unipolar | +0.5 | +0.5 |
| Fixed voltage | N/A | (your value) |

## Connection Examples

### Subtle Vibrato
```
[LFO] ──> [Attenuverter (0.1)] ──> [Osc FM]
```

### Inverted Filter Envelope
```
[Envelope] ──> [Attenuverter (-0.8)] ──> [Filter CV]
```

### Modulation Depth Control
```
[LFO] ──> [Attenuverter] ──> [Parameter]
              ↑
         [Mod Wheel] (controls Amount)
```

### Creating Complex Modulation
```
[LFO 1] ──> [Attenuverter 1 (0.5)] ──┐
                                      ├──> [Mixer] ──> [Destination]
[LFO 2] ──> [Attenuverter 2 (-0.3)] ─┘
```

## Tips

1. **Start at 0**: When patching new modulation, start Amount at 0 and slowly increase
2. **Watch polarity**: Some modulations sound better inverted
3. **Use offset for bias**: Shift the modulation center to taste
4. **Chain if needed**: Multiple attenuverters can create complex scaling

## Related Modules

- [LFO](../modulation/lfo.md) - Common source to attenuate
- [ADSR Envelope](../modulation/adsr.md) - Common source to scale/invert
- [Mixer](./mixer.md) - Combine after scaling
- [VCA](./vca.md) - Alternative way to control signal strength
