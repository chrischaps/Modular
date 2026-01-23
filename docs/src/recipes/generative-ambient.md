# Generative Ambient

Create a self-playing ambient patch that evolves endlessly without input.

![Generative Ambient Patch](../images/recipe-generative-ambient.png)
*The generative ambient patch*

## Overview

Generative music creates itself through interconnected systems of clocks, sequences, and randomness. This patch plays indefinitely, always changing, always familiar—perfect for ambient backgrounds, meditation, or sleep.

**Character**: Ethereal, evolving, infinite, peaceful
**Good for**: Ambient backgrounds, meditation, installations, sleep

## Modules Used

- 1x [Clock](../modules/modulation/clock.md)
- 1x [Sequencer](../modules/utilities/sequencer.md)
- 1x [Sample & Hold](../modules/utilities/sample-hold.md)
- 2x [Oscillator](../modules/sources/oscillator.md)
- 1x [SVF Filter](../modules/filters/svf-filter.md)
- 1x [VCA](../modules/utilities/vca.md)
- 2x [ADSR Envelope](../modules/modulation/adsr.md)
- 2x [LFO](../modules/modulation/lfo.md)
- 1x [Delay](../modules/effects/delay.md)
- 1x [Reverb](../modules/effects/reverb.md)
- 1x [Audio Output](../modules/output/audio-output.md)

## The Concept

```
Clock ──▶ Sequencer ──▶ Oscillator ──▶ Filter ──▶ VCA ──▶ Effects ──▶ Output
  │                                       ▲         ▲
  └──▶ S&H ──▶ Filter Mod                 │         │
       (random)                       LFOs      ADSR (from clock)
```

Key elements:
1. **Clock** provides regular timing
2. **Sequencer** creates melodic patterns
3. **Sample & Hold** adds randomness
4. **LFOs** create slow movement
5. **Long envelopes** create gentle dynamics
6. **Heavy effects** create space and blur

## Step-by-Step Setup

### 1. The Clock

The heartbeat of the patch:

**Clock Settings**:
| Parameter | Value |
|-----------|-------|
| BPM | 40 (very slow) |
| Pulse Width | 50% |

### 2. Melodic Sequencer

Create a simple, pentatonic sequence:

```
[Clock 1/4] ──▶ [Sequencer Clock]
```

**Sequencer Settings**:
- Length: **8 steps**
- Direction: **Forward**

Program a pentatonic scale (no "wrong" notes):

| Step | CV (Note) |
|------|-----------|
| 1 | C (0.0) |
| 2 | D (0.167) |
| 3 | E (0.333) |
| 4 | G (0.583) |
| 5 | A (0.75) |
| 6 | G (0.583) |
| 7 | E (0.333) |
| 8 | D (0.167) |

Gates: All ON (or create rhythm by turning some OFF)

### 3. Random Modulation

Add controlled randomness:

```
[LFO 1 (slow triangle)] ──▶ [S&H Input]
[Clock 1/8] ──▶ [S&H Trigger]
[S&H Output] ──▶ [Filter Cutoff CV]
```

This creates stepped, random-ish filter movement.

### 4. Dual Oscillators

```
[Sequencer CV] ──▶ [Osc 1 V/Oct]
               ──▶ [Osc 2 V/Oct]
```

**Oscillator 1**: Triangle wave
**Oscillator 2**: Sine wave, +1 octave

Mix together:
```
[Osc 1] ──▶ [Mixer Ch 1] (0.8)
[Osc 2] ──▶ [Mixer Ch 2] (0.4)
```

### 5. Filter

```
[Mixer] ──▶ [Filter Input]
```

**Filter Settings**:
- Cutoff: **1500 Hz**
- Resonance: **0.2**
- CV Amount: **0.4** (from S&H)

### 6. Amplitude Envelope

```
[Sequencer Gate] ──▶ [ADSR 1 Gate]
[ADSR 1] ──▶ [VCA CV]
```

