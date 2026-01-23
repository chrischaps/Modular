# Oscilloscope

**Module ID**: `util.oscilloscope`
**Category**: Visualization
**Header Color**: Cyan

![Oscilloscope Module](../../images/module-oscilloscope.png)
*The Oscilloscope module*

## Description

The Oscilloscope provides real-time visualization of audio and control signals. It displays waveforms, helping you understand what's happening in your patch, debug signal problems, and learn how different modules affect signals.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Input 1** | Any (matches input) | First signal to display (typically audio) |
| **Input 2** | Any (matches input) | Second signal to display (overlay) |
| **Trigger** | Gate (Green) | External trigger for stable display |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Thru 1** | Any | Passthrough of Input 1 |
| **Thru 2** | Any | Passthrough of Input 2 |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Time** | 1 ms - 500 ms | 20 ms | Time window displayed |
| **Scale 1** | 0.1x - 10x | 1x | Vertical scale for Input 1 |
| **Scale 2** | 0.1x - 10x | 1x | Vertical scale for Input 2 |
| **Trigger Level** | -1.0 to +1.0 | 0.0 | Trigger threshold |
| **Trigger Mode** | Auto/Normal/Single | Auto | Triggering behavior |

## Display Elements

### Waveform Display

The main area shows the signal amplitude over time:
- **Horizontal axis**: Time (left = past, right = present)
- **Vertical axis**: Amplitude (-1.0 to +1.0)
- **Channel 1**: Typically blue trace
- **Channel 2**: Typically orange trace (overlay)

### Grid

Reference grid helps estimate values:
- Horizontal lines at -1, -0.5, 0, +0.5, +1
- Vertical time divisions based on Time setting

### Measurements

May display:
- **Frequency**: Detected fundamental frequency
- **Peak-to-Peak**: Amplitude range
- **DC Offset**: Average signal level

## Usage Tips

### Viewing Oscillator Waveforms

See what your oscillator outputs:

```
[Oscillator] ──> [Oscilloscope Input 1]
             ──> [Rest of patch]
```

Verify waveform shape, frequency, and level.

### Comparing Two Signals

View two signals overlaid:

```
[Oscillator 1] ──> [Scope Input 1]
[Oscillator 2] ──> [Scope Input 2]
```

Useful for:
- Comparing waveforms
- Checking phase relationships
- Viewing before/after processing

### Viewing Envelopes

See envelope shape in real-time:

```
[ADSR Output] ──> [Oscilloscope Input 1]
```

Adjust Time to 100-500ms to see the full envelope cycle.

### Viewing LFO

Check LFO waveform and rate:

```
[LFO] ──> [Oscilloscope Input 1]
```

Time setting should be longer than one LFO cycle.

### Debugging Signal Problems

No sound? Check the scope:

```
[Mystery Signal] ──> [Oscilloscope]
```

- **Flat line**: No signal
- **Clipped/squared-off peaks**: Distortion/clipping
- **DC offset**: Signal not centered around zero
- **Expected waveform**: Signal is fine, problem is elsewhere

### Stable Display with External Trigger

For synced display of periodic signals:

```
[Clock] ──> [Oscilloscope Trigger]
[Signal] ──> [Oscilloscope Input]
```

The display starts at the same point each cycle.

### Trigger Modes

**Auto**: Triggers automatically if no trigger detected
**Normal**: Only displays when trigger threshold is crossed
**Single**: Captures one sweep, then freezes (for transients)

### Using Thru Outputs

The scope passes signals through, so it can be inserted anywhere:

```
[Osc] ──> [Scope Input 1]
[Scope Thru 1] ──> [Filter] ──> [Scope Input 2]
[Scope Thru 2] ──> [VCA] ──> [Output]
```

See signal before and after filter.

### Setting Time Scale

| Signal Type | Time Setting |
|-------------|--------------|
| Audio (440 Hz) | 5-10 ms |
| Bass (100 Hz) | 20-50 ms |
| LFO (1 Hz) | 500-1000 ms |
| Envelope | 100-500 ms |
| Control signals | 50-200 ms |

### Amplitude Scaling

If signal is too quiet or too loud:
- Use Scale knob to zoom in/out
- 1x = full range display
- 2x = shows half range (zoomed in)
- 0.5x = shows double range (zoomed out)

## What to Look For

### Healthy Signals

| Signal Type | Expected Appearance |
|-------------|---------------------|
| Sine | Smooth curve |
| Square | Flat tops and bottoms |
| Saw | Diagonal ramp |
| Triangle | Symmetric slopes |
| Envelope | Rising/falling shape |
| Gate | Flat at 0 or 1 |

### Problem Signs

| Appearance | Possible Problem |
|------------|------------------|
| Flat line | No signal |
| All noise | Broken connection |
| Clipped tops | Input too hot |
| DC shift | DC offset added |
| Unstable | Feedback loop |
| Too fast | Time setting too slow |

## Connection Examples

### Signal Chain Analysis
```
[Osc] ──> [Scope In 1, Thru 1] ──> [Filter] ──> [Scope In 2, Thru 2] ──> [Output]
```

### Envelope Visualization
```
[Gate] ──> [ADSR] ──> [Scope In 1]
```

### LFO Phase Check
```
[LFO 1] ──> [Scope In 1]
[LFO 2] ──> [Scope In 2]
```

### Pre/Post Effect
```
[Audio] ──> [Scope In 1, Thru 1] ──> [Distortion] ──> [Scope In 2]
```

## Tips

1. **Insert anywhere**: Use Thru outputs to monitor without breaking the signal chain
2. **Match time to signal**: Audio needs fast time, envelopes need slow time
3. **Use trigger**: For stable display of periodic signals
4. **Two channels**: Compare before/after or two related signals
5. **Check levels**: Scope shows you clipping before you hear it

## Related Modules

- [Oscillator](../sources/oscillator.md) - Waveforms to visualize
- [LFO](../modulation/lfo.md) - Modulation to visualize
- [ADSR Envelope](../modulation/adsr.md) - Envelopes to visualize
- [MIDI Monitor](../midi/midi-monitor.md) - MIDI visualization alternative
