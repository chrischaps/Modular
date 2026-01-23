# Lush Pad

Create rich, evolving pad sounds with multiple oscillators, modulation, and effects.

![Lush Pad Patch](../images/recipe-lush-pad.png)
*The lush pad patch*

## Overview

Pads are sustained, atmospheric sounds that fill space and create ambience. This recipe combines detuned oscillators, slow modulation, and effects to create a rich, evolving pad sound.

**Character**: Warm, wide, evolving, dreamy
**Good for**: Ambient, cinematic, chillout, background textures

## Modules Used

- 2x [Oscillator](../modules/sources/oscillator.md)
- 1x [Mixer](../modules/utilities/mixer.md)
- 1x [SVF Filter](../modules/filters/svf-filter.md)
- 1x [VCA](../modules/utilities/vca.md)
- 1x [ADSR Envelope](../modules/modulation/adsr.md)
- 2x [LFO](../modules/modulation/lfo.md)
- 1x [Chorus](../modules/effects/chorus.md)
- 1x [Reverb](../modules/effects/reverb.md)
- 1x [Keyboard Input](../modules/midi/keyboard.md)
- 1x [Audio Output](../modules/output/audio-output.md)

## Patch Diagram

```
┌──────────┐
│ Keyboard │─V/Oct─┬─▶ [Osc 1 (Saw)]──┐
│          │       │                   ├─▶[Mixer]─▶[Filter]─▶[VCA]─▶[Chorus]─▶[Reverb]─▶[Out]
└────┬─────┘       └─▶ [Osc 2 (Saw)]──┘      ▲              ▲
     │                  (+7 cents)           │              │
     │ Gate                                  │              │
     └──────────────────────────────────────▶│◀────[ADSR]───┘
                                             │
                                        [LFO 1]
                                       (slow)

[LFO 2] ─────────────────────────────▶ [Osc 1 PWM]
(very slow)                           [Osc 2 PWM]
```

## Step-by-Step Setup

### 1. Dual Detuned Oscillators

The foundation of a thick pad is detuned oscillators:

**Oscillator 1**:
- Waveform: **Saw** (or Square for PWM)
- Detune: **0 cents** (reference)

**Oscillator 2**:
- Waveform: **Saw** (or Square for PWM)
- Detune: **+7 cents** (slight detune for thickness)

Connect both to the Mixer:
```
[Osc 1 Audio] ──▶ [Mixer Ch 1]
[Osc 2 Audio] ──▶ [Mixer Ch 2]
```

Both should track the keyboard:
```
[Keyboard V/Oct] ──▶ [Osc 1 V/Oct]
                 ──▶ [Osc 2 V/Oct]
```

### 2. Signal Path

```
[Mixer Out] ──▶ [Filter Input]
[Filter Lowpass] ──▶ [VCA Input]
[VCA Output] ──▶ [Chorus Input]
[Chorus Output] ──▶ [Reverb Input]
[Reverb Output] ──▶ [Audio Output]
```

### 3. Slow Attack Envelope

Pads have gentle attacks:

```
[Keyboard Gate] ──▶ [ADSR Gate]
[ADSR Env] ──▶ [VCA CV]
```

**ADSR Settings**:
| Parameter | Value | Why |
|-----------|-------|-----|
| Attack | 800 ms | Slow fade in |
| Decay | 500 ms | Gentle settle |
| Sustain | 0.8 | Nearly full while held |
| Release | 2000 ms | Long fade out |

### 4. Filter for Warmth

**Filter Settings**:
- Cutoff: **3000 Hz** (removes harshness)
- Resonance: **0.15** (subtle color)

### 5. Slow Filter Modulation

Add movement with LFO:

```
[LFO 1] ──▶ [Filter Cutoff CV]
```

**LFO 1 Settings**:
| Parameter | Value |
|-----------|-------|
| Waveform | Sine |
| Rate | 0.1 Hz (very slow) |
| Bipolar | On |

