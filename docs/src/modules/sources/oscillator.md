# Oscillator

**Module ID**: `osc.sine`
**Category**: Sources
**Header Color**: Blue

![Oscillator Module](../../images/module-oscillator.png)
*The Oscillator module*

## Description

The Oscillator is the primary sound source in Modular Synth. It is a **VCO** (Voltage Controlled Oscillator): a tuned waveform whose pitch follows a keyboard, an LFO, or another oscillator.

Beyond the four classic waveforms it has the features that make an oscillator an instrument:

- a **tune section** in octaves, semitones and cents;
- **hard sync** to another oscillator;
- **through-zero linear FM** and **exponential FM**;
- a **sub-oscillator** one octave down;
- **unison** of up to seven voices, with stereo spread. Seven detuned saws is the classic *supersaw*.

Every waveform is band-limited. Each voice runs at twice the sample rate with polyBLEP corrections on its edges and polyBLAMP corrections on its corners, then a halfband filter brings it back down. A 5 kHz saw's aliasing sits around −80 dB, so high notes and sync sweeps stay clean.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **V/Oct** | Control (Orange) | 1 V/octave pitch. Each +1.0 raises the pitch one octave, added to the tune section |
| **FM** | Control (Orange) | Through-zero linear FM, scaled by **FM Depth** |
| **Exp FM** | Control (Orange) | Exponential FM, **Exp FM Depth** octaves per unit |
| **PWM** | Control (Orange) | Pulse width modulation around **Pulse Width** (±1 moves it ±0.4) |
| **Sync** | Control (Orange) | Hard sync. Each rising zero crossing restarts the cycle. Accepts audio, control or gates |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Out** | Audio (Blue) | All voices, mono |
| **Sub** | Audio (Blue) | Square wave one octave below the tuned pitch |
| **Out L** | Audio (Blue) | Unison voices spread to the left |
| **Out R** | Audio (Blue) | Unison voices spread to the right |

## Parameters

The knobs sit in three rows: **tune**, **timbre**, **ensemble**.

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Oct** | −4 to +4 | 0 | Octave above or below C4. Clicks in whole octaves |
| **Semi** | −12 to +12 | 0 | Semitones. Clicks in whole semitones |
| **Fine** | ±100 cents | 0 | Fine tuning |
| **FM** | 0 – 5 | 0 | Linear FM index (see below) |
| **Exp FM** | 0 – 4 oct | 1 | Octaves of pitch change per unit at the Exp FM input |
| **PW** | 0.1 – 0.9 | 0.5 | Pulse width of the square (0.5 = 50% duty cycle) |
| **Voices** | 1 – 7 | 1 | Unison voices |
| **Detune** | 0 – 100% | 40% | How far apart the unison voices are tuned |
| **Spread** | 0 – 100% | 50% | How wide the unison voices sit on Out L / Out R |
| **Waveform** | Sine / Saw / Square / Tri | Sine | Dropdown on the node |

With every tune knob at 0 the oscillator plays **C4 (261.63 Hz)**. Keyboard and MIDI modules send 0.0 for C4, so they play in tune with nothing else to set. MIDI note 69 plays 440 Hz to within a tenth of a cent.

Pitch is smoothed in octaves, not Hz, so a jump of an octave up takes as long as an octave down.

## Waveforms

### Sine
![Sine Wave](../../images/waveform-sine.png)

Pure tone with no harmonics: sub-bass, FM carriers and modulators, flute-like tones.

### Sawtooth (Saw)
![Saw Wave](../../images/waveform-saw.png)

Every harmonic, falling as 1/n: classic leads and basses, strings, brass. The richest source for filtering, and the one to use for supersaw.

### Square
![Square Wave](../../images/waveform-square.png)

Odd harmonics only: hollow, woody, clarinet-like. Pulse width reshapes it, from the square at 0.5 to thin and nasal near 0.1 or 0.9.

### Triangle
![Triangle Wave](../../images/waveform-triangle.png)

Odd harmonics falling fast: softer than square, brighter than sine.

## Features

### Tune section

**Oct** and **Semi** set the note; **Fine** trims it in cents. To tune a second oscillator a fifth up, set **Semi** to +7. To beat slowly against the first, add a few cents of **Fine**.

### Hard sync

Patch another oscillator's **Out** into **Sync**:

```
[Osc 2 (master)] ──Out──> [Osc 1 Sync]
```

