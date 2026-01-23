# FM Synthesis

Create bell-like tones, metallic sounds, and complex timbres using frequency modulation.

![FM Synthesis Patch](../images/recipe-fm-synthesis.png)
*The FM synthesis patch*

## Overview

FM (Frequency Modulation) synthesis uses one oscillator (modulator) to modulate the frequency of another (carrier). This creates complex harmonic and inharmonic spectra without traditional filtering, producing distinctive metallic, bell-like, and electric piano tones.

**Character**: Bright, metallic, bell-like, electric
**Good for**: Bells, electric piano, brass, bass, experimental

## Modules Used

- 2x [Oscillator](../modules/sources/oscillator.md) (Carrier + Modulator)
- 1x [VCA](../modules/utilities/vca.md)
- 2x [ADSR Envelope](../modules/modulation/adsr.md)
- 1x [Keyboard Input](../modules/midi/keyboard.md)
- 1x [Audio Output](../modules/output/audio-output.md)

Optional:
- 1x [Attenuverter](../modules/utilities/attenuverter.md)

## Patch Diagram

```
┌──────────┐     ┌───────────────┐      ┌──────────────┐      ┌─────┐
│ Keyboard │─V/Oct─▶ Modulator   │─Audio─▶   Carrier    │─Audio─▶ VCA │─▶ Output
│          │     │  Oscillator  │  FM   │  Oscillator  │      │     │
└────┬─────┘     │    (Sine)    │       │    (Sine)    │      └──▲──┘
     │           └──────▲───────┘       └──────▲───────┘         │
     │                  │                      │                  │
     │ V/Oct            │                      │ V/Oct            │
     ├──────────────────┤                      │                  │
     │                  │                      │                  │
     │ Gate        ┌────┴─────┐                │           ┌──────┴──────┐
     │             │ ADSR 1   │────────────────┘           │   ADSR 2    │
     └────────────▶│(FM Depth)│                            │ (Amplitude) │
               │   └──────────┘                            └─────────────┘
               │                                                  ▲
               └──────────────────────────────────────────────────┘
```

## Key Concept: Carrier and Modulator

**Carrier**: The oscillator you hear. Its frequency determines the pitch.

**Modulator**: Modulates the carrier's frequency. Its frequency determines the timbre complexity.

**FM Amount**: How much the modulator affects the carrier. More = brighter, more complex.

### Frequency Ratios

The ratio between modulator and carrier frequencies determines the harmonic content:

| Ratio (M:C) | Result |
|-------------|--------|
| 1:1 | Simple, adds harmonics |
| 2:1 | Octave harmonics |
| 3:1 | Fifth harmonics |
| 1.41:1 | Inharmonic, bell-like |
| 3.5:1 | Very inharmonic, metallic |

Integer ratios = harmonic sounds
Non-integer ratios = inharmonic/metallic sounds

## Step-by-Step Setup

### 1. Add Modules

Position:
1. **Keyboard Input** (left)
2. **Modulator Oscillator** (center-left)
3. **Carrier Oscillator** (center)
4. **VCA** (center-right)
5. **Audio Output** (right)
6. **Two ADSRs** (below)

### 2. Connect Pitch Control

Both oscillators track the keyboard:

```
[Keyboard V/Oct] ──▶ [Modulator V/Oct]
                 ──▶ [Carrier V/Oct]
```

### 3. Create FM Connection

The modulator modulates the carrier:

```
[Modulator Audio Out] ──▶ [Carrier FM Input]
```

### 4. Audio Output Path

```
[Carrier Audio Out] ──▶ [VCA Input]
[VCA Output] ──▶ [Audio Output Mono]
```

### 5. Amplitude Envelope

```
[Keyboard Gate] ──▶ [ADSR 2 Gate]
[ADSR 2 Env] ──▶ [VCA CV]
```

### 6. FM Depth Envelope (Optional but Recommended)

The FM amount can be envelope-controlled for dynamic timbre:

