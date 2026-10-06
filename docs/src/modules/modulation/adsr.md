# ADSR Envelope

**Module ID**: `mod.adsr`
**Category**: Modulation
**Header Color**: Orange

![ADSR Envelope Module](../../images/module-adsr.png)
*The ADSR Envelope module*

## Description

The ADSR Envelope generates a control signal that shapes how a sound evolves over time. When triggered by a gate signal (like pressing a key), it produces a predictable voltage curve through four stages: Attack, Decay, Sustain, and Release.

Every stage takes exactly the time on its knob: a 100 ms release is silent 100 ms after you let go. Each stage's **Curve** sets its shape, from a straight line to a deep analog curve, and the display on the node draws the same curve the audio follows.

Envelopes are essential for:
- Controlling amplitude (volume shape) via VCA
- Modulating filter cutoff for timbral changes
- Adding dynamic movement to any parameter

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Gate** | Gate (Green) | Trigger input. Rising edge starts Attack, falling edge starts Release |
| **Retrig** | Gate (Green) | Retrigger input. While the gate is held, a rising edge restarts Attack from the current level |
| **Velocity** | Control (Orange) | Note velocity (0.0 to 1.0), read at each note on. Scales the peak; see **Vel** |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Out** | Control (Orange) | Envelope output (0.0 to 1.0) |

## Parameters

The knobs sit in two rows: the times and level on top, and under each time the curve of that stage.

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Atk** (Attack) | 1 ms - 10 s | 10 ms | Time to rise from the current level to the peak |
| **Dec** (Decay) | 1 ms - 10 s | 100 ms | Time to fall from the peak to the Sustain level |
| **Sus** (Sustain) | 0.0 - 1.0 | 0.7 | Level held while the gate is high, as a fraction of the peak |
| **Rel** (Release) | 1 ms - 10 s | 300 ms | Time to fall from the current level to 0 after the gate goes low |
| **A Crv** (Attack Curve) | 0 - 100% | 20% | Attack shape: 0% is a straight ramp, higher bows it outward |
| **D Crv** (Decay Curve) | 0 - 100% | 50% | Decay shape: 0% is a straight ramp, higher drops fast then eases in |
| **Vel** (Velocity Amount) | 0 - 100% | 50% | How much the Velocity input scales the peak |
| **R Crv** (Release Curve) | 0 - 100% | 50% | Release shape: 0% is a straight ramp, higher drops fast then trails off |

## Envelope Stages

![ADSR Diagram](../../images/envelope-adsr-diagram.png)
*The four stages of an ADSR envelope*

### Attack

The **Attack** phase begins when the gate goes high (key pressed). The envelope rises from where it is (0, or wherever a release had got to) to its peak, in exactly the Attack time.

- **Short attack** (1-10 ms): Instant, percussive start (drums, plucks)
- **Medium attack** (10-100 ms): Soft start (strings, pads)
- **Long attack** (100 ms+): Gradual swell (ambient, swells)

### Decay

The **Decay** phase begins the moment Attack reaches the peak. The envelope falls from the peak to the Sustain level in exactly the Decay time.

- **Short decay** (10-50 ms): Percussive, plucky sounds
- **Medium decay** (50-200 ms): Piano-like sounds
- **Long decay** (200 ms+): Smooth, gradual transition

### Sustain

The **Sustain** phase holds a level while the gate remains high (key held). Unlike the other parameters (which are times), Sustain is a **level** from 0.0 to 1.0. Turning it while a note is held glides to the new level over a few milliseconds instead of jumping, so it never clicks.

- **Low sustain** (0.0-0.3): Percussive, the sound dies away while key is held
- **Medium sustain** (0.3-0.7): Balanced, natural decay to held level
- **High sustain** (0.7-1.0): Full, organ-like sustained sound

### Release

The **Release** phase begins when the gate goes low (key released). The envelope falls from the current level to 0 in exactly the Release time, even if the key was let go mid-attack.

- **Short release** (10-50 ms): Abrupt stop, staccato
- **Medium release** (50-300 ms): Natural fade
- **Long release** (300 ms+): Lingering, ambient tails

## Curves

Each stage is an analog-style RC curve aimed *past* its target, so that it lands on time instead of creeping toward the target forever. The Curve knob sets how far past:

- **0%**: aimed far past, so the stage is a straight line.
- **20%** (Attack default): aimed about 15% past the peak. The rise is nearly straight, which is what makes an analog attack sound punchy.
- **50%** (Decay and Release default): a classic RC fall. It drops quickly, then eases into its target, which suits how the ear hears loudness.
- **100%**: a deep curve. Most of the change happens in the first fifth of the stage, then a long tail.

