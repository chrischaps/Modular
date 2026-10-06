# Polyphony

A cable can carry up to **8 channels** at once: one per voice. Patch a chord through a single chain of modules and every note gets its own oscillator, its own filter and its own envelope. That makes it a chord of separate voices, not one voice playing a blend of notes.

This is the approach VCV Rack takes. You don't build eight copies of your voice; you build one, and the cables carry the voices through it.

## Where Voices Come From

The [Poly MIDI](../modules/midi/poly-midi.md) module sets the voice count. Its **Voices** knob says how many channels its Pitch, Gate and Velocity cables carry, and each note you play is given a channel of its own.

## Polyphonic Modules

These modules run one voice per channel of their widest input:

| Module | Per voice |
|--------|-----------|
| [Oscillator](../modules/sources/oscillator.md) | Phase, sync, unison |
| [SVF Filter](../modules/filters/svf-filter.md) | Filter memory and resonance |
| [Ladder Filter](../modules/filters/ladder-filter.md) | Filter memory and resonance |
| [ADSR Envelope](../modules/modulation/adsr.md) | Stage and level |
| [VCA](../modules/utilities/vca.md) | Gain |
| [Attenuverter](../modules/utilities/attenuverter.md) | Scaling |
| [Sample & Hold](../modules/utilities/sample-hold.md) | Held value |

Their outputs carry as many channels as their widest input. The knobs are shared: turning Cutoff moves every voice's cutoff.

## Mixing Mono and Poly

A **mono cable into a polyphonic module** is shared by every voice. One LFO into a polyphonic filter's Cutoff sweeps all the voices together. Use a polyphonic source, such as the envelope or velocity, to move each voice on its own.

A **polyphonic cable into a mono module** is folded down:

- **Audio inputs** hear every channel summed, so a polyphonic voice goes straight into the Mixer, the effects or the Output.
- **Control and gate inputs** take the first channel, since adding CVs together rarely means anything.

Voices add up, so a four-note chord is about four times as loud as one note. Leave headroom with the VCA's Level or the Mixer.

## A Polyphonic Voice

```
[Poly MIDI Pitch] ──> [Oscillator V/Oct]
[Poly MIDI Gate] ──> [ADSR Gate]
[Poly MIDI Velocity] ──> [ADSR Velocity]
[Oscillator Out] ──> [SVF Filter In]
[ADSR Out] ──> [SVF Filter Cutoff]
[SVF Filter LowPass] ──> [VCA In]
[ADSR Out] ──> [VCA CV]
[VCA Out] ──> [Reverb] ──> [Audio Output]
```

Every module up to the VCA runs one copy per voice. The Reverb hears the voices summed.

## Cost

Each voice is a full copy of the module, so 8 voices use about 8 times the CPU of one. If a patch is heavy, turn **Voices** down on the Poly MIDI module.
