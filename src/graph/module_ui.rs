//! Per-module UI hints: the few things about a node that its DSP definition
//! doesn't say.
//!
//! Ports, parameters, ranges, defaults and names all come from the module's
//! `DspModule` definition (see [`super::catalog`]). This table only adds
//! presentation: short labels, knob order, which parameters stay hidden,
//! and which custom display a node draws. A module with no entry still gets
//! a working node, with a knob for every continuous parameter.

use super::node_data::NodeDisplay;

/// A knob in a node's bottom row.
#[derive(Clone, Copy, Debug)]
pub struct KnobHint {
    /// Parameter name, as declared by the module.
    pub param: &'static str,
    /// Short label under the knob.
    pub label: &'static str,
    /// When the parameter has a CV input: true if CV modulates around the
    /// knob, false if CV replaces it. Ignored for parameters without a CV input.
    pub modulatable: bool,
}

/// A knob labeled with the parameter's own name.
const fn knob(param: &'static str) -> KnobHint {
    KnobHint { param, label: param, modulatable: false }
}

/// A knob with a short label.
const fn knob_as(param: &'static str, label: &'static str) -> KnobHint {
    KnobHint { param, label, modulatable: false }
}

/// A knob whose CV input modulates around the knob's value instead of replacing it.
const fn modulatable(param: &'static str, label: &'static str) -> KnobHint {
    KnobHint { param, label, modulatable: true }
}

/// Presentation hints for one module.
#[derive(Debug)]
pub struct ModuleUi {
    pub module_id: &'static str,
    /// The knob row, in order. Every continuous parameter must be a knob or hidden.
    pub knobs: &'static [KnobHint],
    /// Short labels for inline toggles and dropdowns (parameter name → label).
    pub labels: &'static [(&'static str, &'static str)],
    /// Parameters with no control on the node. A trailing `*` matches a prefix.
    pub hidden: &'static [&'static str],
    /// Leading parameters driven by live input (the computer keyboard)
    /// rather than by the graph. Parameter sync leaves these alone.
    pub live_params: usize,
    /// Output ports the engine reports values for (lit gate ports, phase).
    pub monitor: &'static [&'static str],
    /// Custom visualization drawn above the knob row.
    pub display: NodeDisplay,
    /// Knobs per row before wrapping (0 keeps them all on one row).
    pub knobs_per_row: usize,
}

impl ModuleUi {
    const DEFAULT: ModuleUi = ModuleUi {
        module_id: "",
        knobs: &[],
        labels: &[],
        hidden: &[],
        live_params: 0,
        monitor: &[],
        display: NodeDisplay::None,
        knobs_per_row: 0,
    };

    /// Short inline label for a toggle or dropdown.
    pub fn label_for(&self, param: &str) -> Option<&'static str> {
        self.labels.iter().find(|(name, _)| *name == param).map(|(_, label)| *label)
    }

    /// Whether a parameter has no control on the node.
    pub fn is_hidden(&self, param: &str) -> bool {
        self.hidden.iter().any(|pattern| match pattern.strip_suffix('*') {
            Some(prefix) => param.starts_with(prefix),
            None => param == *pattern,
        })
    }
}

/// Hints for a module, if it has an entry.
pub fn module_ui(module_id: &str) -> Option<&'static ModuleUi> {
    MODULE_UI.iter().find(|ui| ui.module_id == module_id)
}

