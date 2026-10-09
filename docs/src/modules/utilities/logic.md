# Logic

**Module ID** `util.logic` · **Category** Utility

![Logic Module](../../images/module-logic.png)
*Dividing a sixteenth clock by 16: once a bar. The count is on bead 6, the last of the six the Gate is open for, lit along the arc. A is the clock, so OR and XOR follow it. The LFO on **CV** is above the Threshold line, so its column is green.*

Logic does sums with gates. The [Clock](../modulation/clock.md), the [Step Sequencer](./sequencer.md) and [Audio Input](../sources/audio-input.md) all make gates; Logic is where you work with them. It does four jobs, each with its own jacks:

- **Divide** a clock: fire once every so many pulses, and hold a gate open for as many as you like. A fill every fourth bar, a crash every eighth, a sound on the third beat of every bar.
- **Count**: say how far through the division the clock is, as a control voltage.
- **Combine** two gates with AND, OR and XOR, or turn one upside down with NOT.
- **Compare** a control voltage with a threshold, making a gate from it. This is the way to patch any control signal into a gate input.

The node shows all four. The **ring** has one bead per count, read clockwise from the top like a clock face. The count that fires wears a ring, and the counts the Gate stays open for lie along a green arc, which lights while the gate is open. An orange bead travels round as the clock ticks. The four **lamps** light while AND, OR, XOR and NOT A are high. The **column** at the right is the CV, rising past the **Threshold** line and turning green when it's above. Drag the line to move the Threshold.

## Inputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Clock** | Gate (Green) | Each rising edge counts one |
| **Reset** | Gate (Green) | A rising edge starts the count over and closes the Gate. The next clock is count 0, even one on the same sample |
| **A** | Gate (Green) | The first gate for AND, OR, XOR and NOT A |
| **B** | Gate (Green) | The second gate for AND, OR and XOR |
| **CV** | Control (Orange) | A voltage to compare with **Threshold**. Audio patches in too |

## Outputs

| Port | Signal Type | Description |
|------|-------------|-------------|
| **Trig** | Gate (Green) | The clock's own pulse, on the counts that fire. ÷1 passes every pulse through |
| **Gate** | Gate (Green) | Opens on a count that fires and stays open for **Length** clocks |
| **Count** | Control (Orange) | Where the count is: 0 on count 0, rising in even steps to 1 on the last count, like the sequencer's **Step** |
| **AND** | Gate (Green) | High while A and B are both high |
| **OR** | Gate (Green) | High while A or B is high |
| **XOR** | Gate (Green) | High while exactly one of A and B is high |
| **NOT A** | Gate (Green) | High while A is low. With nothing in A it's always high |
| **Above** | Gate (Green) | High while CV is above **Threshold** |

## Parameters

| Knob | Range | Default | Description |
|------|-------|---------|-------------|
| **Div** | 1 – 128 | 4 | Fires once every this many clocks |
| **Offset** | 0 – 127 | 0 | Which count fires. 0 is the first clock after a reset; offsets past **Div** wrap round |
| **Length** | 1 – 128 | 1 | How many clocks the Gate stays open |
| **Thresh** | -1 – 1 | 0.5 | Where **Above** switches |

## How it works

### Dividing

Logic counts rising edges on **Clock**. The first edge is count 0, then 1, 2, and so on up to **Div** − 1, and round again. It fires on the count set by **Offset**:

```text
Clock:  _|‾|_|‾|_|‾|_|‾|_|‾|_|‾|_|‾|_|‾|_|‾|_    (Div 4, Offset 0, Length 2)
Count:   0   1   2   3   0   1   2   3   0
Trig:   _|‾|_____________|‾|_____________|‾|_
Gate:   _|‾‾‾‾‾‾‾|_______|‾‾‾‾‾‾‾|_______|‾‾‾
```

