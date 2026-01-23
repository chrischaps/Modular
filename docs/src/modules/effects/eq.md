# EQ

**Module ID**: `fx.eq`
**Category**: Effects
**Header Color**: Purple

![EQ Module](../../images/module-eq.png)
*The EQ module*

## Description

The 3-Band Parametric EQ allows precise control over the frequency content of your signal. Each band can boost or cut a specific frequency range with adjustable center frequency and bandwidth (Q).

EQ is essential for:
- Shaping tone and timbre
- Fixing frequency problems
- Making sounds fit in a mix
- Creative sound design

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input** | Audio (Blue) | Signal to be equalized |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Output** | Audio (Blue) | Equalized signal |

## Parameters

### Low Band
| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Low Freq** | 20 Hz - 500 Hz | 100 Hz | Center frequency |
| **Low Gain** | -12 dB to +12 dB | 0 dB | Boost or cut amount |
| **Low Q** | 0.5 - 10.0 | 1.0 | Bandwidth (higher = narrower) |

### Mid Band
| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Mid Freq** | 200 Hz - 5 kHz | 1 kHz | Center frequency |
| **Mid Gain** | -12 dB to +12 dB | 0 dB | Boost or cut amount |
| **Mid Q** | 0.5 - 10.0 | 1.0 | Bandwidth (higher = narrower) |

### High Band
| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **High Freq** | 1 kHz - 20 kHz | 8 kHz | Center frequency |
| **High Gain** | -12 dB to +12 dB | 0 dB | Boost or cut amount |
| **High Q** | 0.5 - 10.0 | 1.0 | Bandwidth (higher = narrower) |

## Understanding EQ

### Frequency Ranges

| Range | Frequency | Character |
|-------|-----------|-----------|
| Sub | 20-60 Hz | Felt more than heard, rumble |
| Bass | 60-250 Hz | Weight, warmth, punch |
| Low Mid | 250-500 Hz | Body, muddiness |
| Mid | 500 Hz - 2 kHz | Presence, boxiness, clarity |
| Upper Mid | 2-5 kHz | Definition, attack, harshness |
| High | 5-10 kHz | Brightness, sibilance, air |
| Air | 10-20 kHz | Sparkle, air, presence |

### Q (Bandwidth)

Q controls how wide or narrow the affected frequency range is:

- **Low Q (0.5-1.5)**: Wide, gentle curves, affects many frequencies
- **Medium Q (2-4)**: Focused but still musical
- **High Q (5-10)**: Surgical, narrow cuts, affects specific frequencies

### Boost vs Cut

General rule: **Cut narrow, boost wide**

- Cuts are good for removing problem frequencies (use higher Q)
- Boosts add character but can sound unnatural (use lower Q)

## Usage Tips

### Removing Mud

Clean up muddy sounds:

```
Mid Freq: 300 Hz
Mid Gain: -3 to -6 dB
Mid Q: 2.0
```

This is the common "mud" frequency range.

### Adding Warmth

Add body and warmth:

```
Low Freq: 100 Hz
Low Gain: +3 dB
Low Q: 1.0
```

### Adding Presence

Make sounds cut through:

```
High Freq: 3 kHz
High Gain: +3 dB
High Q: 1.5
```

### Telephone Effect

Dramatic filtering for effect:

```
Low Freq: 300 Hz, Gain: -12 dB
High Freq: 3 kHz, Gain: -12 dB
```

Cuts lows and highs, leaving only mids.

### De-Boxing

Remove boxy sound from recordings:

```
Mid Freq: 400-600 Hz
Mid Gain: -3 dB
Mid Q: 2.5
```

### Air and Sparkle

Add high-end shine:

```
High Freq: 12 kHz
High Gain: +2 dB
High Q: 0.7
```

Wide boost adds open, airy quality.

### Finding Problem Frequencies

1. Set one band's Q to high (5.0+)
2. Boost the gain to +6 dB
3. Sweep the frequency while listening
4. When you hear the problem clearly, reduce gain to cut

### Subtractive EQ

Start by cutting problem frequencies rather than boosting:

- Often sounds more natural
- Less likely to cause clipping
- Maintains headroom

### Frequency Slot Carving

Give each sound its own space:

```
Bass: Boost 80 Hz, cut 300 Hz
Synth: Cut 80 Hz, boost 500 Hz
Lead: Cut 500 Hz, boost 2 kHz
```

Each element has its own frequency territory.

## Frequency Cheat Sheet

### Problem Frequencies
| Problem | Frequency | Solution |
|---------|-----------|----------|
| Rumble | < 40 Hz | Cut |
| Boomy | 100-200 Hz | Cut narrow |
| Muddy | 250-400 Hz | Cut |
| Boxy | 400-600 Hz | Cut |
| Nasal | 800 Hz - 1 kHz | Cut |
| Harsh | 2-4 kHz | Cut narrow |
| Sibilant | 5-8 kHz | Cut narrow |

### Enhancement Frequencies
| Quality | Frequency | Action |
|---------|-----------|--------|
| Weight | 80-100 Hz | Boost |
| Warmth | 200 Hz | Boost wide |
| Body | 300-500 Hz | Boost carefully |
| Presence | 2-4 kHz | Boost |
| Clarity | 5-7 kHz | Boost |
| Air | 10-15 kHz | Boost |

## Connection Examples

### Channel Strip
```
[Sound] ──> [EQ] ──> [Compressor] ──> [Output]
```

### Post-Effect Processing
```
[Synth] ──> [Distortion] ──> [EQ] ──> [Output]
```

EQ after distortion can tame harsh frequencies.

### Reverb Shaping
```
[Sound] ──> [Reverb] ──> [EQ] ──> [Output]
```

Cut lows from reverb to prevent mud.

## Tips

1. **Cut before boost**: Try cutting unwanted frequencies first
2. **Less is more**: Small EQ moves often sound best
3. **Use your ears**: Trust what sounds good, not what looks right
4. **A/B test**: Bypass the EQ to compare processed and original
5. **Watch levels**: Boosting increases overall level

## Related Modules

- [SVF Filter](../filters/svf-filter.md) - Dramatic filtering
- [Compressor](./compressor.md) - Dynamics after EQ
- [Distortion](./distortion.md) - Harmonics to then EQ
- [Reverb](./reverb.md) - Shape reverb tone with EQ
