# Basic Subtractive Synth

A classic subtractive synthesizer with oscillators, filter, envelope, and modulation.

![Basic Subtractive Patch](../images/recipe-basic-subtractive.png)
*The basic subtractive synth patch*

## Overview

Subtractive synthesis starts with a harmonically rich waveform (saw or square) and uses a filter to "subtract" frequencies, shaping the tone. Combined with envelopes for dynamic control, this creates expressive, versatile sounds.

**Character**: Warm, punchy, expressive
**Good for**: Bass, leads, pads, keys

## Modules Used

- 1x [Oscillator](../modules/sources/oscillator.md)
- 1x [SVF Filter](../modules/filters/svf-filter.md)
- 1x [VCA](../modules/utilities/vca.md)
- 2x [ADSR Envelope](../modules/modulation/adsr.md)
- 1x [Keyboard Input](../modules/midi/keyboard.md)
- 1x [Audio Output](../modules/output/audio-output.md)

## Patch Diagram

```
┌──────────┐      ┌──────────┐      ┌─────┐      ┌────────┐
│ Keyboard │─V/Oct─▶│Oscillator│─Audio─▶│Filter│─Audio─▶│  VCA   │─▶ Output
│          │      │  (Saw)   │      │(SVF) │      │        │
└────┬─────┘      └──────────┘      └───▲──┘      └───▲────┘
     │                                  │             │
     │ Gate                             │             │
     │         ┌──────────┐             │      ┌──────┴──────┐
     └────────▶│  ADSR 1  │─────────────┘      │   ADSR 2    │
               │ (Filter) │                    │ (Amplitude) │
               └──────────┘                    └─────────────┘
                                                      ▲
                                                      │ Gate
               ┌──────────────────────────────────────┘
               │
        [From Keyboard Gate]
```

## Step-by-Step Setup

### 1. Add Core Modules

Add and position:
1. **Keyboard Input** (left side)
2. **Oscillator** (center-left)
3. **SVF Filter** (center)
4. **VCA** (center-right)
5. **Audio Output** (right)

### 2. Create the Audio Path

Connect the main audio signal:

```
[Oscillator Audio Out] ──▶ [Filter Input]
[Filter Lowpass Out] ──▶ [VCA Input]
[VCA Output] ──▶ [Audio Output Mono]
```

### 3. Add Pitch Control

```
[Keyboard V/Oct] ──▶ [Oscillator V/Oct]
```

### 4. Add Amplitude Envelope

Add **ADSR 1** for volume control:

```
[Keyboard Gate] ──▶ [ADSR 1 Gate]
[ADSR 1 Env] ──▶ [VCA CV]
```

**ADSR 1 Settings (Amplitude)**:
| Parameter | Value | Reason |
|-----------|-------|--------|
| Attack | 5 ms | Quick start |
| Decay | 200 ms | Initial drop |
| Sustain | 0.7 | Held level |
| Release | 300 ms | Smooth fade |

### 5. Add Filter Envelope

Add **ADSR 2** for filter movement:

```
[Keyboard Gate] ──▶ [ADSR 2 Gate]
[ADSR 2 Env] ──▶ [Filter Cutoff CV]
```

**ADSR 2 Settings (Filter)**:
| Parameter | Value | Reason |
|-----------|-------|--------|
| Attack | 1 ms | Immediate brightness |
| Decay | 300 ms | Gradual close |
| Sustain | 0.3 | Darker sustained tone |
| Release | 200 ms | Follow amp |

### 6. Set Module Parameters

**Oscillator**:
- Waveform: **Saw** (harmonically rich)
- Frequency: 440 Hz (controlled by keyboard)

**Filter**:
- Cutoff: **800 Hz** (base cutoff)
- Resonance: **0.3** (slight emphasis)
- CV Amount: **0.6** (envelope range)

**VCA**:
- Level: **1.0**
- Response: **Exponential**

## Playing the Patch

1. Press keys on your computer keyboard
2. Adjust filter **Cutoff** for brightness
3. Adjust filter **Resonance** for character
4. Modify envelopes for different articulation

## Variations

### Punchy Bass

```
Oscillator: Square wave
Filter Cutoff: 400 Hz
Filter Resonance: 0.1
ADSR 1: A:1ms D:100ms S:0.5 R:50ms
ADSR 2: A:1ms D:200ms S:0.1 R:50ms
```

### Smooth Lead

```
Oscillator: Saw wave
Filter Cutoff: 2000 Hz
Filter Resonance: 0.2
ADSR 1: A:50ms D:100ms S:0.8 R:500ms
ADSR 2: A:20ms D:500ms S:0.5 R:300ms
Keyboard Glide: 100ms
```

### Plucky Keys

```
Oscillator: Saw wave
Filter Cutoff: 1500 Hz
Filter Resonance: 0.4
ADSR 1: A:1ms D:200ms S:0.0 R:100ms
ADSR 2: A:1ms D:150ms S:0.0 R:100ms
```

### Soft Pad

```
Oscillator: Triangle wave
Filter Cutoff: 3000 Hz
Filter Resonance: 0.1
ADSR 1: A:500ms D:200ms S:0.8 R:1000ms
ADSR 2: A:200ms D:500ms S:0.6 R:800ms
```

## Enhancements

### Add a Second Oscillator

For thicker sound:

```
[Oscillator 2 (Saw, +7 cents detune)] ──▶ [Mixer]
[Oscillator 1] ──▶ [Mixer]
[Mixer] ──▶ [Filter]
```

### Add Vibrato

For expressiveness:

```
[LFO (5 Hz, low depth)] ──▶ [Oscillator FM]
```

### Add Effects

For space:

```
[VCA] ──▶ [Delay] ──▶ [Reverb] ──▶ [Output]
```

## Troubleshooting

**No sound**: Check all connections, ensure keyboard is focused

**Always sounds**: Check envelope gate connections

**Too quiet**: Increase VCA level or Output level

**Too bright**: Lower filter cutoff or envelope CV amount

**Too dull**: Raise filter cutoff or use Saw waveform

## What You've Learned

- Basic subtractive synthesis signal flow
- Using separate envelopes for amplitude and timbre
- How filter cutoff and resonance affect tone
- Creating variations with envelope settings
