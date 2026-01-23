# Keyboard Input

**Module ID**: `midi.keyboard`
**Category**: MIDI
**Header Color**: Magenta

![Keyboard Input Module](../../images/module-keyboard.png)
*The Keyboard Input module*

## Description

The Keyboard Input module converts computer keyboard presses into CV and Gate signals, allowing you to play synthesizer patches without external MIDI hardware. It's the quickest way to test sounds and play melodies.

## Inputs

*This module has no inputs—it generates signals from keyboard events.*

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **V/Oct** | Control (Orange) | Pitch control voltage (1V per octave) |
| **Gate** | Gate (Green) | High while key is pressed |
| **Velocity** | Control (Orange) | Fixed velocity output (can be adjusted) |
| **Aftertouch** | Control (Orange) | Simulated aftertouch (if supported) |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Octave** | -2 to +4 | 0 | Octave offset for keyboard mapping |
| **Velocity** | 0.0 - 1.0 | 0.8 | Fixed velocity output level |
| **Glide** | 0 ms - 1000 ms | 0 ms | Portamento/glide time between notes |

## Keyboard Layout

The computer keyboard is mapped to a piano-style layout:

### Lower Row (Z to M) - Lower Octave
```
| Z | X | C | V | B | N | M |
| C | D | E | F | G | A | B |

| S | D |   | G | H | J |   |
| C#| D#|   | F#| G#| A#|   |
```

### Upper Row (Q to P) - Higher Octave
```
| Q | W | E | R | T | Y | U | I | O | P |
| C | D | E | F | G | A | B | C | D | E |

| 2 | 3 |   | 5 | 6 | 7 |   | 9 | 0 |   |
| C#| D#|   | F#| G#| A#|   | C#| D#|   |
```

### Octave Controls
- **Number keys (or dedicated keys)**: Change octave
- **Octave knob**: Set base octave offset

## Usage Tips

### Basic Synth Playing

Connect to an oscillator for instant playability:

```
[Keyboard V/Oct] ──> [Oscillator V/Oct]
[Keyboard Gate] ──> [ADSR Gate]
```

### Monophonic vs Polyphonic

The Keyboard module is **monophonic**—only the most recent key press is active. For polyphonic playing, you'll need multiple keyboard modules or a polyphonic MIDI setup.

### Using Glide/Portamento

Set Glide > 0 for smooth pitch transitions:

```
Glide: 100 ms (subtle slide)
Glide: 300 ms (noticeable portamento)
Glide: 500+ ms (dramatic slide)
```

The V/Oct output smoothly transitions between notes instead of jumping.

### Velocity for Dynamics

Connect Velocity output to create expressive patches:

```
[Keyboard Velocity] ──> [VCA CV] (louder at higher velocity)
                    ──> [Filter Cutoff CV] (brighter at higher velocity)
```

Since computer keyboards don't have velocity sensitivity, adjust the Velocity parameter manually or use a MIDI controller.

### Octave Switching

Use the Octave parameter to shift the entire keyboard up or down:

- **-2**: Very low bass notes
- **-1**: Bass range
- **0**: Middle C centered
- **+1**: Higher register
- **+2 to +4**: High leads

### Playing Techniques

**Legato**: Hold one key while pressing another—gate stays high, only pitch changes.

**Staccato**: Quick key presses for short notes.

**Trills**: Rapidly alternate between two adjacent keys.

### Focus and Keyboard Capture

The module receives keyboard input when the application window is focused. Some tips:

- Click on the canvas to ensure focus
- Modifier keys (Ctrl, Alt, Shift) may not be captured
- Some shortcuts may conflict with application functions

## Connection Examples

### Simple Monosynth
```
[Keyboard V/Oct] ──> [Oscillator V/Oct]
[Keyboard Gate] ──> [ADSR] ──> [VCA CV]
[Oscillator] ──> [Filter] ──> [VCA] ──> [Output]
```

### Velocity-Sensitive Patch
```
[Keyboard V/Oct] ──> [Oscillator V/Oct]
[Keyboard Gate] ──> [ADSR Gate]
[Keyboard Velocity] ──> [Attenuverter] ──> [Filter Cutoff CV]
[Keyboard Velocity] ──> [VCA Level CV]
```

### Portamento Lead
```
[Keyboard V/Oct] ──> [Oscillator V/Oct]
         (Glide: 200ms)
```

### Dual Oscillator Tracking
```
[Keyboard V/Oct] ──> [Oscillator 1 V/Oct]
                 ──> [Oscillator 2 V/Oct]
```

Both oscillators track the keyboard pitch.

## Keyboard Shortcuts

| Key | Function |
|-----|----------|
| Z-M | Lower octave (C-B) |
| A-J | Lower octave sharps/flats |
| Q-P | Upper octave (C-E) |
| 2-0 | Upper octave sharps/flats |
| Octave buttons | Shift octave range |

## Comparison with MIDI Note

| Feature | Keyboard | MIDI Note |
|---------|----------|-----------|
| Input source | Computer keyboard | MIDI device |
| Velocity | Fixed | True velocity |
| Aftertouch | Simulated | Real (if supported) |
| Polyphony | Mono | Depends on mode |
| Setup required | None | MIDI connection |

Use Keyboard for quick testing; use MIDI Note for performance.

## Tips

1. **Start with Keyboard**: Test patches quickly before setting up MIDI
2. **Use Glide**: Makes simple patches more expressive
3. **Mind the focus**: Click the window if keys aren't responding
4. **Combine with MIDI**: Use Keyboard for development, MIDI Note for performance

## Related Modules

- [MIDI Note](./midi-note.md) - External MIDI input
- [Oscillator](../sources/oscillator.md) - Primary V/Oct destination
- [ADSR Envelope](../modulation/adsr.md) - Gate destination
- [VCA](../utilities/vca.md) - Velocity destination