Curves change only the *shape* of a stage. Its time stays exactly the knob's.

## Velocity

Patch a velocity source (the **Velocity** output of [Keyboard Input](../midi/keyboard.md) or a MIDI Note module) into **Velocity**, and the **Vel** knob sets how much it matters:

- **Vel 0%**: every note peaks at 1.0.
- **Vel 50%**: the softest note peaks at 0.5 and the hardest at 1.0.
- **Vel 100%**: the peak equals the velocity.

Sustain is a fraction of the peak, so a soft note sustains lower too. With velocity patched, the node's display draws a faint second envelope for the softest note. The gap between the two lines is your dynamic range.

With nothing patched into **Velocity**, every note peaks at 1.0, whatever the knob says.

## Usage Tips

### Basic Volume Envelope

Connect envelope to VCA for note-shaped volume:

```
[Keyboard] ──Gate──> [ADSR] ──> [VCA CV]
[Oscillator] ──> [VCA In] ──> [Output]
```

### Filter Envelope

Create dynamic timbral changes:

```
[Keyboard] ──Gate──> [ADSR] ──> [Filter Cutoff CV]
```

- Fast attack + fast decay = "plucky" brightness
- Slow attack = gradual brightening
- Combine with VCA envelope for complex shapes

### Dual Envelopes

Use separate envelopes for amplitude and filter:

```
[Keyboard Gate] ──> [ADSR 1] ──> [VCA CV] (volume shape)
                ──> [ADSR 2] ──> [Filter CV] (timbre shape)
```

This allows independent control:
- VCA envelope: Long release for sustained notes
- Filter envelope: Short decay for initial brightness

### Velocity to Brightness

Patch velocity into the filter envelope only, so harder notes are brighter but no louder:

```
[Keyboard] ──Gate──────> [ADSR 1] ──> [VCA CV]
           ──Gate──────> [ADSR 2] ──> [Filter Cutoff CV]
           ──Velocity──> [ADSR 2 Velocity]
```

### Inverted Envelope

Run the envelope through an [Attenuverter](../utilities/attenuverter.md) set to a negative amount for "reversed" modulation:

```
[ADSR] ──> [Attenuverter (−)] ──> [Filter Cutoff CV]
```

- Filter closes as the note opens, and opens as it releases
- Creates unusual, "backwards" effects

### Retrigger Behavior

While the gate is held, the **Retrig** input restarts Attack from the current level:

```
[Clock] ──> [ADSR Retrig]
```

- Creates rhythmic re-articulation of a held note
- Useful for tremolo-like effects
- Each trigger restarts the Attack phase

## Common Envelope Shapes

### Pluck (Piano, Guitar)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 1 ms | 200 ms | 0.0 | 100 ms |

Fast attack, immediate decay to silence, short release. Push D Crv toward 80% for a sharper snap.

### Pad (Strings, Ambient)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 500 ms | 1 s | 0.7 | 2 s |

Slow attack, gradual decay, sustained level, long release. A Crv near 0% gives an even swell.

### Organ (Sustained)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 1 ms | 10 ms | 1.0 | 50 ms |

Instant attack, no decay, full sustain, quick release.

### Brass (Soft Attack)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 100 ms | 300 ms | 0.8 | 200 ms |

Moderate attack (breath), slight decay, high sustain.

### Percussion (Drum, Pluck)
| Attack | Decay | Sustain | Release |
|--------|-------|---------|---------|
| 1 ms | 100 ms | 0.0 | 50 ms |

Instant attack, quick decay, no sustain.

## Connection Examples

### Standard Synth Voice
```
[Keyboard] ──V/Oct──> [Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
           ──Gate───> [ADSR 1] ─────────────────────┘
                      [ADSR 2] ──> [Filter Cutoff CV]
```

### Triggered Drone
```
[Clock] ──> [ADSR Gate]
            [ADSR] ──> [VCA CV]
[Oscillator] ──> [VCA] ──> [Output]
```

### Envelope Following
```
[ADSR] ──> [Attenuverter] ──> [Multiple Destinations]
```

## Related Modules

- [VCA](../utilities/vca.md) - Control amplitude with envelope
- [SVF Filter](../filters/svf-filter.md) - Modulate filter with envelope
- [Keyboard Input](../midi/keyboard.md) - Gate and velocity source for envelope
- [LFO](./lfo.md) - Alternative modulation source
