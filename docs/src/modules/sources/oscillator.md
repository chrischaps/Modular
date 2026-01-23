# Oscillator

**Module ID**: `osc.sine`
**Category**: Sources
**Header Color**: Blue

![Oscillator Module](../../images/module-oscillator.png)
*The Oscillator module*

## Description

The Oscillator is the primary sound source in Modular Synth. It generates periodic waveforms at audio frequencies, producing the raw tones that can be shaped by filters, envelopes, and effects.

This is a **VCO** (Voltage Controlled Oscillator), meaning its frequency can be controlled by external signals, enabling keyboard tracking and vibrato effects.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **V/Oct** | Control (Orange) | 1V/octave pitch control. Each 1.0 increase raises pitch by one octave |
| **FM** | Audio/Control (Blue/Orange) | Frequency modulation input. Modulates pitch at the rate of incoming signal |
| **PWM** | Control (Orange) | Pulse width modulation for square wave. Controls duty cycle |
| **Sync** | Gate (Green) | Hard sync input. Resets waveform phase on rising edge |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Audio Out** | Audio (Blue) | Main audio output with selected waveform |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Waveform** | Sine/Saw/Square/Triangle | Sine | Selects the output waveform shape |
| **Frequency** | 20 Hz - 20 kHz | 440 Hz | Base frequency when no V/Oct is connected |
| **Detune** | -100 to +100 cents | 0 | Fine tune adjustment in cents (1/100th of a semitone) |
| **FM Amount** | 0.0 - 1.0 | 0.0 | Depth of frequency modulation from FM input |
| **PW** | 0.1 - 0.9 | 0.5 | Pulse width for square wave (0.5 = 50% duty cycle) |

## Waveforms

### Sine
![Sine Wave](../../images/waveform-sine.png)

Pure tone with no harmonics. Useful for:
- Sub-bass
- FM synthesis carriers
- Pure, flute-like tones
- Smooth modulation sources

### Sawtooth (Saw)
![Saw Wave](../../images/waveform-saw.png)

Contains all harmonics with decreasing amplitude. Useful for:
- Classic synth leads and basses
- Strings and brass sounds
- Rich source material for filtering

### Square
![Square Wave](../../images/waveform-square.png)

Contains odd harmonics only. Useful for:
- Hollow, woody tones
- Clarinet-like sounds
- PWM for chorus-like effects

### Triangle
![Triangle Wave](../../images/waveform-triangle.png)

Contains odd harmonics with rapid rolloff. Useful for:
- Softer than square, brighter than sine
- Flute and soft lead sounds
- Smooth modulation

## Usage Tips

### Basic Pitch Control

Connect a keyboard or MIDI module's **V/Oct** output to control pitch:

```
[Keyboard] ──V/Oct──> [Oscillator]
```

The V/Oct standard means:
- 0.0 = Base frequency (set by Frequency knob)
- 1.0 = One octave up
- -1.0 = One octave down
- 0.083 = One semitone up (1/12 of an octave)

### Vibrato with LFO

Connect an LFO to the FM input for vibrato:

```
[LFO] ──> [Oscillator FM]
```

- Use low FM Amount (0.1-0.2) for subtle vibrato
- Higher amounts create more dramatic pitch wobble
- LFO rate of 5-7 Hz is typical for vibrato

### FM Synthesis

Connect another oscillator to FM input for FM synthesis:

```
[Oscillator 2] ──> [Oscillator 1 FM]
```

- Sine waves work best for clean FM tones
- Integer frequency ratios (2:1, 3:1) create harmonic sounds
- Non-integer ratios create inharmonic, bell-like tones
- FM Amount controls brightness/complexity

### Pulse Width Modulation

For square wave, modulate pulse width with an LFO:

```
[LFO] ──> [Oscillator PWM]
```

- Creates a chorus-like, animated effect
- Extreme PW values (near 0.1 or 0.9) create thin, nasal tones
- Slow LFO rates create subtle movement
- Fast rates create more dramatic timbral changes

### Hard Sync

Connect a sync signal to reset the waveform cycle:

```
[Oscillator 2 Out] ──> [Oscillator 1 Sync]
```

Hard sync creates complex, aggressive tones:
- Oscillator 1 (synced) resets when Oscillator 2 completes a cycle
- Sweep Oscillator 1's frequency for the classic sync sweep sound
- Works best when synced osc is higher frequency than master

### Detuning for Thickness

Use two oscillators slightly detuned for a thicker sound:

```
[Osc 1 (Detune: 0)] ──┐
                      ├──> [Mixer] ──> [Filter]
[Osc 2 (Detune: +7)] ─┘
```

- Small detune values (5-15 cents) create subtle chorusing
- Larger values (20-50 cents) create a "supersaw" effect

## Connection Examples

### Typical Synthesis Chain
```
[Keyboard] ──V/Oct──> [Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
```

### FM Bell Patch
```
[Oscillator 1] ──FM──> [Oscillator 2] ──> [VCA] ──> [Output]
     (Modulator)           (Carrier)
```

### Dual Oscillator with PWM
```
[LFO] ──> [Osc 1 PWM]
          [Osc 1] ──┐
                    ├──> [Mixer] ──> [Filter]
          [Osc 2] ──┘
```

## Related Modules

- [LFO](../modulation/lfo.md) - For vibrato and PWM modulation
- [SVF Filter](../filters/svf-filter.md) - Shape the oscillator's harmonics
- [VCA](../utilities/vca.md) - Control oscillator volume
- [ADSR Envelope](../modulation/adsr.md) - Shape the sound over time
