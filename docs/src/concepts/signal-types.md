# Signal Types

Modular Synth uses a type system for signals that helps you understand what kind of data flows through each connection. Each signal type has a distinctive color, making it easy to trace the flow of audio, control, gate, and MIDI signals through your patch.

## Overview

| Type | Color | Range | Primary Use |
|------|-------|-------|-------------|
| **Audio** | Blue | -1.0 to 1.0 | Sound signals |
| **Control** | Orange | 0.0 to 1.0 (unipolar) or -1.0 to 1.0 (bipolar) | Modulation, CV |
| **Gate** | Green | 0.0 or 1.0 | Triggers, on/off states |
| **MIDI** | Purple | Structured data | Note/CC messages |

![Signal Types](../images/signal-types-overview.png)
*The four signal types with their colors*

---

## Audio Signals

**Color: Blue**

Audio signals carry the actual sound you hear. They oscillate rapidly (typically 20 Hz to 20 kHz) and represent the waveform that will be sent to your speakers.

### Characteristics

- **Range**: -1.0 to 1.0 (bipolar)
- **Sample Rate**: Matches your audio interface (typically 44.1 kHz or 48 kHz)
- **Bandwidth**: Full audio spectrum

### Common Sources

- Oscillators (all waveforms)
- Filter outputs
- Effect outputs
- Sample playback

### Common Destinations

- Filter inputs
- Effect inputs
- VCA inputs
- Audio Output module

### Signal Level

Audio signals should stay within the -1.0 to 1.0 range to avoid clipping (distortion). The Audio Output module includes a limiter to prevent harsh digital clipping, but it's best to manage levels throughout your patch.

---

## Control Signals

**Color: Orange**

Control signals (also called CV or Control Voltage) carry slower-moving data used to modulate parameters. They don't produce sound directly but shape and control other modules.

### Characteristics

- **Unipolar Range**: 0.0 to 1.0 (e.g., envelope output, LFO with offset)
- **Bipolar Range**: -1.0 to 1.0 (e.g., bipolar LFO)
- **Bandwidth**: Typically low frequency (< 100 Hz), but can be audio rate

### Common Sources

- Envelopes (ADSR)
- LFOs
- Sequencers
- MIDI CC (converted to CV)
- Attenuverters

### Common Destinations

- Filter cutoff
- Oscillator frequency (FM)
- VCA CV input
- Effect parameters
- Any "modulatable" parameter

### Unipolar vs. Bipolar

**Unipolar (0.0 to 1.0)**:
- Always positive
- Good for controlling parameters that shouldn't go negative
- Examples: envelope output, volume control

**Bipolar (-1.0 to 1.0)**:
- Swings positive and negative
- Good for vibrato, filter sweeps that go both ways
- Examples: LFO output, pitch modulation

### V/Oct (Volts per Octave)

A special control signal convention where each 1.0 increase represents one octave up in pitch. This allows precise musical pitch control:

- 0.0 = Base frequency (e.g., C0)
- 1.0 = One octave up (C1)
- 2.0 = Two octaves up (C2)
- 0.5 = Half octave up (F#0)
- -1.0 = One octave down (C-1)

The Keyboard and MIDI Note modules output V/Oct signals for controlling oscillator pitch.

---

## Gate Signals

**Color: Green**

Gate signals are binary on/off signals used for triggering events. Unlike audio or control signals that vary continuously, gates are either fully on (1.0) or fully off (0.0).

### Characteristics

- **Range**: 0.0 (off) or 1.0 (on)
- **Transitions**: Rising edge (0→1) and falling edge (1→0)
- **Duration**: The time the gate stays high

### Gate vs. Trigger

While both use the green color, there's a conceptual difference:

**Gate**: Stays high for a duration (like holding a key)
- Used for: Envelope gate input, held notes

**Trigger**: Brief pulse (like a drum hit)
- Used for: Clock pulses, one-shot events

Most modules respond appropriately to both.

### Common Sources

- Keyboard/MIDI Note (key pressed/released)
- Clock modules (rhythmic pulses)
- Sequencers (step triggers)
- LFOs in square wave mode

### Common Destinations

- Envelope gate input
- Sample & Hold trigger
- Sequencer clock input
- Any module that responds to triggers

### Edge Detection

Some modules respond to:
- **Rising edge**: The moment gate goes from 0 to 1
- **Falling edge**: The moment gate goes from 1 to 0
- **Gate high**: While the gate is 1
- **Gate low**: While the gate is 0

For example, an ADSR envelope:
- Begins Attack on rising edge
- Enters Release on falling edge

---

## MIDI Signals

**Color: Purple**

MIDI signals carry structured musical data including note events, control changes, and other MIDI messages. Unlike the other signal types which are continuous values, MIDI signals contain discrete events.

### Characteristics

- **Format**: Structured messages (Note On/Off, CC, etc.)
- **Data**: Note number, velocity, channel, CC values
- **Timing**: Event-based rather than continuous

### Common Sources

- MIDI Note module (from external MIDI devices)
- Keyboard module (from computer keyboard)

### Common Destinations

- MIDI Monitor (for debugging)
- Modules that accept MIDI input directly

### MIDI to CV Conversion

Most modules don't work with MIDI directly. The MIDI Note module converts MIDI to:

- **V/Oct**: Note number → pitch CV
- **Gate**: Note On/Off → gate signal
- **Velocity**: Note velocity → control signal

This conversion allows standard synthesis modules to respond to MIDI input.

---

## Signal Type Compatibility

### Automatic Conversion

Some connections perform automatic conversion:

| From | To | Conversion |
|------|-----|------------|
| Audio | Control | Treated as control signal |
| Control | Audio | Treated as audio (modulation) |
| Gate | Control | 0.0 or 1.0 control value |
| Control | Gate | Threshold at 0.5 |

### Best Practices

While some conversions work, it's best to match signal types:

1. **Audio to audio**: Full bandwidth sound processing
2. **Control to control**: Modulation and CV routing
3. **Gate to gate**: Trigger and timing signals
4. **MIDI to MIDI modules**: Then convert to CV

### Audio-Rate Modulation

Control signals can run at audio rate for special effects:

- **FM Synthesis**: Audio-rate modulation of oscillator frequency
- **Ring Modulation**: Audio-rate amplitude modulation
- **Filter FM**: Audio-rate cutoff modulation for unusual timbres

---

## Visual Identification

### Port Colors

Input and output ports are colored to indicate the expected signal type:

![Port Colors](../images/signal-port-colors.png)
*Ports showing their signal type colors*

### Cable Colors

Cables inherit the color of the signal they carry, making it easy to trace signal flow:

![Cable Colors](../images/signal-cable-colors.png)
*Cables colored by signal type*

### Module Headers

Module header colors indicate the category, not signal type:

- Blue header = Source (produces audio signals)
- Green header = Filter (processes audio signals)
- Orange header = Modulation (produces control signals)
- etc.

---

## Next Steps

- **[Connections](./connections.md)** - Learn the rules for connecting modules
- **[Module Reference](../modules/README.md)** - See signal types for each module
