# Installation

Modular Synth is built from source using Rust's Cargo build system.

## Prerequisites

### Rust Toolchain

Install the Rust toolchain via [rustup](https://rustup.rs/):

```bash
# On Windows (PowerShell)
winget install Rustlang.Rustup

# On macOS/Linux
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify the installation:

```bash
rustc --version
cargo --version
```

### Platform-Specific Dependencies

#### Windows

No additional dependencies required. The WASAPI audio backend is included with Windows.

#### macOS

No additional dependencies required. CoreAudio is included with macOS.

#### Linux

Install the ALSA development libraries:

```bash
# Debian/Ubuntu
sudo apt install libasound2-dev

# Fedora
sudo dnf install alsa-lib-devel

# Arch Linux
sudo pacman -S alsa-lib
```

## Building from Source

### Clone the Repository

```bash
git clone https://github.com/your-repo/modular.git
cd modular
```

### Build and Run

For development (faster compilation, slower runtime):

```bash
cargo run
```

For release (slower compilation, optimized runtime):

```bash
cargo run --release
```

The release build is recommended for actual music-making, as it provides significantly better audio performance with lower CPU usage.

## Build Options

### Debug Build

```bash
cargo build
```

Creates an unoptimized binary in `target/debug/` with debug symbols for development and troubleshooting.

### Release Build

```bash
cargo build --release
```

Creates an optimized binary in `target/release/` suitable for regular use.

### Running Tests

```bash
cargo test
```

## Troubleshooting

### Audio Device Not Found

If you receive an audio device error:

1. Check that your audio device is connected and working
2. Verify no other application has exclusive access to the audio device
3. Try a different sample rate if available

### High CPU Usage

If you experience high CPU usage or audio glitches:

1. Use the release build (`cargo run --release`)
2. Reduce the number of active modules
3. Check that your audio buffer size is appropriate (larger buffers reduce CPU but increase latency)

### Linux: ALSA Underruns

If you experience audio dropouts on Linux:

1. Ensure the ALSA development libraries are installed
2. Try increasing the audio buffer size
3. Consider running with real-time priority (requires appropriate permissions)

## Next Steps

Once you have Modular Synth running:

1. **[Interface Overview](./interface-overview.md)** - Learn to navigate the UI
2. **[Your First Patch](./your-first-patch.md)** - Build your first synthesizer
