# Interface Overview

Modular Synth uses a node-graph interface where modules are represented as nodes that can be connected together to create synthesizer patches.

![Interface Overview](../images/interface-overview.png)
*The main Modular Synth interface*

## The Canvas

The main area of the interface is the **node graph canvas**. This is where you create and connect modules.

### Navigation

| Action | Mouse | Keyboard |
|--------|-------|----------|
| **Pan** | Middle-click drag | Arrow keys |
| **Zoom** | Scroll wheel | `+` / `-` |
| **Fit to view** | - | `Home` |
| **Select module** | Left-click | - |
| **Multi-select** | Shift + left-click | - |
| **Box select** | Left-click drag on empty space | - |
| **Delete** | - | `Delete` or `Backspace` |

### Canvas Tips

- Press `Space` or `Tab` over the canvas to add a module by name
- Use the scroll wheel to zoom in for detailed work or out for an overview
- Modules can be freely positioned anywhere on the canvas
- Hover a jack's name or a knob to see what it does. Jacks also show their signal type in its cable colour, and knobs their range
- The background grid moves and zooms with the patch, with a brighter line every five squares

## Adding Modules

### Context Menu

**Right-click** on empty canvas space to open the module browser:

![Context Menu](../images/interface-context-menu.png)
*The module browser context menu*

Modules are organized by category:

- **Sources** - Oscillators and sound generators
- **Filters** - Frequency shaping
- **Modulation** - Envelopes, LFOs, clocks
- **Utilities** - VCAs, mixers, signal processing
- **Effects** - Delays, reverbs, distortion
- **MIDI** - MIDI input and processing
- **Visualization** - Scopes and meters
- **Output** - Audio output

Click a module name to add it at the cursor position.

### Quick Add

Press **Space** or **Tab** with the mouse over the canvas. A search box opens at the cursor, listing every module by category. Type a few letters to narrow it down, then press **Enter**, and the module appears where the box opened.

The search is forgiving. Letters only need to appear in order, so `lfo`, `svf`, `dly` and `s&h` all find what you'd expect. Typing a category name (`effect`, `mod`) lists that category. Use the arrow keys or `Tab` to move through the list, and `Escape` to close it.

## Editing Modules

### The Module Menu

**Right-click** a module's header or body (anywhere but a knob) to open its menu: **Duplicate**, **Copy**, **Bypass** (filters and effects), **Reset to defaults** and **Delete**. If the module is part of a selection, the action applies to the whole selection.

### Copy, Paste and Duplicate

- **Duplicate** (`Ctrl + D`) copies the selected modules a little down and to the right. Cables between them are copied too; cables to the rest of the patch aren't. The copies are selected, so pressing `Ctrl + D` again makes a row of them.
- **Copy** (`Ctrl + C`) and **Cut** (`Ctrl + X`) put the selected modules on the clipboard. **Paste** (`Ctrl + V`) puts them at the mouse cursor with their layout, settings and the cables between them. Pasting again without moving the mouse fans the copies out.
- The clipboard holds them as patch JSON. You can paste modules into another Modular window, or paste the text of a whole `.json` patch file to add its modules to the current patch.
- MIDI mappings stay with the original modules.

Each of these is one undo step, named after what it did, e.g. *Duplicate 3 modules*.

## Module Anatomy

Each module has a consistent structure:

![Module Anatomy](../images/interface-module-anatomy.png)
*Parts of a module*

### Header Bar

The colored bar at the top shows:
- **Module name** - The type of module
- **Category color** - Indicates the module's function category

### Input Ports (Left Side)

Circular connectors on the left side receive signals from other modules:
- **Port color** indicates the expected signal type
- **Port label** describes what the input controls
- Hover over a port to see a tooltip with details

### Output Ports (Right Side)

Circular connectors on the right side send signals to other modules:
- **Port color** indicates the signal type produced
- Multiple modules can connect to the same output

### Parameter Knobs (Bottom)

Rotary knobs for adjusting module parameters:
- **Drag vertically** to adjust the value
- **Double-click** to reset to default
- **Ctrl + click** for fine adjustment
- Value readout shows the current setting

### Exposed Parameters

Some parameters can be controlled both manually and via external signals. When an external signal is connected:

- The knob becomes **read-only** (dimmed appearance)
- The knob **animates** to show the incoming signal value
- An **orange indicator** shows external control is active

When disconnected, the knob returns to manual control.

## Making Connections

### Creating a Connection

1. Click and hold on an **output port** (right side of a module)
2. Drag to an **input port** (left side of another module)
3. Release to complete the connection

![Making a Connection](../images/interface-connection.png)
*Dragging a connection from output to input*

### Connection Rules

- Outputs connect to inputs (never output-to-output or input-to-input)
- Signal types should match (Audio to Audio, Control to Control, etc.)
- Some inputs accept multiple signal types (automatic conversion)
- Multiple cables can connect to the same output
- Only one cable can connect to each input

### Connection Colors

Cables are colored by signal type:

| Color | Signal Type |
|-------|-------------|
| **Blue** | Audio |
| **Orange** | Control/CV |
| **Green** | Gate/Trigger |
| **Purple** | MIDI |

