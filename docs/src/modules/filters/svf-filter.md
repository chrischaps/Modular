# SVF Filter

**Module ID**: `filter.svf`
**Category**: Filters
**Header Color**: Green

![SVF Filter Module](../../images/module-svf-filter.png)
*The SVF Filter module*

## Description

The State Variable Filter (SVF) is a versatile multi-mode filter that provides simultaneous lowpass, highpass, bandpass and notch outputs from a single input signal. This architecture allows you to blend different filter responses or switch between them without repatching.

The SVF design offers:
- Self-oscillation at full resonance: a clean, self-limiting sine at the cutoff frequency
- Cutoff that moves in octaves, so sweeps sound even from bass to treble
- Analog-style resonance that saturates instead of getting louder without limit
- Stable operation across all settings, right up to 20 kHz
- Simultaneous multi-mode outputs

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Main audio input to be filtered |
| **Cutoff** | Control (Orange) | Cutoff CV, 1 per octave: +1 doubles the cutoff, -1 halves it. The same scale as V/Oct, so a keyboard's pitch tracks directly |
| **Resonance** | Control (Orange) | Adds to the Resonance knob (+1 adds 0.5) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Lowpass** | Audio (Blue) | Lowpass output - passes frequencies below cutoff |
| **Highpass** | Audio (Blue) | Highpass output - passes frequencies above cutoff |
| **Bandpass** | Audio (Blue) | Bandpass output - passes frequencies around cutoff |
| **Notch** | Audio (Blue) | Notch output - passes everything except the cutoff |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Cutoff** | 20 Hz - 20 kHz | 1000 Hz | Filter cutoff frequency |
| **Resonance** | 0.0 - 1.0 | 0.5 | Emphasis at the cutoff frequency. Self-oscillates above about 0.97 |
| **Drive** | 1x - 10x | 1x | Input gain into a soft saturator, for warmth and grit |

## Filter Modes

### Lowpass (LP)

![Lowpass Response](../../images/filter-lowpass.png)
*Lowpass frequency response*

- Passes frequencies **below** the cutoff
- Removes high frequencies, creating a "darker" or "warmer" sound
- Most common filter type for synthesis
- At 12dB/octave slope (2-pole)

**Use cases:**
- Warming up bright oscillators
- Classic subtractive synthesis
- Bass sounds
- Removing harshness

### Highpass (HP)

![Highpass Response](../../images/filter-highpass.png)
*Highpass frequency response*

- Passes frequencies **above** the cutoff
- Removes low frequencies, creating a "thinner" or "brighter" sound
- At 12dB/octave slope (2-pole)

**Use cases:**
- Removing mud/rumble
- Creating thin, airy sounds
- Hi-hat and cymbal synthesis
- Clearing space in a mix

### Bandpass (BP)

![Bandpass Response](../../images/filter-bandpass.png)
*Bandpass frequency response*

- Passes frequencies **around** the cutoff
- Removes both low and high frequencies
- Width controlled by resonance

**Use cases:**
- Vocal/formant-like sounds
- Telephone/radio effect
- Isolating specific frequency ranges
- Wah-wah effects

### Notch

- Removes a narrow band **around** the cutoff and passes everything else
- The notch is deepest exactly at the cutoff, and narrower at higher resonance
- Equal to the lowpass and highpass outputs added together

**Use cases:**
- Phaser-like sweeps (modulate the cutoff with a slow LFO)
- Hollowing out a sound without darkening it
- Removing a single resonant frequency or hum

## Usage Tips

### Basic Filtering

Connect an oscillator to soften its harmonics:

```
[Oscillator] ──> [Filter Input]
                 [Filter LP] ──> [VCA] ──> [Output]
```

- Start with cutoff around 1000 Hz
- Adjust cutoff to taste - lower = darker, higher = brighter
- Add slight resonance (0.2-0.4) for character

### Filter Envelope

Create dynamic filter sweeps with an envelope:

