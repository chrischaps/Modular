# Circuit View Exploration

*"See the soul of the module"*

## The Idea

When you select a module (filter, oscillator, etc.), an expanded view reveals the **analog circuitry** that would create that signal processing in the real world. Educational, aesthetic, and maybe even interactive.

---

## Option 1: Traditional Schematic View

**What it looks like:** Standard electronics schematic symbols — op-amps as triangles, resistors as zig-zags, capacitors as parallel lines, etc.

```
        ┌───────────┐
    ────┤           ├────
   R1   │    ∞      │   
        │   /│\     │    Output
 In ────┼──/ | \────┼────
        │   \|/     │
        │    │      │
        └────┼──────┘
             │
            ─┴─ C1
             │
            GND
```

**Pros:**
- Authentic and educational
- Familiar to anyone who's built hardware
- Can reference real circuits (Moog ladder, SVF, etc.)
- SVG/vector rendering is straightforward in egui

**Cons:**
- Dense and intimidating for non-EE folks
- Schematic symbols are abstract
- Doesn't "pop" visually

**Editability potential:** 
- Component values as labels → click to edit R, C values
- Changes map to module parameters (e.g., R value → cutoff frequency)

**Implementation complexity:** Medium
- SVG rendering or custom egui paths
- Need a schematic data format (nodes, components, connections)

---

## Option 2: Skeuomorphic Component View

**What it looks like:** Realistic-looking electronic components — chunky resistors with color bands, cylindrical capacitors, IC chips with legs, maybe on a breadboard or PCB aesthetic.

```
┌────────────────────────────────────────┐
│  [===█===]     ┌──┬──┐    [===█===]   │
│    10kΩ        │TL│07│      4.7kΩ     │
│                └──┴──┘                 │
│  ──┤├──  0.1µF              ──┤├──    │
│                                        │
│    ◉ Input        ◉ Output             │
└────────────────────────────────────────┘
```

**Pros:**
- Very visual and tactile
- Accessible to beginners ("oh, those are the little striped things!")
- Cool factor — feels like opening up a device
- Could animate signal flow through wires