### Removing Connections

- **Right-click** on a connection to delete it
- **Click** on an input port with an existing connection, then press `Escape` to disconnect
- **Delete a module** to remove all its connections

## Adjusting Parameters

### Knob Interaction

![Knob Interaction](../images/interface-knob.png)
*Adjusting a parameter knob*

| Action | Result |
|--------|--------|
| **Drag up/down** | Adjust value |
| **Ctrl + drag** | Fine adjustment |
| **Double-click** | Reset to default |
| **Right-click** | Open value entry / MIDI learn |

### Value Display

Below each knob is a value readout showing:
- The current numeric value
- The unit (Hz, ms, dB, etc.) where applicable

## Patch Management

### Saving Patches

| Action | Shortcut |
|--------|----------|
| **Save** | `Ctrl + S` |
| **Save As** | `Ctrl + Shift + S` |

Patches are saved as `.json` files containing all module settings and connections.

### Loading Patches

| Action | Shortcut |
|--------|----------|
| **Open** | `Ctrl + O` |
| **New** | `Ctrl + N` |

### Undo and Redo

**Undo** (`Ctrl + Z`) and **Redo** (`Ctrl + Shift + Z` or `Ctrl + Y`) are in the toolbar's **Edit** group. Hover either button to see which edit it will undo or redo, such as *Move Oscillator* or *Set SVF Filter Cutoff*.

Undo covers adding, deleting, moving and bypassing modules, patching and unpatching cables, and turning knobs. A whole drag is one step: turning a knob from 200 Hz to 2 kHz and back undoes in one go. A deleted module comes back with its settings, its cables and its MIDI mappings. Knobs moved by a MIDI controller aren't recorded, and opening a patch starts a fresh history.

### Recent Patches

The **Recent** menu in the toolbar's **File** group lists the last eight patches you opened or saved, newest first. Hover one to see where it lives. A file that has since been moved or deleted is greyed out. **Clear Recent** empties the list.

### Unsaved Changes

While a patch has changes you haven't saved, the window title starts with a dot (`● Lush Pad · Modular Synth`), and so does its name in the status bar. Undoing back to the saved patch clears the dot.

**New**, **Open**, opening an example or a recent file, and closing the window all ask first when there are unsaved changes:

- **Save** saves the patch (asking where, if it has never been saved), then carries on. Cancelling the save dialog cancels the whole thing.
- **Don't Save** (**Quit Without Saving**, when closing) carries on and lets the changes go.
- **Cancel** (or `Escape`) goes back to the patch.

### Autosave and Recovery

Every 30 seconds, a patch with unsaved changes is autosaved alongside the app's settings. Saving the patch, or choosing **Quit Without Saving**, clears the autosave. So the only way one survives is if Modular closes without asking, after a crash or a forced quit.

The next time Modular starts, it offers the patch back: **Recover** reopens it exactly as it was at the last autosave, still marked unsaved, and **Discard** lets it go. You lose at most the last 30 seconds of work.

Recent files, the autosave and the window's size and position are kept in Modular's settings file (`%APPDATA%\Modular Synth\data\app.ron` on Windows, `~/.local/share/modularsynth/app.ron` on Linux, `~/Library/Application Support/Modular-Synth/app.ron` on macOS).

## MIDI Setup

### Enabling MIDI Input

1. Add a **MIDI Note** or **Keyboard** module to your patch
2. The module will automatically receive input from connected MIDI devices

### MIDI Learn

To assign a MIDI controller to a knob:

1. **Right-click** the knob
2. Select **MIDI Learn**
3. Move the desired MIDI controller
4. The knob is now mapped to that controller

### Computer Keyboard

The **Keyboard** module allows playing notes using your computer keyboard:

- **Z-M** row: Lower octave (C3-B3)
- **Q-P** row: Upper octave (C4-B4)
- **Number keys**: Octave selection

## Keyboard Shortcuts

### General

| Shortcut | Action |
|----------|--------|
| `Ctrl + N` | New patch |
| `Ctrl + O` | Open patch |
| `Ctrl + S` | Save patch |
| `Ctrl + Shift + S` | Save patch as |
| `Ctrl + Z` | Undo |
| `Ctrl + Shift + Z` or `Ctrl + Y` | Redo |
| `Escape` | Deselect / Cancel |

### Navigation

| Shortcut | Action |
|----------|--------|
| `Home` | Fit all to view |
| `+` / `-` | Zoom in / out |
| Arrow keys | Pan canvas |

### Modules

| Shortcut | Action |
|----------|--------|
| `Space` or `Tab` | Quick add a module at the cursor |
| `Delete` or `Backspace` | Delete selected |
| `Ctrl + D` | Duplicate selected |
| `Ctrl + C` | Copy selected |
| `Ctrl + X` | Cut selected |
| `Ctrl + V` | Paste at the cursor |
| `Ctrl + B` | Bypass selected |

## Next Steps

Now that you understand the interface:

- **[Your First Patch](./your-first-patch.md)** - Build a simple synthesizer step by step
- **[Signal Types](../concepts/signal-types.md)** - Understand the different signal types
- **[Module Reference](../modules/README.md)** - Explore all available modules
