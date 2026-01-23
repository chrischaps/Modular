# Rhythmic Sequence

Create driving, rhythmic synthesizer patterns with sequencers and clock divisions.

![Rhythmic Sequence Patch](../images/recipe-rhythmic-sequence.png)
*The rhythmic sequence patch*

## Overview

This patch creates a complete rhythmic synthesizer pattern with a driving bassline, sequenced filter movement, and rhythmic interest. It demonstrates how to build compelling electronic music patterns from scratch.

**Character**: Driving, hypnotic, rhythmic, energetic
**Good for**: Techno, house, electro, acid, dance music

## Modules Used

- 1x [Clock](../modules/modulation/clock.md)
- 2x [Sequencer](../modules/utilities/sequencer.md)
- 1x [Oscillator](../modules/sources/oscillator.md)
- 1x [SVF Filter](../modules/filters/svf-filter.md)
- 1x [VCA](../modules/utilities/vca.md)
- 1x [ADSR Envelope](../modules/modulation/adsr.md)
- 1x [Distortion](../modules/effects/distortion.md)
- 1x [Delay](../modules/effects/delay.md)
- 1x [Audio Output](../modules/output/audio-output.md)

Optional:
- 1x [LFO](../modules/modulation/lfo.md)
- 1x [Compressor](../modules/effects/compressor.md)

## Patch Diagram

```
┌───────┐     ┌────────────┐     ┌─────────┐     ┌────────┐     ┌──────┐
│ Clock │──▶  │ Sequencer 1│─CV──▶│Oscillator│─Audio─▶│ Filter │─Audio─▶│ VCA  │
│(120BPM)│     │  (Pitch)   │     │  (Saw)  │      │ (SVF)  │      │      │
└───┬───┘     └─────┬──────┘     └─────────┘      └───▲────┘      └──▲───┘
    │               │                                  │             │
    │1/16           │Gate                              │             │
    │               ▼                                  │             │
    │         ┌─────────┐                              │      ┌──────┴──────┐
    │         │  ADSR   │──────────────────────────────┘      │    ADSR     │
    │         │(Filter) │                                     │ (Amplitude) │
    │         └────▲────┘                                     └──────▲──────┘
    │              │                                                  │
    │              │                                                  │
    │    ┌─────────┴───────────────────────────────────────┐         │
    └───▶│                   [Sequencer Gate]               │─────────┘
         └─────────────────────────────────────────────────┘

[VCA]──▶[Distortion]──▶[Delay]──▶[Output]
```

## Step-by-Step Setup

### 1. Clock - The Foundation

Set up the master clock:

**Clock Settings**:
| Parameter | Value |
|-----------|-------|
| BPM | 120 |
| Pulse Width | 50% |

### 2. Pitch Sequencer

Create a 16-step pattern:

```
[Clock 1/16] ──▶ [Sequencer 1 Clock]
```

**Sequencer 1 Settings**:
- Length: **16 steps**
- Direction: **Forward**

Example acid bassline pattern (1 octave range):

| Step | Note | CV | Gate |
|------|------|-----|------|
| 1 | C2 | 0.00 | ON |
| 2 | C2 | 0.00 | ON |
| 3 | - | - | OFF |
| 4 | G2 | 0.58 | ON |
| 5 | C2 | 0.00 | ON |
| 6 | - | - | OFF |
| 7 | Eb2 | 0.25 | ON |
| 8 | C2 | 0.00 | ON |
| 9 | C3 | 1.00 | ON |
| 10 | - | - | OFF |
| 11 | G2 | 0.58 | ON |
| 12 | C2 | 0.00 | ON |
| 13 | - | - | OFF |
| 14 | Eb2 | 0.25 | ON |
| 15 | F2 | 0.42 | ON |
| 16 | G2 | 0.58 | ON |

### 3. Oscillator

```
[Sequencer 1 CV] ──▶ [Oscillator V/Oct]
```

**Oscillator Settings**:
- Waveform: **Saw** (classic acid sound)
- Base Frequency: C2 (or lower for bass)

### 4. Envelopes

Connect gate to both envelopes:

```
[Sequencer 1 Gate] ──▶ [ADSR 1 Gate] (Filter)
                   ──▶ [ADSR 2 Gate] (Amplitude)
```

**ADSR 1 (Filter)**:
| Parameter | Value | Why |
|-----------|-------|-----|
| Attack | 1 ms | Instant |
| Decay | 200 ms | Quick close |
| Sustain | 0.1 | Mostly closed |
| Release | 50 ms | Quick |

