# Mixer

**Module ID**: `util.mixer`
**Category**: Utilities
**Header Color**: Yellow

![Mixer Module](../../images/module-mixer.png)
*The Mixer module*

## Description

The Mixer combines multiple audio or control signals into a single output. This 2-channel mixer allows you to blend signals with independent level control for each channel, plus a master output level.

Mixers are essential for:
- Combining multiple oscillators
- Blending modulation sources
- Creating submixes before effects
- Layering sounds

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Ch 1** | Audio/Control (Blue/Orange) | First input channel |
| **Ch 2** | Audio/Control (Blue/Orange) | Second input channel |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Mix** | Audio/Control (Blue/Orange) | Combined output of both channels |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Ch 1 Level** | 0.0 - 1.0 | 1.0 | Level of channel 1 in the mix |
| **Ch 2 Level** | 0.0 - 1.0 | 1.0 | Level of channel 2 in the mix |
| **Master** | 0.0 - 1.0 | 1.0 | Overall output level |

## How It Works

The mixer sums the inputs after applying their respective levels:

```
Mix = (Ch1 × Ch1Level + Ch2 × Ch2Level) × Master
```

**Important**: Summing two full-scale signals can exceed the -1 to +1 range. Use the channel levels to prevent clipping, or rely on the output module's limiter.

## Usage Tips

### Combining Oscillators

Create a richer sound by mixing multiple oscillators:

```
[Oscillator 1 (Saw)] ──> [Mixer Ch 1]
[Oscillator 2 (Square)] ──> [Mixer Ch 2]
[Mixer] ──> [Filter] ──> [VCA] ──> [Output]
```

- Detune oscillators slightly for thickness
- Use different waveforms for complexity
- Adjust levels to taste

### Detuned Unison

Classic "supersaw" technique:

```
[Osc 1 (Detune: 0)] ──> [Mixer Ch 1]
[Osc 2 (Detune: +7 cents)] ──> [Mixer Ch 2]
```

The slight pitch difference creates a chorusing effect.

### Octave Layering

Add harmonic richness:

```
[Osc 1 (C3)] ──> [Mixer Ch 1]
[Osc 2 (C4, octave up)] ──> [Mixer Ch 2] (Level: 0.5)
```

Lower the higher octave to keep the fundamental prominent.

### Blending Modulation

Combine modulation sources:

```
[LFO (slow)] ──> [Mixer Ch 1]
[Envelope] ──> [Mixer Ch 2]
[Mixer] ──> [Filter Cutoff CV]
```

The filter responds to both the cyclic LFO and the triggered envelope.

### Wet/Dry Effect Blend

Mix processed and original signals:

```
[Audio] ──> [Effect Input]
        ──> [Mixer Ch 1] (Dry)
[Effect Output] ──> [Mixer Ch 2] (Wet)
[Mixer] ──> [Output]
```

Adjust channel levels to control effect intensity.

### Audio + Sub-Oscillator

Add weight with a sub-bass:

```
[Main Osc (Saw)] ──> [Mixer Ch 1]
[Sub Osc (Sine, -1 octave)] ──> [Mixer Ch 2] (Level: 0.6)
```

### Chaining Mixers

Need more than 2 channels? Chain mixers:

```
[Osc 1] ──> [Mixer A Ch 1]
[Osc 2] ──> [Mixer A Ch 2]
[Mixer A] ──> [Mixer B Ch 1]
[Osc 3] ──> [Mixer B Ch 2]
[Mixer B] ──> [Output]
```

Or use multiple mixers into a final mixer.

### Level Staging

Manage levels to avoid clipping:

1. Set individual channel levels to ~0.7 each
2. Adjust Master to compensate
3. Watch output levels (use Oscilloscope if needed)

### Crossfading

Create a crossfade with complementary levels:

```
Ch 1 Level: 1.0 → 0.0
Ch 2 Level: 0.0 → 1.0
```

As one fades out, the other fades in. Automate with an LFO or envelope.

## Connection Examples

### Dual Oscillator Synth
```
[Keyboard V/Oct] ──> [Osc 1 V/Oct]
                 ──> [Osc 2 V/Oct]
[Osc 1] ──> [Mixer Ch 1]
[Osc 2] ──> [Mixer Ch 2]
[Mixer] ──> [Filter] ──> [VCA] ──> [Output]
```

### Parallel Modulation
```
[LFO] ──> [Mixer Ch 1]
[Random/S&H] ──> [Mixer Ch 2]
[Mixer] ──> [Parameter CV]
```

### Submix for Effects
```
[Lead Synth] ──> [Mixer Ch 1]
[Pad Synth] ──> [Mixer Ch 2]
[Mixer] ──> [Reverb] ──> [Output]
```

## Gain Staging Tips

| Scenario | Ch 1 | Ch 2 | Master |
|----------|------|------|--------|
| Equal blend | 0.7 | 0.7 | 1.0 |
| Ch 1 dominant | 0.9 | 0.4 | 1.0 |
| Subtle layer | 1.0 | 0.2 | 1.0 |
| Quiet mix | 1.0 | 1.0 | 0.5 |

## Audio vs Control Signals

The mixer works with both audio and control signals:

**Audio Mixing**:
- Combines waveforms
- Creates complex timbres
- Watch for clipping

**Control Mixing**:
- Combines modulation sources
- Creates complex modulation shapes
- No clipping concerns (but consider destination range)

## Related Modules

- [Oscillator](../sources/oscillator.md) - Primary signals to mix
- [VCA](./vca.md) - Level control for individual sources
- [Attenuverter](./attenuverter.md) - Scale signals before mixing
- [Audio Output](../output/audio-output.md) - Final destination with metering