Because it counts pulses rather than timing them, a divider can't drift from its clock. Every pulse it fires on is one of the clock's own pulses, on the same sample, after ten minutes as after ten seconds, at any tempo. Change the Clock's **BPM** and the phrase follows.

**Trig** is the clock's own pulse, as long as the clock's gate. **Gate** is measured in whole clocks: at **Length** 1 it's open from the pulse that fires to the next one, and at **Length** 16 for sixteen. It closes on the clock edge that ends it, before any module it drives hears that edge. A Length of **Div** or more keeps it open all the time, dipping for a single sample each time the count fires, so an envelope it drives still sees a new edge.

If the clock stops while the Gate is open, the Gate lets go once two clock periods have passed with no pulse, so a stopped clock can't leave it open.

### Combining

The logic outputs follow A and B sample by sample, with no count or memory. Anything above 0.5 counts as high, as on every gate input.

| A | B | AND | OR | XOR | NOT A |
|---|---|-----|----|-----|-------|
| low | low | low | low | low | high |
| high | low | low | high | high | low |
| low | high | low | high | high | high |
| high | high | high | high | low | low |

### Comparing

**Above** goes high when CV rises past **Threshold** + 0.01, and low again when it falls below **Threshold** − 0.01. That little gap is hysteresis: a slow or noisy signal crossing the threshold switches once instead of chattering.

Gate outputs can feed control inputs, but control outputs can't feed gate inputs, because a control signal has no clear on and off. **Above** is how you say where on and off are.

## Patches

### A fill every fourth bar

```text
[Clock Gate] ──> [Logic Clock]                 (Clock Div 1/16; Logic Div 64, Length 17)
[Logic Gate] ──> [Tom Sequencer Run]  [Tom Sequencer Reset]
```

At sixteenths, 64 clocks are four bars. The Gate opens on the downbeat of bar 1 and resets the tom sequencer to step 1 as it starts it running. It stays open for 17 clocks: the fill bar and the downbeat after it, so the sequencer takes one more step, wraps round, and its **EOC** can strike a crash where the fill lands. This is how [Backbeat](../../recipes/backbeat.md#a-simpler-phrase-logic) can schedule its fill. Set **Div** to 128 for a fill every eighth bar, or **Offset** to 48 to put the fill in bar 4.

### A beat that isn't every beat

```text
[Clock Gate] ──> [Logic Clock]                 (Clock Div 1/4; Logic Div 4, Offset 2)
[Logic Trig] ──> [ADSR Gate]
```

A clap on beat 3 of every bar, and nowhere else. Turn **Offset** to move it round the bar.

### Polyrhythm

Two Logic modules on the same sixteenth clock, one at **Div** 3 and one at **Div** 4, play three against four. They meet again every twelve sixteenths. Patch their **Trig** outputs into **A** and **B** of a third Logic, and its **AND** fires only where they meet.

### Only on the beat

```text
[Audio Input Gate] ──> [Logic A]
[Clock Gate] ──> [Logic B]
[Logic AND] ──> [ADSR Gate]
```

Hits from a drummer or a microphone come through only while the clock's gate is high, so loose playing is cut back to the grid.

### Make a gate from an LFO

```text
[LFO Out] ──> [Logic CV]                       (Thresh 0.5)
[Logic Above] ──> [ADSR Gate]
```

The envelope opens each time the LFO rises past the threshold and closes as it falls back. Raise the threshold and the gates get shorter. Patch a [Sequencer](./sequencer.md)'s **Pitch** into **CV** instead, with the threshold between two notes, and the high notes open a gate the low ones don't.

## Related modules

- [Clock](../modulation/clock.md): the pulses to divide
- [Step Sequencer](./sequencer.md): its **Run** and **Reset** take Logic's Gate; its **EOC** marks the end of each pass
- [Sample & Hold](./sample-hold.md): Logic's **Trig** makes a sparser trigger to sample on
- [Attenuverter](./attenuverter.md): scale a control signal before **CV**, or turn a gate into a control voltage