**ADSR 1 (Amplitude)**:
| Parameter | Value | Why |
|-----------|-------|-----|
| Attack | 300 ms | Soft entry |
| Decay | 500 ms | Gentle fall |
| Sustain | 0.5 | Held tone |
| Release | 2000 ms | Long fade |

### 7. Slow LFO Movement

Add slow evolution:

```
[LFO 2] ──▶ [Osc 1 Detune] (subtle)
        ──▶ [Filter Resonance] (subtle)
```

**LFO 2 Settings**:
- Waveform: Sine
- Rate: **0.03 Hz** (30+ seconds per cycle)
- Bipolar: On

### 8. Effects Chain

```
[VCA] ──▶ [Delay] ──▶ [Reverb] ──▶ [Output]
```

**Delay Settings**:
| Parameter | Value |
|-----------|-------|
| Time L | 600 ms |
| Time R | 800 ms |
| Feedback | 0.5 |
| Mix | 0.4 |
| HP Filter | 200 Hz |
| LP Filter | 4000 Hz |

**Reverb Settings**:
| Parameter | Value |
|-----------|-------|
| Decay | 8 s |
| Pre-Delay | 100 ms |
| Size | 0.9 |
| Damping | 0.5 |
| Mix | 0.6 |

## Making It More Generative

### Add Probability

Not every step triggers:

Make some sequencer gates OFF to create rests and variation.

### Multiple Time Scales

Add a slower sequence layer:

```
[Clock 1/2] ──▶ [Sequencer 2 (4 steps)] ──▶ [Drone Oscillator]
```

This creates an even slower-moving bass drone.

### Evolving Sequence

Occasionally change the sequence:

```
[Very slow clock] ──▶ [Random trigger to sequence edit]
```

Or manually tweak sequence values occasionally.

### Self-Modifying Patch

Route slow LFOs to sequence CV inputs:

```
[LFO (very slow)] ──▶ [Attenuverter (tiny amount)] ──▶ [Seq Step 1 CV]
```

The sequence gradually shifts.

## Variations

### Darker Ambient

```
Filter Cutoff: 800 Hz
Remove high oscillator
Reverb Damping: 0.7
BPM: 30
```

### Brighter Ambient

```
Filter Cutoff: 4000 Hz
Add shimmer (short delay with high feedback, filtered)
Reverb Damping: 0.2
```

### Rhythmic Ambient

```
BPM: 60
More complex gate pattern
Shorter ADSR release (500ms)
Delay synced to tempo
```

### Drone-Based

```
Remove sequencer
Use very slow S&H for pitch
Attack: 5000 ms
Release: 10000 ms
```

## Advanced Techniques

### Multiple Voices

Add a second, independent voice with different timing:

```
[Clock 1/3] ──▶ [Sequencer 2] ──▶ [Voice 2]
```

The different divisions create polyrhythmic patterns.

### Feedback Networks

Carefully route modulation outputs back to modulation inputs for chaotic evolution.

### External Control

Feed LFO rate from S&H to create meta-modulation:

```
[S&H 2] ──▶ [LFO 1 Rate CV]
```

## Tips for Good Generative Patches

1. **Pentatonic scales**: Can't sound "wrong"
2. **Slow tempos**: Creates space
3. **Long envelopes**: Soft dynamics
4. **Heavy effects**: Blurs harsh edges
5. **Multiple time scales**: Creates depth
6. **Constraint + randomness**: Structure with surprise

## Troubleshooting

**Too static**: Add more modulation, especially to filter

**Too chaotic**: Reduce S&H influence, simplify sequence

**Too quiet**: Check VCA levels, reduce reverb mix

**Notes don't fade**: Increase envelope release, check gate length

**Too busy**: Slow down clock, add more rests in sequence

## What You've Learned

- Building self-playing patches
- Using clocks and sequencers for automation
- Adding controlled randomness
- Layering multiple time scales
- Creating space with effects
