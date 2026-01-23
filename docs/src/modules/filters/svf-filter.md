# SVF Filter

**Module ID**: `filter.svf`
**Category**: Filters
**Header Color**: Green

![SVF Filter Module](../../images/module-svf-filter.png)
*The SVF Filter module*

## Description

The State Variable Filter (SVF) is a versatile multi-mode filter that provides simultaneous lowpass, highpass, and bandpass outputs from a single input signal. This architecture allows you to blend different filter responses or switch between them without repatching.

The SVF design offers:
- Self-oscillation at high resonance (can be used as a sine oscillator)
- Smooth cutoff sweeps without artifacts
- Stable operation across all settings
- Simultaneous multi-mode outputs

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Main audio input to be filtered |
| **Cutoff** | Control (Orange) | Modulation input for cutoff frequency |
| **Resonance** | Control (Orange) | Modulation input for resonance/Q |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Lowpass** | Audio (Blue) | Lowpass output - passes frequencies below cutoff |
| **Highpass** | Audio (Blue) | Highpass output - passes frequencies above cutoff |
| **Bandpass** | Audio (Blue) | Bandpass output - passes frequencies around cutoff |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Cutoff** | 20 Hz - 20 kHz | 1000 Hz | Filter cutoff frequency |
| **Resonance** | 0.0 - 1.0 | 0.0 | Resonance/Q - emphasis at cutoff frequency |
| **CV Amount** | -1.0 - +1.0 | 0.0 | How much the Cutoff CV input affects the cutoff |

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
- Use positive CV Amount
- Short attack/decay creates "plucky" sounds
- Long attack creates "swelling" sounds

### Filter + LFO (Wobble)

Create rhythmic filter movement:

```
[LFO] ──> [Filter Cutoff CV]
```

- Square LFO creates choppy, rhythmic effect
- Triangle/Sine LFO creates smooth wobble
- Adjust LFO rate and CV Amount for intensity

### Self-Oscillation

At high resonance (near 1.0), the filter will self-oscillate, producing a sine wave at the cutoff frequency:

- Set resonance to ~0.95 or higher
- No input signal needed
- Control pitch via Cutoff CV
- Useful for pure sine tones and sound effects

**Note:** Self-oscillation can be loud - reduce output level first.

### Tracking Keyboard

Make filter cutoff follow the keyboard:

```
[Keyboard] ──V/Oct──> [Oscillator V/Oct]
           ──V/Oct──> [Filter Cutoff CV]
```

This keeps the filter's relative brightness consistent across different pitches. Set CV Amount to achieve 1:1 tracking.

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

### Notch Filter (Advanced)

Combine highpass and lowpass outputs to create a notch:

```
[Filter LP] ──> [Mixer] (inverted) ──┐
[Filter HP] ──> [Mixer] ─────────────┴──> [Output]
```

The phase relationship creates a notch at the cutoff frequency.

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
