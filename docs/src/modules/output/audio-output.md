# Audio Output

**Module ID**: `output.audio`
**Category**: Output
**Header Color**: Red

![Audio Output Module](../../images/module-audio-output.png)
*The Audio Output module*

## Description

The Audio Output module is the final destination for your audio signal, sending sound to your computer's audio interface. It provides master level control, metering, and a built-in limiter to prevent clipping.

Every patch that makes sound needs exactly one Audio Output module.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Left** | Audio (Blue) | Left stereo channel input |
| **Right** | Audio (Blue) | Right stereo channel input |
| **Mono** | Audio (Blue) | Mono input (sent to both L and R) |

## Outputs

*This module has no outputs—it sends audio to the system.*

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Level** | -∞ to +6 dB | 0 dB | Master output level |
| **Limiter** | On/Off | On | Enable/disable output limiter |
| **Limiter Threshold** | -12 dB to 0 dB | -1 dB | Limiter ceiling |

## Display Elements

### Level Meters

Stereo LED-style meters showing:
- **Green**: Safe levels (-∞ to -6 dB)
- **Yellow**: Moderate levels (-6 to -3 dB)
- **Red**: Hot levels (-3 to 0 dB)
- **Clip indicator**: Flashes on limiting/clipping

### Limiter Activity

LED indicates when limiter is actively reducing gain.

## How It Works

1. **Mixing**: Left, Right, and Mono inputs are summed appropriately
2. **Level**: Master level is applied
3. **Limiter**: If enabled, prevents signal from exceeding threshold
4. **Output**: Signal is sent to the audio hardware

### Input Routing

- **Left only**: Duplicated to both outputs (mono)
- **Right only**: Duplicated to both outputs (mono)
- **Mono only**: Sent to both outputs
- **Left + Right**: True stereo
- **All three**: Mono added to stereo

## Usage Tips

### Basic Connection

Simplest setup—one source to mono:

```
[VCA] ──> [Output Mono]
```

Sound comes from both speakers equally.

### Stereo Connection

True stereo from stereo effects:

```
[Delay Left] ──> [Output Left]
[Delay Right] ──> [Output Right]
```

### Mono + Stereo

Add a mono source to a stereo mix:

```
[Bass VCA] ──> [Output Mono]
[Pad Left] ──> [Output Left]
[Pad Right] ──> [Output Right]
```

The bass appears center, pad is stereo.

### Proper Gain Staging

For best sound quality:

1. Keep individual module outputs at reasonable levels
2. Use VCAs and mixers to control levels
3. Set Output Level near 0 dB
4. Watch for limiter activation—occasional is fine, constant indicates too hot

### Limiter Usage

The built-in limiter prevents harsh digital clipping:

**Limiter On (recommended)**:
- Peaks are caught and reduced
- Protects your ears and speakers
- Slight compression on peaks

**Limiter Off**:
- True clipping on overs
- Harsh digital distortion
- May be desired for effect

### Monitoring Levels

Watch the meters while working:

| Level | Action |
|-------|--------|
| Mostly green | Good, safe levels |
| Occasional yellow | Fine, healthy levels |
| Frequent yellow/red | Consider reducing input levels |
| Constant red/clipping | Definitely reduce levels |

### Avoiding Clipping

If limiter is constantly engaged:

1. Lower Level knob on Output module
2. Lower levels earlier in the signal chain
3. Use VCAs to control dynamics
4. Consider a compressor before output

### Testing Patches

When building patches:

1. Start with Output Level low
2. Gradually increase while playing
3. Find a comfortable level
4. Leave headroom for dynamics

### Multiple Sound Sources

When mixing multiple voices:

```
[Voice 1] ──> [Mixer Ch 1]
[Voice 2] ──> [Mixer Ch 2]
[Mixer] ──> [Output Mono]
```

Use a mixer before output rather than connecting multiple sources.

## Connection Examples

### Simple Mono Synth
```
[Oscillator] ──> [Filter] ──> [VCA] ──> [Output Mono]
```

### Stereo Synth with Effects
```
[Synth] ──> [Reverb L] ──> [Output Left]
            [Reverb R] ──> [Output Right]
```

### Full Mix
```
[Bass] ──> [Mixer Ch 1]
[Lead] ──> [Mixer Ch 2]
[Pad L/R] ──> [Mixer Ch 3/4]
[Mixer L] ──> [Output Left]
[Mixer R] ──> [Output Right]
```

## Troubleshooting

### No Sound

1. Check that Output module exists in patch
2. Verify inputs are connected
3. Check Level knob isn't at minimum
4. Verify system audio output settings
5. Check speakers/headphones are connected
6. Use Oscilloscope to verify signal is reaching Output

### Distorted Sound

1. Lower Level knob
2. Watch for constant limiter activation
3. Reduce levels earlier in chain
4. Check for feedback loops in patch

### One Channel Only

1. Check if only Left or Right is connected
2. Verify stereo effect settings
3. Use Mono input for mono sources

### Audio Glitches/Dropouts

1. Reduce patch complexity
2. Build with release mode (`cargo run --release`)
3. Close other audio applications
4. Check audio buffer settings

## Technical Notes

### Sample Rate

Operates at system audio sample rate (typically 44.1 kHz or 48 kHz).

### Bit Depth

Internal processing at 32-bit float, converted to system format on output.

### Latency

Minimal latency determined by audio buffer size. Lower buffer = lower latency but higher CPU.

## Tips

1. **One output module per patch**: Multiple outputs will conflict
2. **Use the limiter**: It's there to protect you
3. **Watch your levels**: Meters are there for a reason
4. **Gain stage properly**: Don't rely on the limiter for level control
5. **Start quiet**: You can always turn up, but can't unhear loud surprises

## Related Modules

- [VCA](../utilities/vca.md) - Level control before output
- [Mixer](../utilities/mixer.md) - Combine signals before output
- [Compressor](../effects/compressor.md) - Dynamics control before output
- [Oscilloscope](../visualization/oscilloscope.md) - Visualize what you're sending
