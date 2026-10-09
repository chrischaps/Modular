# Mixer

**Module ID** `util.mixer` · **Category** Utility

![Mixer Module](../../images/module-mixer.png)
*Four channel strips, a stereo field, a send to an effect, and a master.*

The Mixer brings up to four signals together into a stereo pair. Each channel has its own level, pan, mute and send. Use it to place sounds left and right, to pan one to and fro with an LFO, to open a polyphonic pad across the stereo field, to share one reverb among every channel, or to add two modulation sources into one CV. For more than four channels, chain Mixers: the stereo mix and the sends of one pass through the next.

It works on any signal that isn't MIDI. Audio and control signals both patch straight in, so the same module mixes sound or modulation.

## Reading the display

The display reads like a mixing desk: each channel is a column, from its light at the top down to its send knob.

- **The panorama**, at the top, is the stereo field from **L** to **R**, with a lane for each channel. A light shows where each channel sits. A polyphonic channel has a light per voice, fanned out by **Spread**. The lights brighten with the channel's level. A channel with its **Send** up blooms sideways along its lane, wider the more it sends, the way a sound spreads into a room. A hollow ring is a voice with nothing to play, and a grey ring is a muted channel.
- **The meters** show each channel's level after its fader. The pair on the right, over **Master**, is the stereo output. A notch either side of each meter marks full scale, and a white line holds the latest peak for a moment.
- **The M buttons** mute a channel. A muted channel's button turns red and its meter dims. Muting fades over 10 ms, so it doesn't click.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Ch 1** – **Ch 4** | Audio (Blue) | The channels. A polyphonic cable keeps its voices apart, for **Spread** |
| **Level 1** – **Level 4** | Control (Orange) | CV added to each channel's level |
| **Pan 1** – **Pan 4** | Control (Orange) | CV added to each channel's pan. An LFO here pans it to and fro |
| **Chain L**, **Chain R** | Audio (Blue) | A stereo mix added after the faders, such as another Mixer's **Out L** and **Out R**. **Chain R** copies **Chain L** when only the left is patched |
| **Chain Send L**, **Chain Send R** | Audio (Blue) | Another Mixer's **Send L** and **Send R**, added to this one's send bus |
| **Return L**, **Return R** | Audio (Blue) | An effect's output, added to the mix at the **Return** knob. It may come from an effect fed by this same Mixer. **Return R** copies **Return L** when only the left is patched |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Out** | Audio (Blue) | Mono sum of every channel at its level, ignoring pan and spread |
| **Out L** | Audio (Blue) | Left side of the stereo mix |
| **Out R** | Audio (Blue) | Right side of the stereo mix |
| **Send L**, **Send R** | Audio (Blue) | The send bus: each channel at its **Send** knob, after its fader and pan. Patch it to a reverb or delay |

## Parameters

| Control | Range | Default | Description |
|---------|-------|---------|-------------|
| **Lv 1** – **Lv 4** (Level) | 0 – 100% | 100% | Each channel's volume |
| **Pan 1** – **Pan 4** | L 100 – R 100 | C | Each channel's place in the stereo field |
| **M** (Mute 1 – 4) | on / off | off | Silences a channel |
| **Master** | −60 – +6 dB | 0 dB | Volume of **Out**, **Out L** and **Out R**. At −60 dB they are silent. The sends don't follow it |
| **Spread** | 0 – 100% | 0% | How far a polyphonic channel's voices fan out around its pan |
| **Send 1** – **Send 4** | 0 – 100% | 0% | How much of each channel goes to **Send L** and **Send R** |
| **Return** | 0 – 100% | 100% | Level of **Return L** and **Return R** in the mix |

Levels, pans, sends, **Return**, **Master** and **Spread** are smoothed, so you can ride them while the patch plays without clicks. A **Pan** knob's arc grows outward from the centre, so you can read which side a channel leans to at a glance. The readout says the same: **C**, or **L** or **R** and how far, out of 100.

## How it works

