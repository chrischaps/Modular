# Reverb

**Module ID**: `fx.reverb`
**Category**: Effects
**Header Color**: Purple

![Reverb Module](../../images/module-reverb.png)
*The Reverb module*

## Description

The Reverb simulates acoustic spaces by creating a dense wash of reflections that decay over time. From small rooms to vast halls, reverb places your sounds in a virtual environment and adds depth, dimension, and atmosphere.

### How it works

The Reverb is an 8-line **feedback delay network** (FDN):

1. **Pre-delay** holds the input back before anything else happens.
2. A four-step **diffuser** splits the sound across eight channels. Each step delays the channels by different amounts, flips some of them, and mixes them all together. One click becomes 8, then 64, then 512, then 4,096 echoes. These are the early reflections.
3. Eight **delay lines** circulate the sound. On every pass the lines are mixed into each other, so each echo spawns eight more and the tail turns into smooth, noise-like decay.
4. Each line has its own gain and a **damping filter**. Both are calculated from that line's length, so the whole tail decays at the rate set by Decay, and every line loses its highs at the same rate.
5. The lines **drift** slowly in length (the Mod knob). This keeps resonances from settling in and adds a gentle chorus to the tail.

Freeverb-style reverbs use a few fixed combs. They tend to ring at the comb lengths and sound metallic on sharp transients like snares and claps. The FDN's tail has no repeating period, so those transients spread into an even wash.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **In L** | Audio (Blue) | Left channel input |
| **In R** | Audio (Blue) | Right channel input (normalled to In L when unpatched) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Out L** | Audio (Blue) | Processed left channel |
| **Out R** | Audio (Blue) | Processed right channel |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Size** | 0% - 100% | 50% | Room size: scales every delay in the network |
| **Decay** | 0.1 s - 30 s | 2.0 s | Time for the tail to fall 60 dB |
| **Damp** | 0% - 100% | 50% | How much faster the highs die than the lows |
| **Mod** | 0% - 100% | 25% | Slow drift of the delay lines (chorus in the tail) |
| **PreD** | 0 ms - 100 ms | 0 ms | Time before the reverb begins |
| **Width** | 0% - 100% | 100% | Stereo width of the reverb |
| **Mix** | 0% - 100% | 30% | Dry/wet balance |

## Parameter Deep Dive

### Decay Time

Decay is the reverb time (RT60) in seconds: how long the tail takes to fall by 60 dB. A test measures it on rendered impulse responses and holds it to within 15% of the knob, at every Size. In practice the low and mid frequencies land within a few percent.

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
- **50-100 ms**: Distinct gap before the reverb, adds depth

Pre-delay helps maintain clarity: the attack of a sound comes through before the reverb does.

### Size

Size sets the room's dimensions. The shortest delay line runs from 12 ms at 0% to 100 ms at 100%, and the early reflections scale with it:

- **Small (0-30%)**: Tight, quick build-up, like a booth or small room
- **Medium (40-60%)**: Balanced, natural
- **Large (70-100%)**: Slow, spacious build-up, like a hall

Size and Decay are independent, so a small room can still ring for a long time. Turning Size while the tail plays bends its pitch briefly as the lines glide to their new lengths, like a tape effect.

### Damping

Damping simulates surfaces absorbing high frequencies. It sets how fast 4 kHz dies relative to the lows. At 0% it decays as long as Decay. At 100% it decays ten times faster. Low frequencies always keep the full Decay time.

- **Low (0-30%)**: Bright, reflective surfaces (tile, glass, plate)
- **Medium (40-60%)**: Balanced, natural decay
- **High (70-100%)**: Dark, absorbed sound (carpet, curtains)

Higher damping makes a darker reverb that's easier to mix.

### Mod

Mod lets each of the eight delay lines drift slowly in length, each at its own rate between 0.3 and 1.1 Hz. The early reflections are left alone.

- **0%**: Static. The most "accurate" setting, but long tails can show faint resonances.
- **15-35%**: Subtle movement that smooths long tails. This is the default range.
- **60-100%**: A noticeable chorus in the tail; lush on pads, seasick on pianos.

### Width

Controls stereo spread:

- **0%**: Mono reverb (centered)
- **50%**: Moderate stereo spread
- **100%**: Full stereo width. The two sides come from different delay lines, so they are almost completely uncorrelated.

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

### Moving Tail

Lush, slowly shifting pad wash:

```
Decay: 6 s
Size: 0.8
Damping: 0.4
Mod: 0.7
Mix: 0.5
```

A deep Mod setting adds a chorus that only lives in the tail, so the dry note stays steady.

### Small Bright Room

Tight, lively ambience for percussion:

```
Size: 0.15
Decay: 0.6 s
Pre-Delay: 0 ms
Damping: 0.2
Mod: 0.1
```

Short lines and little damping give a quick, splashy room.

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

| Space | Decay | Pre-Delay | Size | Damping | Mod |
|-------|-------|-----------|------|---------|-----|
| Closet | 0.2s | 0ms | 0.1 | 0.6 | 0.1 |
| Room | 0.8s | 10ms | 0.3 | 0.5 | 0.2 |
| Chamber | 1.5s | 20ms | 0.5 | 0.4 | 0.25 |
| Hall | 3.0s | 40ms | 0.7 | 0.5 | 0.3 |
| Cathedral | 6.0s | 80ms | 0.9 | 0.4 | 0.3 |
| Infinite | 20s+ | 100ms | 1.0 | 0.3 | 0.5 |

## Related Modules

- [Delay](./delay.md) - Discrete echoes vs diffuse reverb
- [Chorus](./chorus.md) - Thickening without space
- [EQ](./eq.md) - Shape reverb tone
- [VCA](../utilities/vca.md) - Control reverb level dynamically