static MODULE_UI: &[ModuleUi] = &[
    ModuleUi {
        module_id: "osc.sine",
        // Three rows: tune, timbre, ensemble
        knobs: &[
            knob_as("Octave", "Oct"),
            knob_as("Semitone", "Semi"),
            knob("Fine"),
            knob_as("FM Depth", "FM"),
            knob_as("Exp FM Depth", "Exp FM"),
            knob_as("Pulse Width", "PW"),
            knob("Voices"),
            knob("Detune"),
            knob("Spread"),
        ],
        labels: &[("Waveform", "Wave")],
        display: NodeDisplay::OscillatorWave,
        knobs_per_row: 3,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "source.noise",
        knobs: &[modulatable("Level", "Level"), modulatable("Rate", "Rate")],
        display: NodeDisplay::NoiseSpectrum,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "input.keyboard",
        knobs: &[knob_as("Octave", "Oct"), knob_as("Velocity", "Vel")],
        hidden: &["Note", "Gate"],
        live_params: 2,
        monitor: &["Gate"],
        display: NodeDisplay::KeyboardPiano,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "input.midi_note",
        knobs: &[knob_as("Octave", "Oct"), knob_as("Bend Range", "Bend")],
        labels: &[("Channel", "Ch"), ("Retrigger", "Retrig")],
        monitor: &["Gate"],
        display: NodeDisplay::MidiPiano,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "input.poly_midi",
        knobs: &[knob("Voices"), knob_as("Octave", "Oct"), knob_as("Bend Range", "Bend")],
        labels: &[("Channel", "Ch"), ("Allocation", "Mode")],
        monitor: &["Gate"],
        display: NodeDisplay::MidiPiano,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "filter.svf",
        knobs: &[modulatable("Cutoff", "Cutoff"), modulatable("Resonance", "Res"), knob("Drive")],
        display: NodeDisplay::FilterResponse,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "filter.ladder",
        knobs: &[modulatable("Cutoff", "Cutoff"), modulatable("Resonance", "Res"), knob("Drive")],
        display: NodeDisplay::LadderResponse,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "mod.adsr",
        // Two rows: times and level, then each stage's curve under its time
        // (velocity sits under sustain, both being about level)
        knobs: &[
            knob_as("Attack", "Atk"),
            knob_as("Decay", "Dec"),
            knob_as("Sustain", "Sus"),
            knob_as("Release", "Rel"),
            knob_as("Attack Curve", "A Crv"),
            knob_as("Decay Curve", "D Crv"),
            knob_as("Velocity Amount", "Vel"),
            knob_as("Release Curve", "R Crv"),
        ],
        display: NodeDisplay::Envelope,
        knobs_per_row: 4,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "mod.lfo",
        knobs: &[modulatable("Rate", "Rate"), knob("Phase")],
        labels: &[("Waveform", "Wave")],
        monitor: &["Phase"],
        display: NodeDisplay::LfoWave,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.clock",
        knobs: &[knob_as("Tempo", "BPM"), knob_as("Gate Length", "Gate")],
        labels: &[("Division", "Div")],
        monitor: &["Gate"],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.vca",
        knobs: &[knob("Level"), knob_as("CV Amount", "CV Amt")],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.attenuverter",
        knobs: &[knob_as("Amount", "Amt"), knob("Offset")],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.mixer",
        // A column per channel strip, its level over its pan, and a fifth
        // for the master section. The display's meters sit over the columns
        knobs: &[
            modulatable("Level 1", "Lv 1"),
            modulatable("Level 2", "Lv 2"),
            modulatable("Level 3", "Lv 3"),
            modulatable("Level 4", "Lv 4"),
            knob("Master"),
            modulatable("Pan 1", "Pan 1"),
            modulatable("Pan 2", "Pan 2"),
            modulatable("Pan 3", "Pan 3"),
            modulatable("Pan 4", "Pan 4"),
            knob("Spread"),
        ],
        // Each strip's mute is a button under its meter
        hidden: &["Mute *"],
        display: NodeDisplay::MixerStrips,
        knobs_per_row: 5,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.quantizer",
        knobs: &[modulatable("Transpose", "Transpose")],
        hidden: &["Mask"],
        monitor: &["Out", "Trig"],
        display: NodeDisplay::ScalePiano,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.oscilloscope",
        knobs: &[knob_as("Trigger Level", "Trig")],
        display: NodeDisplay::Scope,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "seq.step",
        knobs: &[knob("Steps"), knob_as("Gate Length", "Gate")],
        labels: &[("Direction", "Dir")],
        hidden: &["Step *"],
        monitor: &["Gate", "EOC"],
        display: NodeDisplay::StepGrid,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.delay",
        knobs: &[
            // CV swings Time by ±50% and adds to Feedback, around the knobs
            modulatable("Time", "Time"),
            modulatable("Feedback", "FB"),
            knob("Mix"),
            knob_as("High Cut", "HiCut"),
            knob_as("Low Cut", "LoCut"),
        ],
        labels: &[("Ping-Pong", "P-P")],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.reverb",
        knobs: &[
            // The room and its tail on top, placement in the mix below
            knob("Size"),
            knob("Decay"),
            knob_as("Damping", "Damp"),
            knob("Mod"),
            knob_as("Pre-Delay", "PreD"),
            knob("Width"),
            knob("Mix"),
        ],
        knobs_per_row: 4,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.eq",
        knobs: &[
            knob_as("Low Freq", "LoFrq"),
            knob_as("Low Gain", "LoGn"),
            knob_as("Mid Freq", "MdFrq"),
            knob_as("Mid Gain", "MdGn"),
            knob_as("Mid Q", "MdQ"),
            knob_as("High Freq", "HiFrq"),
            knob_as("High Gain", "HiGn"),
            knob_as("Output", "Out"),
        ],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.distortion",
        // The character of the curve on top, its colour and level below
        knobs: &[
            modulatable("Drive", "Drive"),
            knob_as("Symmetry", "Sym"),
            knob("Rate"),
            knob("Tone"),
            knob("Mix"),
            knob_as("Output", "Out"),
        ],
        knobs_per_row: 3,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.chorus",
        knobs: &[modulatable("Rate", "Rate"), modulatable("Depth", "Depth"), knob("Delay"), knob_as("Feedback", "FB"), knob("Mix")],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "fx.compressor",
        knobs: &[
            knob_as("Threshold", "Thresh"),
            knob("Ratio"),
            knob_as("Attack", "Atk"),
            knob_as("Release", "Rel"),
            knob("Knee"),
            knob_as("Makeup", "Mkup"),
            knob("Mix"),
        ],
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "util.midi_monitor",
        labels: &[("Channel", "Ch"), ("Pitch Bend", "PB")],
        display: NodeDisplay::MidiLog,
        ..ModuleUi::DEFAULT
    },
    ModuleUi {
        module_id: "output.audio",
        knobs: &[knob_as("Volume", "Vol")],
        display: NodeDisplay::OutputMeter,
        ..ModuleUi::DEFAULT
    },
];

/// Every entry in the table.
#[cfg(test)]
pub fn all() -> &'static [ModuleUi] {
    MODULE_UI
}

/// Hints used for a module with no entry in the table.
pub fn default_ui() -> &'static ModuleUi {
    &ModuleUi::DEFAULT
}
