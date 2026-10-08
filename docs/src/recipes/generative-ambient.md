# Generative Ambient

A patch that plays itself. A slow pentatonic melody repeats every twelve seconds, but each note comes out with a different brightness, the tuning drifts, and long echoes and an eight-second reverb blur one phrase into the next. Press Play and leave it running.

> **Load it:** choose **📚 Examples → Generative Ambient** in the toolbar and press **▶ Play**. It needs no keyboard.
> The patch file is [`patches/generative-ambient.json`](https://github.com/chrischaps/Modular/blob/master/patches/generative-ambient.json).

![The Generative Ambient patch](../images/recipe-generative-ambient.png)
*Clock and sequencer play the notes; two slow LFOs keep them from repeating.*

## What it teaches

- **Clock and sequencer.** A clock sets the pace and a step sequencer turns each pulse into a note.
- **Sample & hold.** Freezing a moving signal at each note gives every note its own setting, a step at a time.
- **Cycles that don't line up.** When loops of different lengths run against each other, the combination takes a very long time to repeat. That's where the variety comes from, not from randomness.

## Modules

| Module | Settings |
|--------|----------|
| [Clock](../modules/modulation/clock.md) | **BPM** 40, **Div** 1/4 |
| [Step Sequencer](../modules/utilities/sequencer.md) | **Steps** 8, **Dir** Fwd, **Gate** 60%. Notes C4 D4 E4 G4 A4 G4 E4 D4, step 6 off |
| [Oscillator](../modules/sources/oscillator.md) 1 | **Wave** Tri, **Oct** −1, **Exp FM** 0.01 oct |
| [Oscillator](../modules/sources/oscillator.md) 2 | **Wave** Sine |
| [Mixer](../modules/utilities/mixer.md) | **Lv 1** 100%, **Lv 2** 50% |
| [SVF Filter](../modules/filters/svf-filter.md) | **Cutoff** 1.5 kHz, **Res** 20% |
| [ADSR Envelope](../modules/modulation/adsr.md) | **Atk** 300 ms, **Dec** 500 ms, **Sus** 70%, **Rel** 2 s |
| [VCA](../modules/utilities/vca.md) | Defaults |
| [LFO](../modules/modulation/lfo.md) 1 | **Rate** 0.13 Hz, **Wave** Triangle, **Bipolar** on |
| [Sample & Hold](../modules/utilities/sample-hold.md) | **Slew** 300 ms |
| [LFO](../modules/modulation/lfo.md) 2 | **Rate** 0.03 Hz, **Wave** Sine, **Bipolar** on |
| [Stereo Delay](../modules/effects/delay.md) | **Time** 600 ms, **FB** 50%, **Mix** 40%, **HiCut** 4 kHz, **LoCut** 200 Hz, **P-P** on |
| [Reverb](../modules/effects/reverb.md) | **Size** 90%, **Decay** 8 s, **PreD** 100 ms, **Mix** 60% |
| [Audio Output](../modules/output/audio-output.md) | **Vol** 100% |

## How it's built

### The melody

```text
[Clock Gate] ──> [Step Sequencer Clock]
[Step Sequencer Pitch] ──> [Oscillator 1 V/Oct]
                       ──> [Oscillator 2 V/Oct]
[Step Sequencer Gate] ──> [ADSR Gate]
```

At 40 BPM with **Div** at 1/4, the Clock pulses once a beat, every 1.5 seconds. Each pulse moves the sequencer one step. Its eight steps rise and fall through a C major pentatonic scale, C D E G A G E D, and step 6's gate is off, so the melody takes a breath before it turns around. Eight steps of 1.5 seconds make a twelve-second loop.

The pentatonic scale has no half steps, so no two of its notes clash. That's why the long echoes and reverb can pile notes on top of each other without the result turning muddy.

### Two oscillators, an octave apart

```text
[Oscillator 1 Out] ──> [Mixer Ch 1]
[Oscillator 2 Out] ──> [Mixer Ch 2]
[Mixer Out] ──> [SVF Filter In]
```

Oscillator 1 is a triangle an octave down, and Oscillator 2 a sine at pitch, mixed in at half level. Together they make a soft, hollow tone, closer to a flute or a mallet than to a synth lead.

### Brightness, one note at a time

```text
[LFO 1 Out] ──> [Sample & Hold In]
[Clock Gate] ──> [Sample & Hold Trig]
[Sample & Hold Out] ──> [SVF Filter Cutoff]
```

LFO 1 is a slow triangle, one cycle every 7.7 seconds. On every clock pulse, the Sample & Hold catches the LFO's current value and holds it until the next. The filter's **Cutoff** input works in octaves, so each note gets a cutoff somewhere between 750 Hz and 3 kHz. The 300 ms **Slew** glides between values instead of jumping.

The LFO's 7.7-second cycle doesn't divide evenly into the 1.5-second pulses or the twelve-second loop. So the melody repeats exactly, but the brightness of each note doesn't: every pass through the phrase comes out shaded differently.

### Slow drift

```text
[LFO 2 Out] ──> [Oscillator 1 Exp FM]
            ──> [SVF Filter Resonance]
```

LFO 2 takes over half a minute per cycle. On Oscillator 1's **Exp FM** input, with **Exp FM** set to 0.01 octave, it bends the pitch about 12 cents sharp and flat. Against the steady Oscillator 2, that makes a slow beating, like an instrument that's not quite in tune with itself. The same LFO raises and lowers the filter's resonance, so some passages ring and others are soft.

### Envelope, echoes and space

```text
[SVF Filter LowPass] ──> [VCA In]
[ADSR Out] ──> [VCA CV]
[VCA Out] ──> [Stereo Delay In L]
[Stereo Delay Out L] ──> [Reverb In L]
[Stereo Delay Out R] ──> [Reverb In R]
[Reverb Out L] ──> [Audio Output Left]
[Reverb Out R] ──> [Audio Output Right]
```

The envelope's 300 ms attack takes the edge off each note and its 2-second release lets it ring into the next one. The Stereo Delay repeats every 600 ms with **Ping-Pong** on, so echoes bounce between the speakers. Its **HiCut** and **LoCut** darken and thin each repeat, so the echoes recede instead of building up. The Reverb's large room and 8-second tail turn it all into a wash.

## Variations

**Really generative.** Set the sequencer's **Dir** to Rnd. The notes now come in a random order, and because they're all from the pentatonic scale, every order works.

**Rewrite the melody.** Click a step to turn its gate on or off. Right-click a step to move its pitch by a semitone or an octave. Stay on C, D, E, G and A to keep the pentatonic calm.

**An odd-length loop.** Set **Steps** to 5 or 7 so the phrase falls out of step with the bar.

**Slower still.** Turn the Clock down to 20 BPM, and raise the Reverb's **Decay** to 15 s or more.

**Tape echoes.** Turn on the delay's **Tape** for wobble and saturation in the repeats.

**Wider drift.** Raise Oscillator 1's **Exp FM** to 0.03 octave for a seasick, detuned-tape feel.

## Related

- [Sample & Hold](../modules/utilities/sample-hold.md) – stepped modulation
- [Step Sequencer](../modules/utilities/sequencer.md) – editing steps
- [Rhythmic Sequence](./rhythmic-sequence.md) – the same clock and sequencer, at dance tempo
- [Shoreline](./shoreline.md) – another patch that plays itself, where chance does the varying
