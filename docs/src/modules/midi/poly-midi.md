# Poly MIDI

**Module ID**: `input.poly_midi`
**Category**: Source
**Header Color**: Blue

## Description

Poly MIDI turns MIDI into **polyphonic** CV. Each note gets a voice of its own, and its Pitch, Gate and Velocity cables carry every voice at once, one channel per voice. Patch them into the polyphonic modules (Oscillator, filters, ADSR, VCA) and chords play as separate voices, each with its own envelope. See [Polyphony](../../concepts/polyphony.md).

It plays from the MIDI device chosen in the toolbar. While a Poly MIDI module is in the patch, the computer keyboard plays it too (the same keys as the [Keyboard Input](./keyboard.md) module), so you can play chords without a MIDI keyboard.

## Outputs

| Port | Signal Type | Channels | Description |
|------|-------------|----------|-------------|
| **Pitch** | Control (Orange) | Voices | V/Oct per voice, including pitch bend |
| **Gate** | Gate (Green) | Voices | High while the voice's key, or the sustain pedal, holds its note |
| **Velocity** | Control (Orange) | Voices | Each note's velocity (0.0 - 1.0) |
| **Aftertouch** | Control (Orange) | 1 | Channel pressure (0.0 - 1.0), shared by every voice |

## Parameters

| Control | Range | Default | Description |
|---------|-------|---------|-------------|
| **Ch** | Omni / 1-16 | Omni | Which MIDI channel to respond to |
| **Voices** | 1-8 | 8 | How many voices, and so channels, the cables carry |
| **Mode** | Rotate / Reuse | Rotate | How a new note picks its voice |
| **Oct** | -4 to +4 | 0 | Octave shift |
| **Bend** | 0-12 semitones | 2 | How far the pitch bend wheel bends every voice |

## Voice Allocation

**Rotate** gives each new note the next free voice in turn. A released note keeps ringing through its release while the next note sounds on another voice.

**Reuse** sends a note played again back to the voice that last played it, as a piano string is struck again. Other notes take the voice that has been free longest, so release tails get the most time.

Either way, a note played again while it still sounds (held by the sustain pedal, say) restarts on its own voice rather than taking a second one.

### Voice Stealing

When every voice is busy, a new note takes one over:

1. A voice the **sustain pedal** is holding (its key already up), oldest first
2. Otherwise, the **oldest** note still held

The taken voice's gate drops for one sample, so its envelope starts again for the new note.

## Sustain Pedal

The sustain pedal (CC 64) holds notes after their keys are released. Lifting it releases them all. All Notes Off (CC 123) and All Sound Off (CC 120) release everything, pedal or not.

## Example

```
[Poly MIDI Pitch] ──> [Oscillator V/Oct]
[Poly MIDI Gate] ──> [ADSR Gate]
[Poly MIDI Velocity] ──> [ADSR Velocity]
[Oscillator Out] ──> [Ladder Filter In] ──> [VCA In]
[ADSR Out] ──> [VCA CV]
[VCA Out] ──> [Audio Output Mono]
```

Hold a chord: each note has its own oscillator, filter and envelope. Let go of one note and only that note fades.

## Related Modules

- [MIDI Note](./midi-note.md) - Monophonic MIDI, with note priority
- [Keyboard Input](./keyboard.md) - Computer keyboard, monophonic
