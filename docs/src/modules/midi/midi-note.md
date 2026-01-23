# MIDI Note

**Module ID**: `midi.note`
**Category**: MIDI
**Header Color**: Magenta

![MIDI Note Module](../../images/module-midi-note.png)
*The MIDI Note module*

## Description

The MIDI Note module receives MIDI input from external devices (keyboards, controllers, DAWs) and converts it to CV and Gate signals. It's the bridge between the MIDI world and the modular CV/Gate paradigm.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **MIDI In** | MIDI (Purple) | MIDI data from external source (auto-connected) |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **V/Oct** | Control (Orange) | Pitch as 1V/octave CV |
| **Gate** | Gate (Green) | High during Note On, low on Note Off |
| **Velocity** | Control (Orange) | Note velocity (0.0 - 1.0) |
| **Aftertouch** | Control (Orange) | Channel aftertouch pressure |
| **Mod Wheel** | Control (Orange) | MIDI CC1 (Mod Wheel) |
| **Pitch Bend** | Control (Orange) | Pitch bend wheel (-1.0 to +1.0) |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **MIDI Channel** | 1-16 / Omni | Omni | Which MIDI channel to respond to |
| **Voice Mode** | Last/Low/High | Last | Note priority for monophonic mode |
| **Bend Range** | 0-24 semitones | 2 | Pitch bend range in semitones |
| **Velocity Curve** | Linear/Soft/Hard | Linear | Velocity response curve |

## How It Works

When MIDI data arrives:

1. **Note On**: Gate goes high, V/Oct updates to pitch, Velocity captures velocity
2. **Note Off**: Gate goes low (or when all keys released)
3. **Aftertouch**: Continuous pressure data
4. **Mod Wheel (CC1)**: Updates Mod Wheel output
5. **Pitch Bend**: Updates Pitch Bend output

### V/Oct Conversion

MIDI notes convert to V/Oct standard:
- MIDI note 60 (Middle C) = 0.0V
- MIDI note 72 (C5) = 1.0V (+1 octave)
- MIDI note 48 (C3) = -1.0V (-1 octave)
- Each semitone = 1/12 volt (0.0833...)

## Usage Tips

### Basic MIDI Connection

Connect to any MIDI device:

```
[MIDI Note V/Oct] ──> [Oscillator V/Oct]
[MIDI Note Gate] ──> [ADSR Gate]
[MIDI Note Velocity] ──> [VCA CV] (optional)
```

### Velocity-Sensitive Patch

Use velocity for expression:

```
[MIDI Note Velocity] ──> [Attenuverter] ──> [Filter Cutoff CV]
                     ──> [VCA CV]
```

Harder playing = louder and brighter.

### Pitch Bend

Add pitch bend to oscillator:

```
[MIDI Note V/Oct] ──────────────────────> [Oscillator V/Oct]
[MIDI Note Pitch Bend] ──> [Attenuverter] ──> [Oscillator FM]
```

Scale the pitch bend amount with the attenuverter.

### Mod Wheel Modulation

Use mod wheel for real-time control:

```
[MIDI Note Mod Wheel] ──> [LFO Depth] (vibrato amount)
                      ──> [Filter Cutoff CV]
                      ──> [Effect Parameter]
```

### Channel Selection

**Omni Mode**: Responds to all MIDI channels (default)
**Specific Channel (1-16)**: Only responds to that channel

Use specific channels when:
- Multiple MIDI devices are connected
- Splitting keyboard zones
- Receiving from a DAW with multiple tracks

### Voice Modes

When multiple keys are pressed (monophonic mode):

**Last**: Most recently pressed note takes priority
**Low**: Lowest note takes priority
**High**: Highest note takes priority

### Aftertouch Expression

If your MIDI controller supports aftertouch:

```
[MIDI Note Aftertouch] ──> [Filter Cutoff CV]
                       ──> [VCA CV]
                       ──> [Vibrato Depth]
```

Pressing harder after the initial attack adds modulation.

### Velocity Curves

| Curve | Response |
|-------|----------|
| **Linear** | 1:1 mapping, raw MIDI velocity |
| **Soft** | Easier to reach high velocities |
| **Hard** | Requires stronger playing for high velocities |

Choose based on your playing style and controller.

## MIDI Learn

For parameters that support MIDI Learn:

1. Right-click the parameter knob
2. Select "MIDI Learn"
3. Move the desired MIDI controller
4. The parameter is now mapped

## Connection Examples

### Complete Velocity-Sensitive Synth
```
[MIDI Note V/Oct] ──> [Oscillator V/Oct]
[MIDI Note Gate] ──> [ADSR] ──> [VCA CV]
[MIDI Note Velocity] ──> [Filter Cutoff CV]
                     ──> [ADSR Velocity Scale]
```

### With Pitch Bend and Mod Wheel
```
[MIDI Note V/Oct] ──> [Oscillator 1 V/Oct]
                  ──> [Oscillator 2 V/Oct]
[MIDI Note Pitch Bend] ──> [Mixer] ──> [Oscillator FM]
[MIDI Note Mod Wheel] ──> [LFO] ──> [Mixer]
```

### Multi-Timbral Setup
```
[MIDI Note (Ch 1)] ──> [Synth Voice 1]
[MIDI Note (Ch 2)] ──> [Synth Voice 2]
```

### Performance Controller
```
[MIDI Note Mod Wheel] ──> [Filter Cutoff CV]
[MIDI Note Aftertouch] ──> [Vibrato Amount]
[MIDI Note Pitch Bend] ──> [Pitch CV Offset]
```

## Troubleshooting

### No MIDI Input

1. Check MIDI device is connected and powered
2. Verify MIDI channel settings match
3. Check that the device is selected in system MIDI settings
4. Try "Omni" channel mode

### Wrong Pitch

1. Verify oscillator is calibrated to V/Oct
2. Check for octave offset settings
3. Ensure no unintended pitch modulation

### Stuck Notes

1. Check Gate connections
2. Send All Notes Off from your controller
3. Reload the patch

## Related Modules

- [Keyboard Input](./keyboard.md) - Computer keyboard alternative
- [MIDI Monitor](./midi-monitor.md) - Debug MIDI data
- [Oscillator](../sources/oscillator.md) - V/Oct destination
- [ADSR Envelope](../modulation/adsr.md) - Gate destination
