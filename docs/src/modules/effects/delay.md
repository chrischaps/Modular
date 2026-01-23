# Delay

**Module ID**: `fx.delay`
**Category**: Effects
**Header Color**: Purple

![Delay Module](../../images/module-delay.png)
*The Delay module*

## Description

The Stereo Delay creates echoes and rhythmic repetitions by playing back a delayed copy of the input signal. It features independent left and right delay times, feedback for multiple echoes, and filtering to shape the delay character.

Delays are essential for:
- Adding depth and space
- Creating rhythmic patterns
- Doubling and thickening sounds
- Ambient and experimental textures

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left In** | Audio (Blue) | Left channel input |
| **Right In** | Audio (Blue) | Right channel input (normalled to Left) |
| **Time CV** | Control (Orange) | Modulation input for delay time |
| **Feedback CV** | Control (Orange) | Modulation input for feedback amount |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left Out** | Audio (Blue) | Processed left channel |
| **Right Out** | Audio (Blue) | Processed right channel |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Time L** | 1 ms - 2000 ms | 375 ms | Left channel delay time |
| **Time R** | 1 ms - 2000 ms | 500 ms | Right channel delay time |
| **Feedback** | 0.0 - 0.95 | 0.3 | Amount of output fed back to input |
| **Mix** | 0.0 - 1.0 | 0.5 | Dry/wet balance |
| **HP Filter** | 20 Hz - 2000 Hz | 80 Hz | High-pass filter in feedback path |
| **LP Filter** | 200 Hz - 20 kHz | 12 kHz | Low-pass filter in feedback path |
| **Ping Pong** | On/Off | Off | Bounces echoes between L/R channels |
| **Sync** | On/Off | Off | Sync delay time to tempo (when clock connected) |

## How It Works

1. Input signal enters the delay buffer
2. A delayed copy is played back after the set time
3. This output is mixed with dry signal
4. Feedback routes output back to input for multiple echoes
5. Filters shape each repetition

### Feedback Behavior

- **0%**: Single echo, no repetitions
- **30%**: Several echoes, natural decay
- **50%**: Many echoes, sustained
- **70%+**: Long trails, approaching self-oscillation
- **95%**: Near-infinite repeats (careful!)

### Ping Pong Mode

When enabled, echoes alternate between left and right channels:

```
L: Sound → (silence) → Echo → (silence) → Echo...
R: (silence) → Echo → (silence) → Echo → ...
```

Creates wide stereo movement.

## Usage Tips

### Basic Slapback

Short delay for doubling/thickening:

```
Time L: 80 ms
Time R: 100 ms
Feedback: 0
Mix: 0.3
```

Adds thickness without obvious echoes.

### Rhythmic Delay

Sync to tempo for musical echoes:

```
Time L: 375 ms (1/8 note at 120 BPM)
Time R: 750 ms (1/4 note)
Feedback: 0.4
Ping Pong: On
```

Echoes fall on the beat.

### Tape Delay Simulation

Warm, vintage-style delay:

```
Feedback: 0.5
HP Filter: 200 Hz
LP Filter: 4000 Hz
```

The filters remove highs and lows with each repeat, simulating tape degradation.

### Dub Delay

Dark, spacious echoes:

```
Time: Long (600-1000 ms)
Feedback: 0.6
LP Filter: 2000 Hz
```

Low-passed feedback creates dark, dubbed-out echoes.

### Tempo Sync

With a clock connected and Sync enabled, Time knobs select musical divisions:

| Division | At 120 BPM |
|----------|-----------|
| 1/32 | 62.5 ms |
| 1/16 | 125 ms |
| 1/8 | 250 ms |
| 1/4 | 500 ms |
| 1/2 | 1000 ms |
| 1/1 | 2000 ms |

### Dotted Note Delays

For the classic U2/Edge sound, use dotted eighth notes:

```
Time: 1/8 dotted (375 ms at 120 BPM)
Feedback: 0.4
Mix: 0.5
```

### Modulated Delay (Chorus-like)

Apply slow LFO to Time CV:

```
[LFO (0.5 Hz)] ──> [Delay Time CV]
```

The varying delay time creates pitch modulation effects.

### Self-Oscillation

At high feedback (90%+), the delay can self-oscillate:

1. Send a sound through the delay
2. Turn feedback up high
3. Remove input
4. The delay continues generating sound

Use filters to control the character of oscillation.

### Sidechain-Pumping Delay

Modulate feedback with an envelope:

```
[Kick Gate] ──> [Envelope] ──> [Inverted] ──> [Feedback CV]
```

Feedback ducks on each kick, creating pumping echoes.

## Delay Time Reference

| BPM | 1/4 Note | 1/8 Note | 1/8 Dotted | 1/16 Note |
|-----|----------|----------|------------|-----------|
| 100 | 600 ms | 300 ms | 450 ms | 150 ms |
| 120 | 500 ms | 250 ms | 375 ms | 125 ms |
| 140 | 428 ms | 214 ms | 321 ms | 107 ms |
| 160 | 375 ms | 187 ms | 281 ms | 93 ms |

Formula: `60000 / BPM = quarter note in ms`

## Connection Examples

### Standard Insert
```
[Synth] ──> [Delay Left In]
[Delay Left Out] ──> [Output Left]
[Delay Right Out] ──> [Output Right]
```

### Send/Return
```
[Mixer Send] ──> [Delay]
[Delay] ──> [Mixer Return]
(Mix: 100% wet)
```

### Tempo-Synced
```
[Clock] ──> [Delay Sync]
[Synth] ──> [Delay] ──> [Output]
```

### Modulated Delay
```
[LFO] ──> [Delay Time CV]
[Audio] ──> [Delay] ──> [Output]
```

## Sound Design Tips

| Sound | Time | Feedback | Filters |
|-------|------|----------|---------|
| Slapback | 50-100 ms | 0% | Open |
| Clean echo | 250-500 ms | 30% | Open |
| Tape echo | 300-600 ms | 50% | HP:200, LP:4k |
| Dub | 500-1000 ms | 60% | LP:2k |
| Ambient | 700-1500 ms | 70% | HP:100, LP:8k |
| Self-osc | Any | 90%+ | To taste |

## Related Modules

- [Reverb](./reverb.md) - For ambient space
- [Chorus](./chorus.md) - For thickening without echoes
- [Clock](../modulation/clock.md) - For tempo sync
- [LFO](../modulation/lfo.md) - For time modulation
