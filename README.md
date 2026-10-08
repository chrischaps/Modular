# Modular Synth

A node-based modular synthesizer, written in Rust. Patch oscillators, filters, envelopes and effects together on a canvas, the way you would in Blender's node editor, and hear the result as you go.

![Modular Synth: a two-oscillator voice through a filter, delay and output, with an oscilloscope](Screenshot.png)

It doesn't imitate hardware panels. Every module is a node with jacks on its sides and knobs along its bottom, and every cable is coloured by what it carries. Knobs that are being modulated turn on their own, so you can watch the patch move.

**Documentation:** [docs.chaps.dev/modular](https://docs.chaps.dev/modular/) has a first-patch walkthrough, a page for every module, and recipes to build.

## Features

- **Colour-coded signals.** Blue cables carry audio, orange carry control voltage, green carry gates, and purple carry MIDI. Connections that can't work are refused as you make them.
- **Inputs vs. knobs.** Most parameters have a knob and a jack. Patch a cable into the jack and it takes over: the knob dims and follows the incoming signal, as on an analog modular.
- **Polyphony on a single cable.** Poly MIDI sends up to 8 voices down one cable, and every module after it plays each voice on its own. Poly cables are drawn as a bundle of strands, one per voice.
- **25 modules:**

  | Category | Modules |
  |---|---|
  | Sources | Oscillator (BLEP anti-aliasing, sync, through-zero FM, sub, supersaw unison), Noise (white, pink, brown, smooth random), Keyboard, MIDI Note, Poly MIDI |
  | Filters | SVF Filter (self-oscillating, notch), Ladder Filter (oversampled) |
  | Modulation | ADSR Envelope, LFO, Clock |
  | Effects | Stereo Delay (with tape mode), Reverb (8-line FDN), Chorus, Distortion (oversampled, wavefolder), Compressor, Parametric EQ |
  | Utilities | VCA, Mixer (4 stereo channels, pan, mute, poly spread), Attenuverter, Sample & Hold, Quantizer (scales, custom scale from a clickable piano), Step Sequencer |
  | Output | Audio Output (with metering and limiter), Oscilloscope, MIDI Monitor |

- **Visual feedback.** Waveform, envelope and filter-response displays sit on their modules, LEDs light on active outputs, and the scope shows what's actually there. Hover any jack or knob to see what it does.
- **Fast editing.** Space opens a fuzzy quick-add palette at the cursor. Undo and redo cover every edit. Copy, paste and duplicate work across windows, because the clipboard holds patch JSON.
- **MIDI.** Play from any MIDI controller, map any knob to a CC with MIDI Learn, or play the computer keyboard.
- **Your work is safe.** Modular asks before New, Open or Quit would lose unsaved changes. A patch with unsaved changes is autosaved every 30 seconds and offered back after a crash. Recently opened patches are a menu away.
- **Real-time-safe engine.** The audio thread never allocates or locks. The UI sends it commands over lock-free ring buffers, and CI runs a test that fails if it ever allocates.

## Getting started

You need a [Rust toolchain](https://rustup.rs). On Linux you also need the ALSA and X11/Wayland development packages listed in `.github/workflows/ci.yml`.

```bash
cargo run --release                           # opens with the First Sound example
cargo run --release -- patches/lush-pad.json  # or open a patch
```

Six example patches are in [`patches/`](patches) and in the app's **Examples** menu. Press **Play**, then hold a few keys on the computer keyboard (`Z` to `M` is a white-key octave, with the sharps on the row above).

### Render a patch offline

The `render` binary plays a patch to a WAV file without an audio device, and prints its peak and RMS levels:

```bash
cargo run --release --bin render -- patches/fm-synthesis.json out.wav --seconds 5
cargo run --release --bin render -- patches/lush-pad.json out.wav --audition  # plays a phrase into Keyboard/MIDI modules
```

### Shortcuts

| Keys | Action |
|---|---|
| `Space` / `Tab` | Quick-add a module at the cursor |
| `Ctrl+Z`, `Ctrl+Shift+Z` | Undo, redo |
| `Ctrl+C` / `X` / `V`, `Ctrl+D` | Copy, cut, paste, duplicate |
| `Delete` | Delete the selected modules |
| `Ctrl+B` | Bypass the selected effects |
| `Ctrl+N`, `Ctrl+O`, `Ctrl+S`, `Ctrl+Shift+S` | New, open, save, save as |

## How it's built

```
egui + egui_node_graph2   node editor, custom knobs, cables and displays
          │  commands and feedback over rtrb ring buffers
audio graph engine        compiled plans, pre-allocated buffers, poly channels
          │
cpal                      the audio device
```

Each DSP module declares its own ports, parameters and descriptions, and the editor generates its nodes from them, so the UI can't drift from the sound. Patches are plain JSON.

```bash
cargo test   # 800+ tests, including the audio-thread allocation guard
```

## License

MIT
