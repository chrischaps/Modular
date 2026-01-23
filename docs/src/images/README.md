# Documentation Images

This directory contains images for the Modular Synth documentation.

## Required Images

### Interface Screenshots
- `interface-overview.png` - Main application window
- `interface-context-menu.png` - Right-click module browser
- `interface-module-anatomy.png` - Annotated module parts
- `interface-connection.png` - Creating a connection
- `interface-knob.png` - Knob interaction
- `interface-*.png` - Other UI screenshots

### Signal Type Diagrams
- `signal-types-overview.png` - All four signal types
- `signal-port-colors.png` - Port coloring
- `signal-cable-colors.png` - Cable coloring

### Connection Diagrams
- `connection-direction.png` - Output to input flow
- `connection-multiple.png` - One output to many inputs
- `connection-creating.png` - Dragging a connection
- `connection-colors.png` - Cable colors by type
- `connection-organized.png` - Well-organized patch
- `connection-exposed.png` - Exposed parameter indicator

### Module Screenshots
Each module should have a screenshot:
- `module-oscillator.png`
- `module-svf-filter.png`
- `module-adsr.png`
- `module-lfo.png`
- `module-clock.png`
- `module-vca.png`
- `module-mixer.png`
- `module-attenuverter.png`
- `module-sample-hold.png`
- `module-sequencer.png`
- `module-delay.png`
- `module-reverb.png`
- `module-chorus.png`
- `module-distortion.png`
- `module-eq.png`
- `module-compressor.png`
- `module-keyboard.png`
- `module-midi-note.png`
- `module-midi-monitor.png`
- `module-oscilloscope.png`
- `module-audio-output.png`

### Waveform Diagrams
- `waveform-sine.png`
- `waveform-saw.png`
- `waveform-square.png`
- `waveform-triangle.png`

### Filter Response Diagrams
- `filter-lowpass.png`
- `filter-highpass.png`
- `filter-bandpass.png`

### Envelope Diagrams
- `envelope-adsr-diagram.png`

### LFO Diagrams
- `lfo-sine.png`
- `lfo-triangle.png`
- `lfo-square.png`
- `lfo-saw.png`

### Recipe Patch Diagrams
- `recipe-basic-subtractive.png`
- `recipe-fm-synthesis.png`
- `recipe-lush-pad.png`
- `recipe-generative-ambient.png`
- `recipe-rhythmic-sequence.png`

### Tutorial Screenshots
- `tutorial-first-patch.png`
- `tutorial-add-oscillator.png`
- `tutorial-first-connection.png`
- `tutorial-keyboard-connected.png`
- `tutorial-vca-added.png`
- `tutorial-filter-added.png`
- `tutorial-complete-patch.png`

## Image Guidelines

1. **Format**: PNG preferred for UI screenshots
2. **Size**: Reasonable resolution (1x or 2x for retina)
3. **Background**: Use dark theme for consistency
4. **Annotations**: Use simple arrows/labels when needed
5. **Cropping**: Focus on the relevant UI element

## Creating Screenshots

Screenshots can be captured from the running application:

```bash
cargo run --release
```

Use your system's screenshot tool to capture:
- Full window for overview shots
- Cropped regions for detail shots
- Use annotation tools for callouts if needed