**ADSR 2 (Amplitude)**:
| Parameter | Value | Why |
|-----------|-------|-----|
| Attack | 1 ms | Punchy |
| Decay | 150 ms | Short |
| Sustain | 0.3 | Some body |
| Release | 50 ms | Tight |

### 5. Filter - The Acid Sound

```
[Oscillator Audio] ──▶ [Filter Input]
[ADSR 1 Env] ──▶ [Filter Cutoff CV]
```

**Filter Settings**:
| Parameter | Value |
|-----------|-------|
| Cutoff | 400 Hz (base) |
| Resonance | 0.7 (high!) |
| CV Amount | 0.6 |

The high resonance creates the classic "acid" squelch.

### 6. VCA

```
[Filter Lowpass] ──▶ [VCA Input]
[ADSR 2 Env] ──▶ [VCA CV]
```

### 7. Effects Chain

```
[VCA] ──▶ [Distortion] ──▶ [Delay] ──▶ [Output]
```

**Distortion Settings**:
| Parameter | Value |
|-----------|-------|
| Type | Soft |
| Drive | 0.3 |
| Mix | 0.7 |

**Delay Settings**:
| Parameter | Value |
|-----------|-------|
| Time | 375 ms (dotted 1/8 at 120 BPM) |
| Feedback | 0.3 |
| Mix | 0.25 |
| HP Filter | 300 Hz |

## Creating Variations

### Accent Pattern

Add accents on certain steps:

**Method 1**: Use higher CV values
**Method 2**: Add second envelope for accents

```
[Sequencer 2 (accent pattern)] ──▶ [Attenuverter] ──▶ [Filter Cutoff CV]
```

### Slide/Glide

Add portamento for 303-style slides:

Between certain notes, enable glide on the oscillator or add a slew limiter.

### Gate Length Variation

Vary gate lengths in the sequencer for rhythmic interest:
- Long gates for legato
- Short gates for staccato
- Tied notes for slides

## Variations

### Classic Acid

```
Resonance: 0.85
Filter Cutoff: 300 Hz
Distortion: 0.5
ADSR 1 Decay: 300 ms
```

### Driving Techno Bass

```
Oscillator: Square
Resonance: 0.3
Distortion: 0.4
BPM: 135
Shorter envelope decays
```

### Hypnotic Minimal

```
BPM: 124
Sequence Length: 4 steps
Resonance: 0.5
Delay: 500ms (1/4 note)
Delay Feedback: 0.5
```

### Electro Funk

```
BPM: 110
Syncopated gate pattern
Moderate resonance: 0.4
Add subtle chorus
Longer envelope attack: 10 ms
```

## Adding Groove

### Swing

Apply swing to the clock:

```
Clock Swing: 30%
```

This pushes every other beat slightly late.

### Velocity/Accent Sequencer

Add a parallel sequencer for dynamics:

```
[Clock 1/16] ──▶ [Sequencer 2 Clock]
[Sequencer 2 CV] ──▶ [VCA Level CV] (via attenuverter)
```

Program accent pattern in Sequencer 2.

### Ghost Notes

Add quiet notes between main beats:

1. Program main pattern with normal gates
2. Add additional quiet steps with very low output
3. Creates more complex rhythm

## Advanced Techniques

### Filter Sequencing

Use a second sequencer for filter cutoff:

```
[Clock 1/4] ──▶ [Sequencer 2 Clock]
[Sequencer 2 CV] ──▶ [Filter Cutoff CV]
```

The filter pattern moves independently of the note pattern.

### Polyrhythms

Set sequencers to different lengths:

```
Sequencer 1 (pitch): 16 steps
Sequencer 2 (filter): 12 steps
```

The patterns shift against each other over time.

### External Modulation

Add LFO for evolving character:

```
[LFO (slow)] ──▶ [Resonance CV]
[LFO (slow)] ──▶ [Envelope Decay CV]
```

## Troubleshooting

**No squelch**: Increase resonance, increase filter envelope depth

**Too harsh**: Lower resonance, reduce distortion

**Notes running together**: Shorten envelope release, check gate lengths

**Too quiet**: Check VCA level, output level

**Timing feels off**: Adjust clock BPM, check gate lengths

## Performance Tips

1. **Tweak the filter cutoff**: Main expressive control
2. **Adjust resonance live**: Changes character dramatically
3. **Play with envelope decay**: Longer = more squelch
4. **Use delay feedback**: Build tension
5. **Mute/unmute sequencer gates**: Create arrangement

## What You've Learned

- Building rhythmic sequenced patterns
- Creating acid bass sounds
- Using filter envelopes for character
- Clock divisions for different rhythmic rates
- Adding groove and variation
