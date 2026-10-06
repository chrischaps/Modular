# Compressor

**Module ID**: `fx.compressor`
**Category**: Effects
**Header Color**: Purple

![Compressor Module](../../images/module-compressor.png)
*The Compressor module*

## Description

The Compressor reduces the dynamic range of a signal by attenuating loud portions while leaving quiet portions unchanged. This creates a more consistent level, adds punch, and can create distinctive pumping effects.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Signal to be compressed |
| **Sidechain** | Audio (Blue) | External signal to control compression (optional) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Audio (Blue) | Compressed signal |
| **Gain Reduction** | Control (Orange) | CV output showing compression amount |

## Parameters

| Control | Range | Default | Description |
|---------|-------|---------|-------------|
| **Threshold** | -60 dB to 0 dB | -20 dB | Level above which compression begins |
| **Ratio** | 1:1 to 20:1 | 4:1 | How much gain reduction is applied |
| **Attack** | 0.1 ms - 100 ms | 10 ms | How quickly compression engages |
| **Release** | 10 ms - 1000 ms | 100 ms | How quickly compression releases |
| **Knee** | 0 dB to 12 dB | 6 dB | Width of the soft knee (0 = hard knee) |
| **Makeup** | 0 dB to +24 dB | 0 dB | Level boost after compression |
| **Mix** | 0% to 100% | 100% | Blend of compressed and dry (parallel compression) |
| **Detector** | Peak / RMS | Peak | What the level detector measures (inline select) |

## How It Works

1. The level of the input is measured, or of the sidechain if one is connected
2. When the level exceeds **Threshold**, compression begins
3. **Ratio** determines how much signals above threshold are reduced
4. **Attack** controls how fast compression responds to transients
5. **Release** controls how fast compression recovers
6. **Makeup** compensates for level reduction

Attack and release shape the *gain reduction*, in dB, rather than the measured
level. So a steady tone above threshold comes out exactly where the ratio says,
however fast or slow the timing knobs are set. Attack and release only decide how
it gets there.

### Peak vs RMS

| Detector | Measures | Character |
|----------|----------|-----------|
| **Peak** | Every peak of the waveform | Catches transients; keeps peaks in check. Good for drums and for limiting |
| **RMS** | Average power over ~20 ms | Responds to loudness, as the ear does. Smoother, gentler leveling for buses, pads and vocals |

A sine reads 3 dB lower on RMS than on Peak, so the same settings compress it a
little less.

### Understanding Ratio

- **1:1**: No compression (bypass)
- **2:1**: For every 2 dB above threshold, only 1 dB passes
- **4:1**: For every 4 dB above threshold, only 1 dB passes
- **10:1**: Heavy compression
- **∞:1**: Limiting (nothing passes above threshold)

### Threshold Example

If Threshold = -20 dB and Ratio = 4:1:
- Signal at -30 dB: Unchanged
- Signal at -20 dB: Unchanged (at threshold)
- Signal at -10 dB: Reduced to -17.5 dB

## Usage Tips

### Gentle Leveling

Smooth out dynamics without obvious compression:

```
Threshold: -18 dB
Ratio: 2:1
Attack: 20 ms
Release: 200 ms
Knee: 6 dB
Detector: RMS
```

### Punchy Drums

Add snap and punch:

```
Threshold: -10 dB
Ratio: 4:1
Attack: 10 ms
Release: 50 ms
```

Fast attack catches transients, fast release lets energy through.

### Sustained Bass

Even out bass levels:

```
Threshold: -15 dB
Ratio: 4:1
Attack: 5 ms
Release: 150 ms
```

### Vocal Compression

Consistent vocal level:

```
Threshold: -20 dB
Ratio: 3:1
Attack: 10 ms
Release: 100 ms
Knee: 6 dB
Detector: RMS
```

### Synth Pad Sustain

Make pads sustain more evenly:

```
Threshold: -24 dB
Ratio: 3:1
Attack: 30 ms
Release: 300 ms
```

Slow attack preserves natural swell.

### Sidechain Pumping

Classic EDM pumping effect:

