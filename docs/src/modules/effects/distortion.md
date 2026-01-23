# Distortion

**Module ID**: `fx.distortion`
**Category**: Effects
**Header Color**: Purple

![Distortion Module](../../images/module-distortion.png)
*The Distortion module*

## Description

The Distortion module adds harmonic richness and grit by clipping, saturating, or folding the input signal. From subtle warmth to aggressive destruction, distortion shapes the character of sounds and adds presence.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Signal to be distorted |
| **Drive CV** | Control (Orange) | Modulation input for drive amount |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Audio (Blue) | Distorted signal |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Drive** | 0.0 - 1.0 | 0.3 | Amount of distortion |
| **Type** | Soft/Hard/Fold/Bit | Soft | Distortion algorithm |
| **Tone** | 20 Hz - 20 kHz | 5 kHz | Output filter frequency |
| **Mix** | 0.0 - 1.0 | 1.0 | Dry/wet balance |
| **Output** | -12 dB - +6 dB | 0 dB | Output level compensation |

## Distortion Types

### Soft Clip

Gentle saturation that rounds off peaks:

- Adds warm, even harmonics
- Tube/tape-like character
- Compresses dynamics naturally
- Good for subtle warmth

### Hard Clip

Aggressive clipping that chops off peaks:

- Adds harsh, odd harmonics
- Transistor/digital character
- More aggressive, buzzy sound
- Classic overdrive/fuzz

### Fold

Wave folding that reflects the signal back:

- Complex harmonic content
- Synth-like, metallic character
- Creates additional partials
- West Coast synthesis style

### Bit Crush

Reduces bit depth for lo-fi character:

- Introduces quantization noise
- Gritty, digital degradation
- 8-bit/vintage sampler sound
- Adds "crunchy" character

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
Tone: 2000 Hz
Mix: 0.7
```

The tone filter prevents harsh high frequencies while preserving grind.

### Aggressive Lead

In-your-face distortion:

```
Type: Hard
Drive: 0.8
Tone: 4000 Hz
Mix: 1.0
```

### Lo-Fi Texture

Vintage sampler vibes:

```
Type: Bit
Drive: 0.6
Mix: 0.8
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
Tone: 6000 Hz
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

The Tone knob is a low-pass filter on the output:

| Tone | Character |
|------|-----------|
| 1-2 kHz | Dark, muddy distortion |
| 3-5 kHz | Warm, round distortion |
| 6-10 kHz | Present, cutting distortion |
| 10+ kHz | Bright, harsh distortion |

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
