# Sample & Hold

**Module ID**: `util.samplehold`
**Category**: Utilities
**Header Color**: Yellow

![Sample & Hold Module](../../images/module-sample-hold.png)
*The Sample & Hold module*

## Description

The Sample & Hold (S&H) module captures the instantaneous value of an input signal when triggered, then holds that value constant until the next trigger. This creates stepped, staircase-like outputs from continuous signals.

**Classic uses:**
- Random pitched sequences from noise
- Stepped modulation from LFOs
- Quantized parameter changes
- Creating rhythmic variation

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Any (matches input) | Signal to be sampled |
| **Trigger** | Gate (Green) | Rising edge captures the input value |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Any (matches input) | Held value from last sample |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Slew** | 0 ms - 500 ms | 0 ms | Smoothing time between held values |

## How It Works

1. Input signal changes continuously
2. Trigger receives a rising edge (gate goes from 0 to 1)
3. Output instantly captures input value at that moment
4. Output holds steady until next trigger
5. Repeat

```
Input:   ~~~∿∿~~~∿∿~~~  (continuous signal)
Trigger: _|‾|__|‾|__|‾   (pulses)
Output:  ____‾‾‾‾____‾‾  (stepped values)
```

## Usage Tips

### Random Notes from Noise

The classic S&H patch—random melodies:

```
[Noise] ──> [S&H Input]
[Clock] ──> [S&H Trigger]
[S&H Output] ──> [Oscillator V/Oct]
```

Each clock pulse picks a random voltage from the noise, creating random pitches.

### Stepped LFO

Turn a smooth LFO into stepped modulation:

```
[LFO (Sine)] ──> [S&H Input]
[Clock (faster)] ──> [S&H Trigger]
[S&H Output] ──> [Filter Cutoff]
```

The filter cutoff moves in discrete steps rather than smoothly.

### Rhythmic Variations

Sample a slow LFO at regular intervals:

```
[LFO (very slow)] ──> [S&H Input]
[Clock 1/4] ──> [S&H Trigger]
[S&H Output] ──> [Parameter]
```

Each beat has a different (but related) modulation value.

### Track and Hold

Sample a melodic sequence to create variations:

```
[Sequencer CV] ──> [S&H Input]
[Random Trigger] ──> [S&H Trigger]
[S&H Output] ──> [Another Oscillator V/Oct]
```

The second oscillator plays held notes from the sequence.

### Slew for Portamento

Use the Slew parameter to smooth transitions:

```
[Noise] ──> [S&H] ──> [Osc V/Oct]
             (Slew: 100ms)
```

Instead of instant pitch jumps, notes glide to each new value.

### Quantized Random

For random notes that stay in key, add a quantizer after S&H:

```
[Noise] ──> [S&H] ──> [Quantizer] ──> [Osc V/Oct]
```

(Requires a quantizer module)

### Stutter Effect

Sample audio at regular intervals:

```
[Audio] ──> [S&H Input]
[Fast Clock] ──> [S&H Trigger]
[S&H Output] ──> [Output]
```

Creates a bit-crusher/sample-rate-reduction effect.

### Probability-Based Changes

Use a randomly triggered S&H:

```
[Random Gate (probability)] ──> [S&H Trigger]
[Parameter Source] ──> [S&H Input]
```

The parameter only changes when the random gate fires.

### Self-Patched Chaos

Feed S&H output back with modification:

```
[S&H Output] ──> [Attenuverter] ──> [S&H Input]
[Clock] ──> [S&H Trigger]
```

Creates chaotic, evolving patterns. Adjust attenuverter for different behaviors.

## Slew Control

The Slew parameter adds portamento between held values:

| Slew | Effect |
|------|--------|
| 0 ms | Instant jumps (classic S&H) |
| 10-50 ms | Subtle smoothing |
| 100-200 ms | Noticeable glide |
| 300+ ms | Long slides between values |

With high slew values, the output may not reach the held value before the next trigger—creating even smoother movement.

## Connection Examples

### Random Arpeggio
```
[Noise] ──> [S&H] ──> [Quantizer] ──> [Osc V/Oct]
[Clock 1/16] ──> [S&H Trigger]
```

### Generative Modulation
```
[LFO 1 (slow)] ──> [S&H] ──> [Filter Cutoff CV]
[LFO 2 (fast)] ──> [S&H Trigger]
```

### Held Sequence Notes
```
[Sequencer] ──> [S&H] ──> [Second Voice]
[Random Gate] ──> [S&H Trigger]
```

### Bit Crusher
```
[Audio] ──> [S&H] ──> [Output]
[Clock (very fast)] ──> [S&H Trigger]
```

## Sample Sources

Different inputs create different results:

| Source | Result |
|--------|--------|
| **Noise** | Random, unpredictable values |
| **LFO** | Stepped, cyclic pattern |
| **Envelope** | Held modulation snapshots |
| **Audio** | Crushed/reduced sample rate |
| **Sequencer** | Held sequence steps |

## Related Modules

- [Clock](../modulation/clock.md) - Trigger source for S&H
- [LFO](../modulation/lfo.md) - Input source for stepped modulation
- [Sequencer](./sequencer.md) - Alternative way to create stepped sequences
- [Attenuverter](./attenuverter.md) - Scale S&H output
