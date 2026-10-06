//! Which modules can be bypassed, and where their signal goes when they are.
//!
//! Bypassing a filter or effect takes it out of the signal path, like the
//! footswitch on a pedal: what comes in goes straight out. The module isn't
//! asked to do this itself. It declares audio ports, and the routing is read
//! from them: the first audio output passes the first audio input, the
//! second passes the second, and outputs beyond the last input repeat it (a
//! filter's LowPass, HighPass, BandPass and Notch all pass the one input).

use super::{ModuleCategory, PortDefinition, SignalType};

/// Whether a module can be bypassed: a filter or effect with audio in and out.
pub fn can_bypass(category: ModuleCategory, ports: &[PortDefinition]) -> bool {
    matches!(category, ModuleCategory::Filter | ModuleCategory::Effect)
        && ports.iter().any(|p| p.is_input() && p.signal_type == SignalType::Audio)
        && ports.iter().any(|p| p.is_output() && p.signal_type == SignalType::Audio)
}

/// For each output port, in output order, the input (in input order) that
/// passes straight through it while the module is bypassed. Outputs that
/// aren't audio pass nothing.
pub fn bypass_routes(ports: &[PortDefinition]) -> Vec<Option<usize>> {
    let audio_inputs: Vec<usize> = ports
        .iter()
        .filter(|p| p.is_input())
        .enumerate()
        .filter(|(_, p)| p.signal_type == SignalType::Audio)
        .map(|(index, _)| index)
        .collect();
    let mut audio_outputs_seen = 0;
    ports
        .iter()
        .filter(|p| p.is_output())
        .map(|port| {
            if port.signal_type != SignalType::Audio {
                return None;
            }
            let source = audio_inputs.get(audio_outputs_seen).or(audio_inputs.last()).copied();
            audio_outputs_seen += 1;
            source
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio_in(name: &'static str) -> PortDefinition {
        PortDefinition::input(name, name, SignalType::Audio)
    }

    fn audio_out(name: &'static str) -> PortDefinition {
        PortDefinition::output(name, name, SignalType::Audio)
    }

    #[test]
    fn stereo_effect_passes_left_to_left_and_right_to_right() {
        let ports = [
            audio_in("in_l"),
            audio_in("in_r"),
            PortDefinition::input("time", "Time", SignalType::Control),
            audio_out("out_l"),
            audio_out("out_r"),
        ];
        assert_eq!(bypass_routes(&ports), vec![Some(0), Some(1)]);
    }

    #[test]
    fn every_filter_output_passes_the_one_input() {
        let ports = [
            audio_in("in"),
            PortDefinition::input("cutoff", "Cutoff", SignalType::Control),
            audio_out("lp"),
            audio_out("hp"),
            audio_out("bp"),
        ];
        assert_eq!(bypass_routes(&ports), vec![Some(0); 3]);
    }

    #[test]
    fn sidechain_never_reaches_the_output() {
        let ports = [audio_in("in"), audio_in("sidechain"), audio_out("out")];
        assert_eq!(bypass_routes(&ports), vec![Some(0)]);
    }

    #[test]
    fn control_outputs_pass_nothing() {
        let ports = [
            audio_in("in"),
            audio_out("out"),
            PortDefinition::output("env", "Env", SignalType::Control),
        ];
        assert_eq!(bypass_routes(&ports), vec![Some(0), None]);
    }

    #[test]
    fn only_filters_and_effects_with_audio_through_them_can_bypass() {
        let through = [audio_in("in"), audio_out("out")];
        assert!(can_bypass(ModuleCategory::Effect, &through));
        assert!(can_bypass(ModuleCategory::Filter, &through));
        assert!(!can_bypass(ModuleCategory::Utility, &through));
        assert!(!can_bypass(ModuleCategory::Effect, &[audio_in("in")]));
        assert!(!can_bypass(ModuleCategory::Source, &[audio_out("out")]));
    }
}
