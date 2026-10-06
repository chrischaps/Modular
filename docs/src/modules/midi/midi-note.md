# MIDI Note

**Module ID**: `input.midi_note`
**Category**: Source
**Header Color**: Blue

![MIDI Note Module](../../images/module-midi-note.png)
*The MIDI Note module*

## Description

The MIDI Note module receives MIDI from the device chosen in the toolbar (a keyboard, a controller, a DAW or sequencer) and converts it to CV and gate signals. It's the bridge between the MIDI world and the modular CV/Gate paradigm.

It is monophonic: when several keys are held, **Priority** decides which one sounds.

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Pitch** | Control (Orange) | V/Oct pitch, including pitch bend |
| **Gate** | Gate (Green) | High while any key is held |
| **Velocity** | Control (Orange) | The sounding note's velocity (0.0 - 1.0) |
| **Aftertouch** | Control (Orange) | Channel pressure (0.0 - 1.0) |

## Parameters

| Control | Range | Default | Description |
|---------|-------|---------|-------------|
| **Ch** | Omni / 1-16 | Omni | Which MIDI channel to respond to |
| **Priority** | Last / Low / High | Last | Which held key sounds |
| **Retrig** | Off / On | Off | Restart the gate when moving between held keys |
| **Oct** | -4 to +4 | 0 | Octave shift |
| **Bend** | 0-12 semitones | 2 | How far the pitch bend wheel bends |

## How It Works

1. **Note On**: Gate goes high and Pitch moves to the note, on the exact sample the note arrived
2. **Note Off**: Gate goes low once every key is released. Pitch stays on the last note, so the release tail stays in tune
3. **Pitch Bend**: Bends the Pitch output by up to ±Bend semitones, gliding over a few milliseconds so it never zippers
4. **Aftertouch**: Channel pressure appears on the Aftertouch output
5. **All Notes Off** (CC 123) and **All Sound Off** (CC 120) release every held key

Switching or disconnecting the MIDI device releases any notes still held, so nothing sticks.

### Timing

MIDI is handled on the audio thread. Each message is stamped the moment it arrives and placed at the matching sample of the next audio buffer. Every note is delayed by the same amount (one audio buffer, a few milliseconds), so a steady sequence from a DAW or sequencer plays back steady, with no jitter. Even the shortest note produces a gate.

### V/Oct Conversion

MIDI notes convert to V/Oct standard:
- MIDI note 60 (Middle C) = 0.0V
- MIDI note 72 (C5) = 1.0V (+1 octave)
- MIDI note 48 (C3) = -1.0V (-1 octave)
- Each semitone = 1/12 volt (0.0833...)

## Usage Tips

### Basic MIDI Connection

Choose your device from the MIDI menu in the toolbar, then patch:

```
[MIDI Note Pitch] ──> [Oscillator V/Oct]
[MIDI Note Gate] ──> [ADSR Gate]
[MIDI Note Velocity] ──> [ADSR Velocity] (optional)
```

### Velocity-Sensitive Patch

Use velocity for expression:

```
[MIDI Note Velocity] ──> [Attenuverter] ──> [Filter Cutoff CV]
                     ──> [ADSR Velocity]
```

Harder playing = louder and brighter.

### Channel Selection

**Omni**: Responds to all MIDI channels (default)
**Specific Channel (1-16)**: Only responds to that channel

Use specific channels when:
- Splitting keyboard zones
- Receiving from a DAW with multiple tracks
- Running two MIDI Note modules as two separate voices

### Priority

When more than one key is held:

**Last**: Most recently pressed key sounds
**Low**: Lowest key sounds (classic for basslines)
**High**: Highest key sounds (classic for leads)

Releasing the sounding key falls back to another held key, so you can trill against a held note.

### Retrigger

With **Retrig** off, playing legato (pressing a new key before releasing the old one) changes the pitch but keeps the gate high, so the envelope carries on: smooth, connected lines.

With **Retrig** on, each legato note drops the gate for one sample, so envelopes start again on every note.

### Aftertouch Expression

If your MIDI controller supports aftertouch:

```
[MIDI Note Aftertouch] ──> [Filter Cutoff CV]
                       ──> [VCA CV]
                       ──> [Vibrato Depth]
```

Pressing harder after the initial attack adds modulation.

## MIDI Learn

Knobs on other modules can follow a MIDI CC (a mod wheel, a fader):

1. Right-click the parameter knob
2. Select "MIDI Learn"
3. Move the desired MIDI controller
4. The parameter is now mapped

## Connection Examples

### Complete Velocity-Sensitive Synth
```
[MIDI Note Pitch] ──> [Oscillator V/Oct]
[MIDI Note Gate] ──> [ADSR] ──> [VCA CV]
[MIDI Note Velocity] ──> [Filter Cutoff CV]
                     ──> [ADSR Velocity]
```

### Two Oscillators, One Keyboard
```
[MIDI Note Pitch] ──> [Oscillator 1 V/Oct]
                  ──> [Oscillator 2 V/Oct]
```

### Multi-Timbral Setup
```
[MIDI Note (Ch 1)] ──> [Synth Voice 1]
[MIDI Note (Ch 2)] ──> [Synth Voice 2]
```

## Troubleshooting

### No MIDI Input

1. Check MIDI device is connected and powered
2. Check the device is selected in the toolbar's MIDI menu. On Windows, a device another app has open (a DAW, a browser) can't be opened here too
3. Verify the Ch setting matches what your device sends, or try Omni
4. Press Play: MIDI only sounds while the patch is playing

### Wrong Pitch

1. Check Oct, and that the pitch bend wheel is centred
2. Ensure no unintended pitch modulation

### Stuck Notes

1. Send All Notes Off from your controller
2. Select the MIDI device again, which releases every held note

## Related Modules

- [Keyboard Input](./keyboard.md) - Computer keyboard alternative
- [MIDI Monitor](./midi-monitor.md) - Debug MIDI data
- [Oscillator](../sources/oscillator.md) - V/Oct destination
- [ADSR Envelope](../modulation/adsr.md) - Gate destination