```
[Kick] ──> [Compressor Sidechain]
[Pad/Bass] ──> [Compressor Input]

Threshold: -30 dB
Ratio: 6:1
Attack: 1 ms
Release: 200 ms
```

The kick ducks the pad, creating rhythmic pumping.

### Peak Limiting

Catch peaks without obvious compression:

```
Threshold: -3 dB
Ratio: 10:1 or higher
Attack: 0.1 ms
Release: 50 ms
```

### Parallel Compression

Blend compressed and dry signals:

```
[Input] ──> [Compressor] ──> [Mixer Ch 2]
        ──> [Mixer Ch 1 (dry)]
[Mixer] ──> [Output]
```

Heavy compression (6:1+) on the compressed path, blend to taste.

### Using Gain Reduction Output

Visualize or control other parameters based on compression:

```
[Compressor GR Output] ──> [Other Parameter CV]
```

Could modulate filter cutoff, pan, effects send, etc.

## Attack and Release Guide

### Attack Times

| Attack | Effect |
|--------|--------|
| 0.1-1 ms | Catches all transients (can sound unnatural) |
| 1-10 ms | Fast, punchy, some transients pass |
| 10-30 ms | Balanced, musical compression |
| 30-100 ms | Slow, lets transients through fully |

### Release Times

| Release | Effect |
|---------|--------|
| 10-50 ms | Fast, can cause pumping |
| 50-150 ms | Medium, musical, versatile |
| 150-400 ms | Slow, smooth, sustained |
| 400+ ms | Very slow, sustained compression |

## Knee

### Hard Knee (0 dB)
Compression applies suddenly at threshold. More obvious compression, more aggressive.

### Soft Knee (up to 12 dB)
Compression eases in across a band centred on the threshold: with a 6 dB knee it
starts 3 dB below and reaches the full ratio 3 dB above. More transparent and natural.

## Gain Staging

1. Set Threshold to catch peaks you want to compress
2. Adjust Ratio for desired amount
3. Use Makeup to match bypassed level
4. A/B compare with bypass to verify

## Connection Examples

### Channel Strip
```
[Synth] ──> [EQ] ──> [Compressor] ──> [Output]
```

### Sidechain Setup
```
[Kick] ──> [Compressor Sidechain]
[Bass] ──> [Compressor Input]
[Compressor Output] ──> [Output]
```

### Parallel Compression
```
[Drums] ──> [Compressor (heavy)] ──> [Mixer (wet)]
        ──> [Mixer (dry)]
[Mixer] ──> [Output]
```

### Ducking
```
[Voice/Narration] ──> [Compressor Sidechain]
[Background Music] ──> [Compressor] ──> [Output]
```

Music ducks when voice is present.

## Compression Cheat Sheet

| Use Case | Threshold | Ratio | Attack | Release |
|----------|-----------|-------|--------|---------|
| Gentle leveling | -18 dB | 2:1 | 20 ms | 200 ms |
| Punchy drums | -10 dB | 4:1 | 5 ms | 50 ms |
| Sustained bass | -15 dB | 4:1 | 5 ms | 150 ms |
| Vocal control | -20 dB | 3:1 | 10 ms | 100 ms |
| Pad sustain | -24 dB | 3:1 | 30 ms | 300 ms |
| Sidechain pump | -30 dB | 6:1 | 1 ms | 200 ms |
| Peak limiting | -3 dB | 10:1+ | 0.1 ms | 50 ms |

## Tips

1. **Don't overcompress**: 2-6 dB of gain reduction is usually enough
2. **Match levels**: Use Makeup to fairly compare compressed vs original
3. **Attack is key**: It determines if transients punch through
4. **Release affects groove**: Too fast causes pumping, too slow causes sustained squash
5. **Use your ears**: Watch meters but trust what sounds good

## Related Modules

- [VCA](../utilities/vca.md) - Alternative level control
- [EQ](./eq.md) - Often used before/after compression
- [Distortion](./distortion.md) - Can add saturation like compressor
- [Audio Output](../output/audio-output.md) - Has built-in limiter
