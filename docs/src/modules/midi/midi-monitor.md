# MIDI Monitor

**Module ID**: `midi.monitor`
**Category**: MIDI
**Header Color**: Magenta

![MIDI Monitor Module](../../images/module-midi-monitor.png)
*The MIDI Monitor module*

## Description

The MIDI Monitor displays incoming MIDI data in real-time, helping you debug MIDI connections, verify controller mappings, and understand what data your MIDI devices are sending. It's an essential diagnostic tool.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **MIDI In** | MIDI (Purple) | MIDI data to monitor (auto-connected) |

## Outputs

*This module has no outputs—it's purely for monitoring.*

## Display Elements

The monitor shows real-time MIDI activity:

### Note Display
```
Note: C4 (60) Vel: 100 Ch: 1
```
Shows note name, MIDI number, velocity, and channel.

### CC Display
```
CC 1: 64  (Mod Wheel)
CC 7: 100 (Volume)
```
Shows controller number, value, and common CC names.

### Pitch Bend
```
Bend: +0.50
```
Shows pitch bend position (-1.0 to +1.0).

### Aftertouch
```
AT: 45 (Channel)
```
Shows aftertouch pressure value.

### Activity Indicators
- LED flashes on any MIDI activity
- Separate indicators for Notes, CC, etc.

## Parameters

| Control | Options | Default | Description |
|---------|---------|---------|-------------|
| **Channel Filter** | All / 1-16 | All | Show only specific channel |
| **Show Notes** | On/Off | On | Display note messages |
| **Show CC** | On/Off | On | Display control change messages |
| **Show Bend** | On/Off | On | Display pitch bend |
| **Show AT** | On/Off | On | Display aftertouch |
| **History** | 1-20 lines | 10 | Number of messages to display |

## Usage Tips

### Debugging MIDI Connections

1. Add a MIDI Monitor to your patch
2. Play your MIDI controller
3. Watch the display for incoming data

If nothing appears:
- Check MIDI cable connections
- Verify MIDI device is selected in system settings
- Try a different MIDI channel

### Learning Controller Mappings

Find out what CC numbers your controller sends:

1. Add MIDI Monitor
2. Move a knob/fader on your controller
3. Note the CC number displayed
4. Use that CC number for MIDI Learn

### Verifying Velocity Response

Check that velocity is being transmitted:

1. Watch the velocity values while playing
2. Play soft and hard to see range
3. Adjust velocity curve if needed

### Checking Pitch Bend Range

Verify pitch bend is working:

1. Move the pitch bend wheel fully
2. Watch the Bend value
3. Should range from -1.0 to +1.0

### Troubleshooting Stuck Notes

If notes are stuck:

1. Watch for Note Off messages
2. Check for matching Note On/Off pairs
3. A missing Note Off indicates a MIDI problem

### Channel Identification

When using multiple MIDI devices:

1. Set Channel Filter to "All"
2. Play each device
3. Note which channel each uses
4. Configure MIDI Note modules accordingly

## Common MIDI CC Numbers

| CC | Name | Common Use |
|----|------|------------|
| 1 | Mod Wheel | Vibrato, filter sweep |
| 2 | Breath | Expression |
| 7 | Volume | Channel volume |
| 10 | Pan | Stereo position |
| 11 | Expression | Dynamics |
| 64 | Sustain | Hold pedal |
| 74 | Brightness | Filter cutoff |

## Display Format Examples

### Note On
```
[NOTE] C4 (60) vel:100 ch:1 ON
```

### Note Off
```
[NOTE] C4 (60) ch:1 OFF
```

### Control Change
```
[CC] #1 = 64 ch:1 (Mod Wheel)
```

### Pitch Bend
```
[BEND] +0.245 ch:1
```

### Aftertouch
```
[AT] pressure:78 ch:1
```

## Connection Examples

### Basic Monitoring
```
[MIDI Device] ──> [MIDI Monitor]
              ──> [MIDI Note] ──> [Synth]
```

Monitor MIDI while also using it.

### Multi-Channel Debugging
```
[MIDI Device] ──> [MIDI Monitor (All Channels)]
              ──> [MIDI Note (Ch 1)] ──> [Bass Synth]
              ──> [MIDI Note (Ch 2)] ──> [Lead Synth]
```

### Controller Programming
```
[MIDI Controller] ──> [MIDI Monitor]
```

Use monitor to learn what your controller sends before mapping.

## Tips

1. **Always useful during setup**: Add a monitor when connecting new MIDI gear
2. **Filter noise**: Use channel filtering if one device is too active
3. **Watch for patterns**: MIDI problems often show up as missing events
4. **Remove when done**: Monitor uses some resources; remove in final patches
5. **Check both ends**: If nothing appears, verify MIDI is being sent from the source

## Troubleshooting Guide

| Symptom | Possible Cause | Solution |
|---------|---------------|----------|
| No activity | MIDI not connected | Check cables/settings |
| Wrong channel | Device on different channel | Match channel settings |
| No velocity | Controller sends fixed velocity | Check controller settings |
| No pitch bend | Bent spring broken | Check hardware |
| Stuck notes | Note Off not sending | Check cable, try All Notes Off |
| CC jumps | NRPN interference | Filter NRPN messages |

## Related Modules

- [MIDI Note](./midi-note.md) - Convert monitored MIDI to CV
- [Keyboard Input](./keyboard.md) - Alternative non-MIDI input