```text
Out L  = (Ch 1 × Level 1 × left(Pan 1) + … + Chain L + Return L × Return) × Master
Out R  = (Ch 1 × Level 1 × right(Pan 1) + … + Chain R + Return R × Return) × Master
Out    = (Ch 1 × Level 1 + … + Ch 4 × Level 4 + (Chain L + Chain R + …) × 0.71) × Master
Send L = Ch 1 × Level 1 × left(Pan 1) × Send 1 + … + Chain Send L
Send R = Ch 1 × Level 1 × right(Pan 1) × Send 1 + … + Chain Send R
```

### The pan law

Panning is **equal power**: as a channel moves from left to right, one side fades as the other rises, and their power always adds up to the same. A sound swept across the field keeps its loudness all the way. In the centre each side is at −3 dB. Hard left, the right side is silent.

So a channel panned centre is 3 dB quieter on each output than it is on **Out**. If a patch moves from **Out** to **Out L** and **Out R** and you want its old loudness back, set **Master** to +3 dB.

### Level and Pan CV

Both add to their knob. A **Level** CV can take a channel from silence to full, but never past 100% or below silence. A **Pan** CV of +1 moves a centred channel hard right, and a bipolar LFO swinging ±1 sweeps it from side to side. Set the **Pan** knob off centre and the LFO swings around that point instead, stopping at the edge.

A patched knob shows an orange dot and keeps its own setting. The CV moves the sound around it.

### Headroom

At the default levels, two full-scale signals add up to twice full scale. Anything within ±1 passes through untouched. Past that, each output soft-clips: the sum bends smoothly over and eases toward ±1.5 without ever reaching it, so two full-scale signals come out at about 1.48. The bend rounds off the peaks of loud audio, so when you mix loud sources and want them clean, bring the levels down to around 50–70% each.

The headroom above 1 is deliberate. Summing two envelopes for a filter's **Cutoff**, as the [Rhythmic Sequence](../../recipes/rhythmic-sequence.md) example does, opens the filter further than one envelope can.

### The mono Out

**Out** is the plain sum of the channels at their levels, times **Master**. Pan and **Spread** don't touch it. It's the same sum the Mixer gave when it had two mono channels, so patches built on it sound exactly as they did. Use **Out** when you're adding up CVs, or feeding one mono input.

The stereo pairs coming in on **Chain** and **Return** reach **Out** folded to one: left plus right, times 0.71. A centred sound comes back at the level it had before it was panned, and a hard-panned one at −3 dB.

### Sends and returns

Each channel's **Send** knob taps it after its fader, mute and pan, onto a stereo bus that leaves on **Send L** and **Send R**. Patch the bus into a reverb with its **Mix** at 100%, and bring the reverb back on **Return L** and **Return R**. One reverb then serves every channel, and each channel decides how much of the room it gets, as on a mixing console. A send follows its channel's fader, so a channel faded out leaves the room too.

The sends ignore **Master**: turning the mix down doesn't change what the effect hears. **Return** sets how loud the effect comes back.

**The return loop.** The send leaves the Mixer and the effect comes back into it, so the cables make a loop. Modules normally can't be patched in a loop, because each one needs its inputs before it can run. **Return L** and **Return R** are the exception. When their cable closes a loop, they hear the effect one audio block late: 256 samples, about 5.8 ms at 44.1 kHz and 5.3 ms at 48 kHz. For a reverb or a delay that's like a few more milliseconds of pre-delay, so take them off the reverb's **PreD** if the timing matters. A return fed from anywhere that isn't a loop is heard at once.

Patching a loop into any other input is still refused.

### Chaining Mixers

Two Mixers cascade like a console and its sidecar. Patch one Mixer's **Out L** and **Out R** into the next one's **Chain L** and **Chain R**, and its **Send L** and **Send R** into **Chain Send L** and **Chain Send R**. The first Mixer's stereo image passes through untouched, and its sends join the second's, so one reverb on the last Mixer serves all eight channels. The last Mixer's **Master** sets the volume of the whole mix.

**Chain L** works alone too: a mono source patched into it arrives centred, at its full level on each side.

### Polyphonic cables and Spread

The Mixer hears each voice of a polyphonic cable on its own. With **Spread** at 0% every voice sits at the channel's pan. That's the same as summing them first, so a whole polyphonic voice can go straight into a channel.

