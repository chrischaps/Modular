# Clock

**Module ID** `util.clock` · **Category** Utility

![Clock Module](../../images/module-clock.png)
*The Gate jack lights with every pulse.*

The Clock is the patch's metronome. It sends a steady stream of gate pulses at a tempo you set, for stepping a [Sequencer](../utilities/sequencer.md), firing envelopes on the beat, or triggering a [Sample & Hold](../utilities/sample-hold.md).

It also sets the **patch tempo**. Tempo-synced modules, such as the [Delay](../effects/delay.md) with its **Sync** set to a note length, follow the Clock's **BPM**, and keep following it as you turn the knob.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Sync** | Gate (Green) | A rising edge restarts the beat, so the next pulse starts right away |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Gate** | Gate (Green) | One pulse per division. The jack lights while the gate is high |

## Parameters

| Control | Range | Default | Description |
|---------|-------|---------|-------------|
| **BPM** | 20 – 300 BPM | 120 | Tempo in beats (quarter notes) per minute |
| **Gate** | 1 – 99% | 50% | How much of each pulse the gate stays high |
| **Div** | 1, 1/2, 1/4, 1/8, 1/16 | 1/4 | Dropdown on the node. Pulse rate relative to the beat |
| **Run** | On / Off | On | Checkbox on the node. Off holds the gate low and pauses the clock |

## Divisions

**BPM** counts quarter notes, and **Div** sets how many pulses that makes:

| Div | One pulse every | At 120 BPM |
|-----|-----------------|------------|
| **1** | 4 beats (a bar of 4/4) | 2 s |
| **1/2** | 2 beats | 1 s |
| **1/4** | Beat | 500 ms |
| **1/8** | Half beat | 250 ms |
| **1/16** | Quarter beat | 125 ms |

A Clock has one output, so for two rhythms at once, use two Clocks at the same BPM, each with its own division.

## Gate length

**Gate** sets how long each pulse stays high, as a share of the time between pulses. At 50% the gate is high for half of each pulse. Short gates (5 to 20%) suit drums and plucks: the envelope gets its attack and goes straight to release. Long gates (80 to 99%) hold an envelope in its sustain, so notes run nearly into one another.

## Sync and Run

A rising edge at **Sync** restarts the beat from the top. Use it to line the Clock up with another Clock, a sequencer's **EOC**, or any trigger. The Clock keeps its own tempo; Sync only moves where the beat falls.

Unticking **Run** holds the gate low and freezes the clock where it is. Ticking it again carries on from the same point in the beat.

## The patch tempo

The Clock's **BPM** is the tempo for the whole patch. A Delay set to sync to 1/8 echoes on the eighth note of whatever the Clock is playing, and if you change the BPM the echoes move with it. Without a Clock in the patch, synced modules assume 120 BPM.

If a patch has more than one Clock, synced modules follow just one of them, so give them all the same BPM.

## Patch examples

### Stepping a sequencer

```text
[Clock Gate] ──> [Sequencer Clock]
[Sequencer Pitch] ──> [Oscillator V/Oct]
[Sequencer Gate] ──> [ADSR Gate]
```

Set **Div** to 1/8 or 1/16 for a running bassline.

### Notes on the beat

```text
[Clock Gate] ──> [ADSR Gate]
[ADSR Out] ──> [VCA CV]
[Oscillator Out] ──> [VCA In] ──> [Audio Output]
```

A drone that sounds once per pulse. Lower the **BPM** to 40 to 60 and lengthen the envelope's release for slow, ambient swells.

### Restarting an LFO on the beat

```text
[Clock Gate] ──> [LFO Sync]
```

With **Div** at 1 and the LFO at a matching rate, every bar starts the LFO's sweep from the top.

### Echoes in time

```text
[Clock] (BPM 100)
[Synth voice] ──> [Delay In L]   (Delay Sync: 1/8D)
```

The Delay needs no cable from the Clock to follow its tempo.

## Notes

- The Clock is monophonic. It sends the same gate to every voice of a polyphonic module.
- The Clock runs with the transport. **Stop** halts it with the rest of the patch, and **Play** starts it again from the top of the beat.

## Related modules

- [Sequencer](../utilities/sequencer.md), the Clock's most common partner
- [ADSR Envelope](./adsr.md) to turn pulses into notes
- [LFO](./lfo.md) to restart modulation on the beat
- [Delay](../effects/delay.md), which follows the Clock's tempo when synced