**Cons:**
- More complex to render (sprites or detailed vector art)
- Might clash with Modular's clean node-graph aesthetic
- Layout is harder (physical components don't arrange as neatly as schematics)

**Editability potential:**
- Click a resistor → dial appears to change value
- "Swap component" — choose different capacitor types
- Very hands-on feel

**Implementation complexity:** High
- Need component artwork (sprites or procedural)
- Layout algorithm for placing components naturally
- More visual polish required

---

## Option 3: Signal Flow Block Diagram

**What it looks like:** Simplified boxes showing signal processing stages, halfway between schematic and node graph. Like a zoomed-in, more detailed version of the module itself.

```
┌─────────────────────────────────────────────┐
│                  SVF FILTER                 │
│                                             │
│   ┌─────┐    ┌──────┐    ┌─────┐           │
│   │Input│───▶│ GAIN │───▶│ HP  │──▶ HP Out │
│   └─────┘    │(Drive)│    └──┬──┘           │
│              └──────┘       │               │
│                             ▼               │
│              ┌──────┐    ┌─────┐           │
│              │ COEF │───▶│ BP  │──▶ BP Out │
│              │(Freq)│    └──┬──┘           │
│              └──────┘       │               │
│                    ▲        ▼               │
│              ┌─────┴─┐   ┌─────┐           │
│              │  Q    │◀──│ LP  │──▶ LP Out │
│              │(Res)  │   └─────┘           │
│              └───────┘                      │
└─────────────────────────────────────────────┘
```

**Pros:**
- Clean and readable
- Consistent with existing node-graph aesthetic
- Shows signal flow clearly
- Easy to implement in egui

**Cons:**
- Less "authentic" — doesn't show actual circuit topology
- Might feel redundant (already have nodes)
- Less educational about real hardware

**Editability potential:**
- Each block could expand further (nested detail)
- Parameters map directly to blocks

**Implementation complexity:** Low
- Just more egui nodes/boxes
- Familiar territory

---

## Option 4: Animated Signal Visualization

**What it looks like:** The circuit (any style above) but with **animated signal flow**. Particles or waves traveling through the circuit, transforming as they go.

```
Input ──●●●●──▶[FILTER]──○○○○──▶ Output
         ↑        ↑
      (raw)   (filtered)
```

- Audio signal shown as oscillating waveform traveling through wires
- Filter visibly "removes" high frequencies
- Resonance creates visible feedback loops
- Clipping/distortion shown as waveform hitting limits

**Pros:**
- Incredibly educational ("I can SEE the filter working!")
- Mesmerizing to watch
- Differentiator — no other synth does this well
- Works with any visual style (schematic, skeuomorphic, block)

**Cons:**
- Complex to implement well
- Needs careful performance optimization
- Risk of being gimmicky if not done right

**Editability potential:**
- Tweak a knob, watch the signal change in real-time
- "Probe points" — click anywhere to see signal at that stage

**Implementation complexity:** High
- Need efficient particle/wave rendering
- Must sync with actual audio processing
- Shader-based might be needed for performance

---

## Option 5: Hybrid — PCB Trace View

**What it looks like:** Like looking at a printed circuit board — copper traces on dark substrate, components as labeled rectangles, vias as dots.

```
┌────────────────────────────────────────────┐
│  ┌────┐                         ┌────┐    │
│  │ R1 │═══════╤════════════════│ C1 │    │
│  │10k │       │                 │.1µF│    │
│  └────┘       │                 └────┘    │
│               │      ┌──────────┐         │
│     ══════════╧═════│  TL072   │══════   │
│                      │  Op-Amp  │   OUT   │
│  IN ════════════════│          │         │
│                      └──────────┘         │
│ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ │
│                   (GND plane)             │
└────────────────────────────────────────────┘
```

**Pros:**
- Unique aesthetic
- Shows physical reality of circuits
- Clean and modern looking
- Fits with Modular's dark theme

**Cons:**
- Less intuitive than schematic for understanding topology
- Component relationships less clear
- Harder to make editable

**Implementation complexity:** Medium
- Vector paths for traces
- Simple component rectangles
- Could be quite elegant

---

## Editability Spectrum

### Level 0: View Only
- Pure education/aesthetic
- "This is what's inside"
- No interaction beyond viewing

### Level 1: Inspect
- Hover over components → see values and function
- Tooltips explain what each part does
- "This capacitor sets the cutoff frequency"

### Level 2: Tweakable
- Click components to adjust values
- Changes map to module parameters
- Resistor value → maps to Cutoff knob
- Creates a "deep edit" mode

### Level 3: Fully Editable
- Rewire the circuit
- Add/remove components
- Basically a circuit simulator
- **Way out of scope** — this is a separate app

**Recommendation:** Start with Level 1 (Inspect), design for Level 2 (Tweakable).

---

## Implementation Approach

### Data Model

Each module would have a `CircuitDefinition`:

```rust
struct CircuitDefinition {
    components: Vec<Component>,
    connections: Vec<Wire>,
    parameter_mappings: Vec<ParameterMapping>,
}

struct Component {
    id: String,
    kind: ComponentKind,  // Resistor, Capacitor, OpAmp, etc.
    position: Vec2,
    value: ComponentValue,
    label: String,
}

struct ParameterMapping {
    component_id: String,
    component_property: String,  // "resistance", "capacitance"
    parameter_id: String,        // Module parameter it maps to
    transform: ValueTransform,   // How component value relates to parameter
}
```

### Rendering

For egui, the cleanest approach:
1. **Simple:** Custom `egui::Painter` calls (lines, shapes, text)
2. **Polished:** Embedded SVG rendering or pre-rendered textures
3. **Fancy:** Custom shader for animated signals

### UI Integration

```
┌─────────────────────────────────────────┐
│  [Module Node - Normal View]            │
│  ┌─────┐                                │
│  │ SVF │  Cutoff: 1000Hz               │
│  │Filter│  Resonance: 0.5              │
│  └─────┘                                │
│         [🔍 View Circuit]               │
└─────────────────────────────────────────┘

              ↓ Click ↓

┌─────────────────────────────────────────┐
│  [Expanded Circuit View]                │
│  ┌───────────────────────────────────┐  │
│  │     (circuit diagram here)        │  │
│  │                                   │  │
│  │  R1=10k → Cutoff                  │  │
│  │  R2=47k → Resonance               │  │
│  │                                   │  │
│  └───────────────────────────────────┘  │
│         [✕ Close]                       │
└─────────────────────────────────────────┘
```

Could be:
- Inline expansion (module grows)
- Side panel
- Modal overlay
- Separate "circuit editor" tab

---

## My Recommendation

**Start with: Traditional Schematic + Hover Inspect (Option 1 + Level 1)**

Why:
1. **Authentic** — real circuit diagrams teach real knowledge
2. **Feasible** — vector rendering in egui is doable
3. **Extensible** — can add animation/editability later
4. **Distinctive** — no other software synth does this well

**Phase 1:** Static schematic SVGs for each module type, displayed on hover/click
**Phase 2:** Component hover tooltips ("This resistor sets cutoff → currently 10kΩ = 1000Hz")
**Phase 3:** Editable values that sync to parameters
**Phase 4:** Signal animation (stretch goal)

---

## Circuits to Model

For the existing modules, real-world reference circuits:

| Module | Real Circuit Reference |
|--------|----------------------|
| **SVF Filter** | Chamberlin State Variable Filter (dual op-amp) |
| **Oscillator** | CEM3340 / AS3340 VCO chip topology |
| **ADSR Envelope** | Classic 4-transistor ADSR (or op-amp integrator) |
| **LFO** | Triangle-core LFO with waveshaper |
| **VCA** | OTA-based VCA (CA3080 style) |
| **Distortion** | Op-amp soft clipper / diode clipper stages |
| **Reverb** | Conceptual (spring tank or PT2399 delay chip) |

---

## Questions to Resolve

1. **Where does the circuit view live?** Inline expansion? Side panel? Separate mode?
2. **How accurate should circuits be?** Exact component values, or simplified?
3. **Should parameters bidirectionally sync?** (Change knob → circuit updates, change circuit → knob updates)
4. **Is this a learning feature or a power-user feature?** (Affects complexity of display)

---

## Rough Effort Estimate

| Approach | Effort | Visual Impact |
|----------|--------|---------------|
| Block Diagram (Option 3) | 2-3 days | Low |
| Schematic View-Only (Option 1, Level 0) | 1 week | Medium |
| Schematic + Inspect (Option 1, Level 1) | 2 weeks | High |
| Schematic + Tweakable (Option 1, Level 2) | 3-4 weeks | Very High |
| Skeuomorphic (Option 2) | 3-4 weeks | Very High |
| Signal Animation (Option 4) | 2+ weeks additional | Wow |

---

## Next Steps

1. **Pick a visual style** — schematic vs skeuomorphic vs hybrid
2. **Pick an interaction level** — view-only vs inspect vs tweakable
3. **Prototype one module** — SVF Filter is a good candidate (interesting circuit, moderate complexity)
4. **Define the circuit data format** — JSON/TOML for circuit definitions
5. **Decide on UI integration** — where does this view live?

---

*This could be a really distinctive feature. Most synth software treats modules as black boxes. Showing the circuits makes Modular both educational and aesthetically unique — "the synthesizer that shows you how synthesizers work."*
