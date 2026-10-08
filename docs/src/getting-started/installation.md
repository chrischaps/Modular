# Installation

Modular Synth runs on Windows, macOS and Linux. You can download a prebuilt release or build it from source with Rust.

## Download a release

Prebuilt binaries for Windows, macOS (Intel and Apple Silicon) and Linux are attached to each [release on GitHub](https://github.com/chrischaps/Modular/releases). Download the zip for your system, unzip it, and run `modular_synth`.

Releases are cut from time to time and can trail the source. This manual describes the current source, so if a feature here is missing from your copy, build from source.

## Build from source

### Install Rust

Install the Rust toolchain with [rustup](https://rustup.rs/):

```bash
# Windows
winget install Rustlang.Rustup

# macOS and Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Check that it worked:

```bash
cargo --version
```

### Install system libraries (Linux only)

Windows and macOS need nothing more: Modular Synth uses WASAPI and CoreAudio, which come with the system.

On Linux, install the development packages for ALSA (audio), X11 and keyboard handling. On Debian and Ubuntu:

```bash
sudo apt install libasound2-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

On other distributions, install the equivalent ALSA, xcb and xkbcommon development packages (for example `alsa-lib-devel` on Fedora, `alsa-lib` on Arch).

### Build and run

```bash
git clone https://github.com/chrischaps/Modular.git
cd Modular
cargo run --release
```

The first build takes a few minutes. Always use `--release` to play: the debug build is much slower and can't keep up with a busy patch, so you'll hear dropouts. The finished binary is `target/release/modular_synth`.

The app opens on the **First Sound** example. Press **▶ Play**, then play the `Z` to `M` keys on your computer keyboard. If you hear a note, everything is working.

## Command line

Pass a patch file to open it instead of First Sound:

```bash
cargo run --release -- patches/lush-pad.json
```

The `render` tool plays a patch into a WAV file without opening the app or using an audio device, then prints each channel's peak and RMS level:

```bash
cargo run --release --bin render -- patches/fm-synthesis.json out.wav --seconds 5
```

| Option | Default | Effect |
|--------|---------|--------|
| `--seconds N` | 5 | Length of the render |
| `--sample-rate HZ` | 48000 | Sample rate of the file |
| `--block-size N` | 256 | Samples processed per block |
| `--audition` | off | Plays a short phrase into the patch's Keyboard, MIDI Note and Poly MIDI modules |
| `--input FILE.wav` | none | What [Audio Input](../modules/sources/audio-input.md) modules hear, in place of an input device. It must be at the render's sample rate |

A patch that waits for a player renders silence unless something inside it plays notes (a Clock or Step Sequencer, say) or you add `--audition`. Audio Input modules render silence unless you give them a file with `--input`.

## Microphone access

Modular Synth opens a microphone or other input only when you choose one in the toolbar's **Input** menu.

- **macOS** asks for permission the first time. If you run Modular Synth from a terminal (`cargo run`), macOS asks on behalf of the terminal app, and the permission belongs to it. If you said no, or the input stays silent, turn it on under **System Settings → Privacy & Security → Microphone**, then quit and reopen the terminal or the app.
- **Windows** lets desktop apps use the microphone unless it's turned off under **Settings → Privacy & security → Microphone** (**Let desktop apps access your microphone**).
- **Linux** has no permission prompt; the input appears if ALSA (or PipeWire's ALSA support) lists it.

To run the test suite, use `cargo test`.

## Troubleshooting

**No sound.** Check that **▶ Play** is pressed: the app opens stopped. Then check the **Output** device menu in the toolbar. It lists every output device, with the system default marked **(Default)**; pick the one you're listening on. **🔄 Refresh** at the bottom of the menu picks up a device you plugged in after starting.

**"Audio unavailable" in the toolbar.** Modular Synth couldn't open an output device. Make sure one is connected and that no other application holds it exclusively, then restart.

**Crackles and dropouts.** Make sure you're running the release build. The toolbar's CPU meter, shown while the patch plays, tells you how close the engine is to its limit. If it's near the top, remove modules or lower the voice count on Poly MIDI and the Oscillator's unison.

**My MIDI controller does nothing.** Choose it in the toolbar's **MIDI In** menu: Modular Synth doesn't connect to a controller until you do. A filled dot (●) before the name means it's connected. Connection errors appear in the status bar at the bottom of the window. See [MIDI](./interface-overview.md#midi).