```
[Keyboard Gate] ──▶ [ADSR 1 Gate]
[ADSR 1 Env] ──▶ [Carrier FM Amount CV]
```

Or route modulator through a VCA:

```
[Modulator] ──▶ [VCA 2] ──▶ [Carrier FM]
[ADSR 1] ──▶ [VCA 2 CV]
```

## Settings for FM Bell

Classic bell sound:

**Carrier Oscillator**:
- Waveform: **Sine**
- Frequency: Keyboard controlled

**Modulator Oscillator**:
- Waveform: **Sine**
- Frequency: Keyboard controlled
- (Set slightly higher for inharmonic: multiply frequency by 1.41)

**Carrier FM Amount**: 0.3 - 0.5

**ADSR 2 (Amplitude)**:
| Parameter | Value |
|-----------|-------|
| Attack | 1 ms |
| Decay | 2000 ms |
| Sustain | 0.0 |
| Release | 1000 ms |

**ADSR 1 (FM Depth)**:
| Parameter | Value |
|-----------|-------|
| Attack | 1 ms |
| Decay | 500 ms |
| Sustain | 0.1 |
| Release | 500 ms |

The FM depth envelope makes the sound start bright and become pure as it decays.

## Variations

### Electric Piano (DX7-style)

```
Ratio: 1:1
Modulator: Sine
Carrier: Sine
FM Amount: 0.3
ADSR 2: A:1ms D:800ms S:0.4 R:300ms
ADSR 1: A:1ms D:200ms S:0.2 R:200ms
```

### Tubular Bells

```
Ratio: 3.5:1 (inharmonic)
FM Amount: 0.6
ADSR 2: A:1ms D:3000ms S:0.0 R:2000ms
ADSR 1: A:1ms D:1000ms S:0.05 R:500ms
```

### FM Bass

```
Ratio: 1:1
FM Amount: 0.4
Modulator: Sine (or saw for grit)
ADSR 2: A:1ms D:200ms S:0.6 R:100ms
ADSR 1: A:1ms D:100ms S:0.3 R:50ms
```

### Brass-like

```
Ratio: 1:1
FM Amount: 0.5
ADSR 2: A:50ms D:100ms S:0.8 R:200ms
ADSR 1: A:30ms D:200ms S:0.5 R:200ms
```

### Harsh Digital

```
Ratio: 7:3 (complex)
Modulator: Saw
Carrier: Sine
FM Amount: 0.8
```

## Advanced Techniques

### Velocity-Controlled FM

Harder playing = brighter:

```
[Keyboard Velocity] ──▶ [Attenuverter] ──▶ [FM Amount CV]
```

### Multiple Modulators

Add complexity:

```
[Modulator 1] ──▶ [Mixer] ──▶ [Carrier FM]
[Modulator 2] ──▶ [Mixer]
```

### Feedback FM

Route carrier back to modulate itself:

```
[Carrier Out] ──▶ [Attenuverter (very low)] ──▶ [Carrier FM]
```

Creates chaotic, noisy tones. Use sparingly!

### Filter After FM

Add subtractive element:

```
[Carrier] ──▶ [Filter] ──▶ [VCA] ──▶ [Output]
```

## Tuning the Ratio

To set specific frequency ratios:

1. Set both oscillators to same base frequency
2. Multiply modulator frequency by desired ratio
3. Or use V/Oct input with offset

For a 2:1 ratio:
- Carrier at keyboard pitch
- Modulator at keyboard pitch +1 octave

For 3:2 ratio:
- Carrier at keyboard pitch
- Modulator at keyboard pitch +7 semitones (perfect fifth)

## Troubleshooting

**Too harsh**: Lower FM amount, shorten FM envelope decay

**Too pure**: Increase FM amount, longer FM envelope

**Out of tune**: Ensure both oscillators track keyboard

**No change with FM amount**: Check FM connection to carrier

## What You've Learned

- Basic FM synthesis architecture
- Relationship between frequency ratios and timbre
- Using envelopes to control FM depth dynamically
- Creating classic FM sounds like bells and electric piano