**Filter CV Amount**: 0.2 (subtle movement)

### 6. PWM for Animation (Optional)

If using Square waves, add PWM:

```
[LFO 2] ──▶ [Osc 1 PWM]
        ──▶ [Osc 2 PWM]
```

**LFO 2 Settings**:
| Parameter | Value |
|-----------|-------|
| Waveform | Triangle |
| Rate | 0.3 Hz |
| Bipolar | Off |

### 7. Chorus for Width

**Chorus Settings**:
| Parameter | Value |
|-----------|-------|
| Rate | 0.5 Hz |
| Depth | 0.4 |
| Voices | 2 |
| Stereo | 1.0 |
| Mix | 0.5 |

### 8. Reverb for Space

**Reverb Settings**:
| Parameter | Value |
|-----------|-------|
| Decay | 4.0 s |
| Pre-Delay | 50 ms |
| Size | 0.7 |
| Damping | 0.4 |
| Mix | 0.5 |

## Final Module Settings Summary

| Module | Key Settings |
|--------|--------------|
| Osc 1 | Saw/Square, Detune: 0 |
| Osc 2 | Saw/Square, Detune: +7c |
| Mixer | Ch1: 0.8, Ch2: 0.8 |
| Filter | Cutoff: 3kHz, Res: 0.15 |
| VCA | Level: 1.0 |
| ADSR | A:800ms D:500ms S:0.8 R:2000ms |
| LFO 1 | Sine, 0.1Hz (filter) |
| LFO 2 | Tri, 0.3Hz (PWM) |
| Chorus | Rate:0.5, Depth:0.4, Mix:0.5 |
| Reverb | Decay:4s, Mix:0.5 |

## Variations

### Darker Pad

```
Filter Cutoff: 1500 Hz
Reverb Damping: 0.6
LFO 1 Rate: 0.05 Hz (slower)
```

### Brighter Pad

```
Filter Cutoff: 5000 Hz
Osc Waveforms: Saw
Add subtle high-shelf EQ boost
```

### Evolving Pad

```
Add LFO 3 ──▶ Osc 2 Detune (very slow, very subtle)
LFO 1 Rate: 0.03 Hz (extremely slow)
Reverb Decay: 8s
```

### Sparse Pad

```
Attack: 2000 ms
Reverb Mix: 0.7
Filter Cutoff: 2000 Hz
Remove Chorus
```

### Thick Supersaw

```
Add Osc 3 (Detune: -5c)
Add Osc 4 (Detune: +12c)
Chorus Voices: 4
Reduce Reverb Mix: 0.3
```

## Enhancement Ideas

### Add Sub Oscillator

For weight:
```
[Osc 3 (Sine, -1 octave)] ──▶ [Mixer Ch 3] (low level)
```

### Velocity Expression

```
[Keyboard Velocity] ──▶ [Filter Cutoff CV]
```

Harder playing = brighter pad.

### Modulated Reverb

```
[LFO (very slow)] ──▶ [Reverb Decay CV]
```

Space itself evolves.

### Stereo Detuning

Pan oscillators slightly:
```
Osc 1: Slight left
Osc 2: Slight right
```

## Playing Tips

1. **Hold chords**: Pads are meant to sustain
2. **Use release**: Let notes fade naturally
3. **Layer with other sounds**: Pads provide background
4. **Play simply**: Complex melodies don't suit pads
5. **Use inversions**: Voice chords to avoid bass clutter

## Troubleshooting

**Too thin**: Add more oscillators, increase detune

**Too bright**: Lower filter cutoff

**Too static**: Add more LFO modulation

**Too muddy**: Raise filter cutoff, reduce reverb

**Doesn't cut through**: Reduce reverb mix, raise filter

## What You've Learned

- Detuned oscillators for thickness
- Slow envelopes for pad character
- LFO modulation for movement
- Effects layering for space and width
- Balancing multiple modulation sources
