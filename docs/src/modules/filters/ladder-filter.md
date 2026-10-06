# Ladder Filter

**Module ID**: `filter.ladder`
**Category**: Filters
**Header Color**: Green

## Description

The Ladder Filter is a model of the transistor ladder Robert Moog patented in 1969, the filter behind the Minimoog and still the sound most people mean by "analog". It is a 4-pole lowpass: four one-pole stages in a row, with the last stage's output fed back to the first to make the resonance.

What gives it its character:
- A steep **24 dB/octave** slope, so it darkens a sound decisively, plus a gentler 12 dB/octave tap from the middle of the ladder
- A **saturating stage** at every rung. Push it with Drive and it rounds off and thickens rather than clipping, and the output never goes past full scale, however hard you drive it
- **Bass compensation**. The original ladder loses low end as resonance rises. This one adds the lost bass back, so turning up Resonance adds a peak without thinning the sound
- **Self-oscillation** over the top fifth of the Resonance knob: a sine at the cutoff frequency, in tune to within a cent or two
- **2x oversampling**. The saturators run at twice the sample rate, so a high note driven hard stays clean instead of picking up inharmonic aliasing

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **In** | Audio (Blue) | Audio to be filtered |
| **Cutoff** | Control (Orange) | Cutoff CV, 1 per octave: +1 doubles the cutoff, -1 halves it. The same scale as V/Oct, so a keyboard's pitch tracks directly |
| **Resonance** | Control (Orange) | Adds to the Resonance knob (+1 adds 0.5) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **LP24** | Audio (Blue) | 4-pole lowpass, 24 dB/octave: the classic ladder sound |
| **LP12** | Audio (Blue) | 2-pole lowpass, 12 dB/octave, tapped halfway down the same ladder. Brighter, with the same resonance |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Cutoff** | 20 Hz - 20 kHz | 1000 Hz | Filter cutoff frequency |
| **Resonance** | 0.0 - 1.0 | 0.5 | Emphasis at the cutoff. Self-oscillates above 0.8 |
| **Drive** | 1x - 10x | 1x | Input gain into the saturating stages |

## Ladder vs SVF

Both filters cover the musical range, self-oscillate, and move their cutoff in octaves. They differ in character:

| | Ladder | SVF |
|---|---|---|
| Slope | 24 dB/oct (and 12) | 12 dB/oct |
| Modes | Lowpass only | LP, HP, BP, Notch |
| Saturation | At every stage: thick, rounded | At the input and in the resonance |
| Self-oscillation | Top fifth of the knob, about -20 dBFS | Top few percent, about -12 dBFS |
| Cutoff CV | 1 per octave | 2 octaves per unit |

Reach for the ladder for basses, leads and anything that should sound fat. Reach for the SVF when you want the other modes, or a lighter, more transparent touch.

## Usage Tips

### Basic Filtering

```
[Oscillator (Saw)] ──> [Ladder In]
                       [Ladder LP24] ──> [VCA] ──> [Output]
```

- A saw through LP24 at a few hundred Hz is the classic fat bass
- Switch to LP12 for a brighter, buzzier version of the same sound

### Drive

At Drive 1x a full-scale signal is already warmed slightly. Around 2-4x the ladder thickens audibly and the resonance gets a growl. At 10x it is a distortion in its own right. The level stays roughly constant as you turn Drive up, because the saturating stages cap it.

### Keyboard Tracking

Because the Cutoff input is 1 per octave, a pitch CV patched straight in keeps the filter's brightness the same on every note:

```
[Keyboard] ──Pitch──> [Oscillator V/Oct]
           ──Pitch──> [Ladder Cutoff]
```

With Resonance high enough to self-oscillate and nothing patched into **In**, this turns the ladder into a sine oscillator that plays in tune.

### Self-Oscillation

- Turn Resonance above about 0.8. With nothing patched in, a sine rises out of a tiny noise floor (like the hiss of real components) within a second or so
- The pitch is the Cutoff knob, accurate to a cent or two from bass to treble
- The same saturators that shape the sound hold the oscillation at about -20 dBFS

Feed audio in while it oscillates and the two fight, with the classic squelch.

### Envelope Sweep

```
[Keyboard] ──Gate──> [Envelope] ──> [Ladder Cutoff]
```

The envelope's 0-1 output raises the cutoff by up to one octave. For a wider sweep, scale it up with an Attenuverter or Mixer first.

## Sound Design Tips

| Sound | Cutoff | Resonance | Drive | Output |
|-------|--------|-----------|-------|--------|
| Fat bass | 200-400 Hz | 0.3 | 2x | LP24 |
| Squelchy acid | 400-800 Hz | 0.7 | 3x | LP24 |
| Buzzy lead | 1-3 kHz | 0.4 | 1.5x | LP12 |
| Warm pad | 600 Hz | 0.2 | 1x | LP24 |
| Sine voice | Played by keyboard | 0.9 | 1x | LP24, nothing in **In** |

## Technical Notes

- Each stage is a one-pole lowpass with a `tanh` at its input. The first stage's input is the signal minus the fed-back output, so the same `tanh` saturates the resonance. The loop is solved with zero delay (TPT), with the saturators linearised around a one-pass estimate of the current sample. That keeps the resonance in tune at high cutoffs.
- The oversampler uses polyphase IIR halfband filters: flat to within 0.01 dB up to 20 kHz, and about 100 dB of stopband. In tests, a 5 kHz tone driven into the filter has 30-55 dB less audible aliasing than the same filter run at the base rate. At extreme Drive combined with high Resonance the gain is smaller (about 10 dB), because the harmonics reach beyond even twice the sample rate.
- The response curve drawn on the node is computed from the filter's own transfer function, so the picture matches the sound.

## Related Modules

- [SVF Filter](./svf-filter.md) - Multi-mode, lighter-touch filter
- [Oscillator](../sources/oscillator.md) - Primary input source
- [ADSR Envelope](../modulation/adsr.md) - Modulate cutoff over time
- [LFO](../modulation/lfo.md) - Filter sweeps and wobbles