Every time the master's waveform rises through zero, this oscillator starts its cycle again. The result repeats at the *master's* pitch, but its tone comes from *this* oscillator's pitch. Tune this one above the master and sweep its pitch (a slow LFO or envelope on **V/Oct** or **Exp FM**) for the classic tearing sync sweep.

The reset itself is band-limited, so sync sweeps don't fizz with aliasing.

### Through-zero FM

The **FM** input multiplies the pitch by `1 + FM Depth × FM`:

- At depth 1, an FM signal of −1 just reaches 0 Hz.
- Past 1, the frequency swings *through* zero and the waveform runs backwards for a moment, instead of stalling at 0 Hz.

That keeps the average pitch where it was, so deep FM stays in tune and keeps the bright, glassy character of DX-style FM. Because the depth is relative to the pitch, the timbre stays the same as you play up the keyboard.

```
[Osc 2 (modulator)] ──Out──> [Osc 1 FM]  ──> [VCA] ──> [Output]
```

- Sine into sine gives the cleanest FM tones.
- Whole-number pitch ratios (Semi +12, or +19 for 3:1) sound harmonic.
- Other ratios sound bell-like and metallic.
- Envelope the modulator's level (through a VCA) to make the brightness decay like a struck bell.

### Exponential FM

**Exp FM** works like a second V/Oct input with its own depth knob. An LFO into it gives a vibrato that's even in pitch, the same width in cents at any note. Keep the depth small (Shift-drag the knob for fine steps): 0.02 octaves is about ±24 cents. At audio rates it gives a rougher, less pitch-stable kind of FM than the linear input.

### Sub-oscillator

**Sub** is a square wave one octave below the tuned pitch. It divides the main oscillator's own cycle, so it follows V/Oct, FM and sync. Mix it in under a saw for weight:

```
[Oscillator] ──Out──> [Mixer 1]
             ──Sub──> [Mixer 2] ──> [Filter]
```

### Unison and supersaw

**Voices** stacks copies of the oscillator, each slightly detuned:

- **Detune** sets how far apart they are. Its curve is gentle at the bottom: 20% puts the outer voices about ±4 cents apart, 40% about ±16 cents, 100% a full ±100 cents.
- The voices bunch toward the middle, close to Roland's JP-8000 supersaw. The spacing is deliberately uneven, so the voices never beat in lockstep.
- Each voice starts at a different point in its cycle, so the stack doesn't open with one loud spike.
- **Out** sums all the voices at about the loudness of one voice.
- **Out L** and **Out R** pan the voices across the stereo field, outer voices widest. **Spread** sets the width.

For a supersaw: Waveform **Saw**, **Voices** 7, **Detune** about 40%, and **Out L / Out R** into the output's **Left / Right**.

### Pulse width modulation

For the square wave, modulate pulse width with an LFO:

```
[LFO] ──> [Oscillator PWM]
```

Slow rates give a chorus-like shimmer; fast rates a more dramatic timbral wobble.

## Connection Examples

### Typical synthesis chain
```
[Keyboard] ──V/Oct──> [Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
```

### Sync lead
```
[Keyboard] ──V/Oct──> [Osc 2] ──Out──> [Osc 1 Sync]
           ──V/Oct──> [Osc 1, Oct +1] ──> [Filter] ──> [Output]
[ADSR] ──> [Osc 1 Exp FM]
```

### Supersaw pad
```
[Keyboard] ──V/Oct──> [Oscillator: Saw, 7 voices]
                          ├─Out L──> [Reverb In L] ──> [Output Left]
                          └─Out R──> [Reverb In R] ──> [Output Right]
```

## Notes

- **Older patches** load at the same pitch. Their Frequency setting is split into Oct, Semi and Fine. FM Depth, which used to be in Hz, becomes the matching index at that pitch. A MIDI controller mapped to Frequency moves to Oct over the same range. The old *Frequency* input jack is gone; a cable into it is skipped with a load warning.
- Band-limited edges overshoot a little. Saw and square peak at about 1.3 for a few samples after each edge. This is ringing near 24 kHz, inaudible, and any filter or the output stage takes it away.
- The tune section reaches from about 7.7 Hz to 8.9 kHz. Use V/Oct or Exp FM to go further.

## Related Modules

- [LFO](../modulation/lfo.md) - For vibrato and PWM modulation
- [SVF Filter](../filters/svf-filter.md) - Shape the oscillator's harmonics
- [Ladder Filter](../filters/ladder-filter.md) - The fat, saturating 24 dB alternative
- [VCA](../utilities/vca.md) - Control oscillator volume
- [ADSR Envelope](../modulation/adsr.md) - Shape the sound over time
