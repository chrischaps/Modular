# Chorus

**Module ID**: `fx.chorus`
**Category**: Effects
**Header Color**: Purple

![Chorus Module](../../images/module-chorus.png)
*The Chorus module*

## Description

The Chorus effect creates a thicker, richer sound by layering slightly detuned and delayed copies of the input signal. It simulates the natural variation when multiple performers play the same part, adding warmth and movement without obvious echoes.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left In** | Audio (Blue) | Left channel input |
| **Right In** | Audio (Blue) | Right channel input (normalled to Left) |
| **Rate CV** | Control (Orange) | Modulation input for LFO rate |
| **Depth CV** | Control (Orange) | Modulation input for depth amount |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left Out** | Audio (Blue) | Processed left channel |
| **Right Out** | Audio (Blue) | Processed right channel |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Rate** | 0.1 Hz - 5 Hz | 0.5 Hz | Speed of modulation LFO |
| **Depth** | 0.0 - 1.0 | 0.5 | Amount of delay modulation |
| **Delay** | 1 ms - 30 ms | 10 ms | Base delay time |
| **Voices** | 1 - 4 | 2 | Number of chorus voices |
| **Stereo** | 0.0 - 1.0 | 0.8 | Stereo spread of voices |
| **Mix** | 0.0 - 1.0 | 0.5 | Dry/wet balance |

## How It Works

1. Input signal is copied to multiple delay lines
2. Each delay line has slightly different base delay
3. An internal LFO modulates each delay time
4. The varying delays create pitch shifts and phase differences
5. Mixed together, this creates the characteristic "ensemble" sound

### The Chorus Sound

The effect creates subtle beating and pitch variation:
- **Slow rate**: Gentle swaying, lush
- **Fast rate**: Vibrato-like, more intense
- **Light depth**: Subtle thickening
- **Heavy depth**: Dramatic wobble, almost out of tune

## Usage Tips

### Classic Analog Chorus

Warm, vintage sound:

```
Rate: 0.5 Hz
Depth: 0.4
Delay: 8 ms
Voices: 2
Mix: 0.5
```

### Subtle Thickening

Barely perceptible but adds depth:

```
Rate: 0.3 Hz
Depth: 0.2
Delay: 5 ms
Mix: 0.3
```

Good for vocals and solo instruments.

### Lush Ensemble

Rich, string-machine style:

```
Rate: 0.8 Hz
Depth: 0.6
Delay: 15 ms
Voices: 4
Stereo: 1.0
Mix: 0.6
```

Creates wide, dreamy textures.

### Bass Chorus

Keep low end intact:

```
Rate: 0.4 Hz
Depth: 0.3
Delay: 10 ms
Mix: 0.4
```

Lighter settings prevent the bass from getting muddy.

### Vibrato Mode

Extreme settings for pitch wobble:

```
Rate: 4 Hz
Depth: 0.8
Mix: 1.0 (100% wet)
```

At full wet, you hear only the modulated signal—pure vibrato.

### Leslie Speaker Simulation

For organ-like rotation:

```
Rate: Slow: 0.7 Hz / Fast: 6 Hz (modulate via Rate CV)
Depth: 0.7
Voices: 2
Stereo: 1.0
```

Switch between slow and fast for classic organ effect.

### Stereo Width Enhancement

Use chorus to widen a mono source:

```
[Mono Signal] ──> [Chorus] ──> [Stereo Output]
                  Stereo: 1.0
                  Mix: 0.4
```

The phase differences create stereo spread.

### Clean Guitar Chorus

Classic '80s clean tone:

```
Rate: 0.6 Hz
Depth: 0.5
Delay: 12 ms
Voices: 2
Mix: 0.5
```

### Synth Pad Enhancement

Make pads more interesting:

```
[Pad Oscillators] ──> [Chorus] ──> [Reverb] ──> [Output]
                      Rate: 0.4 Hz
                      Depth: 0.5
                      Voices: 4
```

The chorus adds movement before reverb smears it together.

### Modulating Rate

Create evolving chorus textures:

```
[LFO (very slow)] ──> [Chorus Rate CV]
```

The chorus speed itself changes over time.

## Voice Count Effects

| Voices | Character |
|--------|-----------|
| 1 | Simple, flanger-like |
| 2 | Classic stereo chorus |
| 3 | Richer, more complex |
| 4 | Full ensemble, thick |

More voices = thicker but potentially muddier sound.

## Delay Time Effects

| Delay | Character |
|-------|-----------|
| 1-5 ms | Tight, flanger territory |
| 5-15 ms | Classic chorus zone |
| 15-30 ms | Wide, ADT-like doubling |

## Connection Examples

### Basic Insert
```
[Synth] ──> [Chorus] ──> [Output]
```

### In Effects Chain
```
[Synth] ──> [Chorus] ──> [Delay] ──> [Reverb] ──> [Output]
```

Chorus before time-based effects is typical.

### Parallel Chorus
```
[Signal] ──> [Mixer Ch 1 (dry)]
         ──> [Chorus] ──> [Mixer Ch 2 (wet)]
         [Mixer] ──> [Output]
```

Blend dry and chorused signals independently.

### Modulated Ensemble
```
[LFO 1 (slow)] ──> [Chorus Rate CV]
[LFO 2] ──> [Chorus Depth CV]
[Synth] ──> [Chorus] ──> [Output]
```

## Chorus vs Similar Effects

| Effect | Character |
|--------|-----------|
| **Chorus** | Multiple voices, thickening |
| **Flanger** | Shorter delay, comb filtering, "jet" sound |
| **Phaser** | All-pass filters, hollow sweep |
| **Doubling** | Fixed short delay, no modulation |

## Tips

1. **Less is more**: Subtle chorus often works better than heavy
2. **Watch the bass**: Heavy chorus can muddy low frequencies
3. **Use stereo**: Chorus really shines in stereo
4. **Stack effects**: Chorus into reverb is magical
5. **Try extreme settings**: 100% wet creates interesting vibratos

## Related Modules

- [Delay](./delay.md) - For longer echo effects
- [Reverb](./reverb.md) - Combine for ambient textures
- [LFO](../modulation/lfo.md) - For rate modulation
- [Oscillator](../sources/oscillator.md) - PWM creates similar thickening
