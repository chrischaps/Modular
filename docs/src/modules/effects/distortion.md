# Distortion

**Module ID**: `fx.distortion`
**Category**: Effects
**Header Color**: Purple

![Distortion Module](../../images/module-distortion.png)
*The Distortion module*

## Description

The Distortion module adds harmonic richness and grit by clipping, saturating, folding, or crushing the input signal. From subtle warmth to aggressive destruction, distortion shapes the character of sounds and adds presence.

Every curve runs at four times the sample rate with antiderivative anti-aliasing, so even full drive on a high note adds only harmonics of the note, not the inharmonic "digital fizz" that naive distortion folds back into the audible band.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **In** | Audio (Blue) | Signal to be distorted |
| **Drive CV** | Control (Orange) | Modulates drive around the knob (±1 CV sweeps ±50%) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Out** | Audio (Blue) | Distorted signal |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Type** | Soft/Hard/Fold/Bit/Tube | Soft | Distortion algorithm |
| **Drive** | 0 - 100% | 50% | Amount of distortion |
| **Sym** | -100% - +100% | 0% | Fold only: offsets the wave into the folder |
| **Rate** | 100 Hz - 48 kHz | 48 kHz | Bit only: crushed sample rate (the top of the range is off) |
| **Tone** | 0 - 100% | 50% | Output low-pass, 200 Hz to 20 kHz |
| **Mix** | 0 - 100% | 100% | Dry/wet balance |
| **Out** | -12 dB - +12 dB | 0 dB | Output level compensation |

## Distortion Types

### Soft

Smooth `tanh` saturation that rounds off peaks (input gain 1x to 11x):

- Odd harmonics only, falling off smoothly
- Compresses dynamics naturally
- Good for warmth at low drive, fuzz at high drive

### Hard

Clipping that chops off peaks (threshold falls from 1.0 to 0.1):

- Bright, buzzy odd harmonics
- Transistor/digital character
- Classic overdrive/fuzz

### Fold

A wavefolder in the West Coast tradition of the Serge and Buchla folders. Past the clipping point the wave reflects back toward zero instead of flattening, then reflects again, so each step of drive (1x to 6x) adds another fold and another pair of peaks. The corners of each fold are rounded, as a real diode folder's are.

- Bright, vocal, metallic tones from a plain sine
- Sweeping Drive with CV gives the classic "wavefolder sweep"
- **Sym** shifts the wave off centre: the two halves fold differently and even harmonics appear. Near ±100% a quiet sine sits on a fold's peak and comes out an octave up.

### Bit

Bit depth and sample-rate reduction for lo-fi character:

- **Drive** lowers the bit depth from 16 down to 2 bits
- **Rate** holds each sample for longer, like an old sampler. Below a few kHz the crushed rate's mirror images ring out as metallic, inharmonic tones: that aliasing is the sound. It is relative to Rate only; the engine's own sample rate adds nothing.

### Tube

Asymmetric saturation, like a triode biased off its centre (input gain 1x to 11x):

- The two halves of the wave saturate at different levels, so even harmonics appear from the first touch of drive: warm, thick, slightly hollow
- The DC offset this creates is removed automatically

## Usage Tips

### Subtle Warmth

Add life to sterile digital signals:

```
Type: Soft
Drive: 0.2
Mix: 0.5
```

Just a hint of saturation, barely noticeable but adds presence.

### Bass Overdrive

Add harmonics that cut through the mix:

```
Type: Soft
Drive: 0.4
Tone: 50% (2 kHz)
Mix: 0.7
```

The tone filter prevents harsh high frequencies while preserving grind.

### Aggressive Lead

In-your-face distortion:

```
Type: Hard
Drive: 0.8
Tone: 65% (4 kHz)
Mix: 1.0
```

### Lo-Fi Texture

Vintage sampler vibes:

```
Type: Bit
Drive: 0.5
Rate: 8000 Hz
Mix: 0.8
```

### Tape-ish Warmth