Turn **Spread** up and the voices fan out across the field around the pan. They take evenly spaced places, alternating left and right from the outside in: the first voice hard left, the second hard right, the third just inside the first, and so on. [Poly MIDI](../midi/poly-midi.md) hands notes to voices in turn, and neighbouring voices sit on opposite sides, so even two notes held on an eight-voice cable land either side of the pan, and a chord opens up instead of sitting in the middle. Each voice keeps the power it had, so the chord is as loud spread as it was centred.

A place belongs to a voice, not to a note. A melody played on a polyphonic cable moves from side to side as each note takes the next voice. That can sound lively. To keep a line steady, play it on a mono cable, or turn **Spread** down.

The Mixer's outputs carry one channel each. Effects after it hear a single stereo pair.

## Patches

### A wide pad

Hold a chord, and the Mixer spreads its voices across the field before the effects hear it:

```text
[VCA Out] ──> [Mixer Ch 1]          (VCA on a polyphonic voice, Spread 80%)
[Mixer Out L] ──> [Chorus In L]
[Mixer Out R] ──> [Chorus In R]
```

The [Lush Pad](../../recipes/lush-pad.md) example is built this way.

### Auto-pan

A slow LFO moves a sound from side to side:

```text
[Oscillator Out] ──> [Mixer Ch 1]
[LFO Out] ──> [Mixer Pan 1]         (LFO Bipolar on, 0.2 Hz)
[Mixer Out L] ──> [Audio Output Left]
[Mixer Out R] ──> [Audio Output Right]
```

Turn the LFO's **Rate** up to 4–6 Hz for a tremolo-like shimmer between the speakers.

### Placing parts

Give each part its own place, the way a band stands on a stage. Keep bass and kick in the middle, and move leads and pads out to the sides:

```text
[Bass VCA Out] ──> [Mixer Ch 1]     (Pan C)
[Lead VCA Out] ──> [Mixer Ch 2]     (Pan R 35)
[Pad VCA Out]  ──> [Mixer Ch 3]     (Pan L 30)
[Mixer Out L] ──> [Reverb In L]
[Mixer Out R] ──> [Reverb In R]
```

The [Afterglow](../../recipes/afterglow.md) example sets its arpeggio a little right of its pad, so the ping-pong echoes answer from the other side.

### Two modulation sources

A slow LFO and an envelope together: the filter follows each note and drifts as well.

```text
[LFO Out] ──> [Mixer Ch 1]          (Lv 1 around 30%)
[ADSR Out] ──> [Mixer Ch 2]
[Mixer Out] ──> [SVF Filter Cutoff]
```

### One room for every channel

Send each channel as much reverb as it needs, through one reverb:

```text
[Mixer Send L] ──> [Reverb In L]    (Reverb Mix at 100%)
[Mixer Send R] ──> [Reverb In R]
[Reverb Out L] ──> [Mixer Return L]
[Reverb Out R] ──> [Mixer Return R]
[Mixer Out L] ──> [Audio Output Left]
[Mixer Out R] ──> [Audio Output Right]
```

Turn **Send** up on the channels that should sit further back. A vocal-like lead might send 20%, a pad 50%, and a bass nothing at all.

### More than four channels

Chain Mixers. The first one's mix and sends pass through the second:

```text
[Mixer A Out L] ──> [Mixer B Chain L]
[Mixer A Out R] ──> [Mixer B Chain R]
[Mixer A Send L] ──> [Mixer B Chain Send L]
[Mixer A Send R] ──> [Mixer B Chain Send R]
[Mixer B Send L] ──> [Reverb In L]
[Mixer B Send R] ──> [Reverb In R]
[Reverb Out L] ──> [Mixer B Return L]
[Reverb Out R] ──> [Mixer B Return R]
[Mixer B Out L] ──> [Audio Output Left]
[Mixer B Out R] ──> [Audio Output Right]
```

The [Backbeat](../../recipes/backbeat.md) example mixes a seven-piece drum kit this way, with one room for the whole kit.

## Related modules

- [VCA](./vca.md): level control from a CV
- [Attenuverter](./attenuverter.md): scale, invert or offset a signal before mixing, such as an LFO before it reaches **Pan**
- [Audio Output](../output/audio-output.md): the final mix, with metering and a limiter
