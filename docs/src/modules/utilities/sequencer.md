# Sequencer

**Module ID**: `util.sequencer`
**Category**: Utilities
**Header Color**: Yellow

![Sequencer Module](../../images/module-sequencer.png)
*The Sequencer module*

## Description

The 16-Step Sequencer generates programmable CV and gate patterns that cycle through a sequence of values. It's the heart of pattern-based music, outputting melodies, rhythms, and modulation sequences.

Each step can have:
- A CV value (for pitch, modulation, etc.)
- A gate on/off state (for triggering)
- Adjustable length (1-16 active steps)

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Clock** | Gate (Green) | Advances to next step on rising edge |
| **Reset** | Gate (Green) | Returns to step 1 on rising edge |
| **Run** | Gate (Green) | Gate high = running, gate low = paused |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **CV** | Control (Orange) | Current step's CV value |
| **Gate** | Gate (Green) | Current step's gate state |
| **EOC** | Gate (Green) | End of Cycle pulse when sequence restarts |

## Parameters

| Control | Range | Description |
|---------|-------|-------------|
| **Step 1-16 CV** | 0.0 - 1.0 | CV value for each step (displayed as knobs or sliders) |
| **Step 1-16 Gate** | On/Off | Gate state for each step (toggles) |
| **Length** | 1 - 16 | Number of active steps |
| **Direction** | Forward/Backward/Pendulum/Random | Playback direction |

## How It Works

1. Clock input advances to the next step
2. CV output immediately changes to new step's value
3. Gate output goes high if step's gate is on
4. Gate goes low before next clock (based on gate length)
5. At end of sequence (length reached), EOC pulses and sequence restarts

## Usage Tips

### Basic Melody Sequencing

Create a simple melodic pattern:

```
[Clock 1/8] ──> [Sequencer Clock]
[Sequencer CV] ──> [Oscillator V/Oct]
[Sequencer Gate] ──> [ADSR Gate]
```

1. Set step CV values for your melody
2. Toggle gates on for notes, off for rests
3. Adjust length for pattern size

### Programming Pitches

CV values map to pitch:
- 0.0 = Base note
- 0.083 = +1 semitone
- 0.167 = +2 semitones
- 0.5 = +6 semitones (tritone)
- 1.0 = +1 octave

For a C major scale pattern:
| Step | CV | Note |
|------|-----|------|
| 1 | 0.000 | C |
| 2 | 0.167 | D |
| 3 | 0.333 | E |
| 4 | 0.417 | F |
| 5 | 0.583 | G |
| 6 | 0.750 | A |
| 7 | 0.917 | B |
| 8 | 1.000 | C (octave) |

### Rhythmic Patterns

Use gates for rhythm:

```
Step:  1  2  3  4  5  6  7  8
Gate:  ●  ○  ●  ○  ●  ●  ○  ●
```
(● = on, ○ = off)

This creates a syncopated pattern.

### Modulation Sequences

Use CV output for parameter modulation:

```
[Sequencer CV] ──> [Filter Cutoff CV]
```

Each step changes the filter cutoff, creating rhythmic timbral variation.

### Direction Modes

**Forward**: 1 → 2 → 3 → ... → 16 → 1 → ...

**Backward**: 16 → 15 → 14 → ... → 1 → 16 → ...

**Pendulum**: 1 → 2 → ... → 16 → 15 → ... → 1 → ...

**Random**: Jumps to random step each clock

### Using Reset

Sync sequences to song sections:

```
[Master Clock 1/1 (bar)] ──> [Sequencer Reset]
[Master Clock 1/16] ──> [Sequencer Clock]
```

Sequence resets every bar, keeping it locked to the downbeat.

### EOC for Chaining

Use End of Cycle to trigger events:

```
[Sequencer A EOC] ──> [Sequencer B Reset]
```

Sequencer B resets when A completes a cycle.

### Variable Length Patterns

Set length shorter than 16 for odd meters:

- Length 3: Creates waltz/triplet feel
- Length 5: Creates 5/8 time
- Length 7: Creates 7/8 time
- Length 12: Swing/shuffle patterns

### Polyrhythms

Run two sequencers at the same clock with different lengths:

```
[Clock] ──> [Seq A (Length: 4)] ──> [Osc 1]
        ──> [Seq B (Length: 3)] ──> [Osc 2]
```

4 against 3 creates evolving polyrhythmic patterns.

### Ratcheting

Clock the sequencer faster for a step by using clock multiplication or additional triggers.

### CV and Gate Independence

CV and Gate don't have to move together:

```
[Fast Clock] ──> [Sequencer Clock] ──> [CV to Modulation]
[Slow Clock] ──> [Envelope Gate] (separate rhythm)
```

## Building Sequences

### Bassline (8 steps)
| Step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|------|---|---|---|---|---|---|---|---|
| CV | C | C | G | G | F | F | G | G |
| Gate | ● | ○ | ● | ○ | ● | ○ | ● | ● |

### Arpeggio (4 steps)
| Step | 1 | 2 | 3 | 4 |
|------|---|---|---|---|
| CV | C | E | G | E |
| Gate | ● | ● | ● | ● |

### Filter Sequence (8 steps)
| Step | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|------|---|---|---|---|---|---|---|---|
| CV | 0.2 | 0.5 | 0.8 | 0.5 | 0.3 | 0.6 | 0.9 | 0.4 |

## Connection Examples

### Complete Bass Voice
```
[Clock 1/8] ──> [Sequencer Clock]
[Clock 1/1] ──> [Sequencer Reset]
[Sequencer CV] ──> [Oscillator V/Oct]
[Sequencer Gate] ──> [ADSR Gate]
[Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
```

### Polymetric Setup
```
[Clock] ──> [Seq A (7 steps)] ──> [Voice 1]
        ──> [Seq B (5 steps)] ──> [Voice 2]
```

### Modulation Sequencing
```
[Slow Clock] ──> [Sequencer]
[Seq CV] ──> [Attenuverter] ──> [Filter Cutoff]
[Seq CV] ──> [Attenuverter] ──> [Resonance]
```

## Tips

1. **Start simple**: Begin with 4-step patterns and expand
2. **Use rests**: Gates off create space in the rhythm
3. **Vary length**: Odd lengths create interesting cycles
4. **Reset strategically**: Keep sequences locked to musical sections
5. **Layer sequences**: Multiple sequences at different rates create complexity

## Related Modules

- [Clock](../modulation/clock.md) - Timing source for sequencer
- [Oscillator](../sources/oscillator.md) - CV destination for melody
- [ADSR Envelope](../modulation/adsr.md) - Gate destination for note shaping
- [Sample & Hold](./sample-hold.md) - Alternative for random sequences
