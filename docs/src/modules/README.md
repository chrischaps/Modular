# Module Overview

Modular Synth includes 22 modules organized into functional categories. Each category has a distinctive header color for quick identification.

## Categories

### Sources (Blue Header)

Sound generators that create audio signals from scratch.

| Module | ID | Description |
|--------|-----|-------------|
| [Oscillator](./sources/oscillator.md) | `osc.sine` | Multi-waveform VCO with FM and PWM |

### Filters (Green Header)

Frequency-shaping modules that remove or emphasize parts of the spectrum.

| Module | ID | Description |
|--------|-----|-------------|
| [SVF Filter](./filters/svf-filter.md) | `filter.svf` | State Variable Filter with LP/HP/BP outputs |

### Modulation (Orange Header)

Modules that generate control signals for modulating other parameters.

| Module | ID | Description |
|--------|-----|-------------|
| [ADSR Envelope](./modulation/adsr.md) | `mod.adsr` | Attack/Decay/Sustain/Release envelope generator |
| [LFO](./modulation/lfo.md) | `mod.lfo` | Low Frequency Oscillator with multiple waveforms |
| [Clock](./modulation/clock.md) | `mod.clock` | Clock generator with BPM control and divisions |

### Utilities (Yellow Header)

Signal processing and routing modules.

| Module | ID | Description |
|--------|-----|-------------|
| [VCA](./utilities/vca.md) | `util.vca` | Voltage Controlled Amplifier |
| [Mixer](./utilities/mixer.md) | `util.mixer` | 2-channel audio/CV mixer |
| [Attenuverter](./utilities/attenuverter.md) | `util.attenuverter` | Scale, invert, and offset signals |
| [Sample & Hold](./utilities/sample-hold.md) | `util.samplehold` | Sample input on trigger |
| [Sequencer](./utilities/sequencer.md) | `util.sequencer` | 16-step CV/gate sequencer |

### Effects (Purple Header)

Audio processing effects.

| Module | ID | Description |
|--------|-----|-------------|
| [Delay](./effects/delay.md) | `fx.delay` | Stereo delay with feedback and filtering |
| [Reverb](./effects/reverb.md) | `fx.reverb` | Algorithmic reverb |
| [Chorus](./effects/chorus.md) | `fx.chorus` | Chorus/ensemble effect |
| [Distortion](./effects/distortion.md) | `fx.distortion` | Waveshaping distortion |
| [EQ](./effects/eq.md) | `fx.eq` | 3-band parametric equalizer |
| [Compressor](./effects/compressor.md) | `fx.compressor` | Dynamics compressor |

### MIDI (Magenta Header)

MIDI input and processing modules.

| Module | ID | Description |
|--------|-----|-------------|
| [Keyboard Input](./midi/keyboard.md) | `midi.keyboard` | Computer keyboard to CV/Gate |
| [MIDI Note](./midi/midi-note.md) | `midi.note` | MIDI to V/Oct, Gate, and Velocity |
| [MIDI Monitor](./midi/midi-monitor.md) | `midi.monitor` | Display incoming MIDI data |

### Visualization (Cyan Header)

Visual feedback modules.

| Module | ID | Description |
|--------|-----|-------------|
| [Oscilloscope](./visualization/oscilloscope.md) | `util.oscilloscope` | Waveform display |

### Output (Red Header)

Final audio output.

| Module | ID | Description |
|--------|-----|-------------|
| [Audio Output](./output/audio-output.md) | `output.audio` | Stereo output with limiter |

---

## Signal Type Quick Reference

When connecting modules, match these signal types:

| Signal | Color | Typical Use |
|--------|-------|-------------|
| **Audio** | Blue | Sound signals between oscillators, filters, effects, output |
| **Control** | Orange | Modulation from envelopes, LFOs to parameters |
| **Gate** | Green | Triggers from keyboard, clock, sequencer to envelopes |
| **MIDI** | Purple | MIDI data to MIDI-processing modules |

---

## Common Signal Chains

### Basic Synthesis Path
```
[Oscillator] → [Filter] → [VCA] → [Output]
```

### With Modulation
```
[Keyboard] ──V/Oct──→ [Oscillator] → [Filter] → [VCA] → [Output]
    │                                    ↑         ↑
    └──Gate──→ [Envelope] ──────────────┴─────────┘
```

### With Effects
```
[Oscillator] → [Filter] → [VCA] → [Delay] → [Reverb] → [Output]
```

### Sequenced Pattern
```
[Clock] → [Sequencer] ──CV──→ [Oscillator] → [Filter] → [Output]
              │                                  ↑
              └──Gate──→ [Envelope] ────────────┘
```

---

## Choosing Modules

### I want to...

**Generate a sound**: Start with [Oscillator](./sources/oscillator.md)

**Shape the tone**: Use [SVF Filter](./filters/svf-filter.md)

**Control volume**: Use [VCA](./utilities/vca.md) with an [Envelope](./modulation/adsr.md)

**Add movement**: Connect an [LFO](./modulation/lfo.md) to a parameter

**Play from keyboard**: Add [Keyboard Input](./midi/keyboard.md) or [MIDI Note](./midi/midi-note.md)

**Create rhythmic patterns**: Use [Clock](./modulation/clock.md) and [Sequencer](./utilities/sequencer.md)

**Add space/depth**: Use [Delay](./effects/delay.md) or [Reverb](./effects/reverb.md)

**Mix multiple signals**: Use [Mixer](./utilities/mixer.md)

**See what's happening**: Add [Oscilloscope](./visualization/oscilloscope.md)

**Hear the result**: Connect to [Audio Output](./output/audio-output.md)