```
[Keyboard] ──Gate──> [Envelope] ──> [Filter Cutoff CV]
```

- Set base cutoff low (200-500 Hz)
- The envelope's 0-1 output raises the cutoff by up to one octave. For a wider sweep, scale it up with an Attenuverter or Mixer first
- Short attack/decay creates "plucky" sounds
- Long attack creates "swelling" sounds

### Filter + LFO (Wobble)

Create rhythmic filter movement:

```
[LFO] ──> [Filter Cutoff CV]
```

- Square LFO creates choppy, rhythmic effect
- Triangle/Sine LFO creates smooth wobble
- Adjust the LFO rate, and its level (through an Attenuverter) for intensity

### Self-Oscillation

At the top of the Resonance range the filter self-oscillates, producing a sine wave at the cutoff frequency:

- Turn Resonance all the way up (it starts to sing above about 0.97)
- No input signal needed. The oscillation grows out of a tiny noise floor, like circuit noise in an analog filter
- The tone lands within a few cents of the Cutoff knob, so the knob reads as a pitch
- Play it from a keyboard: patch Pitch into Cutoff and it tracks in tune (1 per octave)
- A saturator in the resonance path holds the level steady (around -12 dBFS), so it won't run away

Feed audio in while it oscillates and the two interact, with the resonance pushing back against loud input.

### Tracking Keyboard

Make filter cutoff follow the keyboard:

```
[Keyboard] ──V/Oct──> [Oscillator V/Oct]
           ──V/Oct──> [Filter Cutoff CV]
```

This keeps the filter's relative brightness consistent across different pitches. The Cutoff input is 1 per octave, the same scale as V/Oct, so the pitch CV tracks 1:1 with no scaling.

### Parallel Filter Modes

Use multiple outputs simultaneously for complex sounds:

```
[Oscillator] ──> [Filter Input]
                 [Filter LP] ──> [Mixer Ch1]
                 [Filter BP] ──> [Mixer Ch2] ──> [Output]
```

Blend lowpass and bandpass for unique timbres.

### Resonant Accents

High resonance emphasizes the cutoff frequency:

- Creates a "peak" or "ping" at the cutoff
- Useful for acid bass lines (TB-303 style)
- Combine with filter envelope for accent effects

### Moving Notch

Patch the **Notch** output and sweep the cutoff slowly with an LFO for a phaser-like movement:

```
[Oscillator (Saw)] ──> [Filter Input]
[LFO (slow)] ──> [Filter Cutoff CV]
                 [Filter Notch] ──> [Output]
```

## Connection Examples

### Classic Subtractive Synth
```
[Keyboard] ──V/Oct──> [Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
           ──Gate───> [Envelope] ──────────┬─────────────┘
                                           └──> [Filter Cutoff CV]
```

### Acid Bass
```
[Sequencer] ──CV──> [Oscillator (Saw)] ──> [Filter LP] ──> [Output]
            ──Gate──> [Envelope] ──> [Filter Cutoff CV]
                                     (Resonance: 0.7-0.9)
```

### Wah Effect
```
[Guitar/Audio In] ──> [Filter BP] ──> [Output]
                      [Expression Pedal] ──> [Filter Cutoff CV]
```

## Sound Design Tips

| Sound | Cutoff | Resonance | Modulation |
|-------|--------|-----------|------------|
| Warm pad | 800 Hz | 0.1 | Slow LFO |
| Acid bass | 300-500 Hz | 0.7-0.9 | Fast envelope |
| Bright lead | 3000 Hz | 0.3 | Medium envelope |
| Sub bass | 200 Hz | 0.0 | None |
| Pluck | 1000 Hz | 0.4 | Fast decay envelope |

## Related Modules

- [Oscillator](../sources/oscillator.md) - Primary input source
- [ADSR Envelope](../modulation/adsr.md) - Modulate cutoff over time
- [LFO](../modulation/lfo.md) - Create filter wobble effects
- [VCA](../utilities/vca.md) - Control filtered output level
