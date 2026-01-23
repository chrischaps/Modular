# Reverb

**Module ID**: `fx.reverb`
**Category**: Effects
**Header Color**: Purple

![Reverb Module](../../images/module-reverb.png)
*The Reverb module*

## Description

The Reverb simulates acoustic spaces by creating a dense wash of reflections that decay over time. From small rooms to vast halls, reverb places your sounds in a virtual environment and adds depth, dimension, and atmosphere.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left In** | Audio (Blue) | Left channel input |
| **Right In** | Audio (Blue) | Right channel input (normalled to Left) |
| **Decay CV** | Control (Orange) | Modulation input for decay time |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left Out** | Audio (Blue) | Processed left channel |
| **Right Out** | Audio (Blue) | Processed right channel |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Decay** | 0.1 s - 30 s | 2.0 s | Reverb tail length |
| **Pre-Delay** | 0 ms - 200 ms | 20 ms | Time before reverb begins |
| **Size** | 0.0 - 1.0 | 0.5 | Room size (affects character) |
| **Damping** | 0.0 - 1.0 | 0.5 | High-frequency absorption |
| **Width** | 0.0 - 1.0 | 1.0 | Stereo width |
| **Mix** | 0.0 - 1.0 | 0.3 | Dry/wet balance |

## Parameter Deep Dive

### Decay Time

How long the reverb tail lasts:

| Decay | Space Type |
|-------|------------|
| 0.1-0.5 s | Small room, tight space |
| 0.5-1.5 s | Medium room, studio |
| 1.5-3.0 s | Large hall, church |
| 3.0-10 s | Cathedral, warehouse |
| 10+ s | Infinite/ambient |

### Pre-Delay

Gap between dry signal and reverb onset:

- **0-10 ms**: Reverb starts immediately, sound feels close
- **20-40 ms**: Natural separation, clear attack
- **50-100 ms**: Distinct echo before reverb, adds depth
- **100+ ms**: Noticeable gap, special effect

Pre-delay helps maintain clarity—the attack of sounds comes through before reverb.

### Size

Affects the density and character of reflections:

- **Small (0.1-0.3)**: Dense, colored reflections
- **Medium (0.4-0.6)**: Balanced, natural
- **Large (0.7-1.0)**: Sparse, spacious reflections

### Damping

Simulates absorption of high frequencies by surfaces:

- **Low (0.0-0.3)**: Bright, reflective surfaces (tile, glass)
- **Medium (0.4-0.6)**: Balanced, natural decay
- **High (0.7-1.0)**: Dark, absorbed sound (carpet, curtains)

Higher damping = darker reverb that's easier to mix.

### Width

Controls stereo spread:

- **0.0**: Mono reverb (centered)
- **0.5**: Moderate stereo spread
- **1.0**: Full stereo width

## Usage Tips

### Vocal Reverb

Clear, present vocals:

```
Decay: 1.5 s
Pre-Delay: 40 ms
Size: 0.5
Damping: 0.6
Mix: 0.2
```

Pre-delay separates the vocal from reverb; damping prevents harshness.

### Drums/Percussion

Tight, punchy room:

```
Decay: 0.5 s
Pre-Delay: 10 ms
Size: 0.3
Damping: 0.5
Mix: 0.25
```

Short decay keeps rhythm tight.

### Synth Pad

Lush, expansive atmosphere:

```
Decay: 4.0 s
Pre-Delay: 50 ms
Size: 0.8
Damping: 0.4
Mix: 0.5
```

Long decay and large size create immersive space.

### Ambient/Experimental

Infinite shimmer:

```
Decay: 15+ s
Pre-Delay: 100 ms
Size: 1.0
Damping: 0.3 (bright)
Mix: 0.7
```

Creates evolving, self-sustaining textures.

### Gated Reverb

80s drum sound (requires gate module):

```
[Drums] ──> [Reverb] ──> [Gate] ──> [Output]
           Decay: 3s    Threshold: 0.3
           Mix: 100%    Attack: 0ms
                        Hold: 200ms
                        Release: 50ms
```

Reverb is cut short by gate for dramatic effect.

### Send/Return Setup

Process multiple sources through one reverb:

```
[Synth 1] ──(send)──┐
[Synth 2] ──(send)──┼──> [Reverb (Mix: 100%)] ──> [Return Mixer]
[Drums]   ──(send)──┘
```

More efficient and creates cohesive space.

### Decay Modulation

Evolving reverb character:

```
[LFO (slow)] ──> [Reverb Decay CV]
```

Reverb time breathes and changes over time.

### Spring Reverb Character

For vintage spring-like sound:

```
Size: 0.2 (small)
Decay: 1.5 s
Pre-Delay: 0 ms
Damping: 0.3
```

Small size creates metallic, sproingy character.

### Plate Reverb Character

Classic studio plate:

```
Size: 0.5
Decay: 2.0 s
Pre-Delay: 0 ms
Damping: 0.4
Width: 1.0
```

Dense, smooth decay.

## Mix Positioning

Reverb level affects perceived distance:

| Mix | Perception |
|-----|------------|
| 10-20% | Close, present, intimate |
| 20-40% | Natural room distance |
| 40-60% | Far away, spacious |
| 60-100% | Distant, atmospheric, effect |

## Connection Examples

### Insert Effect
```
[Synth] ──> [Reverb] ──> [Output]
```

### Send/Return
```
[Mixer] ──Send──> [Reverb (Mix: 100%)]
[Reverb] ──Return──> [Mixer]
```

### Reverb into Delay
```
[Audio] ──> [Reverb] ──> [Delay] ──> [Output]
```

Creates rhythmic echoes of the reverb tail.

### Sidechain Reverb
```
[Audio] ──> [Reverb] ──> [VCA] ──> [Output]
[Audio] ──> [Envelope Follower] ──> [Inverted] ──> [VCA CV]
```

Reverb ducks when dry signal is present.

## Space Presets

| Space | Decay | Pre-Delay | Size | Damping |
|-------|-------|-----------|------|---------|
| Closet | 0.2s | 0ms | 0.1 | 0.6 |
| Room | 0.8s | 10ms | 0.3 | 0.5 |
| Chamber | 1.5s | 20ms | 0.5 | 0.4 |
| Hall | 3.0s | 40ms | 0.7 | 0.5 |
| Cathedral | 6.0s | 80ms | 0.9 | 0.4 |
| Infinite | 20s+ | 100ms | 1.0 | 0.3 |

## Related Modules

- [Delay](./delay.md) - Discrete echoes vs diffuse reverb
- [Chorus](./chorus.md) - Thickening without space
- [EQ](./eq.md) - Shape reverb tone
- [VCA](../utilities/vca.md) - Control reverb level dynamically
