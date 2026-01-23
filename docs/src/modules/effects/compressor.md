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

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Threshold** | -60 dB to 0 dB | -20 dB | Level above which compression begins |
| **Ratio** | 1:1 to ∞:1 | 4:1 | How much gain reduction is applied |
| **Attack** | 0.1 ms - 100 ms | 10 ms | How quickly compression engages |
| **Release** | 10 ms - 1000 ms | 100 ms | How quickly compression releases |
| **Makeup Gain** | 0 dB to +24 dB | 0 dB | Level boost after compression |
| **Knee** | Hard/Soft | Soft | Gradual or abrupt compression onset |

## How It Works

1. Signal level is measured (or sidechain input if connected)
2. When level exceeds **Threshold**, compression begins
3. **Ratio** determines how much signals above threshold are reduced
4. **Attack** controls how fast compression responds to transients
5. **Release** controls how fast compression recovers
6. **Makeup Gain** compensates for level reduction

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
Knee: Soft
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
Knee: Soft
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

## Knee Types

### Hard Knee
Compression applies suddenly at threshold. More obvious compression, more aggressive.

### Soft Knee
Compression gradually increases around threshold. More transparent, natural sound.

## Gain Staging

1. Set Threshold to catch peaks you want to compress
2. Adjust Ratio for desired amount
3. Use Makeup Gain to match bypassed level
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
2. **Match levels**: Use Makeup Gain to fairly compare compressed vs original
3. **Attack is key**: It determines if transients punch through
4. **Release affects groove**: Too fast causes pumping, too slow causes sustained squash
5. **Use your ears**: Watch meters but trust what sounds good

## Related Modules

- [VCA](../utilities/vca.md) - Alternative level control
- [EQ](./eq.md) - Often used before/after compression
- [Distortion](./distortion.md) - Can add saturation like compressor
- [Audio Output](../output/audio-output.md) - Has built-in limiter
