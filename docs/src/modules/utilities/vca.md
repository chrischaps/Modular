# VCA

**Module ID**: `util.vca`
**Category**: Utilities
**Header Color**: Yellow

![VCA Module](../../images/module-vca.png)
*The VCA module*

## Description

The **Voltage Controlled Amplifier** (VCA) controls the amplitude (volume) of a signal based on a control voltage input. It's an essential building block that allows envelopes, LFOs, and other control signals to shape the dynamics of your sound.

Despite the name suggesting audio use only, VCAs can process any signal type—they're equally useful for controlling the amount of modulation in a patch.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Signal to be amplitude controlled |
| **CV** | Control (Orange) | Control voltage input. 0 = silence, 1 = full volume |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Audio (Blue) | Amplitude-controlled signal |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Level** | 0.0 - 1.0 | 1.0 | Base output level (multiplied with CV) |
| **Response** | Linear/Exponential | Exponential | How CV affects amplitude |

## How It Works

The VCA multiplies the input signal by the CV value:

```
Output = Input × CV × Level
```

- **CV = 0**: Output is silent (signal × 0)
- **CV = 0.5**: Output at half amplitude
- **CV = 1**: Output at full amplitude (determined by Level knob)

### Linear vs Exponential Response

**Linear**:
- Direct relationship: double the CV, double the volume
- Best for tremolo and amplitude modulation
- Used for CV processing

**Exponential**:
- Matches human perception of loudness
- Small CV changes are subtle, large changes are dramatic
- Best for envelope-controlled volume (most common use)

## Usage Tips

### Basic Envelope Control

The most common VCA use—shaping volume with an envelope:

```
[Keyboard] ──Gate──> [ADSR] ──> [VCA CV]
[Oscillator] ──> [VCA Input] ──> [Output]
```

When you press a key:
1. Gate triggers the envelope
2. Envelope shapes the CV
3. VCA lets sound through based on envelope level

Without a VCA (or with CV always at 1), the oscillator would drone continuously.

### Tremolo

Use an LFO for rhythmic volume variation:

```
[LFO] ──> [VCA CV]
[Oscillator] ──> [VCA Input] ──> [Output]
```

- **Rate**: 4-8 Hz for classic tremolo
- **Response**: Linear for more pronounced effect
- The sound pulses in volume with the LFO rhythm

### Modulation Amount Control

Use a VCA to control how much modulation reaches a destination:

```
[LFO] ──> [VCA Input]
[Envelope] ──> [VCA CV]
[VCA Output] ──> [Filter Cutoff]
```

This creates modulation that fades in/out with the envelope—the filter wobble increases as the note develops.

### Manual Level Control

Without CV connected, the Level knob acts as a simple volume control:

```
[Signal] ──> [VCA] ──> [Mixer]
             (Level: 0.7)
```

### Ducking/Sidechain

Create pumping effects by using an inverted envelope:

```
[Kick Trigger] ──> [ADSR] ──> [Attenuverter (inverted)] ──> [VCA CV]
[Pad] ──> [VCA Input] ──> [Output]
```

When the kick hits, the pad ducks down, then rises back up.

### Ring Modulation

At audio-rate CV, VCA becomes a ring modulator:

```
[Oscillator 1] ──> [VCA Input]
[Oscillator 2] ──> [VCA CV]
[VCA Output] ──> [Output]
```

This creates sum and difference frequencies—metallic, bell-like tones.

### CV Crossfading

Use a VCA to fade between two signals:

```
[Signal A] ──> [VCA 1 Input]
[Signal B] ──> [VCA 2 Input]
[Crossfade CV] ──> [VCA 1 CV]
[Inverted Crossfade CV] ──> [VCA 2 CV]
[VCA 1 + VCA 2] ──> [Mixer] ──> [Output]
```

### Velocity Sensitivity

Scale envelope output by MIDI velocity:

```
[MIDI Note Velocity] ──> [VCA CV]
[ADSR Output] ──> [VCA Input]
[VCA Output] ──> [Final VCA CV]
```

Harder key presses result in louder notes.

## VCA Placement in Signal Chain

VCAs typically go near the end of the audio chain:

```
[Oscillator] ──> [Filter] ──> [VCA] ──> [Effects] ──> [Output]
                              ↑
                         [Envelope]
```

**Why this order?**
- Oscillator generates sound
- Filter shapes tone
- VCA controls volume
- Effects process the shaped sound

## Connection Examples

### Standard Synth Voice
```
[Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
                    ↑            ↑
              [Envelope 1]  [Envelope 2]
```

### Tremolo Effect
```
[Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
                                 ↑
                              [LFO]
```

### Modulation Depth Control
```
[LFO] ──> [VCA] ──> [Filter Cutoff CV]
            ↑
      [Mod Wheel]
```

### Velocity-Sensitive Patch
```
[MIDI Velocity] ──> [VCA 1 CV]
[Envelope] ──> [VCA 1 Input]
[VCA 1 Output] ──> [VCA 2 CV]
[Oscillator] ──> [Filter] ──> [VCA 2] ──> [Output]
```

## Tips

1. **Always use a VCA** for envelope-controlled sounds—it's what turns a drone into a playable note
2. **Exponential response** sounds more natural for volume changes
3. **Linear response** is better for AM/tremolo effects
4. **Chain VCAs** for complex amplitude control
5. **Use the Level knob** to balance signals in your patch

## Related Modules

- [ADSR Envelope](../modulation/adsr.md) - Primary CV source for VCA
- [LFO](../modulation/lfo.md) - Tremolo modulation source
- [Mixer](./mixer.md) - Combine multiple VCA outputs
- [Attenuverter](./attenuverter.md) - Scale CV before VCA