Thicken a bass or a drum bus:

```
Type: Tube
Drive: 0.25
Tone: 60%
Mix: 0.6
```

### Octave Fold

A sine folded off-centre jumps an octave:

```
[Sine Oscillator] ──> [Distortion (Fold)]
                      Drive: 0, Sym: 100%
```

### Synth Processing

Add complex harmonics to simple waveforms:

```
[Sine Oscillator] ──> [Distortion (Fold)] ──> [Filter] ──> [Output]
                      Drive: 0.5
```

Wave folding turns a simple sine into a complex tone.

### Drum Processing

Add punch and presence:

```
Type: Soft
Drive: 0.3
Tone: 74% (6 kHz)
Mix: 0.6
```

### Parallel Distortion

Keep clean low end, distort highs:

```
[Input] ──> [High Pass] ──> [Distortion] ──> [Mixer Ch 2]
        ──> [Low Pass] ────────────────────> [Mixer Ch 1]
[Mixer] ──> [Output]
```

The clean bass stays tight while harmonics are added to mids/highs.

### Modulated Drive

Dynamic distortion amount:

```
[Envelope] ──> [Distortion Drive CV]
```

More distortion during attack, cleaner during sustain.

### Creative Textures

Use wave folding for synth-like sounds:

```
[LFO] ──> [Attenuverter] ──> [Distortion (Fold)] ──> [Filter]
```

Even slow control signals become complex audio when folded.

## Drive Amount Guide

| Drive | Effect |
|-------|--------|
| 0.0-0.2 | Subtle warmth, slight compression |
| 0.2-0.4 | Noticeable saturation, crunchy |
| 0.4-0.6 | Clear distortion, harmonics prominent |
| 0.6-0.8 | Heavy distortion, aggressive |
| 0.8-1.0 | Extreme, destructive |

## Tone Control

The Tone knob is a low-pass filter on the distorted signal, from 200 Hz (0%) to 20 kHz (100%), spaced evenly in octaves:

| Tone | Cutoff | Character |
|------|--------|-----------|
| 0-30% | 200-800 Hz | Dark, muffled |
| 30-60% | 0.8-3 kHz | Warm, round |
| 60-80% | 3-8 kHz | Present, cutting |
| 80-100% | 8-20 kHz | Bright, open |

Distortion creates high harmonics—use Tone to control harshness.

## Connection Examples

### Standard Insert
```
[Synth] ──> [Distortion] ──> [Filter] ──> [VCA] ──> [Output]
```

### Post-Filter Distortion
```
[Oscillator] ──> [Filter] ──> [Distortion] ──> [VCA] ──> [Output]
```

Different character—filter first can prevent extreme harshness.

### Send Effect
```
[Mixer Send] ──> [Distortion (Mix: 100%)] ──> [Mixer Return]
```

Blend clean and distorted in the mixer.

### Dynamic Distortion
```
[Input] ──> [Distortion]
[Input] ──> [Envelope Follower] ──> [Distortion Drive CV]
```

Louder input = more distortion.

## Distortion in the Signal Chain

**Before Filter**: Maximum harmonics, filter can tame harshness
**After Filter**: Cleaner distortion, filter cutoff is clean
**Before VCA**: Consistent distortion amount
**After VCA**: Distortion varies with dynamics

Most common: Oscillator → Filter → Distortion → VCA

## Tips

1. **Use output compensation**: Distortion can increase level significantly
2. **Consider the Tone knob**: Bright distortion can be harsh
3. **Try different orders**: Filter → Distortion vs Distortion → Filter
4. **Don't overdo it**: Subtlety often works better in a mix
5. **Watch your ears**: High-frequency distortion can be fatiguing

## Related Modules

- [SVF Filter](../filters/svf-filter.md) - Shape distortion harmonics
- [Compressor](./compressor.md) - Control dynamics before/after
- [EQ](./eq.md) - Fine-tune distortion character
- [VCA](../utilities/vca.md) - Control distortion input level
