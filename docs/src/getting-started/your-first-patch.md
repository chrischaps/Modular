# Your First Patch

Let's build a simple synthesizer patch from scratch. By the end of this tutorial, you'll have a playable synthesizer with an oscillator, filter, envelope, and output.

![Completed First Patch](../images/tutorial-first-patch.png)
*The completed first patch*

## Step 1: Add an Oscillator

Every synthesizer needs a sound source. Let's start with an oscillator.

1. **Right-click** on the canvas to open the module browser
2. Navigate to **Sources > Oscillator**
3. Click to add the oscillator

![Adding an Oscillator](../images/tutorial-add-oscillator.png)
*Adding an oscillator from the context menu*

The oscillator generates a continuous tone. By default, it produces a **sine wave** at **440 Hz** (the note A4).

### Oscillator Settings

- **Waveform**: Select between Sine, Saw, Square, or Triangle
- **Frequency**: The pitch in Hz (or controlled by V/Oct input)
- **Detune**: Fine-tune adjustment in cents

Try changing the waveform to **Saw** for a brighter, more harmonically rich sound.

## Step 2: Add Audio Output

To hear the oscillator, we need to connect it to the audio output.

1. **Right-click** on the canvas
2. Navigate to **Output > Audio Output**
3. Click to add the output module

Position it to the right of the oscillator.

### Connect the Oscillator to Output

1. Click on the oscillator's **Audio Out** port (right side, blue)
2. Drag to the output's **Left** input port
3. Release to create the connection

![First Connection](../images/tutorial-first-connection.png)
*Connecting the oscillator to the output*

You should now hear a continuous tone! If not, check that:
- Your audio device is working
- The output module's **Level** knob is turned up
- Your system volume is audible

### Stereo Output

For stereo sound, also connect the oscillator to the **Right** input, or use the **Mono** input which sends to both channels.

## Step 3: Control the Pitch

A synthesizer that plays only one note isn't very useful. Let's add keyboard control.

1. **Right-click** > **MIDI > Keyboard Input**
2. Position it to the left of the oscillator

### Connect Keyboard to Oscillator

1. Connect the keyboard's **V/Oct** output to the oscillator's **V/Oct** input
2. Connect the keyboard's **Gate** output (we'll use this later)

![Keyboard Connected](../images/tutorial-keyboard-connected.png)
*Keyboard controlling the oscillator pitch*

Now press keys on your computer keyboard:
- **Z, X, C, V, B, N, M** play notes C through B
- **A, S, D, F, G, H, J** play sharps/flats
- **Q-P** row plays an octave higher

The oscillator pitch follows your keyboard input!

## Step 4: Add an Envelope

Right now, the sound plays continuously. An **envelope** shapes the sound over time, giving it a beginning and end.

1. **Right-click** > **Modulation > ADSR Envelope**
2. Position it between the keyboard and output

### Envelope Parameters

The ADSR envelope has four stages:

- **Attack**: How quickly the sound rises (0 = instant, higher = gradual fade in)
- **Decay**: How quickly it falls to the sustain level
- **Sustain**: The level held while the key is pressed
- **Release**: How quickly the sound fades after key release

Set these initial values:
- Attack: **10ms** (quick start)
- Decay: **200ms** (moderate decay)
- Sustain: **0.5** (half volume while held)
- Release: **300ms** (gentle fade out)

### Connect the Envelope

1. Connect the keyboard's **Gate** output to the envelope's **Gate** input
2. Connect the envelope's **Env** output to... we need a VCA!

## Step 5: Add a VCA

A **VCA** (Voltage Controlled Amplifier) controls the volume of a signal. We'll use it to apply the envelope to our oscillator.

1. **Right-click** > **Utilities > VCA**
2. Position it between the oscillator and output

### Connect Everything

1. Disconnect the oscillator from the output (right-click the cable)
2. Connect the oscillator's **Audio Out** to the VCA's **Input**
3. Connect the envelope's **Env** output to the VCA's **CV** input
4. Connect the VCA's **Output** to the audio output's **Mono** input

![VCA Added](../images/tutorial-vca-added.png)
*The patch with VCA and envelope*

Now when you press a key:
- The **keyboard** sends Gate and V/Oct
- The **envelope** responds to the Gate
- The **VCA** shapes the oscillator volume based on the envelope

Try adjusting the envelope parameters to change the character of the sound!

## Step 6: Add a Filter

Filters shape the harmonic content of a sound by removing frequencies. Let's add a **low-pass filter** to warm up our tone.

1. **Right-click** > **Filters > SVF Filter**
2. Position it between the oscillator and VCA

### Connect the Filter

1. Disconnect the oscillator from the VCA
2. Connect oscillator **Audio Out** to filter **Input**
3. Connect filter **Lowpass** output to VCA **Input**

### Filter Settings

- **Cutoff**: The frequency where filtering begins (lower = darker sound)
- **Resonance**: Emphasizes frequencies at the cutoff (creates a peak)

Set cutoff to around **1000 Hz** and resonance to **0.3** for a warm, slightly vocal quality.

![Filter Added](../images/tutorial-filter-added.png)
*Adding the SVF filter to the signal chain*

## Step 7: Modulate the Filter (Optional)

For a more dynamic sound, let's make the filter open and close with each note using the envelope.

### Add Filter Envelope Control

You can use the same envelope or add a second one:

1. Connect the envelope's **Env** output to the filter's **Cutoff** input

Now the filter cutoff follows the envelope shape:
- Filter opens during attack
- Closes during decay
- Stays partially open during sustain
- Closes during release

Adjust the **Cutoff** knob to set the baseline, and the envelope adds movement on top.

## Complete Patch Overview

Here's the final signal flow:

```
[Keyboard] ──V/Oct──> [Oscillator] ──Audio──> [Filter] ──Audio──> [VCA] ──Audio──> [Output]
    │                                            ↑                   ↑
    └───────Gate────> [ADSR Envelope] ───────────┴───────────────────┘
```

![Complete Patch](../images/tutorial-complete-patch.png)
*The complete first patch*

## What You've Learned

- **Adding modules** from the context menu
- **Connecting modules** by dragging between ports
- **Signal flow**: Oscillator → Filter → VCA → Output
- **Control signals**: Gate triggers the envelope, V/Oct controls pitch
- **Modulation**: Using the envelope to control both VCA and filter

## Experimentation Ideas

Try these modifications:

1. **Change the waveform** - Saw and Square have more harmonics for the filter to work with
2. **Add an LFO** - Connect it to the filter cutoff for a wobbling effect
3. **Increase resonance** - Higher resonance creates a more dramatic filter sweep
4. **Adjust envelope** - Long attack creates pad sounds, short attack creates plucks
5. **Add reverb** - Insert a reverb effect between VCA and output

## Next Steps

Now that you've built your first patch:

- **[Signal Types](../concepts/signal-types.md)** - Understand the different signal types in depth
- **[Module Reference](../modules/README.md)** - Explore all available modules
- **[Basic Subtractive Synth](../recipes/basic-subtractive.md)** - A more complete subtractive synthesizer recipe
- **[FM Synthesis](../recipes/fm-synthesis.md)** - Try a different synthesis technique
