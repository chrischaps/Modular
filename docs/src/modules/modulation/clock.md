# Clock

**Module ID**: `mod.clock`
**Category**: Modulation
**Header Color**: Orange

![Clock Module](../../images/module-clock.png)
*The Clock module*

## Description

The Clock module generates rhythmic pulse signals at a specified tempo. It's the heartbeat of sequenced and rhythmic patches, providing timing signals to sequencers, envelopes, sample & hold circuits, and any module that needs regular triggers.

The clock outputs multiple synchronized divisions of the main tempo, allowing complex polyrhythmic patterns from a single clock source.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Ext Clock** | Gate (Green) | External clock input. When connected, overrides internal tempo |
| **Reset** | Gate (Green) | Reset all divisions to beat 1 on rising edge |
| **Run** | Gate (Green) | Gate high = running, gate low = stopped |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **1/1** | Gate (Green) | Whole note (1 pulse per bar in 4/4) |
| **1/2** | Gate (Green) | Half note (2 pulses per bar) |
| **1/4** | Gate (Green) | Quarter note (4 pulses per bar, main beat) |
| **1/8** | Gate (Green) | Eighth note (8 pulses per bar) |
| **1/16** | Gate (Green) | Sixteenth note (16 pulses per bar) |
| **1/32** | Gate (Green) | Thirty-second note (32 pulses per bar) |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **BPM** | 20 - 300 | 120 | Tempo in beats per minute |
| **Swing** | 0% - 75% | 0% | Swing amount for odd-numbered pulses |
| **Pulse Width** | 1% - 99% | 50% | Gate duration as percentage of beat |
| **Run** | On/Off | On | Start/stop the clock |

## Understanding Divisions

Clock divisions relate to musical note values:

| Output | Name | Pulses per Bar (4/4) | Use Case |
|--------|------|----------------------|----------|
| 1/1 | Whole | 1 | Downbeat, once per bar |
| 1/2 | Half | 2 | Half-time feel |
| 1/4 | Quarter | 4 | Main beat, typical tempo |
| 1/8 | Eighth | 8 | Double-time, hi-hat patterns |
| 1/16 | Sixteenth | 16 | Fast sequencing, rolls |
| 1/32 | Thirty-second | 32 | Very fast, trills |

At 120 BPM:
- 1/4 note = 2 pulses per second (500ms apart)
- 1/8 note = 4 pulses per second (250ms apart)
- 1/16 note = 8 pulses per second (125ms apart)

## Usage Tips

### Basic Sequencer Clocking

Drive a sequencer at eighth-note speed:

```
[Clock 1/8] ──> [Sequencer Clock In]
```

### Multiple Rhythmic Elements

Use different divisions for different parts:

```
[Clock 1/4] ──> [Kick Envelope Gate]
[Clock 1/8] ──> [Hi-Hat Envelope Gate]
[Clock 1/16] ──> [Sequencer Clock]
```

### Adding Swing

Swing shifts every other pulse slightly late, creating a "groove" feel:

- **0%**: Straight, mechanical timing
- **25%**: Light swing, subtle groove
- **50%**: Medium swing, jazzy feel
- **67%**: Heavy swing, triplet-like
- **75%**: Maximum swing, very loose

Swing is applied to the even-numbered pulses of each division.

### Syncing LFOs

Reset LFO phase on each beat:

```
[Clock 1/4] ──> [LFO Sync]
```

This ensures the LFO always starts at the same point on each beat.

### Reset for Song Start

Use reset to synchronize everything:

```
[Start Button] ──> [Clock Reset]
                   [Clock Run]
```

Reset brings all divisions back to beat 1, ensuring everything starts together.

### External Clock Sync

Sync to external gear or DAW:

```
[MIDI Clock In] ──> [Clock Ext Clock]
```

When Ext Clock is connected, the internal BPM is ignored and the clock follows the external tempo.

### Creating Polyrhythms

Combine divisions for polyrhythmic patterns:

```
[Clock 1/4] ──> [Sequencer A Clock] (4 steps)
[Clock 1/8] ──> [Sequencer B Clock] (6 steps)
```

The different cycle lengths create evolving patterns.

### Gate Length (Pulse Width)

Pulse Width controls how long each gate stays high:

- **Short (10-25%)**: Staccato, percussive triggers
- **Medium (50%)**: Standard gate length
- **Long (75-99%)**: Legato, overlapping notes

```
[Clock] (Pulse Width: 75%) ──> [ADSR Gate]
```

Longer gates give envelopes more time in the sustain phase.

### Run/Stop Control

Use the Run input or button to start/stop:

```
[Toggle Button] ──> [Clock Run]
```

When stopped, clock outputs go low. When restarted, timing resumes (use Reset for consistent restart position).

## Building Patterns

### 4-on-the-Floor
```
[Clock 1/4] ──> [Kick Trigger]
```

### Basic Rock Beat
```
[Clock 1/4] ──> [Kick] (beats 1, 3)
[Clock 1/4] ──> [Snare] (beats 2, 4 - offset)
[Clock 1/8] ──> [Hi-Hat]
```

### Driving Sequence
```
[Clock 1/16] ──> [Sequencer Clock]
[Clock 1/1] ──> [Sequencer Reset]
```

### Ambient Pulses
```
[Clock 1/2] ──> [Envelope Gate]
(BPM: 40-60)
```

## Connection Examples

### Complete Rhythm Section
```
[Clock] ──1/4──> [Kick ADSR]
        ──1/8──> [Sequencer] ──> [Bass Oscillator]
        ──1/16──> [Hi-Hat ADSR]
        ──1/1──> [Sequencer Reset]
```

### Synced Modulation
```
[Clock] ──1/4──> [LFO Sync]
        ──1/8──> [Sample & Hold Trigger]
```

### Polymetric Setup
```
[Clock 1/8] ──> [Sequencer A (8 steps)]
            ──> [Sequencer B (6 steps)]
            ──> [Sequencer C (5 steps)]
```

## Tips for Tight Timing

1. **Use Reset**: Always reset when starting to ensure all modules are synchronized
2. **Match Pulse Widths**: If modules expect specific gate lengths, adjust Pulse Width
3. **Consider Latency**: Audio processing has some latency; very fast divisions may drift
4. **External Sync**: For critical timing with other gear, use external clock from your DAW

## Related Modules

- [Sequencer](../utilities/sequencer.md) - Primary clock destination
- [ADSR Envelope](./adsr.md) - Gate inputs for rhythmic envelopes
- [LFO](./lfo.md) - Sync input for tempo-locked modulation
- [Sample & Hold](../utilities/sample-hold.md) - Clock-triggered sampling
